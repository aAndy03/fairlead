//! The hierarchy model (30-hierarchy): units with a layer and a container, nestable containers,
//! derived from the registry's declarations. Pure: built from a `Registry`, ordered by ID.
use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::ids::Id;

use crate::config::HierConfig;
use crate::registry::{FieldValue, Item, ItemId, Lookup, Registry};

/// A unit: an item of a kind that lists `layers`, or any item with `layer=` (R-HIER-003).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Unit {
    pub id: ItemId,
    pub layer: Option<String>,
    /// `container=` as written; more than one is `hier.single-container` (R-HIER-010, R-HIER-012).
    pub container_raw: Vec<String>,
    /// The resolved single container.
    pub container: Option<ItemId>,
    /// `path=`: the unit binds by glob (R-HIER-014).
    pub path: Option<String>,
    /// The unit has a code `def` extent (a site).
    pub code_bound: bool,
    /// `implements=`: ports this unit implements; edges to them are exempt from rank (R-HIER-023).
    pub implements: Vec<ItemId>,
}

/// A container: named by a unit's `container=` or a container's `parent=`, or a non-unit item that
/// declares `parent=` or `path=` (R-HIER-013).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Container {
    pub id: ItemId,
    pub parent_raw: Option<String>,
    pub parent: Option<ItemId>,
    /// Scope-relative `path=` claims.
    pub paths: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Model {
    pub units: BTreeMap<ItemId, Unit>,
    pub containers: BTreeMap<ItemId, Container>,
}

fn list(it: &Item, k: &str) -> Vec<String> {
    match it.fields.get(k) {
        Some(FieldValue::List(v)) => v.clone(),
        Some(FieldValue::Scalar(s)) => s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

fn scalar(it: &Item, k: &str) -> Option<String> {
    match it.fields.get(k)? {
        FieldValue::Scalar(s) => Some(s.clone()),
        FieldValue::List(v) => v.first().cloned(),
    }
}

/// The root's hierarchy vocabulary, if the root control file parsed.
pub fn config(reg: &Registry) -> Option<&HierConfig> {
    Some(&reg.scope(reg.root_scope())?.control.as_ref()?.hier)
}

fn resolve(reg: &Registry, from: &ItemId, raw: &str) -> Option<ItemId> {
    let id = Id::parse(raw).ok()?;
    match reg.lookup(&from.scope, &id) {
        Lookup::Found(i) => Some(i),
        _ => None,
    }
}

impl Model {
    pub fn build(reg: &Registry) -> Model {
        let kind_layers = |it: &Item| {
            reg.chain(&it.id.scope)
                .into_iter()
                .rev()
                .find_map(|s| {
                    reg.scope(s)?
                        .control
                        .as_ref()?
                        .kinds
                        .get(it.id.kind.as_str())
                })
                .is_some_and(|k| !k.layers.is_empty())
        };
        let mut m = Model::default();
        let mut named: BTreeSet<ItemId> = BTreeSet::new();
        for it in reg.items() {
            if !(kind_layers(it) || it.fields.contains_key("layer")) {
                continue;
            }
            let container_raw = list(it, "container");
            let container = match container_raw.as_slice() {
                [one] => resolve(reg, &it.id, one),
                _ => None,
            };
            named.extend(container.clone());
            m.units.insert(
                it.id.clone(),
                Unit {
                    id: it.id.clone(),
                    layer: scalar(it, "layer"),
                    container_raw,
                    container,
                    path: scalar(it, "path"),
                    code_bound: !reg.sites_of(&it.id).is_empty(),
                    implements: list(it, "implements")
                        .iter()
                        .filter_map(|x| resolve(reg, &it.id, x))
                        .collect(),
                },
            );
        }
        for it in reg.items() {
            if m.units.contains_key(&it.id) || matches!(it.id.kind.as_str(), "rule" | "file") {
                continue;
            }
            let parent_raw = scalar(it, "parent");
            if parent_raw.is_none() && !it.fields.contains_key("path") && !named.contains(&it.id) {
                continue;
            }
            let parent = parent_raw.as_deref().and_then(|p| resolve(reg, &it.id, p));
            m.containers.insert(
                it.id.clone(),
                Container {
                    id: it.id.clone(),
                    parent_raw,
                    parent,
                    paths: list(it, "path"),
                },
            );
        }
        // A container named only as another's parent is a container too.
        let parents: Vec<ItemId> = m
            .containers
            .values()
            .filter_map(|c| c.parent.clone())
            .filter(|p| !m.containers.contains_key(p) && !m.units.contains_key(p))
            .collect();
        for p in parents {
            if let Some(it) = reg.item(&p) {
                let parent_raw = scalar(it, "parent");
                let parent = parent_raw.as_deref().and_then(|x| resolve(reg, &p, x));
                m.containers.insert(
                    p.clone(),
                    Container {
                        id: p,
                        parent_raw,
                        parent,
                        paths: list(it, "path"),
                    },
                );
            }
        }
        m
    }

    /// The parent chain of a container, nearest first; `Err` with the cycle when it loops back.
    pub fn ancestors(&self, c: &ItemId) -> Result<Vec<ItemId>, Vec<ItemId>> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::from([c.clone()]);
        let mut cur = self.containers.get(c).and_then(|x| x.parent.clone());
        while let Some(p) = cur {
            if !seen.insert(p.clone()) {
                let mut cycle = vec![c.clone()];
                cycle.extend(out);
                cycle.push(p);
                return Err(cycle);
            }
            cur = self.containers.get(&p).and_then(|x| x.parent.clone());
            out.push(p);
        }
        Ok(out)
    }
}

/// A `use` edge between two units (R-HIER-020), at the `use` site.
#[derive(Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct UnitEdge {
    pub from: ItemId,
    pub to: ItemId,
    pub path: crate::path::RepoPath,
    pub line: u32,
    pub col: u32,
}

