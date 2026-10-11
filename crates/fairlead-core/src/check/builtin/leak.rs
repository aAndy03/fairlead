//! Literal token leak: declared value plus property context (R-CHK-017..022, D-43), with the
//! 0.3 additions of R-VAR-093 to R-VAR-098: canonical colours and lengths, near misses
//! (`HN-T006`), `match=` constants and one level of `var()`.
use crate::tokens::canon::{self, Value};
use crate::{
    check::{
        CheckCx,
        builtin::{bare_id, field_list, field_str},
        finding::{DiagB, clip},
        rules::{Check, DeclaredRule, RuleCheck},
    },
    classify::FileClass,
    diag::{Diagnostic, codes::*},
    extract::{
        FileRecord, LiteralSite,
        literals::{normalize_value, rel_to},
    },
    path::RepoPath,
    registry::{ItemId, Lookup, Registry, ScopeId},
};
use fairlead_syntax::{
    diag::DiagCode,
    directive::{Extent, Verb},
    ids::Id,
};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::BTreeSet;

pub struct LeakCheck;
const CODES: [DiagCode; 2] = [HN_T001, HN_T006];

struct Tok {
    id: String,
    item: ItemId,
    applies: BTreeSet<String>,
    min_len: usize,
    /// 0.1 normalised values (R-CHK-020).
    values: BTreeSet<String>,
    /// 0.3 canonical values (R-VAR-093), every theme.
    canon: Vec<Value>,
    /// `match=` (R-VAR-096): the string key of a distinctive constant.
    matches: Option<String>,
    def_lines: BTreeSet<(RepoPath, u32)>,
}

impl Tok {
    fn add(&mut self, raw: &str, root_px: f64) {
        self.values.insert(normalize_value(raw));
        if let Some(c) = canon::parse(raw, root_px, false).filter(|c| !c.ambiguous()) {
            self.canon.push(c);
        }
    }
    /// A property is covered by `applies` when listed, or when it is a longhand of a listed
    /// box shorthand (`padding-top` under `padding`).
    fn applies_to(&self, prop: &str) -> bool {
        self.applies.contains(prop)
            || ["padding", "margin", "inset"].iter().any(|s| {
                self.applies.contains(*s)
                    && prop.strip_prefix(s).is_some_and(|r| r.starts_with('-'))
            })
    }
}

/// `var(--x)` or `var(--x, fallback)`: the bare name `x`.
pub(crate) fn var_target(v: &str) -> Option<&str> {
    let inner = v.trim().strip_prefix("var(")?;
    let name = inner.split([',', ')']).next()?.trim();
    name.starts_with("--").then(|| name.trim_start_matches('-'))
}

fn token(cx: &CheckCx<'_>, from: &ScopeId, id: &Id, root_px: f64) -> Option<Tok> {
    let Lookup::Found(iid) = cx.registry.lookup(from, id) else {
        return None;
    };
    let it = cx.registry.item(&iid)?;
    let applies: BTreeSet<String> = field_list(it, "applies").into_iter().collect();
    // R-VAR-096: a constant is scanned only with `match=`, and never an ambiguous one.
    let matches = field_str(it, "match")
        .filter(|m| m.chars().count() >= 8)
        .map(crate::extract::literals::string_key);
    if applies.is_empty() && matches.is_none() {
        return None;
    } // opt-in (R-CHK-017, R-FILE-039)
    let min_len = field_str(it, "min-len")
        .and_then(|s| s.parse().ok())
        .unwrap_or(4usize);
    let mut t = Tok {
        id: bare_id(&iid),
        item: iid,
        applies,
        min_len,
        values: BTreeSet::new(),
        canon: Vec::new(),
        matches,
        def_lines: BTreeSet::new(),
    };
    if let Some(v) = field_str(it, "value") {
        t.add(v, root_px);
    }
    // R-VAR-090: `css=` names the custom property (0.1: a `sym` starting with `--`).
    let css = field_str(it, "css").or_else(|| field_str(it, "sym").filter(|s| s.starts_with("--")));
    if let Some(sym) = css {
        let name = sym.trim_start_matches('-');
        let sites = |n: &str| -> Vec<(&RepoPath, &crate::extract::CssTokenSite)> {
            cx.corpus
                .records
                .iter()
                .flat_map(|(p, r)| r.css_tokens.iter().map(move |s| (p, s)))
                .filter(|(_, s)| s.name.trim_start_matches('-') == n && !s.in_foreign)
                .collect()
        };
        for (p, s) in sites(name) {
            t.def_lines.insert((p.clone(), s.span.line));
            match var_target(&s.value) {
                // R-VAR-098: one level of `var()` is resolved; a deeper chain is never compared.
                Some(next) => {
                    for (_, s2) in sites(next) {
                        if var_target(&s2.value).is_none() {
                            t.add(&s2.value, root_px);
                        }
                    }
                }
                None => t.add(&s.value, root_px),
            }
        }
    }
    Some(t)
}

