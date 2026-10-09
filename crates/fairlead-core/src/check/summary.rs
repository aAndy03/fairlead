//! Summary and exit mapping (R-CHK-028/029, R-CHK-040, C-22).
use std::collections::BTreeMap;

use fairlead_syntax::diag::{DiagCode, Severity};

use crate::check::options::CheckOptions;
use crate::check::{CheckReport, Finding, FindingState};
use crate::config::Stage;
use crate::diag::codes::HN_G001;
use crate::outcome::{ExitCode, VerbOutcome};
use crate::registry::ScopeId;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaiverCount {
    pub active: u32,
    pub expiring_30d: u32,
    pub budget: u32,
    pub renewals: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub errors: u32,
    pub warns: u32,
    pub infos: u32,
    pub baselined: u32,
    pub waived: u32,
    pub observe: u32,
    pub accepted: u32,
    pub waivers: BTreeMap<ScopeId, WaiverCount>,
    pub relaxations: Vec<String>,
    pub exceptions_delta: i32,
    pub codeowners_uncovered: u32,
    pub requests_open: u32,
    pub oldest_request_days: Option<u32>,
}

pub(crate) fn count(fs: &[Finding]) -> Summary {
    let mut s = Summary::default();
    for f in fs {
        match f.state {
            FindingState::Baselined => s.baselined += 1,
            FindingState::Waived => s.waived += 1,
            FindingState::New | FindingState::WaiverIneffective if f.stage == Stage::Observe => {
                s.observe += 1
            }
            FindingState::New | FindingState::WaiverIneffective => match f.severity {
                Severity::Error => s.errors += 1,
                Severity::Warn => s.warns += 1,
                Severity::Info => s.infos += 1,
            },
        }
    }
    s
}

fn family(code: DiagCode) -> Option<u8> {
    code.0.as_bytes().get(3).copied()
}

impl CheckReport {
    /// R-CHK-040 mapping, exactly as C-22; precedence is `ExitCode::resolve`'s.
    pub fn outcome(&self, opts: &CheckOptions) -> VerbOutcome {
        let live = |f: &&Finding| {
            matches!(f.state, FindingState::New | FindingState::WaiverIneffective)
                && f.stage != Stage::Observe
        };
        let (mut usage, mut findings, mut stale) = (!self.unknown_scopes.is_empty(), false, false);
        for f in self.findings.iter().filter(live) {
            usage |= f.severity == Severity::Error && matches!(family(f.code), Some(b'S' | b'C'));
            findings |= (f.severity == Severity::Error && f.stage == Stage::Enforce)
                || (opts.ramp.deny_warnings && f.severity >= Severity::Warn);
            stale |= f.code == HN_G001;
        }
        findings |= self.zero_declarations && opts.require_declarations;
        let mut o = VerbOutcome::ok();
        if usage {
            o.signal(ExitCode::Usage);
        }
        if findings {
            o.signal(ExitCode::Findings);
        }
        if stale {
            o.signal(ExitCode::StaleGenerated);
        }
        if self.partial && !(usage || findings || stale) {
            o.signal(ExitCode::Usage);
        }
        o
    }
}
