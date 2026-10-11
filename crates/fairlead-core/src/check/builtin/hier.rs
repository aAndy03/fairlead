//! Hierarchy structure (30-hierarchy, P1): containment and binding (R-HIER-010 to R-HIER-014) and
//! a unit's layer (R-HIER-003), switched on by the adoption level (R-HIER-080).
use fairlead_syntax::diag::DiagCode;

use crate::check::builtin::{bare_id, origin_loc};
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::hier::{Model, literal_prefix, matcher, prefixes_overlap};
use crate::registry::{ItemId, ScopeId};

pub struct HierCheck;

/// A container edge: from, to, why (for the cycle report), and where it is written.
type ContainerEdge = (ItemId, ItemId, String, (crate::path::RepoPath, u32, u32));
const CODES: [DiagCode; 28] = [
    HN_H001, HN_H002, HN_H003, HN_H004, HN_H005, HN_H006, HN_H007, HN_H008, HN_H009, HN_H010,
    HN_H011, HN_H012, HN_H013, HN_H014, HN_H015, HN_H016, HN_H017, HN_H018, HN_H019, HN_H020,
    HN_H021, HN_H022, HN_H023, HN_H024, HN_H025, HN_E004, HN_D004, HN_D005,
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
    cargo: Vec<ContainerEdge>,
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
    // (from, to, description, location) of every container edge: unit edges aggregated, and
    // Cargo dependencies between the containers that claim the two crates (R-HIER-027).
    let mut causes: Vec<ContainerEdge> = Vec::new();
    for e in edges {
        if let (Some(a), Some(b)) = (ctr(&e.from), ctr(&e.to))
            && a != b
        {
            let why = format!(
                "{} -> {} ({}:{})",
                bare_id(&e.from),
                bare_id(&e.to),
                e.path.as_str(),
                e.line
            );
            causes.push((a, b, why, (e.path.clone(), e.line, e.col)));
        }
    }
    causes.extend(cargo);
    let mut graph: BTreeMap<ItemId, BTreeSet<ItemId>> = BTreeMap::new();
    for (a, b, _, _) in &causes {
        graph.entry(a.clone()).or_default().insert(b.clone());
    }
    for comp in crate::hier::cycles(&graph) {
        let Some(first) = comp.first().filter(|f| f.scope == *scope) else {
            continue;
        };
        let inside: Vec<_> = causes
            .iter()
            .filter(|(a, b, _, _)| comp.contains(a) && comp.contains(b))
            .collect();
        let Some((_, _, _, (path, line, col))) = inside.first() else {
            continue;
        };
        let names: Vec<String> = comp.iter().map(bare_id).collect();
        let why: Vec<&str> = inside.iter().map(|(_, _, w, _)| w.as_str()).collect();
        out.push(
            DiagB::new(
                HN_H009,
                path,
                *line,
                *col,
                format!(
                    "containers in a cycle: {}; caused by {}",
                    names.join(", "),
                    why.join(", ")
                ),
            )
            .rule("hier-cycle")
            .hint(format!(
                "extract the shared piece into a lower container (R-HIER-022), or waive rule.hier-cycle with until= while it is transitional (R-HIER-103); {LOWER_BOUND}"
            ))
            .related(bare_id(first))
            .text(bare_id(first))
            .build(),
        );
    }
}

/// R-HIER-027: the container edges of the Cargo dependencies between workspace crates, each
/// crate mapped to the container with the deepest `path=` claim that contains it.
fn cargo_edges(cx: &CheckCx<'_>, m: &Model) -> Vec<ContainerEdge> {
    let claims: Vec<(ItemId, Vec<String>)> = m
        .containers
        .keys()
        .map(|c| (c.clone(), claims(cx, m, c)))
        .collect();
    let owner = |dir: &crate::path::RepoPath| -> Option<ItemId> {
        claims
            .iter()
            .flat_map(|(c, ps)| ps.iter().map(move |p| (c, p)))
            .filter(|(_, p)| {
                p.is_empty()
                    || dir.as_str() == p.as_str()
                    || dir.as_str().starts_with(&format!("{p}/"))
            })
            .max_by_key(|(c, p)| (p.len(), (*c).clone()))
            .map(|(c, _)| c.clone())
    };
    let mut out = Vec::new();
    for (a, deps) in &cx.facts.manifest_hints.crate_deps {
        let Some(ca) = owner(a) else { continue };
        for b in deps {
            if let Some(cb) = owner(b).filter(|cb| *cb != ca) {
                let manifest = crate::path::RepoPath::new(&if a.is_root() {
                    "Cargo.toml".to_owned()
                } else {
                    format!("{}/Cargo.toml", a.as_str())
                })
                .unwrap_or_else(|_| a.clone());
                let why = format!(
                    "crate {} depends on {} (evidence=cargo)",
                    a.as_str(),
                    b.as_str()
                );
                out.push((ca.clone(), cb, why, (manifest, 0, 0)));
            }
        }
    }
    out
}

/// R-HIER-027: within one ladder, a Cargo edge must not point to a container whose highest unit
/// layer ranks above the dependent container's.
fn cargo_rank(
    h: &crate::config::HierConfig,
    m: &Model,
    cargo: &[ContainerEdge],
    scope: &ScopeId,
    out: &mut Vec<Diagnostic>,
) {
    let top = |c: &ItemId| -> Option<(&str, u32, &str)> {
        m.units
            .values()
            .filter(|u| u.container.as_ref() == Some(c))
            .filter_map(|u| {
                let l = u.layer.as_deref()?;
                h.rank(l).map(|(ladder, r)| (ladder, r, l))
            })
            .max_by_key(|(ladder, r, _)| (*r, *ladder))
    };
    for (a, b, why, (path, line, col)) in cargo.iter().filter(|(a, ..)| a.scope == *scope) {
        let (Some((la, ra, layer_a)), Some((lb, rb, layer_b))) = (top(a), top(b)) else {
            continue;
        };
        if la == lb && rb > ra {
            out.push(
                DiagB::new(
                    HN_H007,
                    path,
                    *line,
                    *col,
                    format!(
                        "container {} (up to {layer_a}) depends on {} (up to {layer_b}): {why}",
                        bare_id(a),
                        bare_id(b)
                    ),
                )
                .rule("hier-rank")
                .hint("move the shared part into a lower container, or invert the dependency (R-HIER-027)")
                .related(bare_id(b))
                .text(bare_id(b))
                .build(),
            );
        }
    }
}

