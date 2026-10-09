//! Effective views with provenance (R-SCOPE-021, R-ARCH-040, R-ARCH-042).
use std::collections::BTreeMap;

use crate::diag::Location;
use crate::hash::ContentHash;
use crate::config::Policy;
use crate::registry::item::{FieldValue, ItemId};
use crate::registry::ovr::OvrId;
use crate::registry::scope::ScopeId;
use crate::registry::ovr::{Eval, Evaluated, RawOvr, evaluate_scope};
use crate::diag::Diagnostic;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Provenance {
    pub home: ScopeId,
    pub declared_at: Location,
    pub applied: Vec<OvrId>,
    pub grants: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectiveField {
    pub value: FieldValue,
    pub provenance: Provenance,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectiveItem {
    pub id: ItemId,
    pub policy: Policy,
    pub fields: BTreeMap<String, EffectiveField>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectiveView {
    pub scope: ScopeId,
    pub chain: Vec<ScopeId>,
    pub items: BTreeMap<ItemId, EffectiveItem>,
    pub authored_digest: ContentHash,
}

use crate::hash::hash_raw;
use crate::registry::scope::ScopeTable;

pub(crate) fn empty_view(scope: &ScopeId, table: &ScopeTable) -> EffectiveView {
    EffectiveView {
        scope: scope.clone(),
        chain: table.chain_ids(scope).into_iter().cloned().collect(),
        items: BTreeMap::new(),
        authored_digest: hash_raw(b""),
    }
}

use crate::registry::item::Item;
use crate::registry::scope::ScopeState;

fn effective_item(item: &Item, loc: &Location) -> EffectiveItem {
    EffectiveItem {
        id: item.id.clone(),
        policy: item.policy,
        fields: item
            .fields
            .iter()
            .map(|(k, v)| {
                let provenance = Provenance {
                    home: item.id.scope.clone(),
                    declared_at: loc.clone(),
                    applied: Vec::new(),
                    grants: Vec::new(),
                };
                (k.clone(), EffectiveField { value: v.clone(), provenance })
            })
            .collect(),
    }
}

pub(crate) struct Built {
    pub(crate) views: BTreeMap<ScopeId, EffectiveView>,
    pub(crate) evals: Vec<Evaluated>,
}

pub(crate) fn build_views(
    ev: &Eval<'_>,
    locs: &BTreeMap<ItemId, Location>,
    mut raws: BTreeMap<ScopeId, Vec<RawOvr>>,
    diags: &mut Vec<Diagnostic>,
) -> Built {
    let (table, items) = (ev.idx.table, ev.idx.items);
    let mut views: BTreeMap<ScopeId, EffectiveView> = BTreeMap::new();
    let mut evals = Vec::new();
    for scope in table.in_depth_order() {
        let mut v = empty_view(&scope.id, table);
        if scope.state == ScopeState::Resolved {
            if let Some(p) = scope.parent.as_ref().and_then(|p| views.get(p)) {
                v.items = p.items.clone();
            }
            for (id, item) in items.iter().filter(|(k, _)| k.scope == scope.id) {
                if let Some(loc) = locs.get(id) {
                    v.items.insert(id.clone(), effective_item(item, loc));
                }
            }
            let mine = raws.remove(&scope.id).unwrap_or_default();
            evals.extend(evaluate_scope(ev, &scope.id, &mut v, mine, diags));
        }
        views.insert(scope.id.clone(), v);
    }
    Built { views, evals }
}
