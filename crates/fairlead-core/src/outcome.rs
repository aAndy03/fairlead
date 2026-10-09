//! Exit codes are defined only in 15-cli.md R-CLI-003; this module names them.

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ExitCode {
    Ok = 0,
    Findings = 1,
    Usage = 2,
    StaleGenerated = 3,
    Refused = 4,
    Internal = 5,
}

impl ExitCode {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Rank in the precedence 5, 2, 4, 1, 3, 0 (higher wins).
    const fn precedence(self) -> u8 {
        match self {
            ExitCode::Internal => 5,
            ExitCode::Usage => 4,
            ExitCode::Refused => 3,
            ExitCode::Findings => 2,
            ExitCode::StaleGenerated => 1,
            ExitCode::Ok => 0,
        }
    }

    /// The winning code of several signals; `Ok` when there are none (R-CLI-003).
    pub fn resolve(codes: &[ExitCode]) -> ExitCode {
        codes
            .iter()
            .copied()
            .max_by_key(|c| c.precedence())
            .unwrap_or(ExitCode::Ok)
    }
}

/// What a verb reports besides text; the bin turns it into the process exit code.
#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct VerbOutcome {
    signals: Vec<ExitCode>,
}

impl VerbOutcome {
    pub fn ok() -> Self {
        Self::default()
    }

    pub fn signal(&mut self, c: ExitCode) {
        self.signals.push(c);
    }

    pub fn merge(&mut self, o: VerbOutcome) {
        self.signals.extend(o.signals);
    }

    pub fn exit_code(&self) -> ExitCode {
        ExitCode::resolve(&self.signals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ExitCode::*;

    #[test]
    fn r_cli_003__resolve_precedence_is_5_2_4_1_3_0() {
        assert_eq!(ExitCode::resolve(&[]), Ok);
        assert_eq!(ExitCode::resolve(&[Ok, Ok]), Ok);
        assert_eq!(ExitCode::resolve(&[StaleGenerated, Ok]), StaleGenerated);
        assert_eq!(ExitCode::resolve(&[Findings, StaleGenerated]), Findings);
        assert_eq!(ExitCode::resolve(&[Refused, Findings]), Refused);
        assert_eq!(ExitCode::resolve(&[Usage, Refused, Findings]), Usage);
        assert_eq!(
            ExitCode::resolve(&[Internal, Usage, Refused, Findings, StaleGenerated]),
            Internal
        );
        assert_eq!(ExitCode::resolve(&[StaleGenerated, Internal]), Internal);
    }

    #[test]
    fn r_cli_003__resolve_is_order_independent() {
        let all = [Ok, Findings, Usage, StaleGenerated, Refused, Internal];
        for a in all {
            for b in all {
                assert_eq!(ExitCode::resolve(&[a, b]), ExitCode::resolve(&[b, a]));
            }
        }
    }

    #[test]
    fn r_cli_003__numeric_values_are_stable() {
        assert_eq!(
            [Ok, Findings, Usage, StaleGenerated, Refused, Internal].map(ExitCode::as_u8),
            [0, 1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn r_cli_003__verb_outcome_merges_and_resolves() {
        let mut a = VerbOutcome::ok();
        assert_eq!(a.exit_code(), Ok);
        a.signal(StaleGenerated);
        let mut b = VerbOutcome::default();
        b.signal(Findings);
        a.merge(b);
        assert_eq!(a.exit_code(), Findings);
        a.signal(Internal);
        assert_eq!(a.exit_code(), Internal);
    }
}