/// R-HIER-040 to R-HIER-046 (level 3): entries, reachability and `hier.stale`.
fn entries(
    cx: &CheckCx<'_>,
    h: &crate::config::HierConfig,
    m: &Model,
    edges: &[crate::hier::UnitEdge],
    bound: &std::collections::BTreeMap<ItemId, std::collections::BTreeSet<crate::path::RepoPath>>,
    scope: &ScopeId,
    out: &mut Vec<Diagnostic>,
) {
    use crate::hier::{Reach, SHIPPED_ETYPES};
    let reg = cx.registry;
    let all = m.entries(reg);
    for e in all.iter().filter(|e| e.id.scope == *scope) {
        if !SHIPPED_ETYPES.contains(&e.etype.as_str()) && !h.settings.etypes.contains(&e.etype) {
            out.extend(at(
                cx,
                &e.id,
                HN_E004,
                format!("entry {} has etype {:?}, which is not declared", bare_id(&e.id), e.etype),
                "use user-action, cli, export, test or registered, or add the word to [hier] etypes (R-HIER-040)",
            ));
        }
        match (&e.unit_raw, &e.unit) {
            (Some(raw), None) => out.extend(at(
                cx,
                &e.id,
                HN_H004,
                format!(
                    "entry {} names unit {raw}, which is not a declared unit",
                    bare_id(&e.id)
                ),
                "name the unit the entry starts at with unit=<id> (R-HIER-040)",
            )),
            (None, _) => out.extend(at(
                cx,
                &e.id,
                HN_H004,
                format!("entry {} names no unit", bare_id(&e.id)),
                "add unit=<id>: the unit the entry starts at (R-HIER-040)",
            )),
            _ => {}
        }
        if let (Reach::To(raw, target), Some(u)) = (&e.reaches, &e.unit) {
            let chain = m.reach_from(&std::collections::BTreeSet::from([u.clone()]), edges);
            if target.as_ref().is_none_or(|t| !chain.contains(t)) {
                out.extend(at(
                    cx,
                    &e.id,
                    HN_H018,
                    format!(
                        "entry {} declares reaches={raw}, but its chain from {} does not reach it",
                        bare_id(&e.id),
                        bare_id(u)
                    ),
                    "fix reaches=, or declare the missing use edges (R-HIER-044)",
                ));
            }
        }
    }
    let mut skip_stale = false;
    if all.is_empty() {
        // R-HIER-042: with zero entries declared, stale is skipped, once, at the root.
        if scope == reg.root_scope()
            && let Some(cp) = crate::check::builtin::control_path(reg, scope)
        {
            out.push(
                DiagB::new(HN_H018, &cp, 0, 0, "no entries declared: hier.stale is skipped")
                    .severity(fairlead_syntax::diag::Severity::Info)
                    .hint("declare entries with def <kind>.<name> etype=<word> unit=<id> (R-HIER-040, R-HIER-042)")
                    .build(),
            );
        }
        // Dormancy does not depend on entries; stale and forgotten do.
        skip_stale = true;
    }
    let reached = m.reached(reg, &all, edges);
    // R-HIER-043, R-HIER-045: dormant (digest unchanged for N seals, from the ledger) and
    // forgotten (stale and dormant).
    let history = crate::hier::digest_history(reg, &cx.facts.ledgers);
    let n = usize::try_from(h.settings.dormant_seals)
        .unwrap_or(usize::MAX)
        .max(1);
    let dormant = |u: &crate::hier::Unit| -> Option<(usize, String)> {
        let past = history.get(&u.id)?;
        let now = crate::hier::unit_digest(bound.get(&u.id)?, cx.corpus);
        let same = past.iter().rev().take_while(|r| r.ud == now).count();
        let since = past.iter().rev().nth(same.checked_sub(1)?)?.v.to_string();
        (same >= n).then_some((same, since))
    };
    // R-HIER-046: units named in `adds` or `touches` of an unsealed change doc are exempt.
    let (docs, _) = crate::docs::collect_change_docs(cx.corpus);
    let pending: std::collections::BTreeSet<String> = docs
        .iter()
        .filter(|d| {
            matches!(
                d.status,
                crate::docs::Status::Draft | crate::docs::Status::Ready
            )
        })
        .flat_map(|d| {
            ["adds", "touches"]
                .iter()
                .filter_map(|k| d.extra.get(*k))
                .flat_map(|v| crate::lifecycle::doc_ids(v))
                .collect::<Vec<_>>()
        })
        .collect();
    for u in m.units.values().filter(|u| u.id.scope == *scope) {
        let named =
            pending.contains(&bare_id(&u.id)) || pending.contains(&u.id.to_id().to_string());
        let quiet = dormant(u);
        if let Some((same, since)) = &quiet {
            out.extend(at(
                cx,
                &u.id,
                HN_H019,
                format!(
                    "{} is unchanged for {same} seals (since {since})",
                    bare_id(&u.id)
                ),
                "information only: dormancy reads the seal ledger, never git (R-HIER-043)",
            ));
        }
        if skip_stale
            || reached.contains(&u.id)
            || named
            || bound.get(&u.id).is_none_or(|b| b.is_empty())
        {
            continue;
        }
        if let Some((same, since)) = &quiet {
            out.extend(at(
                cx,
                &u.id,
                HN_H020,
                format!("{} is forgotten: reached by no entry and unchanged for {same} seals (since {since})", bare_id(&u.id)),
                "retire it to the graveyard (fairlead flag remove, then seal), or reach it from an entry (R-HIER-043)",
            ));
            continue;
        }
        out.extend(at(
            cx,
            &u.id,
            HN_H018,
            format!("{} is reached by no entry", bare_id(&u.id)),
            &format!("declare an entry that reaches it, use it from a reached unit, or retire it (R-HIER-041); {LOWER_BOUND}"),
        ));
    }
}

/// R-HIER-060 to R-HIER-063, R-CHK-043 (level 3): a bridge declares its keys, each end carries
/// its `def <id> side=ts|rs` marker and its `#symbol`, the wire name appears on both sides (a TS
/// call literal, a Rust fn name), `ver` agrees on the end markers and on consumers' `pin=`, and a
/// dynamic `invoke(name)` is unverifiable rather than guessed. Text presence only: payload fields
/// stay unverified (R-HIER-061).
fn bridges(cx: &CheckCx<'_>, m: &Model, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
    use crate::extract::idents::mentions;
    let reg = cx.registry;
    let dirs_of = |p: &crate::path::RepoPath| {
        cx.corpus
            .records
            .get(p)
            .and_then(|r| r.extraction.as_ref())
            .map(|x| x.directives.clone())
            .unwrap_or_default()
    };
    let contracts = crate::hier::contract_history(reg, &cx.facts.ledgers);
    let (docs, _) = crate::docs::collect_change_docs(cx.corpus);
    let touched: std::collections::BTreeSet<String> = docs
        .iter()
        .filter(|d| {
            matches!(
                d.status,
                crate::docs::Status::Draft | crate::docs::Status::Ready
            )
        })
        .flat_map(|d| {
            d.extra
                .get("touches")
                .map(|v| crate::lifecycle::doc_ids(v))
                .unwrap_or_default()
        })
        .collect();
    for b in m.bridges(reg).iter().filter(|b| b.id.scope == *scope) {
        let id = bare_id(&b.id);
        let missing: Vec<&str> = crate::hier::BRIDGE_KEYS
            .iter()
            .copied()
            .filter(|k| b.get(k).is_none())
            .collect();
        if !missing.is_empty() {
            out.extend(at(
                cx,
                &b.id,
                HN_H021,
                format!("bridge {id} declares no {}", missing.join(", ")),
                "a bridge declares proto, dir, wire, ver, params and one ts= and one rs= end as path#symbol (R-HIER-060)",
            ));
        }
        let ver = b.get("ver");
        let wire = b.get("wire");
        for side in ["ts", "rs"] {
            let Some(raw) = b.get(side) else { continue };
            let Some((path, sym)) = b.end(reg, side) else {
                out.extend(at(
                    cx,
                    &b.id,
                    HN_H021,
                    format!("bridge {id}: {side}={raw} is not path#symbol"),
                    "write the end as path#symbol (R-HIER-060)",
                ));
                continue;
            };
            let Some(rec) = cx.corpus.records.get(&path) else {
                out.extend(at(
                    cx,
                    &b.id,
                    HN_H021,
                    format!(
                        "bridge {id}: the {side} end {} does not exist",
                        path.as_str()
                    ),
                    "fix the path, or restore the file (R-HIER-062)",
                ));
                continue;
            };
            let marker = dirs_of(&path).into_iter().find(|d| {
                d.verb == fairlead_syntax::directive::Verb::Def
                    && d.ids
                        .first()
                        .is_some_and(|i| i.kind == b.id.kind && i.name == b.id.name)
                    && d.args.iter().any(|a| {
                        a.key == "side" && crate::registry::item::arg_scalar(&a.value) == side
                    })
            });
            let at_end = |code, msg: String, hint: &str, line: u32| {
                DiagB::new(code, &path, line, 0, msg)
                    .hint(hint)
                    .related(id.clone())
                    .text(id.clone())
                    .build()
            };
            match &marker {
                None => out.push(at_end(
                    HN_H021,
                    format!(
                        "bridge {id}: {} has no `def {id} side={side}` marker",
                        path.as_str()
                    ),
                    "mark the end next to its symbol (R-HIER-062)",
                    0,
                )),
                Some(d) => {
                    let mv = d
                        .args
                        .iter()
                        .find(|a| a.key == "ver")
                        .map(|a| crate::registry::item::arg_scalar(&a.value));
                    if let (Some(mv), Some(v)) = (mv.as_deref(), ver)
                        && mv != v
                    {
                        out.push(at_end(
                            HN_H022,
                            format!("bridge {id}: the {side} end marker says ver={mv}, the declaration ver={v}"),
                            "update the end, then its ver (R-HIER-063)",
                            d.span.line,
                        ));
                    }
                }
            }
            if !mentions(&rec.idents, &sym) {
                out.push(at_end(
                    HN_H021,
                    format!("bridge {id}: the {side} end never mentions {sym}"),
                    "fix the #symbol, or restore it (R-HIER-062)",
                    0,
                ));
            }
            if let Some(w) = wire
                && !mentions(&rec.idents, w)
            {
                match rec.dyn_invokes.first() {
                    Some(line) if side == "ts" => out.push(at_end(
                        HN_H024,
                        format!("bridge {id}: the ts end calls invoke with a dynamic name; wire {w} is unverifiable"),
                        "use the literal wire name at the call, or accept the warning (R-CHK-043)",
                        *line,
                    )),
                    _ => out.push(at_end(
                        HN_H025,
                        format!("bridge {id}: the {side} end has no wire {w}"),
                        if side == "ts" {
                            "call the command with its wire literal (R-HIER-063)"
                        } else {
                            "name the Rust fn after the wire (R-HIER-063)"
                        },
                        0,
                    )),
                }
            }
        }
        // R-HIER-064, R-HIER-066: a contract changed since the last seal needs a pending change
        // doc that touches the bridge, and the files generated from it must have changed too.
        if let Some((cpath, ch, gh)) = crate::hier::contract_hashes(reg, cx.corpus, b) {
            match (&ch, contracts.get(&b.id)) {
                (None, _) => out.extend(at(
                    cx,
                    &b.id,
                    HN_H021,
                    format!("bridge {id}: contract {} does not exist", cpath.as_str()),
                    "fix contract=, or restore the file (R-HIER-064)",
                )),
                (Some(now), Some((v, was, gen_was))) if now != was => {
                    let qualified = b.id.to_id().to_string();
                    if !touched.contains(&id) && !touched.contains(&qualified) {
                        out.push(
                            DiagB::new(
                                HN_D004,
                                &cpath,
                                0,
                                0,
                                format!("the contract of bridge {id} changed since {v}, and no draft or ready change doc touches it"),
                            )
                            .hint("add the bridge to touches of a change doc that covers both ends (R-HIER-064)")
                            .related(id.clone())
                            .text(id.clone())
                            .build(),
                        );
                    }
                    if !gh.is_empty() && gen_was.as_deref() == Some(gh.as_str()) {
                        out.push(
                            DiagB::new(
                                HN_H022,
                                &cpath,
                                0,
                                0,
                                format!("bridge {id}: the files generated from its contract were not regenerated after it changed"),
                            )
                            .hint("rerun the binding generator (R-HIER-066)")
                            .related(id.clone())
                            .text(id.clone())
                            .build(),
                        );
                    }
                }
                _ => {}
            }
        }
        // R-SYN-049: a consumer's `use <bridge> pin=<N>` must equal the declared ver.
        for e in reg
            .edges()
            .iter()
            .filter(|e| e.resolved.as_ref() == Some(&b.id))
        {
            let pin = dirs_of(&e.from.path).into_iter().find_map(|d| {
                (d.verb == fairlead_syntax::directive::Verb::Use && d.span.line == e.from.span.line)
                    .then(|| {
                        d.args
                            .iter()
                            .find(|a| a.key == "pin")
                            .map(|a| crate::registry::item::arg_scalar(&a.value))
                    })
                    .flatten()
            });
            if let (Some(p), Some(v)) = (pin.as_deref(), ver)
                && p != v
            {
                out.push(
                    DiagB::new(
                        HN_H022,
                        &e.from.path,
                        e.from.span.line,
                        e.from.span.col,
                        format!("use {id} pin={p}, but the bridge is ver={v}"),
                    )
                    .hint("review the consumer against the new version, then raise its pin (R-HIER-063, R-SYN-049)")
                    .related(id.clone())
                    .text(id.clone())
                    .build(),
                );
            }
        }
    }
}

