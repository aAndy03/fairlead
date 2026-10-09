//! Check engine (P5a). Pure: no filesystem, no git, no clock (R-ARCH-050, R-CHK-004).
#[allow(dead_code, reason = "wired by P5a-T29; remove once used")]
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

pub const FAIRLEAD_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    /// The subset of `unknown_scopes` absent from the working tree: exit 5, not 2 (A-45).
    pub absent_scopes: Vec<crate::registry::ScopeId>,
    /// Zero declarations notice (R-CHK-003).
    pub zero_declarations: bool,
}

/// Everything `run` reads (contract 4.9). `load_diags` are the walk and control-file diagnostics.
pub struct CheckInput<'a> {
    pub registry: &'a Registry,
    pub corpus: &'a Corpus,
    pub facts: &'a Facts,
    pub load_diags: &'a [crate::diag::Diagnostic],
    pub baseline: Option<&'a Baseline>,
    pub options: &'a CheckOptions,
}

/// R-ARCH-102: call before `run`; `Err` maps to exit 5, never to a pass (A-17).
pub fn preflight(input: &CheckInput<'_>) -> Result<(), crate::error::Error> {
    let g = &input.facts.git;
    if input.options.changed {
        if let Some(why) = &g.unreadable {
            return Err(crate::error::Error::CannotDetermine(format!(
                "git state is unreadable: {why}"
            )));
        }
        if g.changed.is_none() {
            return Err(crate::error::Error::CannotDetermine(
                "--changed needs a git repository".to_string(),
            ));
        }
    }
    Ok(())
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

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::testkit::{Built, World};

    fn input<'a>(b: &'a Built, o: &'a CheckOptions) -> CheckInput<'a> {
        CheckInput {
            registry: &b.registry,
            corpus: &b.corpus,
            facts: &b.facts,
            load_diags: &[],
            baseline: None,
            options: o,
        }
    }

    #[test]
    fn r_arch_102__unreadable_git_under_changed_is_cannot_determine() {
        use crate::error::Error::CannotDetermine;
        let mut b = World::new().build();
        let mut o = b.options();
        assert!(preflight(&input(&b, &o)).is_ok()); // not asked for --changed
        o.changed = true;
        assert!(matches!(preflight(&input(&b, &o)), Err(CannotDetermine(_)))); // no git facts
        b.facts.git.changed = Some(crate::facts::ChangedSet(Default::default()));
        assert!(preflight(&input(&b, &o)).is_ok());
        b.facts.git.unreadable = Some("index locked".into());
        assert!(
            matches!(preflight(&input(&b, &o)), Err(CannotDetermine(m)) if m.contains("index locked"))
        );
    }
}
