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