impl Model {
    /// The unit `use` edges: a `use` inside a unit's code `def` extent (the innermost) comes from
    /// that unit; otherwise from the one unit whose `path=` claim covers the file. Edges whose
    /// target is not a unit are not hierarchy edges. Declared edges only (R-HIER-034).
    pub fn edges(&self, reg: &Registry, files: &[&crate::path::RepoPath]) -> Vec<UnitEdge> {
        use fairlead_syntax::directive::Extent;
        let mut extents: BTreeMap<&crate::path::RepoPath, Vec<(u32, u32, &ItemId)>> =
            BTreeMap::new();
        for u in self.units.values() {
            for s in reg.sites_of(&u.id) {
                let (a, b) = match s.extent {
                    Extent::Lines { first, last } => (first, last),
                    Extent::File | Extent::Doc => (0, u32::MAX),
                };
                extents.entry(&s.path).or_default().push((a, b, &u.id));
            }
        }
        let by_path = self.bindings(reg, files);
        let mut out = Vec::new();
        for e in reg.edges() {
            let Some(to) = e.resolved.as_ref().filter(|t| self.units.contains_key(*t)) else {
                continue;
            };
            let line = e.from.span.line;
            // A `use` binds to a block as a `def` does; it belongs to the unit whose extent
            // contains its own (a `def` and a `use` on the lines above one block share it).
            let (ua, ub) = match e.from.extent {
                Extent::Lines { first, last } => (first, last),
                Extent::File | Extent::Doc => (0, u32::MAX),
            };
            let inner = extents
                .get(&e.from.path)
                .and_then(|v| {
                    v.iter()
                        .filter(|(a, b, _)| *a <= ua && ub <= *b)
                        .min_by_key(|(a, b, id)| (b - a, (*id).clone()))
                })
                .map(|(_, _, id)| (*id).clone());
            let from = inner.or_else(|| {
                let mut claimed = self
                    .units
                    .values()
                    .filter(|u| u.path.is_some())
                    .filter(|u| by_path.get(&u.id).is_some_and(|f| f.contains(&e.from.path)));
                match (claimed.next(), claimed.next()) {
                    (Some(u), None) => Some(u.id.clone()),
                    _ => None,
                }
            });
            if let Some(from) = from.filter(|f| f != to) {
                out.push(UnitEdge {
                    from,
                    to: to.clone(),
                    path: e.from.path.clone(),
                    line,
                    col: e.from.span.col,
                });
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Strongly connected components with more than one node (Tarjan), each sorted, in the order of
/// their smallest node. Deterministic: nodes and successors are visited in `Ord` order.
pub fn cycles<T: Ord + Clone>(graph: &BTreeMap<T, BTreeSet<T>>) -> Vec<Vec<T>> {
    struct St<'a, T: Ord> {
        g: &'a BTreeMap<T, BTreeSet<T>>,
        index: BTreeMap<&'a T, usize>,
        low: BTreeMap<&'a T, usize>,
        stack: Vec<&'a T>,
        on: BTreeSet<&'a T>,
        next: usize,
        out: Vec<Vec<&'a T>>,
    }
    fn visit<'a, T: Ord>(s: &mut St<'a, T>, v: &'a T) {
        s.index.insert(v, s.next);
        s.low.insert(v, s.next);
        s.next += 1;
        s.stack.push(v);
        s.on.insert(v);
        if let Some(succ) = s.g.get(v) {
            for w in succ {
                // Successors outside the graph's keys have no edges and cannot close a cycle.
                let Some((w, _)) = s.g.get_key_value(w) else {
                    continue;
                };
                if !s.index.contains_key(w) {
                    visit(s, w);
                    let lw = s.low.get(w).copied().unwrap_or(usize::MAX);
                    if lw < s.low.get(v).copied().unwrap_or(usize::MAX) {
                        s.low.insert(v, lw);
                    }
                } else if s.on.contains(w) {
                    let iw = s.index.get(w).copied().unwrap_or(usize::MAX);
                    if iw < s.low.get(v).copied().unwrap_or(usize::MAX) {
                        s.low.insert(v, iw);
                    }
                }
            }
        }
        if s.low.get(v) == s.index.get(v) {
            let mut comp = Vec::new();
            while let Some(w) = s.stack.pop() {
                s.on.remove(w);
                comp.push(w);
                if std::ptr::eq(w, v) {
                    break;
                }
            }
            s.out.push(comp);
        }
    }
    let mut s = St {
        g: graph,
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        stack: Vec::new(),
        on: BTreeSet::new(),
        next: 0,
        out: Vec::new(),
    };
    for v in graph.keys() {
        if !s.index.contains_key(v) {
            visit(&mut s, v);
        }
    }
    let mut out: Vec<Vec<T>> = s
        .out
        .into_iter()
        .filter(|c| c.len() > 1)
        .map(|c| {
            let mut c: Vec<T> = c.into_iter().cloned().collect();
            c.sort();
            c
        })
        .collect();
    out.sort();
    out
}

/// A glob matcher with `*` stopping at `/`, as the registry matches globs.
pub fn matcher(glob: &str) -> Option<globset::GlobMatcher> {
    globset::GlobBuilder::new(glob.trim_start_matches("./"))
        .literal_separator(true)
        .build()
        .ok()
        .map(|g| g.compile_matcher())
}

impl Model {
    /// The files each unit and container binds (R-HIER-014): the files its scope-relative `path=`
    /// claims match, and for a unit the files of its code `def` sites.
    pub fn bindings(
        &self,
        reg: &Registry,
        files: &[&crate::path::RepoPath],
    ) -> BTreeMap<ItemId, BTreeSet<crate::path::RepoPath>> {
        let claim = |id: &ItemId, globs: &[String]| -> BTreeSet<crate::path::RepoPath> {
            let dir = reg
                .scope(&id.scope)
                .map(|s| s.dir.as_str().trim_matches('/').to_owned())
                .unwrap_or_default();
            let ms: Vec<globset::GlobMatcher> = globs.iter().filter_map(|g| matcher(g)).collect();
            if ms.is_empty() {
                return BTreeSet::new();
            }
            files
                .iter()
                .filter(|f| {
                    let rel = if dir.is_empty() {
                        Some(f.as_str())
                    } else {
                        f.as_str()
                            .strip_prefix(dir.as_str())
                            .and_then(|r| r.strip_prefix('/'))
                    };
                    rel.is_some_and(|r| ms.iter().any(|m| m.is_match(r)))
                })
                .map(|f| (*f).clone())
                .collect()
        };
        let mut out: BTreeMap<ItemId, BTreeSet<crate::path::RepoPath>> = BTreeMap::new();
        for c in self.containers.values() {
            out.insert(c.id.clone(), claim(&c.id, &c.paths));
        }
        for u in self.units.values() {
            let mut set = claim(&u.id, u.path.as_slice());
            set.extend(reg.sites_of(&u.id).iter().map(|s| s.path.clone()));
            out.insert(u.id.clone(), set);
        }
        out
    }
}

/// R-HIER-081: `(bound, governed)` over the whole tree, when the root sets `[hier] governed`.
/// `unbound-ok` files are not governed. Existing orphans are absorbed by the baseline (D-28), so a
/// newly unbound file is a new `HN-H003` that needs a waiver: the ratchet.
pub fn coverage(reg: &Registry, corpus: &crate::extract::Corpus) -> Option<(u32, u32)> {
    let h = config(reg)?;
    h.settings.level?;
    let governed: Vec<_> = h
        .settings
        .governed
        .iter()
        .filter_map(|g| matcher(g))
        .collect();
    if governed.is_empty() {
        return None;
    }
    let exempt: Vec<_> = h
        .settings
        .unbound_ok
        .iter()
        .filter_map(|g| matcher(g))
        .collect();
    let files: Vec<&crate::path::RepoPath> = corpus.records.keys().collect();
    let bound: BTreeSet<crate::path::RepoPath> = Model::build(reg)
        .bindings(reg, &files)
        .into_values()
        .flatten()
        .collect();
    let gov: Vec<&&crate::path::RepoPath> = files
        .iter()
        .filter(|f| {
            governed.iter().any(|g| g.is_match(f.as_str()))
                && !exempt.iter().any(|g| g.is_match(f.as_str()))
        })
        .collect();
    let n = |x: usize| u32::try_from(x).unwrap_or(u32::MAX);
    Some((
        n(gov.iter().filter(|f| bound.contains(**f)).count()),
        n(gov.len()),
    ))
}

/// The literal directory prefix of a glob (the segments before the first wildcard).
pub fn literal_prefix(glob: &str) -> String {
    glob.trim_start_matches("./")
        .split('/')
        .take_while(|s| !s.contains(['*', '?', '[', '{']))
        .collect::<Vec<_>>()
        .join("/")
}

/// Two repo-relative prefixes overlap when one is the other or contains it as a directory.
pub fn prefixes_overlap(a: &str, b: &str) -> bool {
    let within = |x: &str, y: &str| y.is_empty() || x == y || x.starts_with(&format!("{y}/"));
    within(a, b) || within(b, a)
}
