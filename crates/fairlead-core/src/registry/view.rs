//! Effective views with provenance (R-SCOPE-021, R-ARCH-040, R-ARCH-042).
use std::collections::BTreeMap;

use crate::config::Policy;
use crate::diag::Diagnostic;
use crate::diag::Location;
use crate::hash::ContentHash;
use crate::registry::item::{FieldValue, ItemId};
use crate::registry::ovr::OvrId;
use crate::registry::ovr::{Eval, Evaluated, RawOvr, evaluate_scope};
use crate::registry::scope::ScopeId;
use std::collections::BTreeSet;

use fairlead_syntax::directive::Verb;
use globset::GlobBuilder;

use crate::config::ConventionAction;
use crate::extract::Corpus;
use crate::path::RepoPath;
use crate::registry::ovr::{FieldChange, Override, OvrOp};

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

pub(crate) fn effective_item(item: &Item, loc: &Location) -> EffectiveItem {
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
                (
                    k.clone(),
                    EffectiveField {
                        value: v.clone(),
                        provenance,
                    },
                )
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
    digests: &BTreeMap<ScopeId, ContentHash>,
    mut raws: BTreeMap<ScopeId, Vec<RawOvr>>,
    diags: &mut Vec<Diagnostic>,
) -> Built {
    let (table, items) = (ev.idx.table, ev.idx.items);
    let mut views: BTreeMap<ScopeId, EffectiveView> = BTreeMap::new();
    let mut evals = Vec::new();
    for scope in table.in_depth_order() {
        let mut v = empty_view(&scope.id, table);
        v.authored_digest = digests
            .get(&scope.id)
            .cloned()
            .unwrap_or_else(|| hash_raw(b""));
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

fn merge(cur: Option<&FieldValue>, ch: &FieldChange, op: OvrOp) -> Option<FieldValue> {
    let drop_from = |l: &Vec<String>, rm: &[String]| {
        FieldValue::List(l.iter().filter(|x| !rm.contains(x)).cloned().collect())
    };
    match (ch, op) {
        (FieldChange::Replace(FieldValue::List(rm)), OvrOp::Remove) => match cur {
            Some(FieldValue::List(l)) => Some(drop_from(l, rm)),
            _ => None,
        },
        (FieldChange::Replace(FieldValue::Scalar(rm)), OvrOp::Remove) => match cur {
            Some(FieldValue::List(l)) => Some(drop_from(l, std::slice::from_ref(rm))),
            _ => None,
        },
        (FieldChange::Replace(v), _) => Some(v.clone()),
        (FieldChange::Append(items), _) => match cur {
            None => Some(FieldValue::List(items.clone())),
            Some(FieldValue::List(l)) => {
                let mut l = l.clone();
                for i in items {
                    if !l.contains(i) {
                        l.push(i.clone());
                    }
                }
                Some(FieldValue::List(l))
            }
            Some(FieldValue::Scalar(_)) => None,
        },
    }
}

/// Folds one effective override into a view, recording provenance (R-ARCH-042).
pub(crate) fn apply_ovr(view: &mut EffectiveView, o: &Override, grants: &[String]) {
    let Some(item) = view.items.get_mut(&o.target) else {
        return;
    };
    for (key, change) in &o.set {
        let cur = item.fields.get(key).map(|f| f.value.clone());
        let Some(new) = merge(cur.as_ref(), change, o.op) else {
            continue;
        };
        let entry = item
            .fields
            .entry(key.clone())
            .or_insert_with(|| EffectiveField {
                value: new.clone(),
                provenance: Provenance {
                    home: o.target.scope.clone(),
                    declared_at: o.at.clone(),
                    applied: Vec::new(),
                    grants: Vec::new(),
                },
            });
        entry.value = new;
        entry.provenance.applied.push(o.id.clone());
        for g in grants {
            if !entry.provenance.grants.contains(g) {
                entry.provenance.grants.push(g.clone());
            }
        }
    }
}

/// Path below the owning scope with `[[convention]] transparent` segments removed (R-SCOPE-018).
pub(crate) fn visible_rel(table: &ScopeTable, scope: &ScopeId, path: &RepoPath) -> String {
    let segs: Vec<&str> = path.as_str().split('/').collect();
    let seg_count = |d: &str| {
        if d.is_empty() {
            0
        } else {
            d.split('/').count()
        }
    };
    let mut hidden: BTreeSet<usize> = BTreeSet::new();
    for sid in table.chain_ids(scope) {
        let Some(sc) = table.get(sid) else { continue };
        let Some(cf) = sc.control.as_ref() else {
            continue;
        };
        let nb = seg_count(sc.dir.as_str().trim_matches('/'));
        for c in cf
            .conventions
            .iter()
            .filter(|c| matches!(c.action, ConventionAction::Transparent))
        {
            let Ok(g) = GlobBuilder::new(&c.glob).literal_separator(true).build() else {
                continue;
            };
            let m = g.compile_matcher();
            for end in (nb + 1)..=segs.len() {
                let prefix = segs.get(nb..end).map(|s| s.join("/")).unwrap_or_default();
                if m.is_match(&prefix) {
                    hidden.insert(end - 1);
                }
            }
        }
    }
    let owner_n = table
        .get(scope)
        .map(|s| seg_count(s.dir.as_str().trim_matches('/')))
        .unwrap_or(0);
    let kept: Vec<&str> = segs
        .iter()
        .enumerate()
        .filter(|(i, _)| *i >= owner_n && !hidden.contains(i))
        .map(|(_, s)| *s)
        .collect();
    kept.join("/")
}

pub(crate) fn own_digests(table: &ScopeTable, corpus: &Corpus) -> BTreeMap<ScopeId, ContentHash> {
    let mut parts: BTreeMap<ScopeId, Vec<u8>> = BTreeMap::new();
    for s in table.scopes.values() {
        let mut b = s.id.0.as_str().as_bytes().to_vec();
        if let Some(c) = &s.control {
            b.extend(c.content_hash.0);
        }
        parts.insert(s.id.clone(), b);
    }
    for (path, rec) in &corpus.records {
        let Some(sid) = table.scope_of(path) else {
            continue;
        };
        let authored = rec.extraction.as_ref().is_some_and(|e| {
            e.directives
                .iter()
                .any(|d| matches!(d.verb, Verb::Def | Verb::Ovr | Verb::Use))
        });
        if let (true, Some(b)) = (authored, parts.get_mut(sid)) {
            b.extend(path.as_str().as_bytes());
            b.extend(rec.hash.0);
        }
    }
    parts.into_iter().map(|(k, v)| (k, hash_raw(&v))).collect()
}

/// Bare `kind.name` -> field values; compares views across scope layouts (R-SCOPE-012).
pub fn view_signature(view: &EffectiveView) -> BTreeMap<String, BTreeMap<String, FieldValue>> {
    view.items
        .iter()
        .map(|(id, it)| {
            let fields = it
                .fields
                .iter()
                .map(|(k, f)| (k.clone(), f.value.clone()))
                .collect();
            (format!("{}.{}", id.kind.as_str(), id.name.as_str()), fields)
        })
        .collect()
}
