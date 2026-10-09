//! Textual literal scanner (R-CHK-019..021). Pure; called by P3's `extract_file`.

/// R-CHK-020: hex 3/4/6/8 to lower 6/8, integer rgb()/rgba() to hex, lengths to number+unit,
/// everything else collapsed and lowercased.
pub fn normalize_value(raw: &str) -> String {
    let t = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    norm_hex(&t)
        .or_else(|| norm_rgb(&t))
        .or_else(|| norm_length(&t))
        .unwrap_or_else(|| t.to_ascii_lowercase())
}

fn norm_hex(t: &str) -> Option<String> {
    let h = t.strip_prefix('#')?;
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let l = h.to_ascii_lowercase();
    match l.len() {
        3 | 4 => Some(format!(
            "#{}",
            l.chars().flat_map(|c| [c, c]).collect::<String>()
        )),
        6 | 8 => Some(format!("#{l}")),
        _ => None,
    }
}

fn norm_rgb(t: &str) -> Option<String> {
    let l = t.to_ascii_lowercase();
    let inner = l
        .strip_prefix("rgba(")
        .or_else(|| l.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<&str> = inner
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 3 && parts.len() != 4 {
        return None;
    }
    let mut out = String::from("#");
    for p in parts.iter().take(3) {
        out.push_str(&format!("{:02x}", p.parse::<u8>().ok()?));
    }
    if let Some(a) = parts.get(3) {
        let v = match a.strip_suffix('%') {
            Some(p) => p.parse::<f32>().ok()? / 100.0,
            None => a.parse::<f32>().ok()?,
        };
        out.push_str(&format!(
            "{:02x}",
            (v.clamp(0.0, 1.0) * 255.0).round() as u8
        ));
    }
    Some(out)
}

const UNITS: [&str; 15] = [
    "px", "rem", "em", "%", "vw", "vh", "vmin", "vmax", "ch", "ex", "pt", "cm", "mm", "in", "pc",
];

fn norm_length(t: &str) -> Option<String> {
    let l = t.to_ascii_lowercase();
    let split = l.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))?;
    let (num, unit) = l.split_at(split);
    if !UNITS.contains(&unit) {
        return None;
    }
    let n: f64 = num.parse().ok()?;
    Some(format!("{n}{unit}"))
}

use crate::extract::LiteralSite;
use crate::path::RepoPath;
use fairlead_syntax::diag::Span;
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::LazyLock};

