//! `Finding` and the pre-finding builder (R-CHK-035, R-ARCH-051).
use crate::{
    check::codes::spec,
    config::Stage,
    diag::{Diagnostic, Location},
    hash::Fingerprint,
    path::RepoPath,
    registry::{OvrId, ScopeId},
};
use fairlead_syntax::{
    diag::{DiagCode, Severity},
    ids::{Id, Name},
};
use std::cmp::Ordering;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct RuleId(pub Name); // kind is always `rule`

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FindingState {
    New,
    Baselined,
    Waived,
    WaiverIneffective,
}

/// `rule.<name>`, the form rule IDs are printed in.
impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rule.{}", self.0.as_str())
    }
}
impl FindingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            FindingState::New => "new",
            FindingState::Baselined => "baselined",
            FindingState::Waived => "waived",
            FindingState::WaiverIneffective => "waiver-ineffective",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Finding {
    pub code: DiagCode,
    pub severity: Severity,
    pub stage: Stage,
    pub location: Location,
    pub scope: ScopeId,
    pub rule: Option<RuleId>,
    pub related: Option<String>,
    pub message: String,
    pub excerpt: Option<String>,
    pub intent: Option<String>,
    pub fix: String,
    pub occurrence: u32,
    pub fingerprint: Fingerprint,
    pub state: FindingState,
    pub waived_by: Option<OvrId>,
}
impl Ord for Finding {
    /// (path, line, col, code), then message (R-CHK-011); fingerprint makes the order total.
    fn cmp(&self, o: &Self) -> Ordering {
        self.location
            .path
            .cmp(&o.location.path)
            .then(self.location.line.cmp(&o.location.line))
            .then(self.location.col.cmp(&o.location.col))
            .then(self.code.cmp(&o.code))
            .then_with(|| self.message.cmp(&o.message))
            .then_with(|| self.fingerprint.cmp(&o.fingerprint))
    }
}
impl PartialOrd for Finding {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// `rule.<name>` text or bare name to a `RuleId`.
pub(crate) fn rule_id(name: &str) -> Option<RuleId> {
    let bare = name.strip_prefix("rule.").unwrap_or(name);
    Id::parse(&format!("rule.{bare}"))
        .ok()
        .map(|id| RuleId(id.name))
}

/// First `max` characters (never splits a code point).
pub(crate) fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Builder for the pre-finding every check emits.
pub(crate) struct DiagB(Diagnostic);
impl DiagB {
    pub(crate) fn new(
        code: DiagCode,
        path: &RepoPath,
        line: u32,
        col: u32,
        message: impl Into<String>,
    ) -> DiagB {
        let severity = spec(code).map_or(Severity::Error, |s| s.default_severity);
        DiagB(Diagnostic {
            code,
            severity,
            location: Location {
                path: path.clone(),
                line,
                col,
            },
            scope: None,
            rule: None,
            related: None,
            message: message.into(),
            expected: None,
            did_you_mean: None,
            hint: String::new(),
            text: String::new(),
            stage: None,
        })
    }
    pub(crate) fn hint(mut self, h: impl Into<String>) -> DiagB {
        self.0.hint = h.into();
        self
    }
    pub(crate) fn text(mut self, t: impl Into<String>) -> DiagB {
        self.0.text = t.into();
        self
    }
    pub(crate) fn rule(mut self, name: &str) -> DiagB {
        self.0.rule = Some(format!("rule.{name}"));
        self
    }
    pub(crate) fn related(mut self, r: impl Into<String>) -> DiagB {
        self.0.related = Some(r.into());
        self
    }
    pub(crate) fn stage(mut self, s: crate::config::Stage) -> DiagB {
        self.0.stage = Some(s);
        self
    }
    pub(crate) fn severity(mut self, s: Severity) -> DiagB {
        self.0.severity = s;
        self
    }
    pub(crate) fn dym(mut self, d: impl Into<String>) -> DiagB {
        self.0.did_you_mean = Some(d.into());
        self
    }
    pub(crate) fn build(self) -> Diagnostic {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes as c;

    fn finding(path: &str, line: u32, col: u32, code: DiagCode, msg: &str, fp: u8) -> Finding {
        Finding {
            code,
            severity: Severity::Error,
            stage: Stage::Enforce,
            location: Location {
                path: RepoPath::new(path).unwrap(),
                line,
                col,
            },
            scope: ScopeId(fairlead_syntax::ids::ScopeSlug::parse("root").unwrap()),
            rule: None,
            related: None,
            message: msg.to_string(),
            excerpt: None,
            intent: None,
            fix: String::new(),
            occurrence: 0,
            fingerprint: Fingerprint::from_digest(&[fp; 32]),
            state: FindingState::New,
            waived_by: None,
        }
    }

    #[test]
    fn r_arch_051__order_is_path_line_col_code_then_message() {
        let mut v = vec![
            finding("b.rs", 1, 1, c::HN_R001, "a", 1),
            finding("a.rs", 9, 1, c::HN_R001, "a", 2),
            finding("a.rs", 2, 7, c::HN_R001, "a", 3),
            finding("a.rs", 2, 3, c::HN_T001, "a", 4),
            finding("a.rs", 2, 3, c::HN_R001, "z", 5),
            finding("a.rs", 2, 3, c::HN_R001, "a", 6),
        ];
        v.sort();
        let got: Vec<(String, u32, u32, &str, String)> = v
            .iter()
            .map(|f| {
                (
                    f.location.path.as_str().to_string(),
                    f.location.line,
                    f.location.col,
                    f.code.0,
                    f.message.clone(),
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                ("a.rs".into(), 2, 3, c::HN_R001.0, "a".into()),
                ("a.rs".into(), 2, 3, c::HN_R001.0, "z".into()),
                ("a.rs".into(), 2, 3, c::HN_T001.0, "a".into()),
                ("a.rs".into(), 2, 7, c::HN_R001.0, "a".into()),
                ("a.rs".into(), 9, 1, c::HN_R001.0, "a".into()),
                ("b.rs".into(), 1, 1, c::HN_R001.0, "a".into()),
            ]
        );
    }

    #[test]
    fn r_chk_011__order_is_total_fingerprint_breaks_remaining_ties() {
        let a = finding("a.rs", 1, 1, c::HN_R001, "m", 1);
        let b = finding("a.rs", 1, 1, c::HN_R001, "m", 2);
        assert_ne!(a.cmp(&b), std::cmp::Ordering::Equal);
        assert_eq!(a.cmp(&a.clone()), std::cmp::Ordering::Equal);
    }

    #[test]
    fn r_chk_035__state_names_are_the_four_of_the_spec() {
        let names: Vec<_> = [
            FindingState::New,
            FindingState::Baselined,
            FindingState::Waived,
            FindingState::WaiverIneffective,
        ]
        .iter()
        .map(|s| s.as_str())
        .collect();
        assert_eq!(names, ["new", "baselined", "waived", "waiver-ineffective"]);
    }

    #[test]
    fn r_agt_017__clip_counts_characters_not_bytes() {
        assert_eq!(clip("héllo", 2), "hé");
        assert_eq!(clip("ab", 10), "ab");
    }

    #[test]
    fn r_arch_103__builder_takes_default_severity_from_the_registry() {
        let p = RepoPath::new("x.css").unwrap();
        let d = DiagB::new(c::HN_R002, &p, 3, 4, "m")
            .hint("h")
            .text("t")
            .rule("r")
            .build();
        assert_eq!(
            (d.severity, d.location.line, d.location.col),
            (Severity::Warn, 3, 4)
        );
        assert_eq!(
            (d.hint.as_str(), d.text.as_str(), d.rule.as_deref()),
            ("h", "t", Some("rule.r"))
        );
    }
}
