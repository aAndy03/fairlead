//! Overrides, their states and fingerprints (R-SCOPE-031..034, 040, 045..047, 064).
use std::collections::BTreeMap;

use crate::diag::Location;
use crate::hash::{ContentHash, Fingerprint};
use crate::registry::item::{FieldValue, ItemId};
use crate::registry::scope::ScopeId;
use std::collections::BTreeSet;

use fairlead_syntax::diag::{DiagCode, Severity};
use fairlead_syntax::directive::{ArgOp, ArgValue, Verb, known_keys};
use fairlead_syntax::ids::Id;

use crate::config::{Approval, ControlFile, OvrDecl, Policy, WaiverPolicy};
use crate::diag::{Diagnostic, codes};
use crate::extract::Corpus;
use crate::hash::{base32, hash_raw};
use crate::path::RepoPath;
use crate::registry::Dg;
use crate::registry::grant::{Allow, matching_grants};
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

struct Verdict {
    state: OvrState,
    approval: ApprovalStatus,
    tighten: bool,
    used_grants: Vec<String>,
    diag: Option<(DiagCode, String, &'static str)>,
}

impl Verdict {
    fn inert(reason: InertReason, diag: Option<(DiagCode, String, &'static str)>) -> Verdict {
        Verdict { state: OvrState::Inert(reason), approval: ApprovalStatus::NotRequired, tighten: false, used_grants: Vec::new(), diag }
    }
}

fn looks_like_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn rank(field: &str, v: &str) -> Option<u8> {
    match (field, v) {
        ("sev", "info") | ("stage", "observe") => Some(0),
        ("sev", "warn") | ("stage", "warn") => Some(1),
        ("sev", "error") | ("stage", "enforce") => Some(2),
        _ => None,
    }
}

/// R-SCOPE-034, R-CHK-006: every touched field moves toward the stricter slot.
fn tightening(it: &Item, working: &EffectiveView, p: &Pre) -> bool {
    if it.id.kind.as_str() != "rule" || p.raw.set.is_empty() {
        return false;
    }
    let cur = working.items.get(&it.id);
    p.raw.set.iter().all(|(k, ch)| match (k.as_str(), ch) {
        ("sev" | "stage", FieldChange::Replace(FieldValue::Scalar(new))) => {
            let now = cur.and_then(|c| c.fields.get(k)).map(|f| &f.value).or(it.fields.get(k));
            match (now, rank(k, new)) {
                (Some(FieldValue::Scalar(c)), Some(n)) => rank(k, c).is_some_and(|cr| n > cr),
                _ => false,
            }
        }
        ("glob", FieldChange::Append(_)) => true,
        ("exclude", _) => p.op == OvrOp::Remove,
        _ => false,
    })
}

fn judge(ev: &Eval<'_>, p: &Pre, item: Option<&Item>, working: &EffectiveView, dup: bool) -> Verdict {
    let table = ev.idx.table;
    let scope = &p.raw.scope;
    let is_floor = p.target.kind.as_str() == "rule" && FLOOR_RULES.contains(&p.target.name.as_str());
    let Some(it) = item.filter(|_| !is_floor) else {
        return Verdict::inert(InertReason::FloorRule, None); // HN-X003 is derived by P5
    };
    if !table.is_ancestor_or_self(&it.id.scope, scope) {
        return Verdict::inert(
            InertReason::OutsideSubtree,
            Some((codes::HN_C003, format!("`ovr {}` targets an item outside this scope's ancestry", p.raw.target_text),
                "exceptions never escape their subtree; escalate with a [[req]]")),
        );
    }
    // [T11: applies glob escaping the subtree]
    if p.touched.contains("policy") {
        return Verdict::inert(
            InertReason::NonDelegable,
            Some((codes::HN_C003, "only the home scope may set an item's policy".to_string(),
                "set `policy=` on the def in the home scope, or escalate with a [[req]]")),
        );
    }
    if p.touched.iter().any(|k| !field_allowed(ev, it, k)) {
        return Verdict::inert(
            InertReason::UnknownField,
            Some((codes::HN_S009, format!("`ovr {}` names a field the item does not have", p.raw.target_text),
                "use a field of the target or declare it under [kinds.<k>.keys]")),
        );
    }
    let _ = dup; // [T11: duplicate]
    let exp_text = p.raw.exp.as_deref();
    if p.raw.until || exp_text.is_some_and(looks_like_version) {
        return Verdict::inert(
            InertReason::VersionExpiry,
            Some((codes::HN_S012, "version-based expiry needs seals (0.2); the override is dropped".to_string(),
                "write exp=YYYY-MM-DD")),
        );
    }
    let bad_date = exp_text.is_some_and(|s| s.parse::<jiff::civil::Date>().is_err());
    let bad_op = p.raw.op_text.as_deref().is_some_and(|s| parse_op(s).is_none());
    if bad_date || bad_op {
        return Verdict::inert(
            InertReason::Incomplete,
            Some((codes::HN_S016, format!("`ovr {}` has an invalid exp or op", p.raw.target_text),
                "exp is an ISO date; op is loosen|tighten|set|replace|add|remove")),
        );
    }
    let tighten = tightening(it, working, p);
    let mut used = Vec::new();
    let mut need_complete = false;
    if !tighten {
        match it.policy {
            Policy::Open => {}
            Policy::NeedsException => need_complete = true,
            Policy::Sealed => {
                let g = matching_grants(ev.grants, table, ev.as_of, &p.target, &p.touched, scope);
                if g.is_empty() {
                    return Verdict::inert(InertReason::SealedNoAllow, None); // HN-X005 is derived by P5
                }
                used = g.iter().map(|a| a.id.clone()).collect();
                need_complete = true;
            }
        }
    }
    if need_complete {
        let have = [("why", p.raw.why.is_some()), ("owner", p.raw.owner.is_some()), ("exp", exp_text.is_some())];
        let missing: Vec<&str> = have.iter().filter(|t| !t.1).map(|t| t.0).collect();
        if !missing.is_empty() {
            return Verdict::inert(
                InertReason::Incomplete,
                Some((codes::HN_S016, format!("`ovr {}` is missing {}", p.raw.target_text, missing.join(", ")),
                    "add why=, owner= and exp=YYYY-MM-DD")),
            );
        }
    }
    // [T10] owner, expiry, stale, approval
    if let Some(owner) = p.raw.owner.as_deref().filter(|_| !tighten) {
        let known = table
            .chain_ids(scope)
            .into_iter()
            .filter_map(|s| table.get(s))
            .any(|s| s.owners.iter().any(|o| o.handle == owner));
        if !known {
            return Verdict::inert(
                InertReason::UnknownOwner,
                Some((codes::HN_X012, format!("owner \"{owner}\" is not a handle in `owners`"),
                    "use a handle from the root [settings] owners list")),
            );
        }
    }
    let exp = exp_text.and_then(|s| s.parse::<jiff::civil::Date>().ok());
    if exp.is_some_and(|d| d < ev.as_of.0) {
        return Verdict::plain(OvrState::Expired); // HN-X001 is derived by P5
    }
    if let Some(pin) = p.raw.pin.as_deref() {
        let now = base32(&target_pin(Some(it), &p.target, &p.touched).0, 12);
        if pin != now {
            return Verdict::plain(OvrState::Stale); // HN-X010 is derived by P5
        }
    }
    let approval = if need_complete && ev.waiver.approval_required {
        match ev.approvals.iter().filter(|a| a.fp == p.id.0).map(|a| a.renewal).max() {
            Some(renewal) => ApprovalStatus::Approved { renewal },
            None => ApprovalStatus::Missing,
        }
    } else {
        ApprovalStatus::NotRequired
    };
    Verdict { state: OvrState::Effective, approval, tighten, used_grants: used, diag: None }
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
        let verdict = judge(ev, p, item, working, false); // Task 11 supplies `dup`
        if let Some((code, msg, hint)) = &verdict.diag {
            diags.push(
                Dg::new(*code, Severity::Error, &p.raw.at.path, p.raw.at.line, p.raw.at.col)
                    .scope(scope)
                    .msg(msg.clone())
                    .hint(*hint)
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
        let applied = verdict.state == OvrState::Effective
            && verdict.approval != ApprovalStatus::Missing
            && !beyond_horizon(ev, exp);
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

impl Verdict {
    fn plain(state: OvrState) -> Verdict {
        Verdict { state, approval: ApprovalStatus::NotRequired, tighten: false, used_grants: Vec::new(), diag: None }
    }
}

pub fn max_horizon_days(root: Option<&ControlFile>) -> i64 {
    let a = root.map(|c| i64::from(c.policies.waiver.max_exp_days)).unwrap_or(90);
    let b = root.and_then(|c| c.exceptions.max_days).map(i64::from).unwrap_or(a);
    a.min(b)
}

fn beyond_horizon(ev: &Eval<'_>, exp: Option<jiff::civil::Date>) -> bool {
    exp.is_some_and(|d| {
        d.since(ev.as_of.0).map(|s| i64::from(s.get_days())).unwrap_or(i64::MAX) > ev.horizon_days
    }) // Verify: jiff 0.2 Date::since returns Result<Span, Error>; Span::get_days() -> i32
}