#[allow(clippy::expect_used)] // static literal pattern, compiled by every test run
fn re(p: &str) -> Regex {
    Regex::new(p).expect("static regex")
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Flavour {
    Css,
    Script,
    Markup,
}

/// (flavour, does `//` start a comment)
fn flavour(ext: &str) -> Option<(Flavour, bool)> {
    match ext {
        "css" | "pcss" => Some((Flavour::Css, false)),
        "scss" | "less" => Some((Flavour::Css, true)),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "vue" | "svelte" => {
            Some((Flavour::Script, true))
        }
        "html" | "htm" | "svg" | "xml" => Some((Flavour::Markup, false)),
        _ => None,
    }
}

/// Byte offset to 1-based line and scalar-value column (C-16).
pub(crate) struct Lines {
    starts: Vec<usize>,
}
impl Lines {
    pub(crate) fn new(text: &str) -> Lines {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Lines { starts }
    }
    pub(crate) fn span(&self, text: &str, start: usize, end: usize) -> Span {
        let idx = self
            .starts
            .partition_point(|&s| s <= start)
            .saturating_sub(1);
        let line_start = self.starts.get(idx).copied().unwrap_or(0);
        let col = text.get(line_start..start).map_or(0, |s| s.chars().count()) + 1;
        Span {
            start: start as u32,
            end: end as u32,
            line: idx as u32 + 1,
            col: col as u32,
        }
    }
}

fn find(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    b.get(from..)?
        .windows(pat.len())
        .position(|w| w == pat)
        .map(|p| from + p)
}

/// Same byte length, comment bytes replaced by spaces (newlines kept). Strings are honoured.
fn blank_comments(text: &str, f: Flavour, line_comments: bool) -> String {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for k in from..to.min(out.len()) {
            if let Some(o) = out.get_mut(k)
                && *o != b'\n'
            {
                *o = b' ';
            }
        }
    };
    let (mut i, mut quote) = (0usize, None::<u8>);
    while let Some(&c) = b.get(i) {
        if let Some(q) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q || (c == b'\n' && q != b'`') {
                quote = None;
            }
            i += 1;
            continue;
        }
        let next = b.get(i + 1).copied();
        match (f, c, next) {
            (Flavour::Markup, b'<', _) if b.get(i..i + 4) == Some(b"<!--".as_slice()) => {
                let end = find(b, i + 4, b"-->").map_or(b.len(), |e| e + 3);
                blank(&mut out, i, end);
                i = end;
            }
            (Flavour::Css | Flavour::Script, b'/', Some(b'*')) => {
                let end = find(b, i + 2, b"*/").map_or(b.len(), |e| e + 2);
                blank(&mut out, i, end);
                i = end;
            }
            (Flavour::Css | Flavour::Script, b'/', Some(b'/')) if line_comments => {
                let end = b
                    .get(i..)
                    .and_then(|s| s.iter().position(|&x| x == b'\n'))
                    .map_or(b.len(), |p| i + p);
                blank(&mut out, i, end);
                i = end;
            }
            (Flavour::Css | Flavour::Script, b'"' | b'\'', _) | (Flavour::Script, b'`', _) => {
                quote = Some(c);
                i += 1;
            }
            _ => i += 1,
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

/// Split a value at whitespace and commas outside parentheses: (offset, component).
fn components(v: &str) -> Vec<(usize, &str)> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0i32, None::<usize>);
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => {
                depth += 1;
                start.get_or_insert(i);
            }
            ')' => {
                depth -= 1;
                start.get_or_insert(i);
            }
            c if (c.is_whitespace() || c == ',') && depth <= 0 => {
                if let Some(s) = start.take()
                    && let Some(x) = v.get(s..i)
                {
                    out.push((s, x));
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(s) = start
        && let Some(x) = v.get(s..)
    {
        out.push((s, x));
    }
    out
}

const SHORTHAND: [(&str, &str); 3] = [
    ("background", "background-color"),
    ("border", "border-color"),
    ("outline", "outline-color"),
];
fn is_colour(v: &str) -> bool {
    v.starts_with('#') || v.to_ascii_lowercase().starts_with("rgb")
}

struct Sink<'a> {
    text: &'a str,
    lines: &'a Lines,
    props: &'a BTreeSet<&'a str>,
    out: Vec<LiteralSite>,
}
impl Sink<'_> {
    /// Emit one site per component of `val` whose (canonical) property is listed.
    fn value(&mut self, prop: &str, val: &str, val_off: usize, hint: &str) {
        for (off, comp) in components(val) {
            if comp.starts_with('!') {
                continue;
            }
            let mut p = prop.to_string();
            if is_colour(comp)
                && let Some((_, long)) = SHORTHAND.iter().find(|(s, _)| *s == prop)
            {
                p = (*long).to_string();
            }
            if !self.props.contains(p.as_str()) {
                continue;
            }
            let start = val_off + off;
            self.out.push(LiteralSite {
                property: p,
                value_norm: normalize_value(comp),
                raw: comp.to_string(),
                span: self.lines.span(self.text, start, start + comp.len()),
                projection_hint: Some(hint.to_string()),
            });
        }
    }
}

static CSS_DECL: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?P<dash>-{0,2})(?P<prop>[A-Za-z][A-Za-z-]*)\s*:\s*(?P<val>[^;{}\n]+)"));

fn scan_css(clean: &str, sink: &mut Sink<'_>) {
    for m in CSS_DECL.captures_iter(clean) {
        let (Some(d), Some(p), Some(v)) = (m.name("dash"), m.name("prop"), m.name("val")) else {
            continue;
        };
        if d.as_str() == "--" {
            continue;
        } // custom property definition, not a use
        sink.value(
            &p.as_str().to_ascii_lowercase(),
            v.as_str(),
            v.start(),
            "css",
        );
    }
}

static KEYED: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"["']?(?P<key>[A-Za-z_$][A-Za-z0-9_$-]*)["']?\s*[:=]\s*(?:"(?P<dq>[^"\n]*)"|'(?P<sq>[^'\n]*)'|`(?P<bq>[^`\n$]*)`)"#,
    )
});
static TAILWIND: LazyLock<Regex> =
    LazyLock::new(|| re(r"\b(?P<prefix>[a-z]+(?:-[a-z]+)?)-\[(?P<val>[^\]\s$]+)\]"));

fn kebab(key: &str) -> String {
    let mut s = String::new();
    for ch in key.chars() {
        if ch.is_ascii_uppercase() {
            s.push('-');
            s.push(ch.to_ascii_lowercase());
        } else {
            s.push(ch);
        }
    }
    s
}