fn skip_sets(reg: &Registry, scope: &ScopeId) -> Vec<(String, GlobSet)> {
    reg.chain(scope)
        .into_iter()
        .filter_map(|s| {
            let sc = reg.scope(s)?;
            let ctl = sc.control.as_ref().filter(|c| !c.scan.skip.is_empty())?;
            let mut b = GlobSetBuilder::new();
            for g in &ctl.scan.skip {
                if let Ok(x) = Glob::new(g) {
                    b.add(x);
                }
            }
            Some((sc.dir.as_str().to_string(), b.build().ok()?))
        })
        .collect()
}
fn foreign_lines(rec: &FileRecord) -> Vec<(u32, u32)> {
    rec.extraction
        .as_ref()
        .map(|e| {
            e.directives
                .iter()
                .filter(|d| {
                    d.verb == Verb::Def
                        && d.ids.first().is_some_and(|i| i.kind.as_str() == "foreign")
                })
                .filter_map(|d| match &d.extent {
                    Extent::Lines { first, last } => Some((*first, *last)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}
/// R-CHK-022: the exact replacement when the token offers a projection, else the token ID.
fn replacement(reg: &Registry, t: &Tok, lit: &LiteralSite) -> String {
    let item = reg.item(&t.item);
    let sym = item
        .and_then(|i| field_str(i, "css"))
        .or_else(|| item.and_then(|i| field_str(i, "sym")))
        .filter(|s| s.starts_with("--"));
    if let Some(prefix) = lit
        .projection_hint
        .as_deref()
        .and_then(|h| h.strip_prefix("tw:"))
    {
        if let Some(c) = item.and_then(|i| field_str(i, "use-tw")) {
            return format!("use {c}");
        }
        if let Some(s) = sym {
            return format!("use {prefix}-{}", s.trim_start_matches('-'));
        }
    } else {
        if let Some(c) = item.and_then(|i| field_str(i, "use-css")) {
            return format!("use {c}");
        }
        if let Some(s) = sym {
            return format!("use var({s})");
        }
    }
    format!("use {}", t.id)
}

impl Check for LeakCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let tcfg = crate::check::builtin::root_control(reg)
            .map(|c| c.tokens.clone())
            .unwrap_or_default();
        let root_px = tcfg.root_px;
        let rules: Vec<(&DeclaredRule, Vec<Tok>)> = cx
            .rules
            .for_scope(scope)
            .iter()
            .filter_map(|r| match &r.check {
                RuleCheck::Leak { tokens } => Some((
                    r,
                    tokens
                        .iter()
                        .filter_map(|id| token(cx, scope, id, root_px))
                        .collect::<Vec<_>>(),
                )),
                RuleCheck::Pattern { .. } => None,
            })
            .filter(|(_, t)| !t.is_empty())
            .collect();
        if rules.is_empty() {
            return;
        }
        let skips = skip_sets(reg, scope);
        for (path, rec) in cx
            .corpus
            .records
            .iter()
            .filter(|(p, _)| reg.scope_of(p) == Some(scope))
        {
            if rec.literals.is_empty() || rec.skip.is_some() || rec.class != FileClass::Authored {
                continue;
            }
            // R-VAR-094: minified files are never scanned.
            let name = path.file_name();
            if name.contains(".min.") {
                continue;
            }
            if skips
                .iter()
                .any(|(dir, set)| rel_to(path.as_str(), dir).is_some_and(|r| set.is_match(r)))
            {
                continue;
            }
            let foreign = foreign_lines(rec);
            for lit in rec.literals.iter().filter(|l| {
                !foreign
                    .iter()
                    .any(|(a, b)| l.span.line >= *a && l.span.line <= *b)
            }) {
                // R-VAR-093: the canonical form; an ambiguous value is never compared (R-VAR-096).
                let c = canon::parse(
                    &lit.raw,
                    root_px,
                    lit.projection_hint.as_deref() == Some("js-px"),
                );
                if lit.property.is_empty() {
                    // a string literal: only `match=` tokens (R-VAR-096)
                    for (rule, toks) in &rules {
                        if !cx.rules.applies(rule, path, reg) {
                            continue;
                        }
                        if let Some(t) = toks
                            .iter()
                            .find(|t| t.matches.as_deref() == Some(lit.value_norm.as_str()))
                        {
                            out.push(
                                DiagB::new(
                                    HN_T001,
                                    path,
                                    lit.span.line,
                                    lit.span.col,
                                    format!("string literal is the match= value of {}", t.id),
                                )
                                .rule(rule.id.0.as_str())
                                .related(t.id.clone())
                                .text(t.id.clone())
                                .severity(rule.sev)
                                .hint(clip(&replacement(reg, t, lit), 100))
                                .build(),
                            );
                        }
                    }
                    continue;
                }
                if c.as_ref().is_some_and(Value::ambiguous) {
                    continue;
                }
                let key = c.as_ref().map(Value::key);
                let len = key
                    .as_ref()
                    .map_or(lit.value_norm.chars().count(), |k| k.chars().count());
                for (rule, toks) in &rules {
                    if !cx.rules.applies(rule, path, reg) {
                        continue;
                    }
                    let usable = |t: &&Tok| {
                        t.applies_to(&lit.property)
                            && !t.def_lines.contains(&(path.clone(), lit.span.line))
                    };
                    let hit = toks.iter().filter(usable).find(|t| {
                        len >= t.min_len
                            && (t.values.contains(&lit.value_norm)
                                || key
                                    .as_ref()
                                    .is_some_and(|k| t.canon.iter().any(|v| v.key() == *k)))
                    });
                    if hit.is_none()
                        && let Some(c) = &c
                    {
                        // R-VAR-095: a near miss is info, with the replacement in the message.
                        let near = toks
                            .iter()
                            .filter(usable)
                            .filter_map(|t| {
                                let d = t
                                    .canon
                                    .iter()
                                    .filter_map(|v| c.distance(v))
                                    .filter(|d| *d > 0 && *d <= tcfg.near)
                                    .min()?;
                                Some((d, t))
                            })
                            .min_by_key(|(d, _)| *d);
                        if let Some((d, t)) = near {
                            let fix = replacement(reg, t, lit);
                            out.push(
                                DiagB::new(
                                    HN_T006,
                                    path,
                                    lit.span.line,
                                    lit.span.col,
                                    format!(
                                        "{} on {} is near {} (distance {d}); {}",
                                        lit.raw,
                                        lit.property,
                                        t.id,
                                        fix.trim_start_matches("use ")
                                    ),
                                )
                                .rule(rule.id.0.as_str())
                                .related(t.id.clone())
                                .text(lit.raw.clone())
                                .hint(clip(&format!("if it is meant to be {}, {fix}", t.id), 100))
                                .build(),
                            );
                        }
                    }
                    if let Some(t) = hit {
                        out.push(
                            DiagB::new(
                                HN_T001,
                                path,
                                lit.span.line,
                                lit.span.col,
                                format!("literal {} is {} ({})", lit.raw, t.id, lit.property),
                            )
                            .rule(rule.id.0.as_str())
                            .related(t.id.clone())
                            .text(lit.raw.clone())
                            .severity(rule.sev)
                            .hint(clip(&replacement(reg, t, lit), 100))
                            .build(),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        check::testkit::{World, of},
        diag::codes as c,
    };

    const RULE: &str = "[kinds.tok]\nshape = \"doc\"\n[rules.no-raw-hex]\ncheck = \"leak\"\ntokens = [\"tok.*\"]\nsev = \"error\"\nstage = \"enforce\"\nglob = [\"**/*.css\", \"**/*.tsx\"]\nexclude = [\"**/fixtures/**\"]\nwhy = \"colours come from tokens\"\n[scan]\nskip = [\"src/gen/**\"]\n";
    fn tok(extra: &str) -> String {
        format!("+++\ndirectives = ['def tok.primary sym=--primary {extra}']\n+++\n# Tokens\n")
    }
    const CSS: &str = ":root { --primary: #2563eb; }\n.dark { --primary: #60a5fa; }\n";
    fn world(token: &str) -> World {
        World::new()
            .root(RULE)
            .file("fairlead/contract/tokens.md", token)
            .file("app/globals.css", CSS)
    }

    #[test]
    fn r_chk_017__r_chk_018__r_var_091__declared_value_in_an_applicable_property_is_reported_in_any_theme()
     {
        let src = "const a = { color: '#2563EB' };\nconst b = { backgroundColor: 'rgb(37, 99, 235)' };\nconst c = { color: '#60A5FA' };\nconst d = { borderColor: '#2563eb' };\nconst e = '#2563eb';\nconst f = { color: '#2563ec' };\n";
        let b = world(&tok("applies=color,background-color"))
            .file("src/Panel.tsx", src)
            .build();
        let ds = b.run_check(&LeakCheck);
        let t = of(&ds, c::HN_T001);
        assert_eq!(
            t.iter().map(|d| d.location.line).collect::<Vec<_>>(),
            vec![1, 2, 3]
        ); // not d (property), e (no context), f (near)
        assert_eq!(
            (
                t[0].rule.as_deref(),
                t[0].related.as_deref(),
                t[0].text.as_str()
            ),
            (Some("rule.no-raw-hex"), Some("tok.primary"), "#2563EB")
        );
        assert_eq!(t[0].hint, "use var(--primary)");
    }

    #[test]
    fn r_file_039__a_token_without_applies_is_never_leak_checked() {
        let b = world(&tok(""))
            .file("src/Panel.tsx", "const a = { color: '#2563eb' };\n")
            .build();
        assert!(of(&b.run_check(&LeakCheck), c::HN_T001).is_empty());
    }

    #[test]
    fn r_chk_020__values_shorter_than_min_len_are_never_checked() {
        let doc = "+++\ndirectives = ['def tok.gap value=4px applies=padding']\n+++\n";
        let src = "const a = { padding: '4px' };\n";
        let b = world(&tok("applies=color"))
            .file("fairlead/contract/gap.md", doc)
            .file("src/a.tsx", src)
            .build();
        assert!(of(&b.run_check(&LeakCheck), c::HN_T001).is_empty());
        let doc3 = "+++\ndirectives = ['def tok.gap value=4px applies=padding min-len=3']\n+++\n";
        let b = world(&tok("applies=color"))
            .file("fairlead/contract/gap.md", doc3)
            .file("src/a.tsx", src)
            .build();
        assert_eq!(of(&b.run_check(&LeakCheck), c::HN_T001).len(), 1);
    }

    #[test]
    fn r_chk_021__foreign_regions_exclude_skip_and_definition_sites_are_exempt() {
        let foreign = "/* fairlead: def foreign.shadcn tool=shadcn +end */\n.x { color: #2563eb; }\n/* fairlead: end */\n.y { color: #2563eb; }\n";
        let hit = "const a = { color: '#2563eb' };\n";
        let b = world(&tok("applies=color"))
            .file("src/a.css", foreign)
            .file("src/fixtures/a.tsx", hit)
            .file("src/gen/b.tsx", hit)
            .file("src/c.tsx", "// color: '#2563eb'\n")
            .build();
        let ds = b.run_check(&LeakCheck);
        let t = of(&ds, c::HN_T001);
        assert_eq!(t.len(), 1);
        assert_eq!(
            (t[0].location.path.as_str(), t[0].location.line),
            ("src/a.css", 4)
        );
    }

    #[test]
    fn r_chk_022__fix_hint_names_the_exact_replacement() {
        let b = world(&tok("applies=background-color"))
            .file("src/a.tsx", "const c = \"bg-[#2563eb]\";\n")
            .build();
        assert_eq!(
            of(&b.run_check(&LeakCheck), c::HN_T001)[0].hint,
            "use bg-primary"
        );
        let doc = "+++\ndirectives = ['def tok.plain value=#abcdef applies=color']\n+++\n";
        let b = World::new()
            .root(RULE)
            .file("fairlead/contract/p.md", doc)
            .file("src/a.tsx", "const a = { color: '#abcdef' };\n")
            .build();
        assert_eq!(
            of(&b.run_check(&LeakCheck), c::HN_T001)[0].hint,
            "use tok.plain"
        );
    }

    const RULE3: &str = "[kinds.tok]\nshape = \"doc\"\n[rules.no-raw]\ncheck = \"leak\"\ntokens = [\"tok.*\"]\nsev = \"error\"\nstage = \"enforce\"\nglob = [\"src/**\"]\nwhy = \"values come from tokens\"\n";
    fn w3(root_extra: &str, defs: &[&str], css: &str) -> World {
        let list = defs
            .iter()
            .map(|d| format!("'{d}'"))
            .collect::<Vec<_>>()
            .join(", ");
        World::new()
            .root(&format!("{RULE3}{root_extra}"))
            .file(
                "fairlead/contract/tokens.md",
                &format!("+++\ndirectives = [{list}]\n+++\n# Tokens\n"),
            )
            .file("app/tokens.css", css)
    }
    const BRAND: &str = "def tok.brand css=--brand applies=color,background-color,border-color,fill,stroke use-css=\"var(--brand)\" use-tw=bg-brand";
    const SPACE: &str = "def tok.space-4 css=--space-4 applies=padding,margin,gap use-css=\"var(--space-4)\" use-tw=p-4";
    const CSS3: &str =
        ":root { --brand: #3B82F6; --space-4: 16px; }\n.dark { --brand: #60A5FA; }\n";

    fn lines(ds: &[Diagnostic], code: DiagCode, path: &str) -> Vec<u32> {
        of(ds, code)
            .iter()
            .filter(|d| d.location.path.as_str() == path)
            .map(|d| d.location.line)
            .collect()
    }

    #[test]
    fn r_var_093__rem_equals_px_at_root_px() {
        let src = ".a { padding: 1rem; }\n.b { margin-top: 16px; }\n.c { gap: 1.5rem; }\n.d { font-size: 16px; }\n";
        let ds = w3("", &[BRAND, SPACE], CSS3)
            .file("src/a.css", src)
            .build()
            .run_check(&LeakCheck);
        assert_eq!(lines(&ds, c::HN_T001, "src/a.css"), vec![1, 2], "{ds:#?}");
        let ds = w3("[tokens]\nroot-px = 10\n", &[BRAND, SPACE], CSS3)
            .file(
                "src/a.css",
                ".a { padding: 1.6rem; }\n.b { padding: 1rem; }\n",
            )
            .build()
            .run_check(&LeakCheck);
        assert_eq!(lines(&ds, c::HN_T001, "src/a.css"), vec![1]);
    }

    #[test]
    fn r_var_093__colour_forms_and_themes_compare_canonically() {
        let src = ".a { color: rgb(59 130 246); }\n.b { background-color: hsl(217.2 91.2% 59.8%); }\n.c { border-color: #60a5fa; }\n.d { color: #38f; }\n";
        let ds = w3("", &[BRAND], CSS3)
            .file("src/a.css", src)
            .build()
            .run_check(&LeakCheck);
        assert_eq!(
            lines(&ds, c::HN_T001, "src/a.css"),
            vec![1, 2, 3],
            "{ds:#?}"
        );
    }

    #[test]
    fn r_var_092__css_declarations_are_a_property_context() {
        let ds = w3("", &[BRAND], CSS3)
            .file("src/a.css", ".a { color: #3b82f6; }\n")
            .build()
            .run_check(&LeakCheck);
        assert_eq!(lines(&ds, c::HN_T001, "src/a.css"), vec![1]);
    }

    #[test]
    fn r_var_092__js_style_objects_read_camel_case_and_unitless_px() {
        let src = "const a = { backgroundColor: '#3b82f6' };\nconst b = { padding: 16 };\nconst c = { fontSize: 16 };\nconst d = { lineHeight: 16 };\n";
        let ds = w3("", &[BRAND, SPACE], CSS3)
            .file("src/a.tsx", src)
            .build()
            .run_check(&LeakCheck);
        assert_eq!(
            lines(&ds, c::HN_T001, "src/a.tsx"),
            vec![1, 2],
            "fontSize is not in applies: {ds:#?}"
        );
    }

    #[test]
    fn r_var_092__presentation_attributes_are_a_property_context() {
        let src = "<svg><path fill=\"#3b82f6\" stroke=\"#3B82F6\" d=\"M0\"/></svg>\n";
        let ds = w3("", &[BRAND], CSS3)
            .file("src/i.svg", src)
            .build()
            .run_check(&LeakCheck);
        assert_eq!(of(&ds, c::HN_T001).len(), 2, "{ds:#?}");
    }

    #[test]
    fn r_var_092__tailwind_arbitrary_values_use_the_prefix_table() {
        let src = "const a = \"bg-[#3B82F6] px-[16px]\";\nconst b = \"surface-[#3b82f6]\";\n";
        let ds = w3("", &[BRAND, SPACE], CSS3)
            .file("src/a.tsx", src)
            .build()
            .run_check(&LeakCheck);
        let t = of(&ds, c::HN_T001);
        assert_eq!(
            t.len(),
            2,
            "an unknown prefix is skipped, not guessed: {ds:#?}"
        );
        assert_eq!(t[0].hint, "use bg-brand");
        let ds = w3(
            "[tokens.tailwind]\nsurface = \"background-color\"\n",
            &[BRAND, SPACE],
            CSS3,
        )
        .file("src/a.tsx", src)
        .build()
        .run_check(&LeakCheck);
        assert_eq!(
            lines(&ds, c::HN_T001, "src/a.tsx"),
            vec![1, 1, 2],
            "the control file's table"
        );
    }

    #[test]
    fn r_var_095__near_miss_is_info_with_replacement() {
        let src = ".a { color: #3B83F6; }\n.b { color: #3B86F6; }\n.c { font-size: #3b83f6; }\n";
        let ds = w3("", &[BRAND], CSS3)
            .file("src/a.css", src)
            .build()
            .run_check(&LeakCheck);
        let n = of(&ds, c::HN_T006);
        assert_eq!(
            n.len(),
            1,
            "distance 4 is not near, font-size not in applies: {ds:#?}"
        );
        assert_eq!(n[0].location.line, 1);
        assert_eq!(n[0].severity, fairlead_syntax::diag::Severity::Info);
        assert!(
            n[0].message
                .contains("near tok.brand (distance 1); var(--brand)"),
            "{}",
            n[0].message
        );
        assert!(of(&ds, c::HN_T001).is_empty());
        let wide = w3("[tokens]\nnear = 4\n", &[BRAND], CSS3)
            .file("src/a.css", src)
            .build()
            .run_check(&LeakCheck);
        assert_eq!(of(&wide, c::HN_T006).len(), 2);
    }

    #[test]
    fn r_var_096__ambiguous_values_never_scanned() {
        let zero = "def tok.none value=0 applies=padding,margin min-len=1";
        let src = ".a { padding: 0; }\n.b { margin: 0px; }\nconst x = { padding: 0 };\n";
        let ds = w3("", &[zero], CSS3)
            .file("src/a.css", src)
            .build()
            .run_check(&LeakCheck);
        assert!(of(&ds, c::HN_T001).is_empty(), "{ds:#?}");
        // a distinctive constant only with match=
        let url = "def tok.api match=\"https://api.example.com/v2\"";
        let src =
            "const u = \"https://api.example.com/v2\";\nconst v = \"https://other.example.com\";\n";
        let ds = w3("", &[url], CSS3)
            .file("src/a.ts", src)
            .build()
            .run_check(&LeakCheck);
        assert_eq!(lines(&ds, c::HN_T001, "src/a.ts"), vec![1], "{ds:#?}");
        let ds = w3("", &[BRAND], CSS3)
            .file("src/a.ts", src)
            .build()
            .run_check(&LeakCheck);
        assert!(
            of(&ds, c::HN_T001).is_empty(),
            "no match= token: strings never scanned"
        );
    }

    #[test]
    fn r_var_098__one_level_of_var_is_resolved_and_deeper_is_not_compared() {
        let side = "def tok.sidebar css=--sidebar applies=background-color";
        let one = ":root { --background: #fafafa; --sidebar: var(--background); }\n";
        let src = ".a { background-color: #fafafa; }\n";
        let ds = w3("", &[side], one)
            .file("src/a.css", src)
            .build()
            .run_check(&LeakCheck);
        assert_eq!(lines(&ds, c::HN_T001, "src/a.css"), vec![1]);
        let two =
            ":root { --base: #fafafa; --background: var(--base); --sidebar: var(--background); }\n";
        let ds = w3("", &[side], two)
            .file("src/a.css", src)
            .build()
            .run_check(&LeakCheck);
        assert!(of(&ds, c::HN_T001).is_empty(), "{ds:#?}");
    }

    #[test]
    fn r_var_094__minified_and_generated_files_are_never_scanned() {
        let ds = w3("", &[BRAND], CSS3)
            .file("src/app.min.css", ".a{color:#3b82f6}\n")
            .build()
            .run_check(&LeakCheck);
        assert!(of(&ds, c::HN_T001).is_empty(), "{ds:#?}");
    }

    #[test]
    fn r_var_099__0_1_leak_results_unchanged() {
        // The 0.1 form (sym=, value=, exact hex) still reports the same sites.
        let src =
            "const a = { color: '#2563EB' };\nconst b = { backgroundColor: 'rgb(37, 99, 235)' };\n";
        let b = world(&tok("applies=color,background-color"))
            .file("src/Panel.tsx", src)
            .build();
        assert_eq!(
            of(&b.run_check(&LeakCheck), c::HN_T001)
                .iter()
                .map(|d| d.location.line)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn r_var_097__a_child_token_with_an_ancestor_name_needs_ovr() {
        let web = "[fairlead]\nscope = \"web\"\n";
        let child = format!(
            "+++\ndirectives = ['{}']\n+++\n# Web tokens\n",
            BRAND.replace("--brand", "--brand-web")
        );
        let b = w3("", &[BRAND], CSS3)
            .scope("web", web)
            .file("web/fairlead/contract/t.md", &child)
            .build();
        let r = b
            .registry
            .diagnostics()
            .iter()
            .filter(|d| d.code == c::HN_R005)
            .count();
        assert_eq!(r, 1, "same name in a child scope without ovr");
        let own = format!(
            "+++\ndirectives = ['{}']\n+++\n# Web tokens\n",
            BRAND.replace("tok.brand", "tok.brand-sidebar")
        );
        let b = w3("", &[BRAND], CSS3)
            .scope("web", web)
            .file("web/fairlead/contract/t.md", &own)
            .build();
        assert!(
            b.registry
                .diagnostics()
                .iter()
                .all(|d| d.code != c::HN_R005),
            "a new name is a new item"
        );
    }
}
