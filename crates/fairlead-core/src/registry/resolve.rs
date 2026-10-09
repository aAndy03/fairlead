//! Seam S6: `resolve(&ResolveInput) -> Registry` (contract Section 5).
use crate::discover::Root;
use crate::config::ControlSet;
use crate::extract::Corpus;
use crate::facts::Facts;
use crate::time::AsOf;

pub struct ResolveInput<'a> {
    pub root: &'a Root,
    pub controls: &'a ControlSet,
    pub corpus: &'a Corpus,
    /// approvals and gen/members shards on disk (HN-C007, HN-C008)
    pub facts: &'a Facts,
    pub as_of: AsOf,
}

use crate::diag::Diagnostic;
use crate::registry::item::{Index, KindTable, collect_items, collect_refs, doc_cands};
use crate::registry::scope::build_scope_table;
use crate::registry::{view, Registry};

pub fn resolve(input: &ResolveInput<'_>) -> Registry {
    let mut diags: Vec<Diagnostic> = Vec::new();
    let table = build_scope_table(input.root, input.controls, input.facts, &mut diags);
    let kinds = KindTable::build(&table, &mut diags);
    let docs = doc_cands(&table, &kinds, input.root.folder.as_str(), input.corpus, &mut diags);
    let decls = collect_items(&table, docs, &mut diags);
    // ... at the end, before building the Registry (R-ARCH-051 style total order):
    diags.sort_by(|a, b| {
        (a.location.path.as_str(), a.location.line, a.location.col, a.code, a.message.as_str())
            .cmp(&(b.location.path.as_str(), b.location.line, b.location.col, b.code, b.message.as_str()))
    });
    let idx = Index { table: &table, kinds: &kinds, items: &decls.items };
    let refs = collect_refs(&idx, input.root.folder.as_str(), input.corpus, &mut diags);
    let views = view::build_views(&table, &decls.items, &decls.locs);
    let empty_view = view::empty_view(&table.root, &table);
    Registry {
        table,
        kinds,
        items: decls.items,
        locs: decls.locs,
        overrides: Vec::new(),
        grants: Vec::new(),
        requests: Vec::new(),
        edges: refs.edges,
        sites: refs.sites,
        views,
        diagnostics: diags,
        as_of: input.as_of,
        empty_view,
    }
}
