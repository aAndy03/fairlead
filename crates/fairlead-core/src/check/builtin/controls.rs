//! Control-file level checks (R-FILE-051, R-SCOPE-017, C-31).
use crate::{
    check::{
        CheckCx,
        builtin::{bare_id, field_str},
        finding::DiagB,
        rules::Check,
    },
    diag::{Diagnostic, codes::*},
    path::RepoPath,
    registry::{Registry, ScopeId},
};
use fairlead_syntax::{
    diag::DiagCode,
    ids::{Id, IdQualifier, PLACEHOLDER},
};
use std::collections::{BTreeMap, BTreeSet};

pub struct ControlsCheck;
const CODES: [DiagCode; 3] = [HN_C001, HN_C012, HN_C013];
type Key = (String, String); // (scope slug, file-item name)

fn name_of(id: &str) -> String {
    Id::parse(id).map_or_else(|_| id.to_string(), |i| i.name.as_str().to_string())
}

/// Nodes of the extends graph; the target is resolved nearest-first up the chain (R-FILE-052).
fn graph(reg: &Registry) -> BTreeMap<Key, (RepoPath, Option<Key>)> {
    let mut nodes: BTreeMap<Key, (RepoPath, Option<String>, ScopeId)> = BTreeMap::new();
    for s in reg.scopes() {
        let Some(ctl) = s.control.as_ref() else {
            continue;
        };
        for f in &ctl.files {
            nodes.insert(
                (s.id.0.as_str().to_string(), name_of(&f.id)),
                (ctl.path.clone(), f.extends.clone(), s.id.clone()),
            );
        }
    }
    let known: BTreeSet<Key> = nodes.keys().cloned().collect();
    nodes
        .into_iter()
        .map(|(k, (path, ext, scope))| {
            let target = ext.and_then(|e| Id::parse(&e).ok()).and_then(|id| {
                let name = id.name.as_str().to_string();
                match &id.qualifier {
                    IdQualifier::Scope(s) => Some((s.as_str().to_string(), name)),
                    IdQualifier::Root => Some((reg.root_scope().0.as_str().to_string(), name)),
                    IdQualifier::Bare => reg
                        .chain(&scope)
                        .into_iter()
                        .rev()
                        .map(|c| (c.0.as_str().to_string(), name.clone()))
                        .find(|k2| known.contains(k2) && *k2 != k),
                }
            });
            (k, (path, target))
        })
        .collect()
}

/// Every cycle once, as its sorted member list.
fn cycles(nodes: &BTreeMap<Key, (RepoPath, Option<Key>)>) -> BTreeSet<Vec<Key>> {
    let mut found = BTreeSet::new();
    for start in nodes.keys() {
        let (mut path, mut cur) = (vec![start], start);
        while let Some(next) = nodes.get(cur).and_then(|n| n.1.as_ref()) {
            if let Some(pos) = path.iter().position(|k| *k == next) {
                let mut cyc: Vec<Key> = path.iter().skip(pos).map(|k| (*k).clone()).collect();
                cyc.sort();
                found.insert(cyc);
                break;
            }
            if !nodes.contains_key(next) {
                break;
            }
            path.push(next);
            cur = next;
        }
    }
    found
}

impl Check for ControlsCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        if let Some(ctl) = reg.scope(scope).and_then(|s| s.control.as_ref()) {
            let slots = std::iter::once(("purpose".to_string(), ctl.settings.purpose.clone()))
                .chain(
                    ctl.rules
                        .values()
                        .map(|r| (format!("rules.{}.why", r.name), r.why.clone())),
                );
            for slot in
                slots.filter_map(|(slot, v)| (v.as_deref() == Some(PLACEHOLDER)).then_some(slot))
            {
                out.push(
                    DiagB::new(
                        HN_C001,
                        &ctl.path,
                        0,
                        0,
                        format!("{slot} still holds the scaffold placeholder"),
                    )
                    .hint("replace the TODO slot with real content")
                    .text(slot)
                    .build(),
                );
            }
        }
        for it in reg.items() {
            let Some(home) = field_str(it, "home") else {
                continue;
            };
            for s in reg.sites_of(&it.id).iter().filter(|s| s.scope == *scope) {
                if !reg.chain(&s.scope).iter().any(|c| c.0.as_str() == home) {
                    out.push(
                        DiagB::new(
                            HN_C013,
                            &s.path,
                            s.span.line,
                            s.span.col,
                            format!(
                                "{} declares home={home}, which is not an ancestor-or-self of this site's scope",
                                bare_id(&it.id)
                            ),
                        )
                        .hint("move the home to the nearest common ancestor of all sites")
                        .text(home)
                        .build(),
                    );
                }
            }
        }
        if scope == reg.root_scope() {
            let nodes = graph(reg);
            for cyc in cycles(&nodes) {
                let Some(path) = cyc.first().and_then(|k| nodes.get(k)).map(|n| n.0.clone()) else {
                    continue;
                };
                let names = cyc
                    .iter()
                    .map(|k| format!("{}:{}", k.0, k.1))
                    .collect::<Vec<_>>()
                    .join(" -> ");
                out.push(
                    DiagB::new(HN_C012, &path, 0, 0, format!("file extends cycle: {names}"))
                        .hint("remove one extends so the chain is acyclic")
                        .text(names)
                        .build(),
                );
            }
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::{
        check::testkit::{World, of},
        diag::codes as c,
    };

    fn files(a_ext: &str, b_ext: &str) -> String {
        format!(
            "[[file]]\nid = \"file.a\"\nkind = \"stylesheet\"\npath = \"a.css\"\n{a_ext}\n[[file]]\nid = \"file.b\"\nkind = \"stylesheet\"\npath = \"b.css\"\n{b_ext}\n"
        )
    }

    #[test]
    fn r_file_051__an_extends_cycle_is_one_c012_and_a_chain_is_fine() {
        let b = World::new()
            .root(&files("extends = \"file.b\"", "extends = \"file.a\""))
            .build();
        let ds = b.run_check(&ControlsCheck);
        assert_eq!(of(&ds, c::HN_C012).len(), 1, "{ds:#?}");
        let b = World::new()
            .root(&files("extends = \"file.b\"", ""))
            .build();
        assert!(of(&b.run_check(&ControlsCheck), c::HN_C012).is_empty());
    }

    #[test]
    fn r_scope_017__home_must_be_an_ancestor_or_self_of_every_site() {
        let doc = "+++\ndirectives = ['def cmp.shared home=web']\n+++\n";
        let b = World::new()
            .root("[kinds.cmp]\ntitle = \"c\"\n")
            .scope("web", "[fairlead]\nscope = \"web\"\n")
            .scope("api", "[fairlead]\nscope = \"api\"\n")
            .file("fairlead/docs/x.md", doc)
            .file(
                "api/src/a.ts",
                "// fairlead: def cmp.shared\nexport const a = 1;\n",
            )
            .file(
                "web/src/b.ts",
                "// fairlead: def cmp.shared\nexport const b = 1;\n",
            )
            .build();
        let ds = b.run_check(&ControlsCheck);
        let d = of(&ds, c::HN_C013);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert_eq!(d[0].location.path.as_str(), "api/src/a.ts");
    }

    #[test]
    fn r_cli_031__a_scaffold_placeholder_in_an_authored_control_slot_is_c001() {
        let b = World::new().root("purpose = \"TODO(fairlead)\"\n").build();
        assert_eq!(of(&b.run_check(&ControlsCheck), c::HN_C001).len(), 1);
    }
}
