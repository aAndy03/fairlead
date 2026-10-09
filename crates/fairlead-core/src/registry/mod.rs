//! Registry: scopes, items, overrides, grants, views, ledger (P4). Pure and immutable (R-ARCH-061).
pub mod grant;
pub mod item;
pub mod ledger;
pub mod ovr;
pub mod resolve;
pub mod scope;
pub mod view;

use std::collections::BTreeMap;

use crate::time::AsOf;

pub use grant::{Allow, Proposal, Req};
pub use item::{Edge, FieldValue, Item, ItemId, Lookup, Origin, Site};
pub use ledger::{LedgerEntry, LedgerView};
pub use ovr::{
    ApprovalStatus, FieldChange, InertReason, Override, OvrId, OvrOp, OvrState, ovr_fingerprint,
};
pub use resolve::ResolveInput;
pub use scope::{Scope, ScopeId, ScopeState};
pub use view::{EffectiveField, EffectiveItem, EffectiveView, Provenance};

use fairlead_syntax::diag::{DiagCode, Severity};

use crate::diag::{Diagnostic, Location};
use crate::path::RepoPath;

/// Immutable result of resolution. Every accessor takes `&self` (R-ARCH-061).
#[derive(Clone, Debug)]
pub struct Registry {
    pub(crate) table: ScopeTable,
    pub(crate) items: BTreeMap<ItemId, Item>,
    pub(crate) overrides: Vec<Override>,
    pub(crate) grants: Vec<Allow>,
    pub(crate) requests: Vec<Req>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) sites: BTreeMap<ItemId, Vec<Site>>,
    pub(crate) views: BTreeMap<ScopeId, EffectiveView>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) as_of: AsOf,
    pub(crate) empty_view: EffectiveView,
    pub(crate) kinds: item::KindTable,
    pub(crate) locs: BTreeMap<ItemId, Location>,
    pub(crate) meta: Vec<ovr::OvrMeta>,
    pub(crate) proposals: Vec<Proposal>,
}

impl Registry {
    pub fn root_scope(&self) -> &ScopeId {
        &self.table.root
    }
    pub fn scopes(&self) -> impl Iterator<Item = &Scope> {
        self.table.scopes.values()
    }
    pub fn scope(&self, id: &ScopeId) -> Option<&Scope> {
        self.table.get(id)
    }
    pub fn scope_of(&self, path: &RepoPath) -> Option<&ScopeId> {
        self.table.scope_of(path)
    }
    pub fn chain(&self, id: &ScopeId) -> Vec<&ScopeId> {
        self.table.chain_ids(id)
    }
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.items.values()
    }
    pub fn item(&self, id: &ItemId) -> Option<&Item> {
        self.items.get(id)
    }
    pub fn view(&self, scope: &ScopeId) -> &EffectiveView {
        self.views.get(scope).unwrap_or(&self.empty_view)
    }
    pub fn overrides(&self) -> &[Override] {
        &self.overrides
    }
    pub fn grants(&self) -> &[Allow] {
        &self.grants
    }
    pub fn requests(&self) -> &[Req] {
        &self.requests
    }
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
    pub fn sites_of(&self, id: &ItemId) -> &[Site] {
        self.sites.get(id).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn as_of(&self) -> AsOf {
        self.as_of
    }
}

/// Diagnostic builder (not part of the contract); keeps call sites readable.
pub(crate) struct Dg(Diagnostic);

impl Dg {
    pub(crate) fn new(code: DiagCode, severity: Severity, path: &RepoPath, line: u32, col: u32) -> Dg {
        Dg(Diagnostic {
            code,
            severity,
            location: Location { path: path.clone(), line, col },
            scope: None,
            rule: None,
            related: None,
            message: String::new(),
            expected: None,
            did_you_mean: None,
            hint: String::new(),
            text: String::new(),
        })
    }
    pub(crate) fn scope(mut self, s: &ScopeId) -> Dg {
        self.0.scope = Some(s.0.clone());
        self
    }
    pub(crate) fn msg(mut self, m: impl Into<String>) -> Dg {
        self.0.message = m.into();
        self
    }
    pub(crate) fn hint(mut self, h: impl Into<String>) -> Dg {
        self.0.hint = h.into();
        self
    }
    pub(crate) fn text(mut self, t: impl Into<String>) -> Dg {
        self.0.text = t.into();
        self
    }
    pub(crate) fn related(mut self, r: impl Into<String>) -> Dg {
        self.0.related = Some(r.into());
        self
    }
    pub(crate) fn expected(mut self, e: impl Into<String>) -> Dg {
        self.0.expected = Some(e.into());
        self
    }
    pub(crate) fn dym(mut self, d: impl Into<String>) -> Dg {
        self.0.did_you_mean = Some(d.into());
        self
    }
    pub(crate) fn done(self) -> Diagnostic {
        self.0
    }
}

pub use resolve::resolve;
pub(crate) use scope::ScopeTable;

impl Registry {
    pub(crate) fn index(&self) -> item::Index<'_> {
        item::Index { table: &self.table, kinds: &self.kinds, items: &self.items }
    }
    pub fn lookup(&self, from: &ScopeId, r: &fairlead_syntax::ids::Id) -> Lookup {
        self.index().lookup(from, r)
    }
    pub fn resolve_target(&self, from: &ScopeId, text: &str) -> Option<(ItemId, Option<String>)> {
        self.index().resolve_target(from, text)
    }
}

impl Registry {
    pub fn proposals(&self) -> &[Proposal] {
        &self.proposals
    }
}
