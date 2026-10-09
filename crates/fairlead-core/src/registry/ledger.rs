//! Exception ledger, computed on read (R-SCOPE-043, 044).
use crate::registry::item::ItemId;
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
