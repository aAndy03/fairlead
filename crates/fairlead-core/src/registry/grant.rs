//! `[[allow]]` grants and `[[req]]` escalations (R-SCOPE-036, 053, 058).
use crate::diag::Location;
use crate::registry::item::ItemId;
use crate::registry::scope::ScopeId;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Allow {
    pub id: String,
    pub granting: ScopeId,
    pub target: ItemId,
    pub field: Option<String>,
    pub for_scopes: Vec<ScopeId>,
    pub why: String,
    pub exp: jiff::civil::Date,
    /// authored in the target's home scope (R-SCOPE-036)
    pub valid: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Req {
    pub from: ScopeId,
    pub to: ScopeId,
    pub target: String,
    pub set: Option<String>,
    pub why: String,
    pub blocking: bool,
    pub at: Location,
}
