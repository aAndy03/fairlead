//! Cascade semantics of stylesheets (R-FILE-035(b, c), R-CHK-046, R-CHK-047): the declared
//! `@layer` order and import order (`LAYER-ORDER`, HN-T008), unlayered overrides of a contract
//! token without an `ovr` (`BARE-OVERRIDE`, HN-T009), and derivations found in the CSS that are
//! not re-declared where their base is overridden (`TOK-DERIVED-GRAPH`, HN-T010). "Later file
//! wins" is never assumed.
use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::diag::DiagCode;

use crate::check::builtin::field_str;
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::extract::FileRecord;
use crate::path::RepoPath;
use crate::registry::ScopeId;

pub struct CascadeCheck;
const CODES: [DiagCode; 3] = [HN_T008, HN_T009, HN_T010];

fn bare(s: &str) -> &str {
    s.trim_start_matches('-')
}

/// `var(--b)` or `var(--b, fallback)`: the base a value derives from.
fn derives_from(value: &str) -> Option<&str> {
    let v = value.trim().strip_prefix("var(")?;
    let name = v.split([',', ')']).next()?.trim();
    name.starts_with("--").then(|| bare(name))
}

fn layer_order(cx: &CheckCx<'_>, path: &RepoPath, rec: &FileRecord, out: &mut Vec<Diagnostic>) {
    let Some(f) = rec.css.as_ref() else { return };
    let mut push = |line: u32, msg: String| {
        out.push(
            DiagB::new(HN_T008, path, line, 1, msg)
                .hint("declare `@layer a, b, c;` first, keep @import at the top, and import in that order")
                .text(path.as_str().to_owned())
                .build(),
        );
    };
    let order: Vec<&String> = f.layer_orders.iter().flat_map(|(_, v)| v.iter()).collect();
    if !f.layer_blocks.is_empty() && f.layer_orders.is_empty() {
        let (line, name) = &f.layer_blocks[0];
        push(
            *line,
            format!("@layer {name} is used without an @layer order statement"),
        );
    }
    for (line, name) in &f.layer_blocks {
        if !f.layer_orders.is_empty() && !order.contains(&name) {
            push(
                *line,
                format!("@layer {name} is not in the declared @layer order"),
            );
        }
    }
    if let Some(first) = f.first_rule {
        for (line, target, _) in f.imports.iter().filter(|(l, _, _)| *l > first) {
            push(
                *line,
                format!("@import \"{target}\" after the first rule is ignored by the cascade"),
            );
        }
    }
    let mut last: Option<(usize, &str)> = None;
    for (line, target, layer) in &f.imports {
        let Some(layer) = layer else { continue };
        let Some(i) = order.iter().position(|n| *n == layer) else {
            if !order.is_empty() {
                push(
                    *line,
                    format!(
                        "@import \"{target}\" goes into @layer {layer}, which the order statement does not declare"
                    ),
                );
            }
            continue;
        };
        if let Some((j, prev)) = last
            && i < j
        {
            push(
                *line,
                format!(
                    "@import \"{target}\" (layer {layer}) comes after an import into the later layer {prev}"
                ),
            );
        }
        last = Some((i, layer.as_str()));
    }
    let _ = cx;
}

