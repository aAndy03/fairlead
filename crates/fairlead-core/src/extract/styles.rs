//! Regex-level CSS custom-property extractor (R-FILE-034). No DOM inference (R-FILE-036).
use super::CssTokenSite;
use crate::config::ControlSet;
use crate::path::RepoPath;
use fairlead_syntax::diag::Span;
use fairlead_syntax::directive::{Extent, Verb};
use fairlead_syntax::extract::Extraction;
use std::collections::BTreeSet;

/// Which files get the CSS extractor: by extension, or because a `[[file]]` item whose file kind
/// has a `css*` extractor declares them.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StyleKinds {
    pub extensions: BTreeSet<String>,
    pub declared: BTreeSet<RepoPath>,
}

impl Default for StyleKinds {
    fn default() -> Self {
        Self {
            extensions: ["css", "scss", "less"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            declared: BTreeSet::new(),
        }
    }
}

impl StyleKinds {
    pub fn applies(&self, path: &RepoPath) -> bool {
        self.declared.contains(path)
            || self
                .extensions
                .contains(&path.extension().to_ascii_lowercase())
    }

    pub fn from_controls(controls: &ControlSet) -> Self {
        let mut out = Self::default();
        let kinds: Vec<(&String, &crate::config::FileKindDecl)> = controls
            .ok_files()
            .flat_map(|c| c.file_kinds.iter())
            .collect();
        for c in controls.ok_files() {
            for f in &c.files {
                let is_css = kinds
                    .iter()
                    .any(|(name, k)| **name == f.kind && k.extractor.starts_with("css"));
                if is_css {
                    let joined = if c.dir.is_root() {
                        f.path.clone()
                    } else {
                        format!("{}/{}", c.dir.as_str(), f.path)
                    };
                    if let Ok(p) = RepoPath::new(&joined) {
                        out.declared.insert(p);
                    }
                }
            }
        }
        out
    }
}

/// Line ranges (1-based, inclusive) of `def foreign.*` regions found by the syntax extraction.
pub fn foreign_ranges(extraction: &Extraction) -> Vec<(u32, u32)> {
    extraction
        .directives
        .iter()
        .filter(|d| {
            d.verb == Verb::Def
                && d.ids
                    .first()
                    .is_some_and(|id| id.kind_name().starts_with("foreign."))
        })
        .filter_map(|d| match d.extent {
            Extent::Lines { first, last } => Some((first, last)),
            _ => None,
        })
        .collect()
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flush(
    seg: &str,
    pos: Option<(usize, u32, u32)>,
    stack: &[String],
    foreign: &[(u32, u32)],
    out: &mut Vec<CssTokenSite>,
) {
    let Some((start, line, col)) = pos else {
        return;
    };
    let Some(rest) = seg.trim().strip_prefix("--") else {
        return;
    };
    let Some((name, value)) = rest.split_once(':') else {
        return;
    };
    let name = name.trim_end();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return;
    }
    let full = format!("--{name}");
    out.push(CssTokenSite {
        value: collapse(value),
        context: stack.join(" > "),
        span: Span {
            start: u32::try_from(start).unwrap_or(u32::MAX),
            end: u32::try_from(start + full.len()).unwrap_or(u32::MAX),
            line,
            col,
        },
        in_foreign: foreign.iter().any(|(a, b)| *a <= line && line <= *b),
        name: full,
    });
}

/// Scan CSS-like text. `foreign` are the line ranges of foreign regions. Pure, total, never panics.
pub fn extract_css_tokens(text: &str, foreign: &[(u32, u32)]) -> Vec<CssTokenSite> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut seg = String::new();
    let mut pos: Option<(usize, u32, u32)> = None;
    let (mut line, mut col) = (1u32, 1u32);
    let mut paren = 0u32;
    let mut i = 0usize;
    while let Some(&(byte, c)) = chars.get(i) {
        let next = chars.get(i + 1).map(|p| p.1);
        if c == '/' && next == Some('*') {
            let mut j = i + 2;
            while let Some(&(_, d)) = chars.get(j) {
                if d == '*' && chars.get(j + 1).map(|p| p.1) == Some('/') {
                    j += 2;
                    break;
                }
                j += 1;
            }
            for &(_, d) in chars.get(i..j.min(chars.len())).unwrap_or(&[]) {
                if d == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
            }
            seg.push(' ');
            i = j;
            continue;
        }
        if c == '"' || c == '\'' {
            if pos.is_none() {
                pos = Some((byte, line, col));
            }
            seg.push(c);
            col += 1;
            i += 1;
            while let Some(&(_, d)) = chars.get(i) {
                seg.push(d);
                i += 1;
                if d == '\n' {
                    line += 1;
                    col = 1;
                    break;
                }
                col += 1;
                if d == '\\' {
                    if let Some(&(_, e)) = chars.get(i) {
                        seg.push(e);
                        i += 1;
                        if e == '\n' {
                            line += 1;
                            col = 1;
                        } else {
                            col += 1;
                        }
                    }
                    continue;
                }
                if d == c {
                    break;
                }
            }
            continue;
        }
        match c {
            '{' if paren == 0 => {
                stack.push(collapse(&seg));
                seg.clear();
                pos = None;
            }
            '}' if paren == 0 => {
                flush(&seg, pos, &stack, foreign, &mut out);
                stack.pop();
                seg.clear();
                pos = None;
            }
            ';' if paren == 0 => {
                flush(&seg, pos, &stack, foreign, &mut out);
                seg.clear();
                pos = None;
            }
            _ => {
                if c == '(' {
                    paren += 1;
                } else if c == ')' {
                    paren = paren.saturating_sub(1);
                }
                if pos.is_none() && !c.is_whitespace() {
                    pos = Some((byte, line, col));
                }
                if !(seg.is_empty() && c.is_whitespace()) {
                    seg.push(c);
                }
            }
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
        i += 1;
    }
    flush(&seg, pos, &stack, foreign, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sites(text: &str) -> Vec<CssTokenSite> {
        extract_css_tokens(text, &[])
    }

    fn triple(s: &CssTokenSite) -> (&str, &str, &str) {
        (s.name.as_str(), s.context.as_str(), s.value.as_str())
    }

    #[test]
    fn r_file_034__one_token_in_root_dark_and_theme_is_three_value_sites() {
        let css = ":root { --primary: #2563eb; --radius: 0.5rem }\n.dark { --primary: #60a5fa; }\n@theme inline { --color-primary: var(--primary); }\n";
        let s = sites(css);
        let got: Vec<(&str, &str, &str)> = s.iter().map(triple).collect();
        assert_eq!(
            got,
            [
                ("--primary", ":root", "#2563eb"),
                ("--radius", ":root", "0.5rem"),
                ("--primary", ".dark", "#60a5fa"),
                ("--color-primary", "@theme inline", "var(--primary)"),
            ]
        );
    }

    #[test]
    fn r_file_034__nested_at_rules_join_their_preludes() {
        let s = sites("@media (min-width: 600px) {\n  :root {\n    --gap:   4px\n  }\n}\n");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].context, "@media (min-width: 600px) > :root");
        assert_eq!(s[0].value, "4px");
    }

