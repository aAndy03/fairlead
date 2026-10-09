//! Maps library results to process exit codes (R-CLI-003, R-ARCH-100). The only status mapping in the bin.

use fairlead_core::error::{Error, ErrorClass};
use fairlead_core::outcome::ExitCode;

/// The status byte comes from `ExitCode` alone (`repr(u8)`, values of R-CLI-003).
pub fn to_process_exit(code: ExitCode) -> std::process::ExitCode {
    std::process::ExitCode::from(code as u8)
}

/// Usage -> 2, Refused -> 4, Internal -> 5 (C-10). An error never maps to 0, 1 or 3.
pub fn exit_for_error(error: &Error) -> ExitCode {
    match error.class() {
        ErrorClass::Usage => ExitCode::Usage,
        ErrorClass::Refused => ExitCode::Refused,
        ErrorClass::Internal => ExitCode::Internal,
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use fairlead_core::discover::PinRefusal;
    use fairlead_core::outcome::VerbOutcome;

    #[test]
    fn r_cli_003__each_exit_code_maps_to_its_status() {
        assert_eq!(
            to_process_exit(ExitCode::Ok),
            std::process::ExitCode::from(0)
        );
        assert_eq!(
            to_process_exit(ExitCode::Findings),
            std::process::ExitCode::from(1)
        );
        assert_eq!(
            to_process_exit(ExitCode::Usage),
            std::process::ExitCode::from(2)
        );
        assert_eq!(
            to_process_exit(ExitCode::StaleGenerated),
            std::process::ExitCode::from(3)
        );
        assert_eq!(
            to_process_exit(ExitCode::Refused),
            std::process::ExitCode::from(4)
        );
        assert_eq!(
            to_process_exit(ExitCode::Internal),
            std::process::ExitCode::from(5)
        );
    }

    #[test]
    fn r_cli_003__error_classes_map_to_usage_refused_internal() {
        let usage = Error::NoRoot {
            searched_from: "/work".to_string(),
        };
        let refused = Error::Pin(PinRefusal {
            install_line: "cargo install fairlead --version 0.1.0".to_string(),
            reason: "binary older than the pin".to_string(),
        });
        let internal = Error::Internal("boom".to_string());
        assert_eq!(exit_for_error(&usage), ExitCode::Usage);
        assert_eq!(exit_for_error(&refused), ExitCode::Refused);
        assert_eq!(exit_for_error(&internal), ExitCode::Internal);
    }

    #[test]
    fn r_cli_003__verb_outcome_follows_precedence_5_2_4_1_3_0() {
        let mut outcome = VerbOutcome::ok();
        outcome.signal(ExitCode::StaleGenerated);
        outcome.signal(ExitCode::Findings);
        outcome.signal(ExitCode::Refused);
        assert_eq!(outcome.exit_code(), ExitCode::Refused);
        outcome.signal(ExitCode::Usage);
        assert_eq!(outcome.exit_code(), ExitCode::Usage);
        outcome.signal(ExitCode::Internal);
        assert_eq!(outcome.exit_code(), ExitCode::Internal);
    }
}
