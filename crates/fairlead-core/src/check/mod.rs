//! Check engine (P5a). Pure: no filesystem, no git, no clock (R-ARCH-050, R-CHK-004).
pub(crate) mod apply;
pub mod baseline;
pub mod builtin;
pub mod codes;
pub mod finding;
pub mod fingerprint;
pub mod floor;
pub mod options;
pub mod rules;
pub mod summary;

pub use baseline::{AcceptRefusal, Baseline, BaselineEntry, BaselineIssue};
pub use finding::{Finding, FindingState, RuleId};
pub use options::{CheckOptions, MaxFindings};
pub use rules::{Check, DeclaredRule, RuleCheck, RuleSet};
pub use summary::{Summary, WaiverCount};

use crate::discover::Edition;
use crate::extract::Corpus;
use crate::facts::Facts;
use crate::generate::GeneratedSet;
use crate::registry::Registry;
use crate::time::AsOf;

pub struct ReportHeader {
    pub fairlead_version: String,
    pub edition: Edition,
    pub as_of: AsOf,
    pub scopes: u32,
    pub files: u32,
    pub rules: u32,
}

pub struct CheckReport {
    pub header: ReportHeader,
    /// ALL findings, totally ordered; truncation is a render concern (R-ARCH-051).
    pub findings: Vec<Finding>,
    pub summary: Summary,
    /// `--budget` deadline hit (R-CLI-023).
    pub partial: bool,
    /// Unresolved or sparse scopes: the verdict depends on them (R-CHK-057).
    pub unknown_scopes: Vec<crate::registry::ScopeId>,
    /// Zero declarations notice (R-CHK-003).
    pub zero_declarations: bool,
}

/// Read-only bundle handed to each `Check` (everything pure).
pub struct CheckCx<'a> {
    pub registry: &'a Registry,
    pub corpus: &'a Corpus,
    pub facts: &'a Facts,
    pub generated: &'a GeneratedSet,
    pub rules: &'a RuleSet,
    pub options: &'a CheckOptions,
}

#[cfg(test)]
pub(crate) mod testkit;