    #[test]
    fn r_file_034__comments_strings_and_urls_do_not_confuse_the_scanner() {
        let css = "/* --fake: 1; */\n:root {\n  content: \"; --nope: 1\";\n  --img: url(data:image/png;base64,AAA);\n  --a: 1 /* x */ 2;\n  --s: 'a;b';\n}\n";
        let s = sites(css);
        let got: Vec<(&str, &str)> = s
            .iter()
            .map(|x| (x.name.as_str(), x.value.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("--img", "url(data:image/png;base64,AAA)"),
                ("--a", "1 2"),
                ("--s", "'a;b'")
            ]
        );
    }

    #[test]
    fn r_file_034__span_is_original_bytes_with_one_based_line_and_scalar_column() {
        let css = "/* é */\n:root {\n  --x: 1;\n}\n";
        let s = sites(css);
        assert_eq!(s.len(), 1);
        let sp = s[0].span;
        assert_eq!((sp.line, sp.col), (3, 3));
        assert_eq!(&css[sp.start as usize..sp.end as usize], "--x");
    }

    #[test]
    fn r_arch_111__crlf_gives_the_same_sites_except_for_byte_offsets() {
        let lf = ":root {\n  --a: 1;\n  --b: 2;\n}\n";
        let crlf = lf.replace('\n', "\r\n");
        let a = sites(lf);
        let b = sites(&crlf);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(
                (&x.name, &x.value, &x.context, x.span.line, x.span.col),
                (&y.name, &y.value, &y.context, y.span.line, y.span.col)
            );
        }
        assert_ne!(
            a[1].span.start, b[1].span.start,
            "offsets refer to the original bytes"
        );
    }

    #[test]
    fn r_file_042__sites_inside_a_foreign_region_are_flagged_not_dropped() {
        let css = ":root {\n  --mine: 1;\n  --shadcn: 2;\n}\n";
        let s = extract_css_tokens(css, &[(3, 3)]);
        assert_eq!(
            s.iter().map(|x| x.in_foreign).collect::<Vec<_>>(),
            [false, true]
        );
    }

    #[test]
    fn r_file_034__garbage_never_panics_and_yields_nothing_wrong() {
        for g in [
            "",
            "}}}{{{",
            "--: 1;",
            "--a",
            "--a:",
            "{ --x: 1",
            "/* open",
            "\"open",
            ":root { --a: url( }",
            "--é-ü: 1;",
            "a{--b:1}c{--d:2}",
        ] {
            let _ = extract_css_tokens(g, &[]);
        }
        assert_eq!(sites("--a:").len(), 1);
        assert_eq!(sites("--a:")[0].value, "");
        assert!(sites("--: 1;").is_empty());
        assert!(sites("--a").is_empty());
        let two = sites("a{--b:1}c{--d:2}");
        assert_eq!(
            two.iter().map(|s| s.context.as_str()).collect::<Vec<_>>(),
            ["a", "c"]
        );
    }

    #[test]
    fn r_file_034__style_kinds_apply_by_extension_or_declaration() {
        let mut k = StyleKinds::default();
        let p = |s: &str| RepoPath::new(s).unwrap();
        assert!(
            k.applies(&p("app/globals.css"))
                && k.applies(&p("a/b.SCSS"))
                && k.applies(&p("x.less"))
        );
        assert!(!k.applies(&p("a.ts")));
        k.declared.insert(p("theme/tokens.txt"));
        assert!(k.applies(&p("theme/tokens.txt")));
    }

    #[test]
    fn r_file_034__style_kinds_follow_declared_stylesheet_file_items() {
        use crate::config::{ControlSet, parse_control};
        let root = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n[file-kinds.stylesheet]\nextractor = \"css-custom-properties\"\nbinding = \"import\"\n[file-kinds.other]\nextractor = \"json\"\nbinding = \"none\"\n[[file]]\nid = \"file.styles\"\nkind = \"stylesheet\"\npath = \"app/theme.pcss\"\n[[file]]\nid = \"file.cfg\"\nkind = \"other\"\npath = \"cfg.json\"\n";
        let mut set = ControlSet::default();
        let path = RepoPath::new("fairlead/control.toml").unwrap();
        set.files
            .insert(path.clone(), parse_control(&path, root.as_bytes(), true));
        let k = StyleKinds::from_controls(&set);
        assert!(k.applies(&RepoPath::new("app/theme.pcss").unwrap()));
        assert!(!k.applies(&RepoPath::new("cfg.json").unwrap()));
    }

    #[test]
    fn r_file_042__foreign_ranges_come_from_def_foreign_regions() {
        use fairlead_syntax::delimiters::CommentTable;
        use fairlead_syntax::extract::ExtractConfig;
        let cfg = ExtractConfig {
            marker: "fairlead".into(),
            comments: CommentTable::builtin(),
            header_skip: vec![],
            max_file: 2_000_000,
            max_directives: 500,
        };
        let css = "/* fairlead: def foreign.shadcn tool=shadcn +end */\n  --chart-1: red;\n/* fairlead: end */\n:root { --mine: 1; }\n";
        let ex = fairlead_syntax::extract::extract(css.as_bytes(), "css", &cfg);
        let ranges = foreign_ranges(&ex);
        assert_eq!(ranges.len(), 1, "{ranges:?}");
        let s = extract_css_tokens(css, &ranges);
        assert_eq!(
            s.iter()
                .map(|x| (x.name.as_str(), x.in_foreign))
                .collect::<Vec<_>>(),
            [("--chart-1", true), ("--mine", false)]
        );
    }
}
