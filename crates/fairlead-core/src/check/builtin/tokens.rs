//! Token contract against authored CSS (R-FILE-033, R-FILE-036, R-CHK-046).
use std::collections::{BTreeMap, BTreeSet};
use fairlead_syntax::{diag::DiagCode, ids::Id};
use crate::{
    check::{builtin::{bare_id, field_str, origin_loc}, finding::DiagB, rules::Check, CheckCx},
    diag::{codes::*, Diagnostic},
    extract::{literals::normalize_value, CssTokenSite},
    path::RepoPath,
    registry::{Item, ItemId, Lookup, ScopeId},
};

pub struct TokensCheck;
const CODES: [DiagCode; 4] = [HN_T002, HN_T003, HN_T004, HN_T005];

fn bare(s: &str) -> &str { s.trim_start_matches('-') }
fn css_name(it: &Item) -> Option<&str> {
    field_str(it, "css").or_else(|| field_str(it, "sym").filter(|s| s.starts_with("--"))).map(bare)
}
fn join(dir: &RepoPath, rel: &str) -> Option<RepoPath> {
    if dir.as_str().is_empty() || dir.as_str() == "." { RepoPath::new(rel).ok() } else { RepoPath::new(&format!("{}/{}", dir.as_str(), rel)).ok() }
}
fn value_of(s: &CssTokenSite) -> Option<String> {
    let v = s.value.trim();
    (!v.starts_with("var(")).then(|| normalize_value(v))
}

impl Check for TokensCheck {
    fn codes(&self) -> &'static [DiagCode] { &CODES }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        // contract tokens visible here: css name -> (id, item)
        let visible: BTreeMap<&str, (&ItemId, &Item)> = reg.view(scope).items.keys()
            .filter_map(|iid| { let it = reg.item(iid)?; Some((css_name(it)?, (iid, it))) }).collect();
        let declared: BTreeSet<RepoPath> = reg.scopes().filter_map(|s| s.control.as_ref().map(|c| (&s.dir, c)))
            .flat_map(|(dir, c)| c.files.iter().filter_map(move |f| join(dir, &f.path))).collect();
        let all_names: BTreeSet<&str> = cx.corpus.records.values().flat_map(|r| r.css_tokens.iter()).map(|s| bare(&s.name)).collect();

