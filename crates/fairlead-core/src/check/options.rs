//! `CheckOptions` (R-CHK-001). The only file of `check/` that reads a clock: the `--budget` deadline.
use std::{collections::BTreeSet, time::{Duration, Instant}};
use fairlead_syntax::diag::DiagCode;
use crate::{path::RepoPath, time::AsOf};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ramp { pub show_observe: bool, pub deny_warnings: bool, pub nudge_days: [u32; 2] }
impl Default for Ramp {
    fn default() -> Self { Ramp { show_observe: false, deny_warnings: false, nudge_days: [60, 120] } }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MaxFindings { Default, Limit(u32), All }
impl MaxFindings {
    /// `None` = unlimited (`--max 0`).
    pub fn cap(&self) -> Option<usize> {
        match self {
            MaxFindings::Default => Some(20),
            MaxFindings::Limit(0) | MaxFindings::All => None,
            MaxFindings::Limit(n) => Some(*n as usize),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CheckOptions {
    pub as_of: AsOf,
    pub changed: bool,
    pub files_from: Option<BTreeSet<RepoPath>>,
    pub paths: Vec<RepoPath>,
    pub rule_filter: Option<DiagCode>,
    pub max: MaxFindings,
    pub ramp: Ramp,
    pub for_agent: bool,
    pub no_baseline: bool,
    pub require_declarations: bool,
    pub budget: Option<Duration>,
}
impl CheckOptions {
    pub fn new(as_of: AsOf) -> Self {
        CheckOptions { as_of, changed: false, files_from: None, paths: Vec::new(), rule_filter: None,
            max: MaxFindings::Default, ramp: Ramp::default(), for_agent: false, no_baseline: false,
            require_declarations: false, budget: None }
    }
    /// True when the report is narrowed; whole-run conclusions (stale exception, fixed baseline entry) are then skipped.
    pub fn is_partial_view(&self) -> bool {
        self.changed || self.files_from.is_some() || !self.paths.is_empty() || self.rule_filter.is_some()
    }
}

/// C-14: integer plus `ms`, `s` or `m`.
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: u64 = num.parse().map_err(|_| format!("bad duration `{s}`: expected <integer><ms|s|m>"))?;
    match unit {
        "ms" => Ok(Duration::from_millis(n)),
        "s" => Ok(Duration::from_secs(n)),
        "m" => Ok(Duration::from_secs(n.saturating_mul(60))),
        _ => Err(format!("bad duration `{s}`: expected <integer><ms|s|m>")),
    }
}

/// `--budget` deadline (R-CLI-023). `Deadline::start(None)` never expires.
#[derive(Copy, Clone, Debug)]
pub struct Deadline { at: Option<Instant> }
impl Deadline {
    pub fn start(budget: Option<Duration>) -> Deadline {
        Deadline { at: budget.and_then(|d| Instant::now().checked_add(d)) }
    }
    pub fn expired(&self) -> bool { self.at.is_some_and(|t| Instant::now() >= t) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn opts() -> CheckOptions { CheckOptions::new(AsOf::parse("2026-10-09").unwrap()) }

    #[test]
    fn r_chk_038__max_default_is_twenty_and_zero_means_all() {
        assert_eq!(MaxFindings::Default.cap(), Some(20));
        assert_eq!(MaxFindings::Limit(5).cap(), Some(5));
        assert_eq!(MaxFindings::Limit(0).cap(), None);
        assert_eq!(MaxFindings::All.cap(), None);
    }

    #[test]
    fn r_chk_033__ramp_defaults() {
        let r = Ramp::default();
        assert_eq!(r.nudge_days, [60, 120]);
        assert!(!r.show_observe && !r.deny_warnings);
    }

    #[test]
    fn r_cli_023__budget_grammar_is_integer_plus_ms_s_m() {
        assert_eq!(parse_duration("90s"), Ok(Duration::from_secs(90)));
        assert_eq!(parse_duration("2m"), Ok(Duration::from_secs(120)));
        assert_eq!(parse_duration("250ms"), Ok(Duration::from_millis(250)));
        for bad in ["", "90", "s", "1h", "-3s", "1.5s"] { assert!(parse_duration(bad).is_err(), "{bad}"); }
    }

    #[test]
    fn r_cli_023__deadline_zero_is_expired_none_never() {
        assert!(Deadline::start(Some(Duration::ZERO)).expired());
        assert!(!Deadline::start(None).expired());
    }

    #[test]
    fn r_cli_022__partial_view_is_any_narrowing_option() {
        assert!(!opts().is_partial_view());
        let mut o = opts(); o.changed = true; assert!(o.is_partial_view());
        let mut o = opts(); o.paths.push(RepoPath::new("src").unwrap()); assert!(o.is_partial_view());
        let mut o = opts(); o.rule_filter = Some(crate::diag::codes::HN_T001); assert!(o.is_partial_view());
    }
}
