//! Hierarchy structure (30-hierarchy, P1): containment and binding (R-HIER-010 to R-HIER-014) and
//! a unit's layer (R-HIER-003), switched on by the adoption level (R-HIER-080).
use fairlead_syntax::diag::DiagCode;

use crate::check::builtin::{bare_id, origin_loc};
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::hier::{Model, literal_prefix, matcher, prefixes_overlap};
use crate::registry::{ItemId, ScopeId};

pub struct HierCheck;
const CODES: [DiagCode; 10] = [
    HN_H001, HN_H002, HN_H003, HN_H004, HN_H005, HN_H007, HN_H008, HN_H009, HN_H010, HN_H023,
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

/// R-HIER-034: every verdict on `use` edges is a lower bound until an import extractor exists.
pub(crate) const LOWER_BOUND: &str = "a lower bound: declared edges only (R-HIER-034)";

/// R-HIER-002, R-HIER-005, R-HIER-006, R-HIER-020, R-HIER-023: the direction of each `use` edge.
fn rank_edges(
    h: &crate::config::HierConfig,
    m: &Model,
    edges: &[crate::hier::UnitEdge],
    scope: &ScopeId,
    out: &mut Vec<Diagnostic>,
) {
    for e in edges {
        if e.from.scope != *scope {
            continue;
        }
        let (Some(su), Some(tu)) = (m.units.get(&e.from), m.units.get(&e.to)) else {
            continue;
        };
        if su.implements.contains(&e.to) {
            continue; // R-HIER-023
        }
        let (Some(ls), Some(lt)) = (su.layer.as_deref(), tu.layer.as_deref()) else {
            continue;
        };
        if !h.knows(ls) || !h.knows(lt) {
            continue; // the layer itself is reported (R-HIER-003)
        }
        let edge = format!("{} ({ls}) uses {} ({lt})", bare_id(&e.from), bare_id(&e.to));
        if let Some(why) = edge_problem(h, ls, lt, su.container == tu.container) {
            out.push(
                DiagB::new(HN_H007, &e.path, e.line, e.col, format!("{edge}: {why}"))
                    .rule("hier-rank")
                    .hint(format!("invert it through a lower port that this side implements, or waive rule.hier-rank with until= (R-HIER-024); {LOWER_BOUND}"))
                    .related(bare_id(&e.to))
                    .text(bare_id(&e.to))
                    .build(),
            );
        }
    }
}

/// R-HIER-021, R-HIER-022: the unit graph is a DAG, and the container graph (the unit edges
/// aggregated) has no strongly connected component. Each cycle is reported once, in the scope of
/// its smallest member, at the first edge that closes it.
fn cycle_edges(
    m: &Model,
    edges: &[crate::hier::UnitEdge],
    scope: &ScopeId,
    out: &mut Vec<Diagnostic>,
) {
    use std::collections::{BTreeMap, BTreeSet};
    let mut units: BTreeMap<ItemId, BTreeSet<ItemId>> = BTreeMap::new();
    for e in edges {
        units
            .entry(e.from.clone())
            .or_default()
            .insert(e.to.clone());
    }
    for comp in crate::hier::cycles(&units) {
        let Some(first) = comp.first().filter(|f| f.scope == *scope) else {
            continue;
        };
        let names: Vec<String> = comp.iter().map(bare_id).collect();
        if let Some(e) = edges
            .iter()
            .find(|e| e.from == *first && comp.contains(&e.to))
        {
            out.push(
                DiagB::new(
                    HN_H008,
                    &e.path,
                    e.line,
                    e.col,
                    format!("units in a use cycle: {}", names.join(", ")),
                )
                .hint(format!("the unit graph must be a DAG: extract the shared part into a lower unit, or invert one edge (R-HIER-021); {LOWER_BOUND}"))
                .related(bare_id(first))
                .text(bare_id(first))
                .build(),
            );
        }
    }
    let ctr = |u: &ItemId| m.units.get(u).and_then(|x| x.container.clone());
    let mut graph: BTreeMap<ItemId, BTreeSet<ItemId>> = BTreeMap::new();
    let mut causes: Vec<(ItemId, ItemId, &crate::hier::UnitEdge)> = Vec::new();
    for e in edges {
        if let (Some(a), Some(b)) = (ctr(&e.from), ctr(&e.to))
            && a != b
        {
            graph.entry(a.clone()).or_default().insert(b.clone());
            causes.push((a, b, e));
        }
    }
    for comp in crate::hier::cycles(&graph) {
        let Some(first) = comp.first().filter(|f| f.scope == *scope) else {
            continue;
        };
        let inside: Vec<&(ItemId, ItemId, &crate::hier::UnitEdge)> = causes
            .iter()
            .filter(|(a, b, _)| comp.contains(a) && comp.contains(b))
            .collect();
        let Some((_, _, lead)) = inside.first() else {
            continue;
        };
        let names: Vec<String> = comp.iter().map(bare_id).collect();
        let why: Vec<String> = inside
            .iter()
            .map(|(_, _, e)| {
                format!(
                    "{} -> {} ({}:{})",
                    bare_id(&e.from),
                    bare_id(&e.to),
                    e.path.as_str(),
                    e.line
                )
            })
            .collect();
        out.push(
            DiagB::new(
                HN_H009,
                &lead.path,
                lead.line,
                lead.col,
                format!(
                    "containers in a cycle: {}; caused by {}",
                    names.join(", "),
                    why.join(", ")
                ),
            )
            .hint(format!(
                "extract the shared piece into a lower container (R-HIER-022); {LOWER_BOUND}"
            ))
            .related(bare_id(first))
            .text(bare_id(first))
            .build(),
        );
    }
}

/// R-HIER-026, R-HIER-028, R-VAR-010, R-VAR-011: a reference to a unit from outside its
/// visibility (`subtree`, the default: the home scope and its descendants; `public`: anywhere).
/// Independent of rank. A qualified sideways reference to an item its scope does not provide is
/// already `HN-R008` and is not reported twice.
fn visibility(cx: &CheckCx<'_>, m: &Model, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
    let reg = cx.registry;
    let chain = reg.chain(scope);
    for e in reg.edges().iter().filter(|e| e.from.scope == *scope) {
        let Some(t) = e.resolved.as_ref().filter(|t| m.units.contains_key(*t)) else {
            continue;
        };
        let vis = reg
            .item(t)
            .and_then(|it| crate::check::builtin::field_str(it, "vis"))
            .unwrap_or("subtree");
        if vis == "public" || chain.contains(&&t.scope) {
            continue;
        }
        if matches!(e.to.qualifier, fairlead_syntax::ids::IdQualifier::Scope(_))
            && !crate::check::builtin::refs::provided(reg, t)
        {
            continue; // HN-R008
        }
        let home = reg.chain(&t.scope);
        let lca = chain
            .iter()
            .rev()
            .find(|s| home.contains(s))
            .map_or("root", |s| s.0.as_str());
        out.push(
            DiagB::new(
                HN_H010,
                &e.from.path,
                e.from.span.line,
                e.from.span.col,
                format!(
                    "scope {} uses {}, which is vis={vis} in scope {}",
                    scope.0.as_str(),
                    bare_id(t),
                    t.scope.0.as_str()
                ),
            )
            .hint(format!(
                "promote to the lowest common ancestor scope {lca}, or mark vis=public (R-VAR-010)"
            ))
            .related(bare_id(t))
            .text(bare_id(t))
            .build(),
        );
    }
}

/// Why an edge from layer `ls` to layer `lt` breaks the ladder rules, if it does.
fn edge_problem(
    h: &crate::config::HierConfig,
    ls: &str,
    lt: &str,
    same_container: bool,
) -> Option<String> {
    use crate::config::hier::SameRank;
    let free_s = h.free_layer(ls);
    let free_t = h.free_layer(lt);
    let same_free = matches!((free_s, free_t), (Some((a, _)), Some((b, _))) if a == b);
    if h.entry_only.iter().any(|l| l == lt) && ls != lt {
        return Some(format!("{lt} is reached only from entries (R-HIER-006)"));
    }
    if let Some((name, f)) = free_s
        && !same_free
        && !f.may_use.iter().any(|p| h.matches(p, lt))
        && free_t.is_none_or(|(_, g)| !g.usable_by.iter().any(|p| h.matches(p, ls)))
    {
        return Some(format!(
            "free layer {name} may use only {}",
            list_or_none(&f.may_use)
        ));
    }
    if let Some((name, g)) = free_t
        && !same_free
        && !g.usable_by.iter().any(|p| h.matches(p, ls))
        && free_s.is_none_or(|(_, f)| !f.may_use.iter().any(|p| h.matches(p, lt)))
    {
        return Some(format!(
            "free layer {name} is usable only by {}",
            list_or_none(&g.usable_by)
        ));
    }
    let ((ladder_s, rs), (ladder_t, rt)) = (h.rank(ls)?, h.rank(lt)?);
    if ladder_s != ladder_t {
        let sanctioned = h
            .flows
            .iter()
            .any(|f| h.matches(&f.from, ls) && f.to.iter().any(|t| h.matches(t, lt)));
        return (!sanctioned).then(|| format!("no [[flow]] sanctions {ladder_s} to {ladder_t}"));
    }
    if rt > rs {
        return Some(format!("rank {rt} is above {rs}, an upward edge"));
    }
    match (rt == rs, h.settings.same_rank) {
        (true, SameRank::Forbid) => Some("same-rank edges are forbidden".into()),
        (true, SameRank::Container) if !same_container => {
            Some("same-rank edges stay inside one container".into())
        }
        _ => None,
    }
}

fn list_or_none(v: &[String]) -> String {
    if v.is_empty() {
        "nothing".to_owned()
    } else {
        v.join(", ")
    }
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
        if level >= 2 {
            let edges = m.edges(cx.registry, &files);
            rank_edges(h, &m, &edges, scope, out);
            cycle_edges(&m, &edges, scope, out);
            visibility(cx, &m, scope, out);
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

    const L2: &str = r#"[layers.core]
primitive = 10
engine = 30
service = 50
shell = 60

[layers.ui]
atom = 10
page = 50

[free.platform]
usable-by = ["core.*"]

[[flow]]
from = "ui.*"
to = ["core.service"]

[layer-policy."core.shell"]
used-by = "entry"

[kinds.un]
shape = "code"

[kinds.ctr]
shape = "code"

[hier]
level = 2
"#;

    fn units(same_rank: &str, code: &str) -> Vec<Diagnostic> {
        let root = format!("{L2}same-rank = \"{same_rank}\"\n");
        World::new()
            .root(&root)
            .file(
                "src/decl.rs",
                "// fairlead: def ctr.a\n// fairlead: def ctr.b\nfn decl() {}\n",
            )
            .file("src/a.rs", code)
            .build()
            .run_check(&HierCheck)
    }

    fn unit(id: &str, layer: &str, ctr: &str, extra: &str, uses: &[&str]) -> String {
        let mut s = format!("// fairlead: def {id} layer={layer} container={ctr}{extra} +end\n");
        for u in uses {
            s.push_str(&format!("// fairlead: use {u}\n"));
        }
        s.push_str("fn f() {}\n// fairlead: end\n");
        s
    }

    #[test]
    fn r_hier_020__a_use_edge_points_down_or_sideways_by_same_rank() {
        let code = [
            unit("un.eng", "core.engine", "ctr.a", "", &["un.prim", "un.svc"]),
            unit("un.prim", "core.primitive", "ctr.a", "", &[]),
            unit("un.svc", "core.service", "ctr.a", "", &[]),
            unit("un.eng2", "core.engine", "ctr.b", "", &["un.eng"]),
        ]
        .concat();
        let allow = units("allow", &code);
        let up = of(&allow, c::HN_H007);
        assert_eq!(up.len(), 1, "{allow:#?}");
        assert!(
            up[0]
                .message
                .contains("un.eng (core.engine) uses un.svc (core.service): rank 50 is above 30"),
            "{}",
            up[0].message
        );
        assert!(up[0].hint.contains("lower bound"), "R-HIER-034");
        // R-HIER-024: an upward edge is waived under the named built-in rule.hier-rank.
        let w = World::new().root(L2).build();
        let rules = crate::check::RuleSet::collect(&w.registry);
        assert_eq!(rules.named_waivable("hier-rank"), Some(true));
        assert_eq!(up[0].rule.as_deref(), Some("rule.hier-rank"));
        let ctr = units("container", &code);
        assert!(
            of(&ctr, c::HN_H007)
                .iter()
                .any(|d| d.message.contains("stay inside one container"))
        );
        let forbid = units("forbid", &code);
        assert!(
            of(&forbid, c::HN_H007)
                .iter()
                .any(|d| d.message.contains("forbidden"))
        );
    }

    #[test]
    fn r_hier_020__a_use_under_a_def_belongs_to_that_unit() {
        // 30-hierarchy, Declarations: `def` and `use` on the lines above one block.
        let code = "// fairlead: def un.eng layer=core.engine container=ctr.a\n// fairlead: use un.svc\nfn f() {\n    g();\n}\n\n// fairlead: def un.svc layer=core.service container=ctr.a\nfn g() {}\n";
        let ds = units("allow", code);
        let d = of(&ds, c::HN_H007);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert_eq!(
            (d[0].location.line, d[0].message.contains("un.eng")),
            (2, true)
        );
    }

    #[test]
    fn r_hier_021__unit_cycle_reported() {
        let code = [
            unit("un.a", "core.engine", "ctr.a", "", &["un.b"]),
            unit("un.b", "core.engine", "ctr.a", "", &["un.c"]),
            unit("un.c", "core.engine", "ctr.a", "", &["un.a"]),
            unit("un.d", "core.engine", "ctr.a", "", &["un.d"]),
        ]
        .concat();
        let ds = units("allow", &code);
        let d = of(&ds, c::HN_H008);
        assert_eq!(
            d.len(),
            1,
            "one cycle, and self-recursion is allowed: {ds:#?}"
        );
        assert!(
            d[0].message.contains("un.a, un.b, un.c"),
            "{}",
            d[0].message
        );
        assert!(d[0].hint.contains("lower bound"));
    }

    #[test]
    fn r_hier_022__container_scc_lists_causing_edges() {
        let code = [
            unit("un.a", "core.engine", "ctr.a", "", &["un.b"]),
            unit("un.b", "core.engine", "ctr.b", "", &["un.c"]),
            unit("un.c", "core.engine", "ctr.a", "", &[]),
        ]
        .concat();
        let ds = units("allow", &code);
        assert!(of(&ds, c::HN_H008).is_empty(), "the unit graph is a DAG");
        let d = of(&ds, c::HN_H009);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(
            d[0].message.contains("ctr.a, ctr.b")
                && d[0].message.contains("un.a -> un.b")
                && d[0].message.contains("un.b -> un.c"),
            "{}",
            d[0].message
        );
    }

    fn siblings(vis: &str) -> Vec<Diagnostic> {
        siblings_with(
            vis,
            "[scope]
provides = [\"un.core\"]
",
        )
    }

    fn siblings_with(vis: &str, lib_extra: &str) -> Vec<Diagnostic> {
        World::new()
            .root(L2)
            .scope(
                "lib",
                &format!(
                    "[fairlead]
scope = \"lib\"
{lib_extra}"
                ),
            )
            .scope(
                "app",
                "[fairlead]
scope = \"app\"
",
            )
            .file(
                "lib/src/a.rs",
                &format!(
                    "// fairlead: def ctr.l
fn d() {{}}
// fairlead: def un.core layer=core.engine container=ctr.l{vis}
fn core() {{}}
"
                ),
            )
            .file(
                "app/src/b.rs",
                "// fairlead: def ctr.p
fn d() {}
// fairlead: def un.svc layer=core.service container=ctr.p
// fairlead: use lib:un.core
fn svc() {}
",
            )
            .build()
            .run_check(&HierCheck)
    }

    #[test]
    fn r_hier_026__sibling_use_of_subtree_unit_flagged() {
        let ds = siblings("");
        let d = of(&ds, c::HN_H010);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(
            d[0].message.contains("vis=subtree") && d[0].hint.contains("scope root"),
            "{d:#?}"
        );
        assert!(of(&siblings(" vis=public"), c::HN_H010).is_empty());
        let unprovided = siblings_with("", "");
        assert!(
            of(&unprovided, c::HN_H010).is_empty(),
            "not provided at all is HN-R008, not reported twice"
        );
    }

    #[test]
    fn r_hier_028__passing_one_never_exempts_the_other() {
        // Public, so visible; but a service using an engine upward is still an upward edge only
        // when the ranks say so: here the service (50) uses an engine (30), which is fine, and a
        // public engine using a service would still be hier.rank.
        let ds = siblings(" vis=public");
        assert!(of(&ds, c::HN_H010).is_empty() && of(&ds, c::HN_H007).is_empty());
        let up = World::new()
            .root(L2)
            .file(
                "src/a.rs",
                "// fairlead: def ctr.a
fn d() {}
// fairlead: def un.svc layer=core.service container=ctr.a vis=public
fn s() {}
// fairlead: def un.eng layer=core.engine container=ctr.a
// fairlead: use un.svc
fn e() {}
",
            )
            .build()
            .run_check(&HierCheck);
        assert_eq!(
            of(&up, c::HN_H007).len(),
            1,
            "visible, still upward: {up:#?}"
        );
    }

    #[test]
    fn r_hier_002__cross_ladder_needs_a_flow() {
        let code = [
            unit("un.page", "ui.page", "ctr.a", "", &["un.svc", "un.eng"]),
            unit("un.svc", "core.service", "ctr.a", "", &[]),
            unit("un.eng", "core.engine", "ctr.a", "", &[]),
        ]
        .concat();
        let ds = units("allow", &code);
        let d = of(&ds, c::HN_H007);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(d[0].message.contains("un.eng") && d[0].message.contains("no [[flow]]"));
    }

    #[test]
    fn r_hier_005__free_layers_are_reached_through_their_lists() {
        let code = [
            unit("un.log", "platform", "ctr.a", "", &[]),
            unit("un.eng", "core.engine", "ctr.a", "", &["un.log"]),
            unit("un.page", "ui.page", "ctr.a", "", &["un.log"]),
        ]
        .concat();
        let ds = units("allow", &code);
        let d = of(&ds, c::HN_H007);
        assert_eq!(d.len(), 1, "{d:#?}");
        assert!(d[0].message.contains("un.page") && d[0].message.contains("usable only by core.*"));
    }

    #[test]
    fn r_hier_006__an_entry_only_layer_is_not_a_use_target() {
        let code = [
            unit("un.cli", "core.shell", "ctr.a", "", &[]),
            unit("un.svc", "core.service", "ctr.a", "", &["un.cli"]),
        ]
        .concat();
        let ds = units("allow", &code);
        let d = of(&ds, c::HN_H007);
        assert!(
            d.iter()
                .any(|x| x.message.contains("reached only from entries")),
            "{d:#?}"
        );
    }

    #[test]
    fn r_hier_023__implements_edges_are_exempt() {
        let code = [
            unit("un.port", "core.primitive", "ctr.a", "", &[]),
            unit(
                "un.ad",
                "core.primitive",
                "ctr.b",
                " implements=un.port",
                &["un.port"],
            ),
        ]
        .concat();
        assert!(
            of(&units("forbid", &code), c::HN_H007).is_empty(),
            "same rank and forbidden, but it implements the port"
        );
        let plain = [
            unit("un.port", "core.primitive", "ctr.a", "", &[]),
            unit("un.ad", "core.primitive", "ctr.b", "", &["un.port"]),
        ]
        .concat();
        assert_eq!(of(&units("forbid", &plain), c::HN_H007).len(), 1);
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
