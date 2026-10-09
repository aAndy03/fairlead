//! Overrides, their states and fingerprints (R-SCOPE-031..034, 040, 045..047, 064).
use std::collections::BTreeMap;

use crate::diag::Location;
use crate::hash::{ContentHash, Fingerprint};
use crate::registry::item::{FieldValue, ItemId};
use crate::registry::scope::ScopeId;

/// Fingerprint of (target, scope, why, owner, exp) (R-CHK-026).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct OvrId(pub Fingerprint);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OvrOp {
    Loosen,
    Tighten,
    Set,
    Replace,
    Add,
    Remove,
}

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