impl Check for CascadeCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let here: Vec<(&RepoPath, &FileRecord)> = cx
            .corpus
            .records
            .iter()
            .filter(|(p, r)| {
                reg.scope_of(p) == Some(scope) && (r.css.is_some() || !r.css_tokens.is_empty())
            })
            .collect();
        if here.is_empty() {
            return;
        }
        // Contract tokens visible here by css name, with whether the contract declares them derived.
        let mut contract: BTreeMap<String, (crate::registry::ItemId, bool)> = BTreeMap::new();
        for iid in reg.view(scope).items.keys() {
            let Some(it) = reg.item(iid) else { continue };
            let name = field_str(it, "css")
                .or_else(|| field_str(it, "sym").filter(|s| s.starts_with("--")))
                .map(bare);
            if let Some(n) = name {
                let derived =
                    field_str(it, "of").is_some() || field_str(it, "derived-from").is_some();
                contract.insert(n.to_owned(), (iid.clone(), derived));
            }
        }
        for (path, rec) in &here {
            layer_order(cx, path, rec, out);
            // T009: an unlayered declaration of a contract token homed in an ancestor scope.
            for s in rec.css_tokens.iter().filter(|s| !s.in_foreign) {
                let Some((iid, _)) = contract.get(bare(&s.name)) else {
                    continue;
                };
                if iid.scope == *scope
                    || s.context.contains("@layer")
                    || s.context.contains("@theme")
                {
                    continue;
                }
                let has_ovr = reg
                    .overrides()
                    .iter()
                    .any(|o| o.target == *iid && o.declared_in == *scope);
                if !has_ovr {
                    out.push(
                        DiagB::new(
                            HN_T009,
                            path,
                            s.span.line,
                            s.span.col,
                            format!(
                                "unlayered override of {} in {} beats every @layer of its home",
                                s.name,
                                if s.context.is_empty() { "the top level" } else { &s.context }
                            ),
                        )
                        .hint("put it in a declared @layer, or add an ovr of the token with why, owner and expiry")
                        .text(s.name.clone())
                        .build(),
                    );
                }
            }
            // T010: derivations found in the CSS, not declared in the contract.
            let mut derived: BTreeMap<&str, (&str, &str)> = BTreeMap::new();
            for s in rec.css_tokens.iter().filter(|s| !s.in_foreign) {
                if let Some(b) = derives_from(&s.value) {
                    derived
                        .entry(bare(&s.name))
                        .or_insert((b, s.context.as_str()));
                }
            }
            for (d, (b, home)) in derived {
                if contract.get(d).is_some_and(|(_, declared)| *declared) {
                    continue; // HN-T005's
                }
                let d_ctx: BTreeSet<&str> = rec
                    .css_tokens
                    .iter()
                    .filter(|s| bare(&s.name) == d)
                    .map(|s| s.context.as_str())
                    .collect();
                for s in rec.css_tokens.iter().filter(|s| {
                    bare(&s.name) == b
                        && !s.in_foreign
                        && s.context != home
                        && !d_ctx.contains(s.context.as_str())
                }) {
                    out.push(
                        DiagB::new(
                            HN_T010,
                            path,
                            s.span.line,
                            s.span.col,
                            format!(
                                "--{d} is var(--{b}) but is not re-declared in {} where --{b} is overridden",
                                s.context
                            ),
                        )
                        .hint(format!("declare --{d} in {} too, or declare it derived in the token contract", s.context))
                        .text(format!("{d}@{}", s.context))
                        .build(),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;

    fn run(w: World) -> Vec<Diagnostic> {
        w.build().run_check(&CascadeCheck)
    }

    #[test]
    fn r_chk_047__layer_order_and_import_order() {
        let ok = "@import \"tailwindcss\" layer(base);\n@layer theme, base, components, utilities;\n@layer components {\n  .btn { color: red; }\n}\n";
        assert!(of(&run(World::new().file("app.css", ok)), c::HN_T008).is_empty());
        let no_order = "@layer components {\n  .btn { color: red; }\n}\n";
        assert_eq!(
            of(&run(World::new().file("app.css", no_order)), c::HN_T008).len(),
            1
        );
        let late = ".a { color: red; }\n@import \"x.css\";\n";
        assert!(
            of(&run(World::new().file("app.css", late)), c::HN_T008)[0]
                .message
                .contains("after the first rule")
        );
        let swapped = "@layer base, utilities;\n@import \"u.css\" layer(utilities);\n@import \"b.css\" layer(base);\n";
        assert!(
            of(&run(World::new().file("app.css", swapped)), c::HN_T008)[0]
                .message
                .contains("later layer utilities")
        );
    }

    #[test]
    fn r_chk_047__bare_override_of_an_ancestor_token() {
        let w = |child_css: &str, ovr: bool| {
            let mut w = World::new()
                .root("[kinds.tok]\nimpl = 1\n")
                .file(
                    "fairlead/docs/tok.md",
                    "+++\ndirectives = ['def tok.primary css=--primary']\n+++\n# T\n",
                )
                .file("app.css", ":root { --primary: #000; }\n")
                .scope("web", "[fairlead]\nscope = \"web\"\n")
                .file("web/site.css", child_css);
            if ovr {
                w = w.file(
                    "web/fairlead/docs/o.md",
                    "+++\ndirectives = ['ovr tok.primary why=\"brand\" owner=@platform exp=2026-12-31']\n+++\n# O\n",
                );
            }
            run(w)
        };
        assert_eq!(
            of(&w(":root { --primary: #111; }\n", false), c::HN_T009).len(),
            1
        );
        assert!(
            of(
                &w("@layer base {\n:root { --primary: #111; }\n}\n", false),
                c::HN_T009
            )
            .is_empty()
        );
        assert!(of(&w(":root { --primary: #111; }\n", true), c::HN_T009).is_empty());
    }

    #[test]
    fn r_chk_046__derivation_found_by_graph() {
        let css = ":root {\n  --background: #fff;\n  --sidebar: var(--background);\n}\n.dark {\n  --background: #000;\n}\n";
        let ds = run(World::new().file("app.css", css));
        let t = of(&ds, c::HN_T010);
        assert_eq!(t.len(), 1, "{ds:?}");
        assert!(t[0].message.contains("--sidebar is var(--background)"));
        let fixed = css.replace(
            "--background: #000;",
            "--background: #000;\n  --sidebar: var(--background);",
        );
        assert!(of(&run(World::new().file("app.css", &fixed)), c::HN_T010).is_empty());
    }
}