/// R-HIER-090 (level 1 and up, on the docs-first ramp): a pending change doc listing an ID that
/// does not exist is `HN-D005`; at level 3, once a seal has recorded the hierarchy, a unit,
/// container or bridge new since then that no pending doc lists in `adds` or `touches` is
/// `HN-D004`.
fn docs_first(cx: &CheckCx<'_>, m: &Model, level: u8, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
    let reg = cx.registry;
    let (stage, _) = crate::check::builtin::coverage::scope_policy(cx, scope);
    let severity = match stage {
        crate::config::Stage::Enforce => fairlead_syntax::diag::Severity::Error,
        _ => fairlead_syntax::diag::Severity::Warn,
    };
    let (docs, _) = crate::docs::collect_change_docs(cx.corpus);
    let pending: Vec<&crate::docs::ChangeDoc> = docs
        .iter()
        .filter(|d| {
            matches!(
                d.status,
                crate::docs::Status::Draft | crate::docs::Status::Ready
            )
        })
        .collect();
    let listed = |d: &crate::docs::ChangeDoc| -> Vec<String> {
        let mut v: Vec<String> = ["adds", "touches"]
            .iter()
            .filter_map(|k| d.extra.get(*k))
            .flat_map(|x| crate::lifecycle::doc_ids(x))
            .collect();
        v.extend(d.covers.iter().cloned());
        v.into_iter()
            .filter(|x| fairlead_syntax::ids::Id::parse(x).is_ok())
            .collect()
    };
    for d in pending
        .iter()
        .filter(|d| reg.scope_of(&d.path) == Some(scope))
    {
        for raw in listed(d) {
            let Ok(id) = fairlead_syntax::ids::Id::parse(&raw) else {
                continue;
            };
            let found = matches!(
                reg.lookup(scope, &id),
                crate::registry::Lookup::Found(_) | crate::registry::Lookup::Ambiguous(_)
            );
            if found || reg.tomb_for(scope, &id).is_some() {
                continue;
            }
            out.push(
                DiagB::new(
                    HN_D005,
                    &d.path,
                    1,
                    1,
                    format!(
                        "change doc {} lists {raw}, which does not exist",
                        d.id.as_str()
                    ),
                )
                .hint("fix the ID, or drop it from adds, touches or covers (R-HIER-090)")
                .severity(severity)
                .stage(stage)
                .related(raw.clone())
                .text(raw)
                .build(),
            );
        }
    }
    if level < 3 {
        return;
    }
    let recorded = crate::hier::recorded_ids(reg, &cx.facts.ledgers);
    if recorded.is_empty() {
        return; // nothing sealed at level 3 yet: the first seal records the hierarchy
    }
    let in_docs: std::collections::BTreeSet<String> = pending
        .iter()
        .flat_map(|d| {
            ["adds", "touches"]
                .iter()
                .filter_map(|k| d.extra.get(*k))
                .flat_map(|x| crate::lifecycle::doc_ids(x))
                .collect::<Vec<_>>()
        })
        .collect();
    let bridges: Vec<ItemId> = m.bridges(reg).into_iter().map(|b| b.id).collect();
    let ids = m
        .units
        .keys()
        .chain(m.containers.keys())
        .chain(bridges.iter())
        .filter(|i| i.scope == *scope);
    for i in ids {
        let name = bare_id(i);
        if recorded.contains(&(i.scope.clone(), name.clone()))
            || in_docs.contains(&name)
            || in_docs.contains(&i.to_id().to_string())
        {
            continue;
        }
        if let Some(it) = reg.item(i) {
            let (p, l, c) = origin_loc(it);
            out.push(
                DiagB::new(
                    HN_D004,
                    &p,
                    l,
                    c,
                    format!(
                        "{name} is new and no draft or ready change doc lists it in adds or touches"
                    ),
                )
                .hint("add it to adds (new) or touches (changed) of a change doc (R-HIER-090)")
                .severity(severity)
                .stage(stage)
                .related(name.clone())
                .text(name)
                .build(),
            );
        }
    }
}

/// R-HIER-030: the units whose `sym` is usable as evidence (at least 6 characters, unique across
/// the declared syms), and the ambiguous ones.
/// Units with their `sym`.
type Syms = Vec<(ItemId, String)>;

fn syms(cx: &CheckCx<'_>, m: &Model) -> (Syms, Syms) {
    let mut all: Vec<(ItemId, String)> = m
        .units
        .keys()
        .filter_map(|u| {
            let s = cx
                .registry
                .item(u)
                .and_then(|it| crate::check::builtin::field_str(it, "sym"))?;
            Some((u.clone(), s.to_owned()))
        })
        .collect();
    all.sort();
    let (good, bad) = all.iter().cloned().partition(|(_, s)| {
        s.len() >= crate::extract::idents::MIN_SYM
            && all.iter().filter(|(_, t)| t == s).count() == 1
    });
    (good, bad)
}

/// R-HIER-030 (L1): an ambiguous `sym` suppresses the unit's evidence checks until fixed.
fn sym_ambiguous(cx: &CheckCx<'_>, m: &Model, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
    let (_, bad) = syms(cx, m);
    for (u, s) in bad.iter().filter(|(u, _)| u.scope == *scope) {
        let why = if s.len() < crate::extract::idents::MIN_SYM {
            "shorter than 6 characters"
        } else {
            "declared by another unit too"
        };
        out.extend(at(
            cx,
            u,
            HN_H006,
            format!(
                "sym {s} of {} is {why}; its evidence checks are off",
                bare_id(u)
            ),
            "pick a distinctive identifier of 6 or more characters (R-HIER-030)",
        ));
    }
}

