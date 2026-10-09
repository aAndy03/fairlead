//! Overrides, their states and fingerprints (R-SCOPE-031..034, 040, 045..047, 064).
use std::collections::BTreeMap;

use crate::diag::Location;
use crate::hash::{ContentHash, Fingerprint};
use crate::registry::item::{FieldValue, ItemId};
use crate::registry::scope::ScopeId;
use std::collections::BTreeSet;

use fairlead_syntax::diag::Severity;
use fairlead_syntax::directive::{ArgOp, ArgValue, Verb, known_keys};
use fairlead_syntax::ids::Id;

use crate::config::{Approval, ControlFile, OvrDecl, WaiverPolicy};
use crate::diag::{Diagnostic, codes};
use crate::extract::Corpus;
use crate::hash::hash_raw;
use crate::path::RepoPath;
use crate::registry::Dg;
use crate::registry::grant::Allow;
use crate::registry::item::{Index, Item, KindTable, Origin, Site, arg_items, arg_scalar, normalise_rules_alias, undeclared_kind};
use crate::registry::scope::ScopeState;
use crate::registry::view::EffectiveView;
use crate::time::AsOf;

/// Fingerprint of (target, scope, why, owner, exp) (R-CHK-026).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct OvrId(pub Fingerprint);

pub use crate::config::OvrOp;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum InertReason {
    SealedNoAllow,
    Incomplete,
    FloorRule,
    NonDelegable,
    UnknownField,
    DuplicateOvr,
    VersionExpiry,
    OutsideSubtree,
    UnknownOwner,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OvrState {
    Effective,
    Inert(InertReason),
    Expired,
    Stale,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ApprovalStatus {
    NotRequired,
    Approved { renewal: u32 },
    Missing,
}

/// R-SYN-013. For `op = Remove` the `Replace` payload lists the values to remove (P4-9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FieldChange {
    Replace(FieldValue),
    Append(Vec<String>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Override {
    pub id: OvrId,
    pub target: ItemId,
    pub field: Option<String>,
    pub op: OvrOp,
    pub set: BTreeMap<String, FieldChange>,
    pub why: Option<String>,
    pub owner: Option<String>,
    pub exp: Option<jiff::civil::Date>,
    pub base: Option<OvrId>,
    pub declared_in: ScopeId,
    pub applies: Option<String>,
    pub at: Location,
    pub state: OvrState,
    pub approval: ApprovalStatus,
    pub target_hash: Option<ContentHash>,
}

/// Rule names that mirror the floor codes (R-SCOPE-039, R-CHK-008). An `ovr` on `rule.<name>` is inert.
pub const FLOOR_RULES: &[&str] = &[
    "baseline-tampered", "control-file", "duplicate-id", "exception-on-floor-rule", "expired-exception",
    "non-delegable-key", "path-hygiene", "root-dup", "secret-like",
];

pub fn ovr_fingerprint(
    target: &ItemId,
    scope: &ScopeId,
    why: &str,
    owner: &str,
    exp: Option<&jiff::civil::Date>,
) -> Fingerprint {
    let why = why.split_whitespace().collect::<Vec<_>>().join(" ");
    let exp = exp.map(|d| d.to_string()).unwrap_or_default();
    let text = format!("{}\u{1f}{}\u{1f}{why}\u{1f}{owner}\u{1f}{exp}", target.to_id(), scope.0.as_str());
    Fingerprint::from_hash(&hash_raw(text.as_bytes())) // Verify: P3 constructor, first 12 base32 chars
}

pub(crate) fn op_name(o: OvrOp) -> &'static str {
    match o {
        OvrOp::Loosen => "loosen",
        OvrOp::Tighten => "tighten",
        OvrOp::Set => "set",
        OvrOp::Replace => "replace",
        OvrOp::Add => "add",
        OvrOp::Remove => "remove",
    }
}

pub(crate) fn parse_op(s: &str) -> Option<OvrOp> {
    [OvrOp::Loosen, OvrOp::Tighten, OvrOp::Set, OvrOp::Replace, OvrOp::Add, OvrOp::Remove]
        .into_iter()
        .find(|o| op_name(*o) == s)
}

#[derive(Clone, Debug)]
pub(crate) enum OvrExtent {
    File(RepoPath),
    Scope(Option<String>),
}

#[derive(Clone, Debug)]
pub(crate) struct RawOvr {
    pub(crate) scope: ScopeId,
    pub(crate) at: Location,
    pub(crate) target_text: String,
    pub(crate) extent: OvrExtent,
    pub(crate) op_text: Option<String>,
    pub(crate) set: BTreeMap<String, FieldChange>,
    pub(crate) why: Option<String>,
    pub(crate) owner: Option<String>,
    pub(crate) exp: Option<String>,
    pub(crate) until: bool,
    pub(crate) base: Option<String>,
    pub(crate) pin: Option<String>,
    pub(crate) text: String,
}

impl RawOvr {
    fn new(scope: &ScopeId, at: Location, target_text: String, extent: OvrExtent, text: String) -> RawOvr {
        RawOvr {
            scope: scope.clone(), at, target_text, extent, op_text: None, set: BTreeMap::new(),
            why: None, owner: None, exp: None, until: false, base: None, pin: None, text,
        }
    }

    fn absorb(&mut self, key: &str, append: bool, v: &ArgValue) {
        match key {
            "why" => self.why = Some(arg_scalar(v)),
            "owner" => self.owner = Some(arg_scalar(v)),
            "exp" => self.exp = Some(arg_scalar(v)),
            "until" => self.until = true,
            "op" => self.op_text = Some(arg_scalar(v)),
            "base" => self.base = Some(arg_scalar(v)),
            "x-pin" => self.pin = Some(arg_scalar(v)),
            k if k.starts_with("x-") => {}
            k => {
                let change = if append {
                    FieldChange::Append(arg_items(v))
                } else {
                    FieldChange::Replace(match v {
                        ArgValue::List(l) => FieldValue::List(l.clone()),
                        other => FieldValue::Scalar(arg_scalar(other)),
                    })
                };
                self.set.insert(k.to_string(), change);
            }
        }
    }
}

pub(crate) fn collect_raw(
    idx: &Index<'_>,
    corpus: &Corpus,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<ScopeId, Vec<RawOvr>> {
    let mut out: BTreeMap<ScopeId, Vec<RawOvr>> = BTreeMap::new();
    for (path, rec) in &corpus.records {
        let Some(sid) = idx.table.scope_of(path) else { continue };
        if !idx.table.is_resolved(sid) {
            continue;
        }
        let Some(ex) = rec.extraction.as_ref() else { continue };
        for d in ex.directives.iter().filter(|d| d.verb == Verb::Ovr) {
            let Some(id) = d.ids.first() else { continue };
            let text = fairlead_syntax::render_directive_body(d);
            if !idx.kinds.is_declared(idx.table, sid, id) {
                diags.push(undeclared_kind(idx.kinds, idx.table, sid, id, path, d.span, &text));
                continue;
            }
            let at = Location { path: path.clone(), line: d.span.line, col: d.span.col };
            let mut r = RawOvr::new(sid, at, id.to_string(), OvrExtent::File(path.clone()), text);
            for a in &d.args {
                r.absorb(&a.key, matches!(a.op, ArgOp::Append), &a.value);
            }
            out.entry(sid.clone()).or_default().push(r);
        }
    }
    for scope in idx.table.in_depth_order() {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else { continue };
        for o in &cf.ovrs {
            let parsed = Id::parse(&normalise_rules_alias(&o.target));
            let Ok(id) = parsed else {
                diags.push(
                    Dg::new(codes::HN_C001, Severity::Error, &cf.path, 1, 1)
                        .scope(&scope.id)
                        .msg(format!("[[ovr]] target \"{}\" is not a valid ID", o.target))
                        .hint("write the target as kind.name or kind.name.field")
                        .done(),
                );
                continue;
            };
            let at = Location { path: cf.path.clone(), line: 0, col: 0 };
            if !idx.kinds.is_declared(idx.table, &scope.id, &id) {
                diags.push(undeclared_kind(idx.kinds, idx.table, &scope.id, &id, &cf.path, fairlead_syntax::diag::Span { start: 0, end: 0, line: 0, col: 0 }, &o.target));
                continue;
            }
            let mut r = RawOvr::new(&scope.id, at, id.to_string(), OvrExtent::Scope(o.applies.clone()), o.target.clone());
            r.op_text = Some(op_name(o.op).to_string());
            r.why = o.why.clone();
            r.owner = o.owner.clone();
            r.exp = o.exp.clone();
            r.base = o.base.clone();
            for (k, v) in &o.set {
                r.absorb(k, o.op == OvrOp::Add, v);
            }
            out.entry(scope.id.clone()).or_default().push(r);
        }
    }
    out
}

// --- evaluation -----------------------------------------------------------------------------

pub(crate) struct Eval<'a> {
    pub(crate) idx: &'a Index<'a>,
    pub(crate) as_of: AsOf,
    pub(crate) grants: &'a [Allow],
    pub(crate) approvals: &'a [Approval],
    pub(crate) waiver: WaiverPolicy,
    pub(crate) horizon_days: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct OvrMeta {
    pub(crate) applied: bool,
    pub(crate) whole: bool,
    pub(crate) extent: OvrExtent,
    pub(crate) used_grants: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct Evaluated {
    pub(crate) ovr: Override,
    pub(crate) meta: OvrMeta,
}

struct Pre {
    raw: RawOvr,
    target: ItemId,
    field: Option<String>,
    id: OvrId,
    base: Option<OvrId>,
    op: OvrOp,
    touched: BTreeSet<String>,
}

struct Verdict {
    state: OvrState,
    approval: ApprovalStatus,
    tighten: bool,
    used_grants: Vec<String>,
}

fn policy_text(p: crate::config::Policy) -> &'static str {
    match p {
        crate::config::Policy::Open => "open",
        crate::config::Policy::NeedsException => "needs-exception",
        crate::config::Policy::Sealed => "sealed",
    }
}

fn field_text(v: Option<&FieldValue>) -> String {
    match v {
        Some(FieldValue::Scalar(s)) => s.clone(),
        Some(FieldValue::List(l)) => l.join(","),
        None => String::new(),
    }
}

/// P4-7: pin of the declared form of the touched fields.
fn target_pin(item: Option<&Item>, target: &ItemId, touched: &BTreeSet<String>) -> ContentHash {
    let mut s = format!("{}", target.to_id());
    if let Some(it) = item {
        s.push('|');
        s.push_str(policy_text(it.policy));
        for k in touched {
            s.push_str(&format!("|{k}={}", field_text(it.fields.get(k))));
        }
    }
    hash_raw(s.as_bytes())
}

fn field_allowed(ev: &Eval<'_>, item: &Item, key: &str) -> bool {
    if item.fields.contains_key(key) {
        return true;
    }
    let kind = item.id.kind.as_str();
    kind != "rule"
        && (known_keys(Verb::Def).contains(&key)
            || ev.idx.kinds.decl(ev.idx.table, &item.id.scope, kind).is_some_and(|k| k.keys.contains_key(key)))
}

fn judge(ev: &Eval<'_>, p: &Pre, item: Option<&Item>, _working: &EffectiveView) -> Verdict {
    let v = |state| Verdict { state, approval: ApprovalStatus::NotRequired, tighten: false, used_grants: Vec::new() };
    // [T9: floor, outside subtree, non-delegable]
    if let Some(it) = item {
        if p.touched.iter().any(|k| !field_allowed(ev, it, k)) {
            return v(OvrState::Inert(InertReason::UnknownField));
        }
    }
    // [T11: duplicate]
    // [T9, T10: expiry shapes, tighten, policy, owner, expiry, stale]
    v(OvrState::Effective)
}

pub(crate) fn evaluate_scope(
    ev: &Eval<'_>,
    scope: &ScopeId,
    working: &mut EffectiveView,
    raws: Vec<RawOvr>,
    diags: &mut Vec<Diagnostic>,
) -> Vec<Evaluated> {
    let mut pres: Vec<Pre> = Vec::new();
    for raw in raws {
        let Some((target, field)) = ev.idx.resolve_target(scope, &raw.target_text) else { continue }; // P4-11
        let exp = raw.exp.as_deref().and_then(|s| s.parse::<jiff::civil::Date>().ok());
        let id = OvrId(ovr_fingerprint(
            &target,
            scope,
            raw.why.as_deref().unwrap_or(""),
            raw.owner.as_deref().unwrap_or(""),
            exp.as_ref(),
        ));
        let base = raw.base.as_deref().and_then(|b| b.parse::<Fingerprint>().ok()).map(OvrId);
        let op = raw.op_text.as_deref().and_then(parse_op).unwrap_or(OvrOp::Set);
        let mut touched: BTreeSet<String> = raw.set.keys().cloned().collect();
        touched.extend(field.clone());
        pres.push(Pre { raw, target, field, id, base, op, touched });
    }
    pres.sort_by(|a, b| {
        (a.target.to_id().to_string(), a.raw.at.path.as_str(), a.raw.at.line, a.raw.at.col)
            .cmp(&(b.target.to_id().to_string(), b.raw.at.path.as_str(), b.raw.at.line, b.raw.at.col))
    });
    let mut out = Vec::new();
    for p in &pres {
        let item = ev.idx.items.get(&p.target);
        let verdict = judge(ev, p, item, working);
        if verdict.state == OvrState::Inert(InertReason::UnknownField) {
            diags.push(
                Dg::new(codes::HN_S009, Severity::Error, &p.raw.at.path, p.raw.at.line, p.raw.at.col)
                    .scope(scope)
                    .msg(format!("`ovr {}` names a field the item does not have", p.raw.target_text))
                    .hint("use a field of the target (see `fairlead explain` for the kind), or declare it under [kinds.<k>.keys]")
                    .text(p.raw.text.as_str())
                    .done(),
            );
        }
        let exp = p.raw.exp.as_deref().and_then(|s| s.parse::<jiff::civil::Date>().ok());
        let applies = match &p.raw.extent {
            OvrExtent::Scope(a) => a.clone(),
            OvrExtent::File(_) => None,
        };
        let whole = matches!(&p.raw.extent, OvrExtent::Scope(None))
            || matches!(&p.raw.extent, OvrExtent::Scope(Some(a)) if a == "**");
        let applied = verdict.state == OvrState::Effective && verdict.approval != ApprovalStatus::Missing;
        out.push(Evaluated {
            ovr: Override {
                id: p.id.clone(),
                target: p.target.clone(),
                field: p.field.clone(),
                op: p.op,
                set: p.raw.set.clone(),
                why: p.raw.why.clone(),
                owner: p.raw.owner.clone(),
                exp,
                base: p.base.clone(),
                declared_in: scope.clone(),
                applies,
                at: p.raw.at.clone(),
                state: verdict.state,
                approval: verdict.approval,
                target_hash: Some(target_pin(item, &p.target, &p.touched)),
            },
            meta: OvrMeta { applied, whole, extent: p.raw.extent.clone(), used_grants: verdict.used_grants },
        });
        let _ = verdict.tighten;
    }
    out
}
