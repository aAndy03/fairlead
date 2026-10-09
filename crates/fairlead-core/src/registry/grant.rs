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

use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::diag::Severity;
use fairlead_syntax::ids::ScopeSlug;

use crate::diag::{codes, Diagnostic};
use crate::registry::item::Index;
use crate::registry::scope::{ScopeState, ScopeTable};
use crate::registry::Dg;
use crate::time::AsOf;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Proposal {
    pub from: ScopeId,
    pub to: ScopeId,
    pub target: String,
    pub why: String,
}

fn scope_id(s: &str) -> Option<ScopeId> {
    ScopeSlug::parse(s).ok().map(ScopeId)
}

pub(crate) fn collect_grants(idx: &Index<'_>, as_of: AsOf, diags: &mut Vec<Diagnostic>) -> Vec<Allow> {
    let _ = as_of; // expiry is judged at match time so an expired grant stays visible
    let mut out = Vec::new();
    for scope in idx.table.in_depth_order() {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else { continue };
        for a in &cf.allows {
            let bad = |msg: String| {
                Dg::new(codes::HN_C001, Severity::Error, &cf.path, 1, 1)
                    .scope(&scope.id)
                    .msg(msg)
                    .hint("fix the [[allow]] table")
                    .text(a.target.as_str())
                    .done()
            };
            let Some((target, suffix)) = idx.resolve_target(&scope.id, &a.target) else {
                diags.push(bad(format!("[[allow]] target \"{}\" does not resolve", a.target)));
                continue;
            };
            let Ok(exp) = a.exp.parse::<jiff::civil::Date>() else {
                diags.push(bad(format!("[[allow]] exp \"{}\" is not an ISO date", a.exp)));
                continue;
            };
            let mut grantees: Vec<ScopeId> = Vec::new();
            for s in &a.for_scopes {
                match scope_id(s).filter(|g| idx.table.get(g).is_some()) {
                    Some(g) => grantees.push(g),
                    None => diags.push(bad(format!("[[allow]] grantee \"{s}\" is not a scope"))),
                }
            }
            grantees.sort();
            grantees.dedup();
            let below = grantees.iter().all(|g| *g != scope.id && idx.table.is_ancestor_or_self(&scope.id, g));
            let valid = scope.id == target.scope && below;
            if !valid {
                diags.push(
                    Dg::new(codes::HN_C003, Severity::Error, &cf.path, 1, 1)
                        .scope(&scope.id)
                        .msg("an [[allow]] is valid only in the home scope of its target and only for scopes below it")
                        .hint("escalate with a [[req]] to the home scope (`fairlead new escalation`)")
                        .text(a.target.as_str())
                        .done(),
                );
            }
            out.push(Allow {
                id: String::new(), // numbered after sorting, below
                granting: scope.id.clone(),
                target,
                field: a.field.clone().or(suffix),
                for_scopes: grantees,
                why: a.why.clone(),
                exp,
                valid,
            });
        }
    }
    out.sort_by(|a, b| {
        (&a.granting, &a.target, &a.field, &a.for_scopes, &a.why, a.exp)
            .cmp(&(&b.granting, &b.target, &b.field, &b.for_scopes, &b.why, b.exp))
    });
    let mut seen: BTreeMap<ScopeId, usize> = BTreeMap::new();
    for g in &mut out {
        let n = seen.entry(g.granting.clone()).or_insert(0);
        *n += 1;
        g.id = format!("{}:allow.{n}", g.granting.0.as_str());
    }
    out
}

pub(crate) fn matching_grants<'a>(
    grants: &'a [Allow],
    table: &ScopeTable,
    as_of: AsOf,
    target: &ItemId,
    touched: &BTreeSet<String>,
    declared_in: &ScopeId,
) -> Vec<&'a Allow> {
    grants
        .iter()
        .filter(|g| {
            g.valid
                && g.exp >= as_of.0
                && g.target == *target
                && g.field.as_ref().is_none_or(|f| touched.iter().all(|k| k == f))
                && g.for_scopes.iter().any(|s| table.is_ancestor_or_self(s, declared_in))
        })
        .collect()
}

pub(crate) fn collect_requests(table: &ScopeTable, diags: &mut Vec<Diagnostic>) -> Vec<Req> {
    let mut out = Vec::new();
    for scope in table.in_depth_order() {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else { continue };
        for r in &cf.reqs {
            let to = scope_id(&r.to)
                .filter(|t| *t != scope.id && table.get(t).is_some() && table.is_ancestor_or_self(t, &scope.id));
            let Some(to) = to else {
                diags.push(
                    Dg::new(codes::HN_C001, Severity::Error, &cf.path, 1, 1)
                        .scope(&scope.id)
                        .msg(format!("[[req]] to \"{}\" is not an ancestor scope", r.to))
                        .hint("name an ancestor scope id in `to`")
                        .text(r.target.as_str())
                        .done(),
                );
                continue;
            };
            out.push(Req {
                from: scope.id.clone(),
                to,
                target: r.target.clone(),
                set: r.set.clone(),
                why: r.why.clone(),
                blocking: r.blocking,
                at: Location { path: cf.path.clone(), line: 0, col: 0 },
            });
        }
    }
    out.sort_by(|a, b| (&a.from, &a.to, &a.target, &a.why).cmp(&(&b.from, &b.to, &b.target, &b.why)));
    out
}

pub(crate) fn collect_proposals(table: &ScopeTable, requests: &[Req]) -> Vec<Proposal> {
    let mut out = Vec::new();
    for scope in table.in_depth_order().into_iter().filter(|s| s.id != table.root) {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else { continue };
        for v in cf.violations.iter().filter(|v| v.code == codes::HN_C003) {
            let target = if v.text.is_empty() { v.message.clone() } else { v.text.clone() };
            if requests.iter().any(|r| r.from == scope.id && r.target == target) {
                continue;
            }
            out.push(Proposal {
                from: scope.id.clone(),
                to: table.root.clone(),
                why: format!("scope \"{}\" cannot set \"{target}\" (non-delegable); the root decides", scope.id.0.as_str()),
                target,
            });
        }
    }
    out.sort_by(|a, b| (&a.from, &a.target).cmp(&(&b.from, &b.target)));
    out
}