/// R-HIER-031 (L2): a file with `use X` that never mentions X's `sym` (`HN-H011`), and a file
/// that mentions it without `use X` (`HN-H012`). Comments are excluded and a match is a whole
/// identifier (`extract::idents`). Markdown registry docs carry no code and are not judged; a
/// `use X dyn=1` declares an edge with no evidence (R-HIER-025).
fn evidence(
    cx: &CheckCx<'_>,
    m: &Model,
    files: &[&crate::path::RepoPath],
    scope: &ScopeId,
    out: &mut Vec<Diagnostic>,
) {
    use crate::extract::idents::mentions;
    let reg = cx.registry;
    let (good, _) = syms(cx, m);
    let sym_of = |u: &ItemId| good.iter().find(|(x, _)| x == u).map(|(_, s)| s.as_str());
    let code = |p: &crate::path::RepoPath| !matches!(p.extension(), "md" | "mdx");
    let idents = |p: &crate::path::RepoPath| {
        cx.corpus
            .records
            .get(p)
            .map(|r| r.idents.as_slice())
            .unwrap_or_default()
    };
    let dynamic = |e: &crate::registry::Edge| {
        cx.corpus
            .records
            .get(&e.from.path)
            .and_then(|r| r.extraction.as_ref())
            .is_some_and(|x| {
                x.directives.iter().any(|d| {
                    d.verb == fairlead_syntax::directive::Verb::Use
                        && d.span.line == e.from.span.line
                        && d.args.iter().any(|a| a.key == "dyn")
                })
            })
    };
    let mut used: std::collections::BTreeSet<(&crate::path::RepoPath, &ItemId)> =
        std::collections::BTreeSet::new();
    for e in reg.edges() {
        let Some(t) = e.resolved.as_ref().filter(|t| m.units.contains_key(*t)) else {
            continue;
        };
        used.insert((&e.from.path, t));
        if e.from.scope != *scope || !code(&e.from.path) || dynamic(e) {
            continue;
        }
        let Some(sym) = e.sym.as_deref().or_else(|| sym_of(t)) else {
            continue;
        };
        if !mentions(idents(&e.from.path), sym) {
            out.push(
                DiagB::new(
                    HN_H011,
                    &e.from.path,
                    e.from.span.line,
                    e.from.span.col,
                    format!("use {} without its sym {sym} in this file", bare_id(t)),
                )
                .hint("call or import the symbol here, or drop the use; mark a dynamic call with use <id> dyn=1 (R-HIER-031)")
                .related(bare_id(t))
                .text(bare_id(t))
                .build(),
            );
        }
    }
    let bound = m.bindings(reg, files);
    // The units each file is bound to; only units, since a container is not a `use` target.
    let mut owners_of: std::collections::BTreeMap<&crate::path::RepoPath, Vec<&ItemId>> =
        std::collections::BTreeMap::new();
    for (u, fs) in bound.iter().filter(|(u, _)| m.units.contains_key(*u)) {
        for f in fs {
            owners_of.entry(f).or_default().push(u);
        }
    }
    for f in files
        .iter()
        .filter(|f| reg.scope_of(f) == Some(scope) && code(f))
    {
        let ids = idents(f);
        let mut reported: std::collections::BTreeSet<&ItemId> = std::collections::BTreeSet::new();
        for (u, sym) in &good {
            if mentions(ids, sym)
                && !used.contains(&(*f, u))
                && !bound.get(u).is_some_and(|b| b.contains(*f))
            {
                reported.insert(u);
                out.push(
                    DiagB::new(
                        HN_H012,
                        f,
                        0,
                        0,
                        format!(
                            "{} mentions {sym}, the sym of {}, without use {}",
                            f.as_str(),
                            bare_id(u),
                            bare_id(u)
                        ),
                    )
                    .hint(
                        "declare the edge with // fairlead: use <id> next to the call (R-HIER-031)",
                    )
                    .related(bare_id(u))
                    .text(bare_id(u))
                    .build(),
                );
            }
        }
        // R-HIER-033: an import of a file bound to another unit, with no `use` of that unit,
        // contradicts the declared edges. Evidence never creates a rank or stale verdict.
        let Some(rec) = cx.corpus.records.get(*f) else {
            continue;
        };
        for imp in &rec.imports {
            let Some(target) = crate::evidence::candidates(f.as_str(), &imp.spec)
                .into_iter()
                .filter_map(|c| crate::path::RepoPath::new(&c).ok())
                .find(|c| cx.corpus.records.contains_key(c))
            else {
                continue;
            };
            let Some(owners) = owners_of.get(&target) else {
                continue;
            };
            for u in owners {
                if used.contains(&(*f, u))
                    || bound.get(u).is_some_and(|b| b.contains(*f))
                    || !reported.insert(u)
                {
                    continue;
                }
                out.push(
                    DiagB::new(
                        HN_H012,
                        f,
                        imp.line,
                        0,
                        format!(
                            "{} imports {}, bound to {}, without use {} (evidence=import)",
                            f.as_str(),
                            target.as_str(),
                            bare_id(u),
                            bare_id(u)
                        ),
                    )
                    .hint("declare the edge with // fairlead: use <id> next to the import (R-HIER-031, R-HIER-033)")
                    .related(bare_id(u))
                    .text(bare_id(u))
                    .build(),
                );
            }
        }
    }
}