        // T003: contract token homed here with no site anywhere (skipped when the repo has no CSS tokens at all)
        for (name, (iid, it)) in visible.iter().filter(|(_, (i, _))| i.scope == *scope) {
            if !all_names.is_empty() && !all_names.contains(name) {
                let (p, l, c) = origin_loc(it);
                out.push(DiagB::new(HN_T003, &p, l, c, format!("contract token {} ({}) is missing from the stylesheets", bare_id(iid), format_args!("--{name}")))
                    .hint("add the custom property to the stylesheet or remove the token").text(bare_id(iid)).build());
            }
        }
        for (path, rec) in cx.corpus.records.iter().filter(|(p, _)| reg.scope_of(p) == Some(scope)) {
            if rec.css_tokens.is_empty() { continue; }
            // T004: only in declared stylesheets
            if declared.contains(path) {
                for s in rec.css_tokens.iter().filter(|s| !s.in_foreign && !visible.contains_key(bare(&s.name))) {
                    out.push(DiagB::new(HN_T004, path, s.span.line, s.span.col, format!("css token --{} is not in the token contract", bare(&s.name)))
                        .hint("declare it in the contract, or move it into def foreign").text(s.name.clone()).build());
                }
            }
            // T002: value differs from the base (home scope, same selector) without an ovr in between
            for (name, (iid, _)) in visible.iter().filter(|(_, (i, _))| i.scope != *scope) {
                let base: BTreeMap<&str, String> = cx.corpus.records.iter().filter(|(p, _)| reg.scope_of(p) == Some(&iid.scope))
                    .flat_map(|(_, r)| r.css_tokens.iter()).filter(|s| bare(&s.name) == *name && !s.in_foreign)
                    .filter_map(|s| Some((s.context.as_str(), value_of(s)?))).collect();
                let chain = reg.chain(scope);
                let below: Vec<_> = chain.iter().skip_while(|c| ***c != iid.scope).skip(1).collect();
                let has_ovr = reg.overrides().iter().any(|o| o.target == **iid && below.iter().any(|c| ***c == o.declared_in));
                for s in rec.css_tokens.iter().filter(|s| bare(&s.name) == *name && !s.in_foreign) {
                    if let (Some(b), Some(v)) = (base.get(s.context.as_str()), value_of(s)) {
                        if *b != v && !has_ovr {
                            out.push(DiagB::new(HN_T002, path, s.span.line, s.span.col, format!("--{name} is {v} here but {b} in its base without an ovr"))
                                .hint(format!("add an ovr of {} with why, owner and exp", bare_id(iid))).text(s.name.clone()).build());
                        }
                    }
                }
            }
            // T005: a theme block that declares the base must also declare the derived token
            for (dname, (_, ditem)) in &visible {
                let Some(base_id) = field_str(ditem, "of").or_else(|| field_str(ditem, "derived-from")).and_then(|s| Id::parse(s).ok()) else { continue };
                let Lookup::Found(biid) = reg.lookup(scope, &base_id) else { continue };
                let Some(bname) = reg.item(&biid).and_then(css_name) else { continue };
                let has_d = |ctx: Option<&str>| rec.css_tokens.iter().any(|s| bare(&s.name) == *dname && ctx.is_none_or(|c| s.context == c));
                if !has_d(None) { continue; }
                for s in rec.css_tokens.iter().filter(|s| bare(&s.name) == bname && !s.in_foreign && !has_d(Some(&s.context))) {
                    out.push(DiagB::new(HN_T005, path, s.span.line, s.span.col,
                            format!("derived token --{dname} is not re-declared in {} where its base --{bname} is overridden", s.context))
                        .hint(format!("declare --{dname} in {}", s.context)).text(format!("{dname}@{}", s.context)).build());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check::testkit::{of, World}, diag::codes as c};

    const ROOT: &str = "[kinds.tok]\nshape = \"doc\"\n[[file]]\nid = \"file.styles\"\nkind = \"stylesheet\"\npath = \"app/globals.css\"\n";
    const TOKENS: &str = "+++\ndirectives = ['def tok.primary sym=--primary', 'def tok.card sym=--card', 'def tok.background sym=--background', 'def tok.sidebar sym=--sidebar of=tok.background']\n+++\n# Tokens\n";
    fn world(css: &str) -> World { World::new().root(ROOT).file("fairlead/contract/tokens.md", TOKENS).file("app/globals.css", css) }

    #[test]
    fn r_file_033__r_file_053__contract_token_missing_from_css_is_t003() {
        let b = world(":root { --primary: #2563eb; --background: #fff; --sidebar: var(--background); }").build();
        let ds = b.run_check(&TokensCheck);
        let t = of(&ds, c::HN_T003);
        assert_eq!(t.len(), 1);
        assert!(t[0].message.contains("tok.card"));
    }

    #[test]
    fn r_file_033__undeclared_css_token_is_t004_but_foreign_regions_are_exempt() {
        let css = ":root { --primary: #2563eb; --card: #fff; --background: #fff; --sidebar: var(--background); --zzz: 1px; }\n/* fairlead: def foreign.shadcn tool=shadcn +end */\n:root { --chart-1: #fff; }\n/* fairlead: end */\n";
        let t = of(&world(css).build().run_check(&TokensCheck), c::HN_T004).len();
        assert_eq!(t, 1);
    }

    #[test]
    fn r_file_033__child_value_differs_from_base_without_ovr_is_t002() {
        let web = "[fairlead]\nscope = \"web\"\n[[file]]\nid = \"file.web\"\nkind = \"stylesheet\"\npath = \"o.css\"\n";
        let base = ":root { --primary: #2563eb; --card: #fff; --background: #fff; --sidebar: var(--background); }";
        let b = world(base).scope("web", web).file("web/o.css", ":root { --primary: #1e40af; }").build();
        let ds = b.run_check(&TokensCheck);
        let t = of(&ds, c::HN_T002);
        assert_eq!((t.len(), t[0].location.path.as_str()), (1, "web/o.css"));
        let with_ovr = format!("{web}[[ovr]]\ntarget = \"tok.primary\"\nop = \"set\"\nwhy = \"dense\"\nowner = \"@platform\"\nexp = \"2026-12-31\"\n");
        let b = world(base).scope("web", &with_ovr).file("web/o.css", ":root { --primary: #1e40af; }").build();
        assert!(of(&b.run_check(&TokensCheck), c::HN_T002).is_empty());
    }

    #[test]
    fn r_file_036__r_chk_046__derived_token_must_be_redeclared_where_its_base_is_overridden() {
        let head = ":root { --primary: #2563eb; --card: #fff; --background: #fff; --sidebar: var(--background); }\n";
        let bad = format!("{head}.dark {{ --background: #000; }}");
        let ds = world(&bad).build().run_check(&TokensCheck);
        let t = of(&ds, c::HN_T005);
        assert_eq!((t.len(), t[0].location.line), (1, 2));
        let good = format!("{head}.dark {{ --background: #000; --sidebar: var(--background); }}");
        assert!(of(&world(&good).build().run_check(&TokensCheck), c::HN_T005).is_empty());
    }
}