/// `key: "value"` / `key="value"` (object keys and JSX or SVG attributes).
fn scan_keyed(clean: &str, hint: &str, sink: &mut Sink<'_>) {
    for m in KEYED.captures_iter(clean) {
        let Some(key) = m.name("key") else { continue };
        let Some(v) = m
            .name("dq")
            .or_else(|| m.name("sq"))
            .or_else(|| m.name("bq"))
        else {
            continue;
        };
        sink.value(&kebab(key.as_str()), v.as_str(), v.start(), hint);
    }
}

fn tw_property(prefix: &str, val: &str) -> Option<&'static str> {
    let colour = is_colour(val);
    Some(match (prefix, colour) {
        ("bg", true) => "background-color",
        ("text", true) => "color",
        ("border", true) => "border-color",
        ("outline", true) => "outline-color",
        ("fill", true) => "fill",
        ("stroke", true) => "stroke",
        ("text", false) => "font-size",
        ("border", false) => "border-width",
        ("p", _) => "padding",
        ("m", _) => "margin",
        ("w", _) => "width",
        ("h", _) => "height",
        ("gap", _) => "gap",
        ("rounded", _) => "border-radius",
        _ => return None,
    })
}

fn scan_tailwind(clean: &str, sink: &mut Sink<'_>) {
    for m in TAILWIND.captures_iter(clean) {
        let (Some(pre), Some(v)) = (m.name("prefix"), m.name("val")) else {
            continue;
        };
        let val = v.as_str().replace('_', " "); // Tailwind writes spaces as underscores; same byte length
        let Some(prop) = tw_property(pre.as_str(), &val) else {
            continue;
        };
        sink.value(prop, &val, v.start(), &format!("tw:{}", pre.as_str()));
    }
}

/// R-CHK-019. `properties` are canonical kebab-case names; nothing is reported for an unlisted property.
pub fn scan_literals(text: &str, ext: &str, properties: &[String]) -> Vec<LiteralSite> {
    let Some((fl, line_comments)) = flavour(ext) else {
        return Vec::new();
    };
    if properties.is_empty() {
        return Vec::new();
    }
    let props: BTreeSet<&str> = properties.iter().map(String::as_str).collect();
    let clean = blank_comments(text, fl, line_comments);
    let lines = Lines::new(text);
    let mut sink = Sink {
        text,
        lines: &lines,
        props: &props,
        out: Vec::new(),
    };
    match fl {
        Flavour::Css => scan_css(&clean, &mut sink),
        Flavour::Script => scan_keyed(&clean, "js", &mut sink),
        Flavour::Markup => scan_keyed(&clean, "svg", &mut sink),
    }
    scan_tailwind(&clean, &mut sink);
    let mut out = sink.out;
    out.sort_by(|a, b| {
        a.span
            .start
            .cmp(&b.span.start)
            .then_with(|| a.property.cmp(&b.property))
    });
    out.dedup_by(|a, b| a.span.start == b.span.start && a.property == b.property);
    out
}

/// Path relative to a scope directory (`""` or `"."` = repository root).
pub(crate) fn rel_to<'a>(path: &'a str, dir: &str) -> Option<&'a str> {
    if dir.is_empty() || dir == "." {
        return Some(path);
    }
    path.strip_prefix(dir)?.strip_prefix('/')
}

