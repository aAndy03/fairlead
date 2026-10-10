//! Seam S6: `resolve(&ResolveInput) -> Registry` (contract Section 5).
use crate::config::ControlSet;
use crate::discover::Root;
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

use crate::config::WaiverPolicy;
use crate::diag::Diagnostic;
use crate::registry::grant::{collect_grants, collect_proposals, collect_requests};
use crate::registry::item::{Index, KindTable, collect_items, collect_refs, doc_cands};
use crate::registry::ovr::{Eval, collect_raw, max_horizon_days};
use crate::registry::scope::build_scope_table;
use crate::registry::{Registry, view};

pub fn resolve(input: &ResolveInput<'_>) -> Registry {
    let mut diags: Vec<Diagnostic> = Vec::new();
    let folder = input.root.folder.as_str();
    let table = build_scope_table(input.root, input.controls, input.facts, &mut diags);
    let kinds = KindTable::build(&table, &mut diags);
    let docs = doc_cands(&table, &kinds, folder, input.corpus, &mut diags);
    let decls = collect_items(&table, docs, &mut diags);
    let idx = Index {
        table: &table,
        kinds: &kinds,
        items: &decls.items,
    };
    let refs = collect_refs(&idx, folder, input.corpus, &mut diags);
    let grants = collect_grants(&idx, input.as_of, &mut diags);
    let requests = collect_requests(&table, &mut diags);
    let proposals = collect_proposals(&table, &requests);
    let raws = collect_raw(&idx, input.corpus, &mut diags);
    let waiver = table
        .root_control()
        .map(|c| c.policies.waiver.clone())
        .unwrap_or(WaiverPolicy {
            max_exp_days: 90,
            budget: 25,
            max_renewals: 2,
            approval_required: true,
        });
    let start = table
        .root_control()
        .and_then(|c| crate::version::Version::parse(&c.version.start))
        .unwrap_or(crate::version::Version::new(0, 1, 0));
    let ev = Eval {
        idx: &idx,
        as_of: input.as_of,
        open: crate::docs::line_of(input.corpus, folder, start).open,
        max_minors: table.root_control().and_then(|c| c.exceptions.max_minors),
        grants: &grants,
        approvals: &input.facts.approvals.approvals,
        horizon_days: max_horizon_days(table.root_control()),
        waiver,
    };
    let digests = view::own_digests(&table, input.corpus);
    let built = view::build_views(&ev, &decls.locs, &digests, raws, &mut diags);
    let mut evals = built.evals;
    evals.sort_by(|a, b| {
        (
            &a.ovr.target,
            &a.ovr.declared_in,
            a.ovr.at.path.as_str(),
            a.ovr.at.line,
            a.ovr.at.col,
            &a.ovr.id,
        )
            .cmp(&(
                &b.ovr.target,
                &b.ovr.declared_in,
                b.ovr.at.path.as_str(),
                b.ovr.at.line,
                b.ovr.at.col,
                &b.ovr.id,
            ))
    });
    let (overrides, meta): (Vec<_>, Vec<_>) = evals.into_iter().map(|e| (e.ovr, e.meta)).unzip();
    diags.sort_by(|a, b| {
        (
            a.location.path.as_str(),
            a.location.line,
            a.location.col,
            a.code,
            a.message.as_str(),
        )
            .cmp(&(
                b.location.path.as_str(),
                b.location.line,
                b.location.col,
                b.code,
                b.message.as_str(),
            ))
    });
    let mut retired = Vec::new();
    for (path, text) in &input.facts.tombstones {
        match crate::lifecycle::parse_tombs(path, text) {
            Ok(tombs) => {
                let scope = table.scope_of(path).cloned();
                retired.extend(tombs.into_iter().map(|mut t| {
                    t.scope = scope.clone();
                    t
                }));
            }
            Err(e) => diags.push(Diagnostic::new(
                crate::diag::codes::HN_C001,
                crate::diag::Severity::Error,
                crate::diag::Location::at(path.clone(), 1, 1),
                format!("tombstone file does not parse: {e}"),
            )),
        }
    }
    let empty_view = view::empty_view(&table.root, &table);
    Registry {
        table,
        kinds,
        items: decls.items,
        locs: decls.locs,
        overrides,
        meta,
        grants,
        requests,
        proposals,
        edges: refs.edges,
        sites: refs.sites,
        views: built.views,
        diagnostics: diags,
        as_of: input.as_of,
        empty_view,
        retired,
    }
}
