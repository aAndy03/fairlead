//! Literal token leak: declared value plus property context (R-CHK-017..022, D-43).
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
const CODES: [DiagCode; 1] = [HN_T001];

struct Tok {
    id: String,
    item: ItemId,
    applies: BTreeSet<String>,
    min_len: usize,
    values: BTreeSet<String>,
    def_lines: BTreeSet<(RepoPath, u32)>,
}

fn token(cx: &CheckCx<'_>, from: &ScopeId, id: &Id) -> Option<Tok> {
    let Lookup::Found(iid) = cx.registry.lookup(from, id) else {
        return None;
    };
    let it = cx.registry.item(&iid)?;
    let applies: BTreeSet<String> = field_list(it, "applies").into_iter().collect();
    if applies.is_empty() {
        return None;
    } // opt-in (R-CHK-017, R-FILE-039)
    let min_len = field_str(it, "min-len")
        .and_then(|s| s.parse().ok())
        .unwrap_or(4usize);
    let (mut values, mut def_lines) = (BTreeSet::new(), BTreeSet::new());
    if let Some(v) = field_str(it, "value") {
        values.insert(normalize_value(v));
    }
    if let Some(sym) = field_str(it, "sym").filter(|s| s.starts_with("--")) {
        for (p, rec) in &cx.corpus.records {
            for s in rec.css_tokens.iter().filter(|s| {
                s.name.trim_start_matches('-') == sym.trim_start_matches('-') && !s.in_foreign
            }) {
                def_lines.insert((p.clone(), s.span.line));
                if !s.value.trim_start().starts_with("var(") {
                    values.insert(normalize_value(&s.value));
                }
            }
        }
    }
    Some(Tok {
        id: bare_id(&iid),
        item: iid,
        applies,
        min_len,
        values,
        def_lines,
    })
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
        .and_then(|i| field_str(i, "sym"))
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
        let rules: Vec<(&DeclaredRule, Vec<Tok>)> = cx
            .rules
            .for_scope(scope)
            .iter()
            .filter_map(|r| match &r.check {
                RuleCheck::Leak { tokens } => Some((
                    r,
                    tokens
                        .iter()
                        .filter_map(|id| token(cx, scope, id))
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
                for (rule, toks) in &rules {
                    if !cx.rules.applies(rule, path, reg) {
                        continue;
                    }
                    let hit = toks.iter().find(|t| {
                        t.applies.contains(&lit.property)
                            && lit.value_norm.chars().count() >= t.min_len
                            && t.values.contains(&lit.value_norm)
                            && !t.def_lines.contains(&(path.clone(), lit.span.line))
                    });
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
}