/// R-HIER-073 (`HN-H017`): a unit edge from a more mature unit to a less mature one, or from a
/// unit that reaches an environment its dependency does not exist in. The 0.2 checks
/// `rule.maturity-gap` (`HN-E006`) and `rule.env-leakage` (`HN-E001`) already judge every `use`
/// inside a `def` site's extent; this one judges the unit edges they cannot see (a `path=`-bound
/// unit, or a `use` written with the `def` above its block), so nothing is reported twice.
fn maturity(
    cx: &CheckCx<'_>,
    edges: &[crate::hier::UnitEdge],
    scope: &ScopeId,
    out: &mut Vec<Diagnostic>,
) {
    use crate::check::builtin::envs::{covers, rank_steps, resolve_axes};
    let reg = cx.registry;
    let Some(root) = crate::check::builtin::root_control(reg) else {
        return;
    };
    if !root.lifecycle_declared() {
        return;
    }
    let v = crate::lifecycle::Vocab::new(&root.vocab);
    let res = resolve_axes(reg, cx.corpus, &v);
    let steps = rank_steps(&v);
    for e in edges.iter().filter(|e| e.from.scope == *scope) {
        let seen_by_env_check = reg
            .sites_of(&e.from)
            .iter()
            .any(|s| s.path == e.path && covers(&s.extent, e.line));
        if seen_by_env_check {
            continue;
        }
        let (Some(fa), Some(ta)) = (res.axes.get(&e.from), res.axes.get(&e.to)) else {
            continue;
        };
        let mut why = Vec::new();
        if let (Some(fr), Some(tr)) = (fa.mat.values().min(), ta.mat.values().min())
            && steps.get(fr).zip(steps.get(tr)).is_some_and(|(a, b)| a > b)
        {
            why.push(format!(
                "{} is {} and {} is {}",
                bare_id(&e.from),
                v.rank_word(*fr),
                bare_id(&e.to),
                v.rank_word(*tr)
            ));
        }
        let missing: Vec<&str> = fa
            .envs
            .difference(&ta.envs)
            .map(|env| v.env_name(*env))
            .collect();
        if !missing.is_empty() {
            why.push(format!(
                "{} reaches {} where {} does not exist",
                bare_id(&e.from),
                missing.join(", "),
                bare_id(&e.to)
            ));
        }
        if !why.is_empty() {
            out.push(
                DiagB::new(HN_H017, &e.path, e.line, e.col, why.join("; "))
                    .hint("raise the dependency's maturity or envs, lower the dependent's, or drop the edge (R-HIER-073)")
                    .related(bare_id(&e.to))
                    .text(bare_id(&e.to))
                    .build(),
            );
        }
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
            let mut any_bound: std::collections::BTreeSet<&crate::path::RepoPath> =
                bound.values().flatten().collect();
            // R-HIER-068: an external build input (`def ext.<name> role=input path=...`) is
            // vendor-owned for scanning, so its files are never orphans.
            let inputs: Vec<(String, Vec<globset::GlobMatcher>)> = cx
                .registry
                .items()
                .filter(|it| crate::check::builtin::field_str(it, "role") == Some("input"))
                .map(|it| {
                    let dir = cx
                        .registry
                        .scope(&it.id.scope)
                        .map(|s| s.dir.as_str().trim_matches('/').to_owned())
                        .unwrap_or_default();
                    let globs = crate::check::builtin::field_list(it, "path")
                        .iter()
                        .filter_map(|g| matcher(g))
                        .collect();
                    (dir, globs)
                })
                .collect();
            for f in &files {
                let hit = inputs.iter().any(|(dir, ms)| {
                    let rel = if dir.is_empty() {
                        Some(f.as_str())
                    } else {
                        f.as_str()
                            .strip_prefix(dir.as_str())
                            .and_then(|r| r.strip_prefix('/'))
                    };
                    rel.is_some_and(|r| ms.iter().any(|m| m.is_match(r)))
                });
                if hit {
                    any_bound.insert(f);
                }
            }
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
        // Built once: the unit edges every level-2 and level-3 check reads.
        let edges = if level >= 2 {
            m.edges(cx.registry, &files)
        } else {
            Vec::new()
        };
        if level >= 3 {
            entries(cx, h, &m, &edges, &bound, scope, out);
            bridges(cx, &m, scope, out);
        }
        if level >= 2 {
            rank_edges(h, &m, &edges, scope, out);
            let cargo = cargo_edges(cx, &m);
            cargo_rank(h, &m, &cargo, scope, out);
            cycle_edges(&m, &edges, cargo, scope, out);
            visibility(cx, &m, scope, out);
            evidence(cx, &m, &files, scope, out);
            maturity(cx, &edges, scope, out);
            for sg in m
                .suggestions(
                    cx.registry,
                    &edges,
                    h,
                    &crate::hier::digest_history(cx.registry, &cx.facts.ledgers),
                )
                .iter()
                .filter(|x| x.unit.scope == *scope)
            {
                let code = match sg.word {
                    "promote" => HN_H013,
                    "demote" => HN_H014,
                    "merge" => HN_H016,
                    _ => HN_H015,
                };
                let to = if sg.to.is_empty() {
                    String::new()
                } else {
                    format!(" to {}", sg.to)
                };
                out.extend(at(
                    cx,
                    &sg.unit,
                    code,
                    format!("suggest {} {}{to}: {}", sg.word, bare_id(&sg.unit), sg.why),
                    "a suggestion only; sync lists them in gen/hier.suggest (R-HIER-072)",
                ));
            }
        }
        sym_ambiguous(cx, &m, scope, out);
        docs_first(cx, &m, level, scope, out);
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
                ([raw], None) => {
                    // R-HIER-102: a retired container's tombstone says where it went.
                    let moved = cx
                        .registry
                        .retired()
                        .iter()
                        .find(|t| t.id == *raw || raw.ends_with(&format!(":{}", t.id)))
                        .and_then(|t| t.moved_to.clone());
                    let hint = match &moved {
                        Some(to) => format!(
                            "{raw} was retired and moved to {to}: name it in container= (R-HIER-102)"
                        ),
                        None => {
                            "declare the container, or name an existing one (R-HIER-050)".to_owned()
                        }
                    };
                    out.extend(at(
                        cx,
                        &u.id,
                        HN_H004,
                        format!(
                            "{} names container {raw}, which does not exist",
                            bare_id(&u.id)
                        ),
                        &hint,
                    ));
                }
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
            // R-HIER-004, D-65: a scope narrows the layers its units may name with `accepts`.
            let problem = problem.or_else(|| {
                let l = u.layer.as_deref()?;
                crate::hier::accepts(cx.registry, scope)
                    .into_iter()
                    .find(|(_, pats)| !pats.iter().any(|p| h.matches(p, l)))
                    .map(|(s, pats)| {
                        format!(
                            "{} names layer {l}; scope {} accepts only {}",
                            bare_id(&u.id),
                            s.0.as_str(),
                            pats.join(", ")
                        )
                    })
            });
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
        assert_eq!(d[0].rule.as_deref(), Some("rule.hier-cycle"), "R-HIER-103");
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
    fn r_hier_027__cargo_edge_feeds_cycle_and_rank() {
        let rp = |s: &str| crate::path::RepoPath::new(s).unwrap();
        let run = |deps: &[(&str, &str)]| {
            let deps: Vec<(String, String)> = deps
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect();
            World::new()
                .root(L2)
                .file(
                    "crates/core/src/lib.rs",
                    "// fairlead: def ctr.core path=\"crates/core/**\"\n// fairlead: def un.eng layer=core.engine container=ctr.core\nfn eng() {}\n",
                )
                .file(
                    "crates/app/src/lib.rs",
                    "// fairlead: def ctr.app path=\"crates/app/**\"\n// fairlead: def un.svc layer=core.service container=ctr.app\nfn svc() {}\n",
                )
                .facts(move |f| {
                    for (a, b) in &deps {
                        f.manifest_hints
                            .crate_deps
                            .entry(rp(a))
                            .or_default()
                            .insert(rp(b));
                    }
                })
                .build()
                .run_check(&HierCheck)
        };
        let down = run(&[("crates/app", "crates/core")]);
        assert!(
            of(&down, c::HN_H007).is_empty() && of(&down, c::HN_H009).is_empty(),
            "{down:#?}"
        );
        let up = run(&[("crates/core", "crates/app")]);
        let r = of(&up, c::HN_H007);
        assert_eq!(r.len(), 1, "{up:#?}");
        assert!(
            r[0].message.contains("evidence=cargo")
                && r[0].location.path.as_str() == "crates/core/Cargo.toml"
        );
        let both = run(&[("crates/core", "crates/app"), ("crates/app", "crates/core")]);
        let cyc = of(&both, c::HN_H009);
        assert_eq!(cyc.len(), 1, "{both:#?}");
        assert!(
            cyc[0].message.contains("ctr.app, ctr.core")
                && cyc[0].message.contains("evidence=cargo")
        );
    }

    fn evidence_world(caller: &str) -> Vec<Diagnostic> {
        World::new()
            .root(L2)
            .file(
                "src/core.rs",
                "// fairlead: def ctr.a\nfn d() {}\n// fairlead: def un.verify layer=core.primitive container=ctr.a sym=verify_manifest\nfn verify_manifest() {}\n// fairlead: def un.tiny layer=core.primitive container=ctr.a sym=go\nfn go() {}\n",
            )
            .file("src/caller.rs", caller)
            .build()
            .run_check(&HierCheck)
    }

    #[test]
    fn r_hier_030__sym_must_be_long_and_unique() {
        let ds = evidence_world("fn x() {}\n");
        let d = of(&ds, c::HN_H006);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(
            d[0].message
                .contains("sym go of un.tiny is shorter than 6 characters")
        );
    }

    #[test]
    fn r_hier_031__use_without_its_sym_is_no_evidence() {
        let head = "// fairlead: def un.eng layer=core.engine container=ctr.a\n// fairlead: use un.verify\n";
        let without = evidence_world(&format!("{head}fn e() {{ other_thing(); }}\n"));
        let d = of(&without, c::HN_H011);
        assert_eq!(d.len(), 1, "{without:#?}");
        assert!(
            d[0].message
                .contains("use un.verify without its sym verify_manifest")
        );
        let with = evidence_world(&format!("{head}fn e() {{ verify_manifest(); }}\n"));
        assert!(of(&with, c::HN_H011).is_empty(), "{with:#?}");
        let dynamic = evidence_world(
            "// fairlead: def un.eng layer=core.engine container=ctr.a\n// fairlead: use un.verify dyn=1\nfn e() {}\n",
        );
        assert!(
            of(&dynamic, c::HN_H011).is_empty(),
            "dyn=1 needs no evidence (R-HIER-025)"
        );
    }

    #[test]
    fn r_hier_031__sym_without_use_is_undeclared() {
        let ds = evidence_world("fn e() { verify_manifest(); }\n// verify_manifest in a comment\n");
        let d = of(&ds, c::HN_H012);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert_eq!(d[0].location.path.as_str(), "src/caller.rs");
        let commented = evidence_world("// only verify_manifest in a comment\nfn e() {}\n");
        assert!(
            of(&commented, c::HN_H012).is_empty(),
            "comments are excluded"
        );
    }

    #[test]
    fn r_hier_033__evidence_never_creates_rank_verdict() {
        let ds = World::new()
            .root(L2)
            .file(
                "src/svc.rs",
                "// fairlead: def ctr.a
fn d() {}
// fairlead: def un.svc layer=core.service container=ctr.a
fn serve_requests() {}
",
            )
            .file(
                "src/eng.rs",
                "// fairlead: def un.eng layer=core.engine container=ctr.a
use crate::svc;
fn e() {}
",
            )
            .build()
            .run_check(&HierCheck);
        let d = of(&ds, c::HN_H012);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(
            d[0].message.contains("imports src/svc.rs, bound to un.svc")
                && d[0].message.contains("evidence=import")
        );
        assert_eq!(d[0].location.line, 2);
        assert!(
            of(&ds, c::HN_H007).is_empty(),
            "an import is never an upward edge on its own"
        );
    }

    #[test]
    fn r_hier_034__reports_state_lower_bound() {
        let code = [
            unit("un.a", "core.engine", "ctr.a", "", &["un.b"]),
            unit("un.b", "core.service", "ctr.a", "", &["un.a"]),
        ]
        .concat();
        let ds = units("allow", &code);
        for code in [c::HN_H007, c::HN_H008] {
            let d = of(&ds, code);
            assert!(
                !d.is_empty() && d.iter().all(|x| x.hint.contains("lower bound")),
                "{ds:#?}"
            );
        }
    }

    #[test]
    fn r_hier_073__a_mature_unit_depends_on_a_less_mature_one() {
        let vocab = "[env]
order = [\"dev\", \"prod\"]

[maturity]
alpha = 0
beta = 1
stable = 2
";
        let ds = World::new()
            .root(&format!("{L2}{vocab}"))
            .file(
                "src/a.rs",
                "// fairlead: def ctr.a
fn d() {}
// fairlead: def un.lib layer=core.primitive container=ctr.a mat=alpha in=dev
fn lib() {}
",
            )
            .file(
                "src/b.rs",
                "// fairlead: def un.app layer=core.engine container=ctr.a mat=stable
// fairlead: use un.lib
fn app() {
    lib();
}
",
            )
            .build()
            .run_check(&HierCheck);
        let d = of(&ds, c::HN_H017);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(
            d[0].message
                .contains("un.app is stable and un.lib is alpha")
                && d[0]
                    .message
                    .contains("reaches prod where un.lib does not exist"),
            "{}",
            d[0].message
        );
    }

    #[test]
    fn r_hier_071__promote_split_and_demote_are_suggested() {
        let mut root = L2.replace(
            "level = 2
",
            "level = 2
promote = { fan-in-containers = 2 }
",
        );
        root.push_str(
            "[kinds.cx]
shape = \"code\"
",
        );
        let ds = World::new()
            .root(&root)
            .file(
                "src/ctr.rs",
                "// fairlead: def cx.top
// fairlead: def cx.leaf parent=cx.top
// fairlead: def cx.b parent=cx.top
// fairlead: def cx.c parent=cx.top
fn d() {}
",
            )
            .file(
                "src/a.rs",
                &[
                    unit("un.shared", "core.primitive", "cx.leaf", "", &[]),
                    unit("un.b", "core.engine", "cx.b", "", &["un.shared"]),
                    unit("un.c", "core.engine", "cx.c", "", &["un.shared"]),
                    unit("un.hub", "core.service", "cx.top", "", &["un.b", "un.c"]),
                    unit("un.lofty", "core.service", "cx.top", "", &[]),
                    unit("un.api", "core.service", "cx.top", " vis=public", &[]),
                ]
                .concat(),
            )
            .build()
            .run_check(&HierCheck);
        let p = of(&ds, c::HN_H013);
        assert_eq!(p.len(), 1, "{ds:#?}");
        assert!(
            p[0].message.contains("promote un.shared to cx.top"),
            "{}",
            p[0].message
        );
        assert!(
            of(&ds, c::HN_H015)
                .iter()
                .any(|d| d.message.contains("split un.hub"))
        );
        let d = of(&ds, c::HN_H014);
        assert!(
            d.iter()
                .any(|x| x.message.contains("demote un.lofty to core.primitive")),
            "{d:#?}"
        );
        assert!(
            !d.iter().any(|x| x.message.contains("un.api")),
            "vis=public is never demoted (R-HIER-072)"
        );
        assert!(
            ds.iter()
                .filter(|x| [c::HN_H013, c::HN_H014, c::HN_H015].contains(&x.code))
                .all(|x| x.severity == fairlead_syntax::diag::Severity::Info)
        );
    }

    #[test]
    fn r_hier_004__a_child_aliases_and_narrows_layers() {
        let ds = World::new()
            .root(L2)
            .scope(
                "web",
                "[fairlead]
scope = \"web\"

[scope]
accepts = [\"core.primitive\", \"core.engine\"]
layer-alias = { widget = \"core.engine\" }
",
            )
            .file(
                "web/src/a.rs",
                "// fairlead: def ctr.w
fn d() {}
// fairlead: def un.ok layer=widget container=ctr.w
fn ok() {}
// fairlead: def un.wide layer=core.service container=ctr.w
fn wide() {}
",
            )
            .build()
            .run_check(&HierCheck);
        let d = of(&ds, c::HN_H007);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(
            d[0].message
                .contains("un.wide names layer core.service; scope web accepts only")
        );
    }

    #[test]
    fn d_65__a_container_kind_is_declared_with_role() {
        let w = World::new()
            .root(&format!(
                "{L2}
[kinds.grp]
role = \"container\"
"
            ))
            .file(
                "src/a.rs",
                "// fairlead: def grp.lonely path=\"src/**\"
fn d() {}
",
            )
            .file(
                "fairlead/docs/g.md",
                "+++
directives = ['def grp.unnamed']
+++
# G
",
            )
            .build();
        let m = Model::build(&w.registry);
        let names: Vec<String> = m.containers.keys().map(bare_id).collect();
        assert!(names.contains(&"grp.unnamed".to_string()), "{names:?}");
    }

    fn l3(extra_files: &[(&str, &str)]) -> Vec<Diagnostic> {
        let root = format!(
            "{}\n[kinds.ent]\nshape = \"code\"\n",
            L2.replace("level = 2\n", "level = 3\n")
        );
        let mut w = World::new().root(&root).file(
            "src/a.rs",
            "// fairlead: def ctr.a\nfn d() {}\n// fairlead: def un.main layer=core.service container=ctr.a\n// fairlead: use un.eng\nfn main_() {}\n// fairlead: def un.eng layer=core.engine container=ctr.a\nfn eng() {}\n// fairlead: def un.lonely layer=core.primitive container=ctr.a\nfn lonely() {}\n",
        );
        for (p, t) in extra_files {
            w = w.file(p, t);
        }
        w.build().run_check(&HierCheck)
    }
    const ENTRY: (&str, &str) = (
        "src/main.rs",
        "// fairlead: def ent.run etype=cli unit=un.main reaches=un.eng\nfn run() {}\n",
    );

    #[test]
    fn r_hier_041__an_unreached_bound_unit_is_stale() {
        let ds = l3(&[ENTRY]);
        let d = of(&ds, c::HN_H018);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(
            d[0].message.contains("un.lonely is reached by no entry")
                && d[0].hint.contains("lower bound")
        );
    }

    #[test]
    fn r_hier_040__etype_must_be_declared() {
        let ds = l3(&[
            ENTRY,
            (
                "src/b.rs",
                "// fairlead: def ent.odd etype=wizard unit=un.main\nfn odd() {}\n",
            ),
        ]);
        let d = of(&ds, c::HN_E004);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(d[0].message.contains("\"wizard\""));
        let root = format!(
            "{}\n[kinds.ent]\nshape = \"code\"\n",
            L2.replace("level = 2\n", "level = 3\netypes = [\"wizard\"]\n")
        );
        let ok = World::new()
            .root(&root)
            .file("src/a.rs", "// fairlead: def ctr.a\nfn d() {}\n// fairlead: def un.main layer=core.service container=ctr.a\nfn m() {}\n")
            .file("src/b.rs", "// fairlead: def ent.odd etype=wizard unit=un.main\nfn odd() {}\n")
            .build()
            .run_check(&HierCheck);
        assert!(
            of(&ok, c::HN_E004).is_empty(),
            "[hier] etypes adds words: {ok:#?}"
        );
    }

    #[test]
    fn r_hier_044__reaches_checked_against_chain() {
        let ds = l3(&[
            ENTRY,
            (
                "src/b.rs",
                "// fairlead: def ent.miss etype=cli unit=un.main reaches=un.lonely\n// fairlead: def ent.local etype=cli unit=un.eng reaches=none\nfn b() {}\n",
            ),
        ]);
        let d: Vec<_> = of(&ds, c::HN_H018)
            .into_iter()
            .filter(|x| x.message.contains("declares reaches"))
            .collect();
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(d[0].message.contains("ent.miss declares reaches=un.lonely"));
    }

    #[test]
    fn r_hier_040__tests_are_auto_entries() {
        let ds = l3(&[
            ENTRY,
            ("tests/lonely.rs", "// fairlead: use un.lonely\nfn t() {}\n"),
        ]);
        assert!(of(&ds, c::HN_H018).is_empty(), "a test reaches it: {ds:#?}");
    }

    #[test]
    fn r_hier_041__dev_only_unit_not_stale_in_prod() {
        let ds = l3(&[
            ENTRY,
            (
                "src/dev.rs",
                "// fairlead: def ent.dev etype=cli unit=un.lonely in=dev\nfn dev() {}\n",
            ),
        ]);
        assert!(
            of(&ds, c::HN_H018).is_empty(),
            "reached by a dev entry: {ds:#?}"
        );
    }

    #[test]
    fn r_hier_046__draft_adds_exempts_until_sealed() {
        let doc = "+++\nkind = \"change\"\nid = \"d-00000001\"\nstatus = \"draft\"\nimpact = \"add\"\ncovers = [\"src/**\"]\nadds = [\"un.lonely\"]\n+++\n# New\n";
        let ds = l3(&[ENTRY, ("fairlead/docs/changes/d-00000001-new.md", doc)]);
        assert!(of(&ds, c::HN_H018).is_empty(), "{ds:#?}");
        let sealed = doc.replace("status = \"draft\"", "status = \"abandoned\"");
        let ds = l3(&[ENTRY, ("fairlead/docs/changes/d-00000001-new.md", &sealed)]);
        assert_eq!(
            of(&ds, c::HN_H018).len(),
            1,
            "an abandoned doc exempts nothing"
        );
    }

    #[test]
    fn r_hier_042__zero_entries_skips_stale() {
        let ds = l3(&[]);
        let d = of(&ds, c::HN_H018);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert_eq!(d[0].severity, fairlead_syntax::diag::Severity::Info);
        assert!(d[0].message.contains("no entries declared"));
    }

    #[test]
    fn r_hier_045__dormancy_from_ledger_not_git() {
        let root = format!(
            "{}\n[kinds.ent]\nshape = \"code\"\n",
            L2.replace("level = 2\n", "level = 3\ndormant-seals = 2\n")
        );
        let base = || {
            World::new()
                .root(&root)
                .file(
                    "src/a.rs",
                    "// fairlead: def ctr.a\nfn d() {}\n// fairlead: def un.main layer=core.service container=ctr.a\nfn main_() {}\n// fairlead: def un.lonely layer=core.primitive container=ctr.a\nfn lonely() {}\n",
                )
                .file("src/main.rs", "// fairlead: def ent.run etype=cli unit=un.main\nfn run() {}\n")
        };
        let w0 = base().build();
        let m = Model::build(&w0.registry);
        let files: Vec<&crate::path::RepoPath> = w0.corpus.records.keys().collect();
        let b = m.bindings(&w0.registry, &files);
        let digest = |name: &str| {
            let id = m.units.keys().find(|k| bare_id(k) == name).unwrap();
            crate::hier::unit_digest(&b[id], &w0.corpus)
        };
        let (lonely, main) = (digest("un.lonely"), digest("un.main"));
        let ledger = format!(
            "# scope=root year=2026 prev=- edition=1\nun.lonely kind=ud ud={lonely} d=aaaaaaaa\nun.main kind=ud ud=changedbefore d=aaaaaaab\n# ckpt v=0.1.0 n=2 d=bbbbbbbb\nun.lonely kind=ud ud={lonely} d=aaaaaaaa\nun.main kind=ud ud={main} d=aaaaaaac\n# ckpt v=0.1.1 n=2 d=cccccccc\n"
        );
        let ds = base()
            .facts(move |f| {
                f.ledgers.insert(
                    crate::path::RepoPath::new("fairlead/ledger/hist/2026.hl").unwrap(),
                    ledger.into(),
                );
            })
            .build()
            .run_check(&HierCheck);
        let dormant: Vec<String> = of(&ds, c::HN_H019)
            .iter()
            .map(|d| d.message.clone())
            .collect();
        assert_eq!(
            dormant,
            ["un.lonely is unchanged for 2 seals (since 0.1.0)"],
            "un.main changed at 0.1.1: {ds:#?}"
        );
        let forgotten = of(&ds, c::HN_H020);
        assert_eq!(forgotten.len(), 1, "{ds:#?}");
        assert!(
            forgotten[0].message.contains("un.lonely is forgotten")
                && forgotten[0].hint.contains("graveyard")
        );
        assert!(
            of(&ds, c::HN_H018).is_empty(),
            "forgotten replaces stale for the same unit"
        );
    }

    #[test]
    fn r_hier_071__merge_and_one_consumer_demote_read_the_seal_records() {
        let root = L2.replace(
            "level = 2\n",
            "level = 2\npromote = { single-consumer-seals = 2 }\n",
        );
        let ledger = "# scope=root year=2026 prev=- edition=1\nun.svc kind=ud ud=x users=1 ctr=ctr.solo d=aaaaaaaa\n# ckpt v=0.1.0 n=1 d=bbbbbbbb\nun.svc kind=ud ud=x users=1 ctr=ctr.solo d=aaaaaaaa\n# ckpt v=0.1.1 n=1 d=cccccccc\n";
        let ds = World::new()
            .root(&root)
            .file(
                "src/a.rs",
                "// fairlead: def ctr.a\n// fairlead: def ctr.solo\nfn d() {}\n// fairlead: def un.main layer=core.service container=ctr.a\n// fairlead: use un.svc\nfn m() {}\n// fairlead: def un.svc layer=core.engine container=ctr.solo\nfn s() {}\n",
            )
            .facts(move |f| {
                f.ledgers.insert(
                    crate::path::RepoPath::new("fairlead/ledger/hist/2026.hl").unwrap(),
                    ledger.into(),
                );
            })
            .build()
            .run_check(&HierCheck);
        let merge = of(&ds, c::HN_H016);
        assert_eq!(merge.len(), 1, "{ds:#?}");
        assert!(
            merge[0]
                .message
                .contains("suggest merge ctr.solo: at most one unit for 2 seals")
        );
        let demote = of(&ds, c::HN_H014);
        assert!(
            demote.iter().any(|d| d
                .message
                .contains("suggest demote un.svc to ctr.a: one consumer for 2 seals")),
            "{demote:#?}"
        );
    }

    const BRG: &str = "// fairlead: def brg.run proto=tauri-command dir=\"ts->rs\" wire=sync_run ver=1 params=req ts=\"app/api.ts#syncRun\" rs=\"core/cmds.rs#sync_run\"\nfn decl() {}\n";
    const TS_END: &str = "// fairlead: def brg.run side=ts ver=1\nexport function syncRun() { return invoke('sync_run'); }\n";
    const RS_END: &str = "// fairlead: def brg.run side=rs ver=1\npub fn sync_run() {}\n";

    fn bridge(decl: &str, ts: &str, rs: &str, extra: &[(&str, &str)]) -> Vec<Diagnostic> {
        let root = format!(
            "{}\n[kinds.brg]\nshape = \"code\"\n",
            L2.replace("level = 2\n", "level = 3\n")
        );
        let mut w = World::new()
            .root(&root)
            .file("src/decl.rs", decl)
            .file("app/api.ts", ts)
            .file("core/cmds.rs", rs);
        for (p, t) in extra {
            w = w.file(p, t);
        }
        w.build().run_check(&HierCheck)
    }
    fn bridge_codes(ds: &[Diagnostic]) -> Vec<&'static str> {
        ds.iter()
            .filter(|d| [c::HN_H021, c::HN_H022, c::HN_H024, c::HN_H025].contains(&d.code))
            .map(|d| d.code.0)
            .collect()
    }

    #[test]
    fn r_hier_060__bridge_requires_both_ends_and_wire() {
        let ok = bridge(BRG, TS_END, RS_END, &[]);
        assert!(bridge_codes(&ok).is_empty(), "{ok:#?}");
        let ds = bridge(&BRG.replace(" params=req", ""), TS_END, RS_END, &[]);
        let d = of(&ds, c::HN_H021);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(d[0].message.contains("bridge brg.run declares no params"));
    }

    #[test]
    fn r_hier_062__an_end_needs_its_marker_and_symbol() {
        let ds = bridge(BRG, TS_END, "pub fn sync_run() {}\n", &[]);
        assert!(
            of(&ds, c::HN_H021)
                .iter()
                .any(|d| d.message.contains("has no `def brg.run side=rs` marker")),
            "{ds:#?}"
        );
        let ds = bridge(BRG, &TS_END.replace("syncRun", "runSync"), RS_END, &[]);
        assert!(
            of(&ds, c::HN_H021)
                .iter()
                .any(|d| d.message.contains("the ts end never mentions syncRun")),
            "{ds:#?}"
        );
    }

    #[test]
    fn r_hier_063__the_wire_appears_on_both_sides() {
        let ds = bridge(BRG, &TS_END.replace("'sync_run'", "'sync_go'"), RS_END, &[]);
        let d = of(&ds, c::HN_H025);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(d[0].message.contains("the ts end has no wire sync_run"));
        let ds = bridge(BRG, TS_END, &RS_END.replace("sync_run", "do_sync"), &[]);
        assert!(bridge_codes(&ds).contains(&c::HN_H025.0), "{ds:#?}");
    }

    #[test]
    fn r_chk_043__a_dynamic_invoke_is_unverifiable() {
        let dynamic = "// fairlead: def brg.run side=ts ver=1\nexport function syncRun(cmd: string) { return invoke(cmd); }\n";
        let ds = bridge(BRG, dynamic, RS_END, &[]);
        let d = of(&ds, c::HN_H024);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert_eq!(d[0].severity, fairlead_syntax::diag::Severity::Warn);
        assert!(
            of(&ds, c::HN_H025).is_empty(),
            "never guessed, never a missing wire"
        );
    }

    #[test]
    fn r_hier_063__ver_and_pin_must_match_the_declaration() {
        let ds = bridge(
            BRG,
            &TS_END.replace("ver=1", "ver=2"),
            RS_END,
            &[("src/user.rs", "// fairlead: use brg.run pin=0\nfn u() {}\n")],
        );
        let d = of(&ds, c::HN_H022);
        assert_eq!(d.len(), 2, "{ds:#?}");
        assert!(
            d.iter()
                .any(|x| x.message.contains("ts end marker says ver=2"))
        );
        assert!(d.iter().any(|x| {
            x.message
                .contains("use brg.run pin=0, but the bridge is ver=1")
        }));
    }

    #[test]
    fn r_hier_064__contract_change_needs_doc_on_both_sides() {
        let decl = BRG.replace(" ts=", " contract=\"docs/c.md\" ts=");
        let binding = "// fairlead: def foreign.specta tool=specta gen=brg.run +end\nexport type X = 1;\n// fairlead: end\n";
        let run = |ledger: String, extra: &[(&str, &str)]| {
            let root = format!(
                "{}\n[kinds.brg]\nshape = \"code\"\n",
                L2.replace("level = 2\n", "level = 3\n")
            );
            let mut w = World::new()
                .root(&root)
                .file("src/decl.rs", &decl)
                .file("app/api.ts", TS_END)
                .file("core/cmds.rs", RS_END)
                .file("docs/c.md", "# Contract v2\n")
                .file("app/gen.ts", binding);
            for (p, t) in extra {
                w = w.file(p, t);
            }
            w.facts(move |f| {
                f.ledgers.insert(
                    crate::path::RepoPath::new("fairlead/ledger/hist/2026.hl").unwrap(),
                    ledger.into(),
                );
            })
            .build()
            .run_check(&HierCheck)
        };
        // The generated-files hash as it is now, recorded at the last seal: not regenerated.
        let w0 = World::new()
            .root(&format!(
                "{}\n[kinds.brg]\nshape = \"code\"\n",
                L2.replace("level = 2\n", "level = 3\n")
            ))
            .file("src/decl.rs", &decl)
            .file("app/gen.ts", binding)
            .file("docs/c.md", "# Contract v2\n")
            .build();
        let m = Model::build(&w0.registry);
        let b = m.bridges(&w0.registry).into_iter().next().unwrap();
        let (_, _, gh) = crate::hier::contract_hashes(&w0.registry, &w0.corpus, &b).unwrap();
        let ledger = format!(
            "# scope=root year=2026 prev=- edition=1\nbrg.run kind=contract ch=oldcontract1 gh={gh} d=aaaaaaaa\n# ckpt v=0.1.0 n=1 d=bbbbbbbb\n"
        );
        let ds = run(ledger.clone(), &[]);
        let d = of(&ds, c::HN_D004);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(
            d[0].message
                .contains("contract of bridge brg.run changed since 0.1.0")
        );
        let doc = "+++\nkind = \"change\"\nid = \"d-00000001\"\nstatus = \"draft\"\nimpact = \"fix\"\ncovers = [\"app/**\", \"core/**\"]\ntouches = [\"brg.run\"]\n+++\n# Contract v2\n";
        let ds = run(ledger, &[("fairlead/docs/changes/d-00000001-c.md", doc)]);
        assert!(
            of(&ds, c::HN_D004).is_empty(),
            "a draft doc touches it: {ds:#?}"
        );
    }

    #[test]
    fn r_hier_066__generated_binding_stale_on_contract_change() {
        let decl = BRG.replace(" ts=", " contract=\"docs/c.md\" ts=");
        let binding = "// fairlead: def foreign.specta tool=specta gen=brg.run +end\nexport type X = 1;\n// fairlead: end\n";
        let root = format!(
            "{}\n[kinds.brg]\nshape = \"code\"\n",
            L2.replace("level = 2\n", "level = 3\n")
        );
        let base = || {
            World::new()
                .root(&root)
                .file("src/decl.rs", &decl)
                .file("app/api.ts", TS_END)
                .file("core/cmds.rs", RS_END)
                .file("docs/c.md", "# Contract v2\n")
                .file("app/gen.ts", binding)
        };
        let w0 = base().build();
        let m = Model::build(&w0.registry);
        let b = m.bridges(&w0.registry).into_iter().next().unwrap();
        let (_, _, gh) = crate::hier::contract_hashes(&w0.registry, &w0.corpus, &b).unwrap();
        let ledger = format!(
            "# scope=root year=2026 prev=- edition=1\nbrg.run kind=contract ch=oldcontract1 gh={gh} d=aaaaaaaa\n# ckpt v=0.1.0 n=1 d=bbbbbbbb\n"
        );
        let ds = base()
            .facts(move |f| {
                f.ledgers.insert(
                    crate::path::RepoPath::new("fairlead/ledger/hist/2026.hl").unwrap(),
                    ledger.into(),
                );
            })
            .build()
            .run_check(&HierCheck);
        let d: Vec<_> = of(&ds, c::HN_H022)
            .into_iter()
            .filter(|x| x.message.contains("not regenerated"))
            .collect();
        assert_eq!(d.len(), 1, "{ds:#?}");
    }

    #[test]
    fn r_hier_068__an_external_input_is_never_an_orphan() {
        let root = format!("{}governed = [\"dist/**\", \"src/**\"]\n", L2);
        let ds = World::new()
            .root(&root)
            .file(
                "src/ext.rs",
                "// fairlead: def ctr.a path=\"src/**\"\nfn d() {}\n",
            )
            .file(
                "fairlead/docs/ext.md",
                "+++\ndirectives = ['def ext.web role=input path=\"dist/**\"']\n+++\n# Ext\n",
            )
            .file("dist/index.html", "<html></html>\n")
            .build()
            .run_check(&HierCheck);
        assert!(of(&ds, c::HN_H003).is_empty(), "{ds:#?}");
    }

    #[test]
    fn r_hier_065__reaches_checked_against_flow() {
        let root = format!(
            "{}\n[kinds.brg]\nshape = \"code\"\n",
            L2.replace(
                "to = [\"core.service\"]",
                "to = [\"core.service\"]\nvia = \"bridge\""
            )
        );
        let run = |bridge_tail: &str| {
            World::new()
                .root(&root)
                .file(
                    "src/a.rs",
                    &format!(
                        "// fairlead: def ctr.a\nfn d() {{}}\n// fairlead: def un.svc layer=core.service container=ctr.a\nfn svc() {{}}\n// fairlead: def un.eng layer=core.engine container=ctr.a\nfn eng() {{}}\n// fairlead: def brg.go proto=tauri-command dir=\"ts->rs\" wire=go ver=1 params=req ts=\"a.ts#go\" rs=\"b.rs#go\"{bridge_tail}\nfn b() {{}}\n"
                    ),
                )
                .file(
                    "src/page.rs",
                    "// fairlead: def un.page layer=ui.page container=ctr.a\n// fairlead: use brg.go\nfn page() {}\n",
                )
                .build()
                .run_check(&HierCheck)
        };
        let to_engine = run(" reaches=un.eng");
        let d = of(&to_engine, c::HN_H007);
        assert_eq!(d.len(), 1, "{to_engine:#?}");
        assert!(
            d[0].message
                .contains("un.page (ui.page) uses un.eng (core.engine): no [[flow]]")
        );
        assert!(
            of(&run(" reaches=un.svc"), c::HN_H007).is_empty(),
            "the flow sanctions ui to core.service"
        );
        assert!(
            of(&run(" implements=un.eng"), c::HN_H007).is_empty(),
            "inverted bridges are exempt"
        );
    }

    fn doc_with(extra: &str) -> String {
        format!(
            "+++\nkind = \"change\"\nid = \"d-00000001\"\nstatus = \"draft\"\nimpact = \"add\"\ncovers = [\"src/**\"]\n{extra}\n+++\n# Doc\n"
        )
    }

    #[test]
    fn r_hier_090__a_doc_listing_a_missing_id_is_dangling() {
        let w = |doc: String| {
            World::new()
                .root(L2)
                .file("src/a.rs", "// fairlead: def ctr.a\nfn d() {}\n// fairlead: def un.x layer=core.engine container=ctr.a\nfn x() {}\n")
                .file("fairlead/docs/changes/d-00000001-doc.md", &doc)
                .facts(|f| {
                    f.tombstones.insert(
                        crate::path::RepoPath::new("fairlead/tombstones/2026.toml").unwrap(),
                        "[[tomb]]\nid = \"un.gone\"\nremoved = \"0.1.0\"\nwhy = \"merged\"\n".into(),
                    );
                })
                .build()
                .run_check(&HierCheck)
        };
        let ds = w(doc_with("touches = [\"un.x\", \"un.nope\", \"un.gone\"]"));
        let d = of(&ds, c::HN_D005);
        assert_eq!(d.len(), 1, "a removed ID is not dangling: {ds:#?}");
        assert!(d[0].message.contains("lists un.nope, which does not exist"));
        assert_eq!(
            d[0].severity,
            fairlead_syntax::diag::Severity::Warn,
            "docs-first ramp, warn by default"
        );
    }

    #[test]
    fn r_hier_090__a_new_unit_needs_adds_or_touches() {
        let root = format!(
            "{}\n[kinds.ent]\nshape = \"code\"\n",
            L2.replace("level = 2\n", "level = 3\n")
        );
        let w = |ledger: Option<&str>, doc: Option<String>| {
            let mut b = World::new()
                .root(&root)
                .file("src/a.rs", "// fairlead: def ctr.a\nfn d() {}\n// fairlead: def un.old layer=core.engine container=ctr.a\nfn o() {}\n// fairlead: def un.new layer=core.engine container=ctr.a\nfn n() {}\n");
            if let Some(d) = doc {
                b = b.file("fairlead/docs/changes/d-00000001-doc.md", &d);
            }
            let ledger = ledger.map(str::to_owned);
            b.facts(move |f| {
                if let Some(l) = ledger {
                    f.ledgers.insert(
                        crate::path::RepoPath::new("fairlead/ledger/hist/2026.hl").unwrap(),
                        l.into(),
                    );
                }
            })
            .build()
            .run_check(&HierCheck)
        };
        let sealed = "# scope=root year=2026 prev=- edition=1\nun.old kind=ud ud=x users=0 ctr=ctr.a d=aaaaaaaa\n# ckpt v=0.1.0 n=1 d=bbbbbbbb\n";
        let ds = w(Some(sealed), None);
        let d = of(&ds, c::HN_D004);
        assert_eq!(d.len(), 1, "{ds:#?}");
        assert!(d[0].message.contains("un.new is new"));
        assert!(
            of(
                &w(Some(sealed), Some(doc_with("adds = [\"un.new\"]"))),
                c::HN_D004
            )
            .is_empty()
        );
        assert!(
            of(&w(None, None), c::HN_D004).is_empty(),
            "nothing sealed at level 3 yet"
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
