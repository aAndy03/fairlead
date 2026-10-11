//! Hierarchy structure (30-hierarchy, P1): containment and binding (R-HIER-010 to R-HIER-014) and
//! a unit's layer (R-HIER-003), switched on by the adoption level (R-HIER-080).
use fairlead_syntax::diag::DiagCode;

use crate::check::builtin::{bare_id, origin_loc};
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::hier::{Model, literal_prefix, matcher, prefixes_overlap};
use crate::registry::{ItemId, ScopeId};

pub struct HierCheck;
const CODES: [DiagCode; 7] = [
    HN_H001, HN_H002, HN_H003, HN_H004, HN_H005, HN_H007, HN_H023,
];

fn at(
    cx: &CheckCx<'_>,
    id: &ItemId,
    code: DiagCode,
    msg: String,
    hint: &str,
) -> Option<Diagnostic> {
    let it = cx.registry.item(id)?;
    let (p, l, c) = origin_loc(it);
    Some(
        DiagB::new(code, &p, l, c, msg)
            .hint(hint)
            .related(bare_id(id))
            .text(bare_id(id))
            .build(),
    )
}

/// A container's claims as repo-relative literal prefixes.
fn claims(cx: &CheckCx<'_>, m: &Model, c: &ItemId) -> Vec<String> {
    let dir = cx
        .registry
        .scope(&c.scope)
        .map(|s| s.dir.as_str().trim_matches('/').to_owned())
        .unwrap_or_default();
    m.containers
        .get(c)
        .map(|x| {
            x.paths
                .iter()
                .map(|g| {
                    let p = literal_prefix(g);
                    match (dir.is_empty(), p.is_empty()) {
                        (true, _) => p,
                        (false, true) => dir.clone(),
                        (false, false) => format!("{dir}/{p}"),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Check for HierCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let Some(h) = crate::hier::config(cx.registry) else {
            return;
        };
        let Some(level) = h.settings.level else {
            return; // no [hier]: hierarchy is off (R-HIER-080)
        };
        let m = Model::build(cx.registry);
        let files: Vec<&crate::path::RepoPath> = cx.corpus.records.keys().collect();
        let bound = m.bindings(cx.registry, &files);
        // L0: a governed file bound to nothing (R-HIER-051); `unbound-ok` globs are exempt.
        let governed: Vec<_> = h
            .settings
            .governed
            .iter()
            .filter_map(|g| matcher(g))
            .collect();
        let exempt: Vec<_> = h
            .settings
            .unbound_ok
            .iter()
            .filter_map(|g| matcher(g))
            .collect();
        if !governed.is_empty() {
            let any_bound: std::collections::BTreeSet<&crate::path::RepoPath> =
                bound.values().flatten().collect();
            for f in files
                .iter()
                .filter(|f| cx.registry.scope_of(f) == Some(scope))
            {
                let p = f.as_str();
                if governed.iter().any(|g| g.is_match(p))
                    && !exempt.iter().any(|g| g.is_match(p))
                    && !any_bound.contains(f)
                {
                    out.push(
                        DiagB::new(HN_H003, f, 0, 0, format!("{p} is governed but bound to no container or unit"))
                            .hint("cover it with a container's path= claim or a unit's def, or list it in [hier] unbound-ok (R-HIER-051)")
                            .text(p.to_owned())
                            .build(),
                    );
                }
            }
        }
        // L0: containment of containers.
        for c in m.containers.values().filter(|c| c.id.scope == *scope) {
            if c.paths.iter().any(|g| g.split('/').any(|s| s == "..")) {
                out.extend(at(
                    cx,
                    &c.id,
                    HN_H001,
                    format!("{} claims a path outside its scope", bare_id(&c.id)),
                    "a path claim is relative to the declaring scope and never leaves it (R-HIER-011)",
                ));
            }
            if let Err(cycle) = m.ancestors(&c.id)
                && cycle.last() == Some(&c.id)
            {
                let names: Vec<String> = cycle.iter().map(bare_id).collect();
                out.extend(at(
                    cx,
                    &c.id,
                    HN_H002,
                    format!("container parent cycle: {}", names.join(" -> ")),
                    "a container's parent chain must end; fix one parent= (R-HIER-011)",
                ));
            }
            let mine = claims(cx, &m, &c.id);
            let lineage = m.ancestors(&c.id).unwrap_or_default();
            for o in m.containers.values().filter(|o| o.id < c.id) {
                let theirs = claims(cx, &m, &o.id);
                let overlap = mine
                    .iter()
                    .any(|a| theirs.iter().any(|b| prefixes_overlap(a, b)));
                let nested = lineage.contains(&o.id)
                    || m.ancestors(&o.id).unwrap_or_default().contains(&c.id);
                if overlap && !nested {
                    out.extend(at(
                        cx,
                        &c.id,
                        HN_H001,
                        format!(
                            "the path claims of {} and {} overlap, but neither container is nested in the other",
                            bare_id(&c.id),
                            bare_id(&o.id)
                        ),
                        "narrow one claim, or make one container the other's parent (R-HIER-010)",
                    ));
                }
            }
        }
        if level < 1 {
            return;
        }
        // L1: a unit's layer, its single container and its single binding.
        for u in m.units.values().filter(|u| u.id.scope == *scope) {
            match (u.container_raw.as_slice(), &u.container) {
                ([], _) => out.extend(at(
                    cx,
                    &u.id,
                    HN_H004,
                    format!("{} has no container", bare_id(&u.id)),
                    "add container=<ctr id> to its def (R-HIER-050)",
                )),
                ([raw], None) => out.extend(at(
                    cx,
                    &u.id,
                    HN_H004,
                    format!(
                        "{} names container {raw}, which does not exist",
                        bare_id(&u.id)
                    ),
                    "declare the container, or name an existing one (R-HIER-050)",
                )),
                _ => {}
            }
            let proposed = cx
                .registry
                .item(&u.id)
                .and_then(|it| crate::check::builtin::field_str(it, "st"))
                == Some("proposed");
            if !proposed && bound.get(&u.id).is_none_or(|b| b.is_empty()) {
                out.extend(at(
                    cx,
                    &u.id,
                    HN_H005,
                    format!("{} binds no file", bare_id(&u.id)),
                    "bind it with a code def next to its symbol or a path= that matches files, or mark it st=proposed (R-HIER-052)",
                ));
            }
            if u.container_raw.len() > 1 {
                out.extend(at(
                    cx,
                    &u.id,
                    HN_H001,
                    format!(
                        "{} names {} containers; a unit has exactly one",
                        bare_id(&u.id),
                        u.container_raw.len()
                    ),
                    "keep one container=, relate the other with see= or split the unit (R-HIER-012)",
                ));
            }
            if u.path.is_some() && u.code_bound {
                out.extend(at(
                    cx,
                    &u.id,
                    HN_H023,
                    format!("{} binds both by path= and by a code def", bare_id(&u.id)),
                    "bind by one: drop path=, or remove the code def (R-HIER-014)",
                ));
            }
            let kind_layers =
                crate::check::builtin::kind_decl(cx.registry, scope, u.id.kind.as_str())
                    .map(|k| k.layers.clone())
                    .unwrap_or_default();
            let problem = match &u.layer {
                None if kind_layers.is_empty() => None,
                None => Some(format!("{} names no layer", bare_id(&u.id))),
                Some(l) if l.chars().all(|c| c.is_ascii_digit()) => Some(format!(
                    "{} gives a rank number ({l}); a unit names a layer, never a rank",
                    bare_id(&u.id)
                )),
                Some(l) if !h.knows(l) => Some(format!(
                    "{} names layer {l}, which is not declared",
                    bare_id(&u.id)
                )),
                Some(l)
                    if !kind_layers.is_empty() && !kind_layers.iter().any(|p| h.matches(p, l)) =>
                {
                    Some(format!(
                        "{} names layer {l}; kind {} allows {}",
                        bare_id(&u.id),
                        u.id.kind.as_str(),
                        kind_layers.join(", ")
                    ))
                }
                Some(_) => None,
            };
            if let Some(msg) = problem {
                out.extend(at(
                    cx,
                    &u.id,
                    HN_H007,
                    msg,
                    "name a layer declared in [layers.<ladder>] that the kind allows (R-HIER-003)",
                ));
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

    const LADDER: &str = "[layers.core]\nprimitive = 10\nengine = 30\n\n[kinds.eng]\nlayers = [\"core.engine\"]\n\n[kinds.ctr]\nshape = \"code\"\n\n[hier]\nlevel = 1\n";

    fn run(code: &str) -> Vec<Diagnostic> {
        World::new()
            .root(LADDER)
            .file("src/a.rs", code)
            .build()
            .run_check(&HierCheck)
    }

    #[test]
    fn r_hier_010__a_unit_has_exactly_one_container() {
        let ds = run(
            "// fairlead: def ctr.a path=\"src/a/**\"\n// fairlead: def ctr.b path=\"src/b/**\"\n// fairlead: def eng.x layer=core.engine container=ctr.a,ctr.b\nfn x() {}\n",
        );
        let d = of(&ds, c::HN_H001);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(d[0].message.contains("2 containers"));
    }

    #[test]
    fn r_hier_010__overlapping_claims_must_be_nested() {
        let flat = run(
            "// fairlead: def ctr.a path=\"src/**\"\n// fairlead: def ctr.b path=\"src/b/**\"\nfn x() {}\n",
        );
        assert_eq!(of(&flat, c::HN_H001).len(), 1, "{flat:#?}");
        let nested = run(
            "// fairlead: def ctr.a path=\"src/**\"\n// fairlead: def ctr.b parent=ctr.a path=\"src/b/**\"\nfn x() {}\n",
        );
        assert!(of(&nested, c::HN_H001).is_empty(), "{nested:#?}");
    }

    #[test]
    fn r_hier_011__parent_chain_is_acyclic_and_claims_stay_in_scope() {
        let ds = run(
            "// fairlead: def ctr.a parent=ctr.b\n// fairlead: def ctr.b parent=ctr.a\n// fairlead: def ctr.c path=\"../x/**\"\nfn x() {}\n",
        );
        let cyc = of(&ds, c::HN_H002);
        assert_eq!(cyc.len(), 2, "both members: {ds:#?}");
        assert!(
            cyc[0].message.contains("ctr.a -> ctr.b -> ctr.a"),
            "{}",
            cyc[0].message
        );
        assert!(
            of(&ds, c::HN_H001)
                .iter()
                .any(|d| d.message.contains("outside its scope"))
        );
    }

    #[test]
    fn r_hier_012__a_second_membership_is_not_a_second_container() {
        let ds = run(
            "// fairlead: def ctr.a\n// fairlead: def eng.x layer=core.engine container=ctr.a see=ctr.b\nfn x() {}\n",
        );
        assert!(
            of(&ds, c::HN_H001).is_empty(),
            "see= is not a container: {ds:#?}"
        );
    }

    #[test]
    fn r_hier_013__containment_is_declared_with_def_keys() {
        let w = World::new()
            .root(LADDER)
            .file(
                "src/a.rs",
                "// fairlead: def ctr.sync path=\"src/**\"\n// fairlead: def eng.verify layer=core.engine container=ctr.sync\nfn verify() {}\n",
            )
            .build();
        let m = Model::build(&w.registry);
        let u = m.units.values().next().unwrap();
        assert_eq!(
            u.container.as_ref().map(bare_id).as_deref(),
            Some("ctr.sync")
        );
        assert!(m.containers.values().any(|c| bare_id(&c.id) == "ctr.sync"));
    }

    #[test]
    fn r_hier_013__a_doc_declaration_up_the_chain_wins_over_a_code_def() {
        let w = World::new()
            .root("[kinds.cmd]\nshape = \"code\"\n")
            .file(
                "fairlead/docs/cmd.md",
                "+++\ndirectives = ['def cmd.search']\n+++\n# C\n",
            )
            .scope("app", "[fairlead]\nscope = \"app\"\n")
            .file(
                "app/src/lib.rs",
                "// fairlead: def cmd.search\nfn search() {}\n",
            )
            .build();
        let homes: Vec<&str> = w
            .registry
            .items()
            .filter(|i| i.id.kind.as_str() == "cmd")
            .map(|i| i.id.scope.0.as_str())
            .collect();
        assert_eq!(
            homes,
            ["root"],
            "the code def is a site, not a redefinition"
        );
    }

    #[test]
    fn r_hier_014__path_and_code_def_is_double_binding() {
        let ds = run(
            "// fairlead: def ctr.a\n// fairlead: def eng.x layer=core.engine container=ctr.a path=\"src/x/**\"\nfn x() {}\n",
        );
        assert_eq!(of(&ds, c::HN_H023).len(), 1, "{ds:#?}");
    }

    #[test]
    fn r_hier_003__unit_names_declared_layer() {
        let ds = run(
            "// fairlead: def ctr.a\n// fairlead: def eng.x layer=core.adapter container=ctr.a\n// fairlead: def eng.y layer=30 container=ctr.a\n// fairlead: def eng.z layer=core.primitive container=ctr.a\nfn x() {}\n",
        );
        let d = of(&ds, c::HN_H007);
        assert_eq!(d.len(), 3, "{ds:#?}");
        assert!(d.iter().any(|x| x.message.contains("not declared")));
        assert!(d.iter().any(|x| x.message.contains("rank number")));
        assert!(
            d.iter()
                .any(|x| x.message.contains("kind eng allows core.engine"))
        );
    }

    #[test]
    fn r_hier_050__a_unit_needs_an_existing_container() {
        let ds = run("// fairlead: def ctr.a
// fairlead: def eng.x layer=core.engine
fn x() {}
// fairlead: def eng.y layer=core.engine container=ctr.gone
fn y() {}
");
        let d = of(&ds, c::HN_H004);
        assert_eq!(d.len(), 2, "{ds:#?}");
        assert!(d.iter().any(|x| x.message.contains("has no container")));
        assert!(
            d.iter()
                .any(|x| x.message.contains("ctr.gone, which does not exist"))
        );
    }

    #[test]
    fn r_hier_051__a_governed_file_bound_to_nothing_is_an_orphan() {
        let w = |extra: &str| {
            World::new()
                .root(&format!(
                    "{LADDER}governed = [\"src/**\"]
unbound-ok = [\"src/tests/**\"]
{extra}"
                ))
                .file(
                    "src/a.rs",
                    "// fairlead: def ctr.a path=\"src/a.rs\"
fn a() {}
",
                )
                .file(
                    "src/b.rs",
                    "fn b() {}
",
                )
                .file(
                    "src/tests/t.rs",
                    "fn t() {}
",
                )
                .file(
                    "docs/x.md",
                    "# x
",
                )
                .build()
                .run_check(&HierCheck)
        };
        let ds = w("");
        let d = of(&ds, c::HN_H003);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert_eq!(d[0].location.path.as_str(), "src/b.rs");
    }

    #[test]
    fn r_hier_052__a_unit_that_binds_no_file_is_hollow_unless_proposed() {
        let w = World::new()
            .root(LADDER)
            .file(
                "fairlead/docs/eng.md",
                "+++
directives = ['def eng.x layer=core.engine container=ctr.a', 'def eng.y layer=core.engine container=ctr.a st=proposed', 'def ctr.a']
+++
# E
",
            )
            .build()
            .run_check(&HierCheck);
        let d = of(&w, c::HN_H005);
        assert_eq!(d.len(), 1, "{w:#?}");
        assert!(d[0].message.contains("eng.x"));
    }

    #[test]
    fn r_hier_080__checks_above_level_are_off() {
        let off = World::new()
            .root(&LADDER.replace("level = 1", "level = 0"))
            .file("src/a.rs", "// fairlead: def eng.x layer=30\nfn x() {}\n")
            .build()
            .run_check(&HierCheck);
        assert!(
            of(&off, c::HN_H007).is_empty(),
            "L1 checks are off at level 0"
        );
        let none = World::new()
            .root(&LADDER.replace("[hier]\nlevel = 1\n", ""))
            .file(
                "src/a.rs",
                "// fairlead: def ctr.a parent=ctr.a\nfn x() {}\n",
            )
            .build()
            .run_check(&HierCheck);
        assert!(none.is_empty(), "no [hier]: off");
    }
}
