//! Test-only builders for `Finding` and `CheckReport` (compiled only under `cfg(test)`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::collections::BTreeMap;

use fairlead_syntax::ids::{Id, ScopeSlug};

use crate::check::{CheckReport, Finding, FindingState, ReportHeader, RuleId, Summary};
use crate::config::Stage;
use crate::diag::{DiagCode, Location, Severity};
use crate::discover::Edition;
use crate::hash::{Fingerprint, base32, hash_raw};
use crate::path::RepoPath;
use crate::registry::ScopeId;
use crate::time::AsOf;

pub(crate) fn scope(id: &str) -> ScopeId {
    ScopeId(ScopeSlug::parse(id).unwrap())
}

#[allow(dead_code, reason = "wired by P5a-T25..T29; remove once used")]
pub(crate) fn rule(name: &str) -> RuleId {
    RuleId(Id::parse(&format!("rule.{name}")).unwrap().name)
}

pub(crate) fn fp(text: &str) -> Fingerprint {
    Fingerprint::parse(text).unwrap()
}

/// An enforce-stage, state `new` finding in scope `web` with a fingerprint derived from its fields.
pub(crate) fn finding(
    code: DiagCode,
    severity: Severity,
    path: &str,
    line: u32,
    col: u32,
    message: &str,
) -> Finding {
    let seed = format!("{}:{path}:{line}:{col}:{message}", code.0);
    Finding {
        code,
        severity,
        stage: Stage::Enforce,
        location: Location {
            path: RepoPath::new(path).unwrap(),
            line,
            col,
        },
        scope: scope("web"),
        rule: None,
        related: None,
        message: message.to_string(),
        excerpt: None,
        intent: None,
        fix: "fix it".to_string(),
        occurrence: 0,
        fingerprint: fp(&base32(&hash_raw(seed.as_bytes()).0, 12)),
        state: FindingState::New,
        waived_by: None,
    }
}

pub(crate) fn with(mut f: Finding, edit: impl FnOnce(&mut Finding)) -> Finding {
    edit(&mut f);
    f
}

/// A report over `findings` (sorted into the total order), header of the spec example.
#[allow(dead_code, reason = "wired by P5a-T25..T29; remove once used")]
pub(crate) fn report(mut findings: Vec<Finding>) -> CheckReport {
    findings.sort();
    let count =
        |s: Severity| u32::try_from(findings.iter().filter(|f| f.severity == s).count()).unwrap();
    let summary = Summary {
        errors: count(Severity::Error),
        warns: count(Severity::Warn),
        infos: count(Severity::Info),
        baselined: 0,
        waived: 0,
        observe: 0,
        accepted: 0,
        waivers: BTreeMap::new(),
        relaxations: Vec::new(),
        exceptions_delta: 0,
        codeowners_uncovered: 0,
        requests_open: 0,
        oldest_request_days: None,
    };
    CheckReport {
        header: ReportHeader {
            fairlead_version: "0.1.0".to_string(),
            edition: Edition(1),
            as_of: AsOf(jiff::civil::date(2026, 10, 8)),
            scopes: 3,
            files: 1204,
            rules: 7,
        },
        findings,
        summary,
        partial: false,
        unknown_scopes: Vec::new(),
        absent_scopes: Vec::new(),
        zero_declarations: false,
    }
}