#[derive(Clone, Debug)]
pub struct PatternProbe {
    pub key: String,
    dir: String,
    re: Regex,
    include: Option<GlobSet>,
    exclude: GlobSet,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PatternHit {
    pub rule_key: String,
    pub span: Span,
    pub text: String,
}

impl PatternProbe {
    pub fn new(
        key: String,
        dir: &RepoPath,
        glob: &[String],
        exclude: &[String],
        forbid: &str,
    ) -> Result<PatternProbe, String> {
        let re = Regex::new(forbid).map_err(|e| format!("bad forbid pattern: {e}"))?;
        let set = |v: &[String]| -> Result<GlobSet, String> {
            let mut b = GlobSetBuilder::new();
            for g in v {
                b.add(Glob::new(g).map_err(|e| format!("bad glob `{g}`: {e}"))?);
            }
            b.build().map_err(|e| e.to_string())
        };
        let include = if glob.is_empty() {
            None
        } else {
            Some(set(glob)?)
        };
        Ok(PatternProbe {
            key,
            dir: dir.as_str().to_string(),
            re,
            include,
            exclude: set(exclude)?,
        })
    }
    pub fn matches_path(&self, p: &RepoPath) -> bool {
        let Some(rel) = rel_to(p.as_str(), &self.dir) else {
            return false;
        };
        self.include.as_ref().is_none_or(|i| i.is_match(rel)) && !self.exclude.is_match(rel)
    }
}

const MAX_HITS_PER_PROBE: usize = 200;

pub fn scan_patterns(text: &str, path: &RepoPath, probes: &[PatternProbe]) -> Vec<PatternHit> {
    let lines = Lines::new(text);
    let mut out = Vec::new();
    for pr in probes.iter().filter(|p| p.matches_path(path)) {
        for m in pr
            .re
            .find_iter(text)
            .filter(|m| !m.as_str().is_empty())
            .take(MAX_HITS_PER_PROBE)
        {
            out.push(PatternHit {
                rule_key: pr.key.clone(),
                span: lines.span(text, m.start(), m.end()),
                text: m.as_str().chars().take(120).collect(),
            });
        }
    }
    out.sort_by(|a, b| {
        a.span
            .start
            .cmp(&b.span.start)
            .then_with(|| a.rule_key.cmp(&b.rule_key))
    });
    out
}

/// Canonical CSS properties the literal-leak scan looks at when a `check = "leak"` rule is declared
/// (amendment A-12). `applies=` on a token narrows this further inside `HN-T001`.
pub const LEAK_PROPERTIES: &[&str] = &[
    "background-color",
    "border-color",
    "caret-color",
    "color",
    "column-rule-color",
    "fill",
    "outline-color",
    "stroke",
    "text-decoration-color",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_chk_020__hex_forms_normalise_to_lower_6_or_8_digits() {
        assert_eq!(normalize_value("#ABC"), "#aabbcc");
        assert_eq!(normalize_value("#ABCD"), "#aabbccdd");
        assert_eq!(normalize_value("#2563EB"), "#2563eb");
        assert_eq!(normalize_value("#2563EB80"), "#2563eb80");
        assert_eq!(normalize_value("#12345"), "#12345"); // invalid length: plain lowercase string
    }

    #[test]
    fn r_chk_020__rgb_integer_forms_become_hex() {
        assert_eq!(normalize_value("rgb(37, 99, 235)"), "#2563eb");
        assert_eq!(normalize_value("RGB(37 99 235)"), "#2563eb");
        assert_eq!(normalize_value("rgba(37,99,235,0.5)"), "#2563eb80");
        assert_eq!(normalize_value("rgba(37,99,235,100%)"), "#2563ebff");
        assert_eq!(normalize_value("rgb(37.5,99,235)"), "rgb(37.5,99,235)"); // not an integer form
    }

    #[test]
    fn r_chk_020__lengths_have_one_canonical_form() {
        assert_eq!(normalize_value(".5REM"), "0.5rem");
        assert_eq!(normalize_value("16.0px"), "16px");
        assert_eq!(normalize_value("16"), "16");
    }

    #[test]
    fn r_chk_020__other_colour_spaces_compare_as_collapsed_lowercase_strings() {
        assert_eq!(
            normalize_value("OKLCH(0.6  0.2   250)"),
            "oklch(0.6 0.2 250)"
        );
    }

    #[test]
    fn r_chk_049__near_colours_are_different_values() {
        assert_ne!(normalize_value("#2563eb"), normalize_value("#2563ec"));
    }

    fn sites(text: &str, ext: &str, props: &[&str]) -> Vec<LiteralSite> {
        let p: Vec<String> = props.iter().map(|s| s.to_string()).collect();
        scan_literals(text, ext, &p)
    }

    #[test]
    fn r_chk_019__css_declaration_in_an_applicable_property() {
        let s = sites(".a { color: #2563EB; margin: 0 }", "css", &["color"]);
        assert_eq!(s.len(), 1);
        let x = &s[0];
        assert_eq!(
            (x.property.as_str(), x.value_norm.as_str(), x.raw.as_str()),
            ("color", "#2563eb", "#2563EB")
        );
        assert_eq!(
            (x.span.line, x.span.col, x.projection_hint.as_deref()),
            (1, 13, Some("css"))
        );
    }

    #[test]
    fn r_chk_021__a_hex_in_a_comment_is_not_a_literal() {
        let s = sites("/* color: #2563eb */\n.a{color:#fff}", "css", &["color"]);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].raw.as_str(), s[0].span.line), ("#fff", 2));
    }

    #[test]
    fn r_chk_019__custom_property_definitions_are_not_property_context() {
        assert!(sites(":root{--color:#2563eb}", "css", &["color"]).is_empty());
    }

    #[test]
    fn r_chk_019__shorthand_colour_component_maps_to_the_longhand() {
        let s = sites(".a{border:1px solid #2563eb}", "css", &["border-color"]);
        assert_eq!((s.len(), s[0].property.as_str()), (1, "border-color"));
    }

    #[test]
    fn r_chk_019__unsupported_extension_or_unlisted_property_yields_nothing() {
        assert!(sites("color: #fff", "md", &["color"]).is_empty());
        assert!(sites(".a{margin:#fff}", "css", &["color"]).is_empty());
        assert!(sites(".a{color:#fff}", "css", &[]).is_empty());
    }

    #[test]
    fn r_chk_019__js_object_key_camel_case_maps_to_the_css_property() {
        let s = sites(
            "const s = { backgroundColor: '#2563EB', margin: 4 };",
            "ts",
            &["background-color"],
        );
        assert_eq!(
            (
                s.len(),
                s[0].value_norm.as_str(),
                s[0].projection_hint.as_deref()
            ),
            (1, "#2563eb", Some("js"))
        );
    }

    #[test]
    fn r_chk_019__jsx_and_svg_presentation_attributes() {
        assert_eq!(
            sites("<path fill=\"#2563eb\" />", "tsx", &["fill"]).len(),
            1
        );
        let s = sites("<path stroke='#fff' />", "svg", &["stroke"]);
        assert_eq!((s.len(), s[0].projection_hint.as_deref()), (1, Some("svg")));
    }

    #[test]
    fn r_chk_019__tailwind_arbitrary_values_map_prefix_to_property() {
        let s = sites(
            r#"<div className="dark:bg-[#2563EB] p-[13px] w-[calc(1px+2px)]" />"#,
            "tsx",
            &["background-color", "padding"],
        );
        assert_eq!(s.len(), 2);
        assert_eq!(
            (
                s[0].property.as_str(),
                s[0].value_norm.as_str(),
                s[0].projection_hint.as_deref()
            ),
            ("background-color", "#2563eb", Some("tw:bg"))
        );
        assert_eq!(
            (s[1].property.as_str(), s[1].raw.as_str()),
            ("padding", "13px")
        );
    }

    #[test]
    fn r_chk_021__computed_class_names_are_not_reported() {
        assert!(
            sites(
                "const c = `bg-${tone}`; const d = 'bg-[${x}]';",
                "tsx",
                &["background-color"]
            )
            .is_empty()
        );
    }

    #[test]
    fn r_chk_021__script_comments_are_blank_but_strings_with_slashes_are_not_comments() {
        assert!(
            sites(
                "// color: \"#fff\"\n/* fill: '#fff' */",
                "tsx",
                &["color", "fill"]
            )
            .is_empty()
        );
        let s = sites(
            "const a = { url: \"http://x\", color: \"#fff\" };",
            "ts",
            &["color"],
        );
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn r_chk_019__columns_count_unicode_scalar_values_of_the_original_line() {
        let s = sites("const é = { color: \"#fff\" }", "ts", &["color"]);
        assert_eq!((s[0].span.line, s[0].span.col), (1, 21));
    }

    fn probe(glob: &[&str], exclude: &[&str], forbid: &str) -> PatternProbe {
        let g: Vec<String> = glob.iter().map(|s| s.to_string()).collect();
        let e: Vec<String> = exclude.iter().map(|s| s.to_string()).collect();
        PatternProbe::new(
            "root:no-console".into(),
            &RepoPath::new("web").unwrap(),
            &g,
            &e,
            forbid,
        )
        .unwrap()
    }

    #[test]
    fn r_chk_060__hits_carry_rule_key_and_one_based_position() {
        let p = probe(&["src/**/*.ts"], &["**/*.test.ts"], r"console\.log");
        let path = RepoPath::new("web/src/a/b.ts").unwrap();
        let hits = scan_patterns("let x;\n  console.log(x);\n", &path, &[p]);
        assert_eq!(hits.len(), 1);
        assert_eq!(
            (
                hits[0].rule_key.as_str(),
                hits[0].span.line,
                hits[0].span.col,
                hits[0].text.as_str()
            ),
            ("root:no-console", 2, 3, "console.log")
        );
    }

    #[test]
    fn r_chk_007__glob_is_relative_to_the_declaring_scope_and_exclude_wins() {
        let p = probe(&["src/**/*.ts"], &["**/*.test.ts"], "x");
        assert!(p.matches_path(&RepoPath::new("web/src/a.ts").unwrap()));
        assert!(!p.matches_path(&RepoPath::new("web/src/a.test.ts").unwrap()));
        assert!(!p.matches_path(&RepoPath::new("other/src/a.ts").unwrap()));
        assert!(!p.matches_path(&RepoPath::new("web/lib/a.ts").unwrap()));
    }

    #[test]
    fn r_chk_005__invalid_forbid_regex_is_an_error_not_a_panic() {
        let r = PatternProbe::new("k".into(), &RepoPath::new("web").unwrap(), &[], &[], "(");
        assert!(r.is_err());
    }
}
