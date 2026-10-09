//! Exception ledger, computed on read (R-SCOPE-043, 044).
use crate::registry::item::{FieldValue, ItemId};
use crate::registry::ovr::{OvrId, OvrState};
use crate::registry::scope::ScopeId;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LedgerEntry {
    pub ovr: OvrId,
    pub target: ItemId,
    pub home: ScopeId,
    pub varied_in: ScopeId,
    pub depth: u32,
    pub why: Option<String>,
    pub owner: Option<String>,
    pub exp: Option<jiff::civil::Date>,
    pub state: OvrState,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LedgerView {
    pub for_scope: ScopeId,
    pub entries: Vec<LedgerEntry>,
}

use crate::registry::{Origin, Registry};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LineClass {
    Def,
    Use,
    Ovr,
    Flag,
    Env,
    Allow,
    Req,
    ControlKey,
    NoteTail,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Propagation {
    pub down: bool, // computed at read time into views of the subtree
    pub up: bool,   // fact visible to ancestors
}

/// The propagation matrix of R-SCOPE-060 for 0.1.
pub fn propagation(c: LineClass) -> Propagation {
    let (down, up) = match c {
        LineClass::Def | LineClass::Use | LineClass::Ovr | LineClass::ControlKey => (true, true),
        LineClass::Flag | LineClass::Env | LineClass::Allow => (true, false),
        LineClass::Req => (false, true),
        LineClass::NoteTail => (false, false),
    };
    Propagation { down, up }
}

impl Registry {
    /// Computed on read (R-SCOPE-044): overrides declared strictly below `scope` whose home is `scope` or above.
    pub fn ledger(&self, scope: &ScopeId) -> LedgerView {
        let Some(x) = self.table.get(scope) else {
            return LedgerView {
                for_scope: scope.clone(),
                entries: Vec::new(),
            };
        };
        let mut entries: Vec<LedgerEntry> = Vec::new();
        for o in &self.overrides {
            let Some(s) = self.table.get(&o.declared_in) else {
                continue;
            };
            let below =
                o.declared_in != *scope && self.table.is_ancestor_or_self(scope, &o.declared_in);
            if below && self.table.is_ancestor_or_self(&o.target.scope, scope) {
                entries.push(LedgerEntry {
                    ovr: o.id.clone(),
                    target: o.target.clone(),
                    home: o.target.scope.clone(),
                    varied_in: o.declared_in.clone(),
                    depth: s.depth.saturating_sub(x.depth),
                    why: o.why.clone(),
                    owner: o.owner.clone(),
                    exp: o.exp,
                    state: o.state,
                });
            }
        }
        // Total order independent of the textual order of the overrides (R-ARCH-060): entries that
        // agree on target, distance, scope and id are ordered by their remaining fields.
        entries.sort_by_cached_key(|e| {
            (
                e.target.clone(),
                e.depth,
                e.varied_in.clone(),
                e.ovr.clone(),
                format!("{:?}", (&e.why, &e.owner, &e.exp, &e.state)),
            )
        });
        LedgerView {
            for_scope: scope.clone(),
            entries,
        }
    }

    /// R-FILE-052: nearest `[[file]]` item of a file kind along the chain, nearest scope first.
    pub fn nearest_file_item(&self, scope: &ScopeId, file_kind: &str) -> Option<&ItemId> {
        self.table.chain_ids(scope).into_iter().rev().find_map(|s| {
            self.items
                .iter()
                .filter(|(id, it)| {
                    id.scope == *s
                        && matches!(it.origin, Origin::ControlTable { .. })
                        && matches!(it.fields.get("kind"), Some(FieldValue::Scalar(k)) if k == file_kind)
                })
                .map(|(id, _)| id)
                .next()
        })
    }
}
