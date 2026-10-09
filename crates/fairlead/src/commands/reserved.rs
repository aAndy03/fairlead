//! Verbs that arrive in a later milestone (R-CLI-001). They parse, then refuse with exit 2 before any work.

use fairlead_core::error::Error;

use crate::cli::GlobalArgs;
use crate::commands::CommandResult;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReservedVerb {
    Move,
    Seal,
    Upgrade,
}

impl ReservedVerb {
    pub fn name(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Seal => "seal",
            Self::Upgrade => "upgrade",
        }
    }

    /// 15-cli.md verb summary: `move`, `seal` and `upgrade` are all 0.2 (D-38).
    pub fn milestone(self) -> &'static str {
        "0.2"
    }
}

/// The refusal is `Error::Reserved`, so its Display text is P3's; the test below pins the exact line.
pub fn run(_global: &GlobalArgs, verb: ReservedVerb) -> CommandResult {
    CommandResult::failed(&Error::Reserved {
        verb: verb.name(),
        milestone: verb.milestone(),
    })
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::cli::GlobalArgs;
    use fairlead_core::outcome::ExitCode;

    #[test]
    fn r_cli_001__each_reserved_verb_refuses_with_its_milestone_and_exit_2() {
        let cases = [
            (ReservedVerb::Move, "move arrives in 0.2\n"),
            (ReservedVerb::Seal, "seal arrives in 0.2\n"),
            (ReservedVerb::Upgrade, "upgrade arrives in 0.2\n"),
        ];
        for (verb, text) in cases {
            let result = run(&GlobalArgs::default(), verb);
            assert_eq!(result.stderr, text);
            assert!(result.stdout.is_empty());
            assert_eq!(result.exit(), ExitCode::Usage);
        }
    }
}
