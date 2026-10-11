//! The `fairlead` binary: panic boundary (R-ARCH-101) and process exit. Holds no check, path or file-format logic (R-ARCH-001).

#![deny(unsafe_code)]

mod cli;
mod commands;
mod exit;
mod output;

use std::panic::{self, AssertUnwindSafe};

use fairlead_core::outcome::ExitCode;

fn main() -> std::process::ExitCode {
    // The default hook is silenced; the boundary prints the single line R-ARCH-101 asks for.
    panic::set_hook(Box::new(|_| {}));
    let code = run_guarded(cli::run_from_env);
    commands::print_stats();
    exit::to_process_exit(code)
}

/// Runs one invocation. A panic becomes exit 5; success is never reported after a panic (R-ARCH-101).
pub fn run_guarded<F>(run: F) -> ExitCode
where
    F: FnOnce() -> ExitCode,
{
    match panic::catch_unwind(AssertUnwindSafe(run)) {
        Ok(code) => code,
        Err(payload) => {
            output::write_internal_error(&panic_text(payload.as_ref()));
            ExitCode::Internal
        }
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "unknown panic".to_string()
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_arch_101__panic_inside_a_command_exits_5() {
        assert_eq!(
            run_guarded(|| panic!("worker exploded")),
            ExitCode::Internal
        );
    }

    #[test]
    fn r_arch_101__without_a_panic_the_result_is_kept() {
        assert_eq!(run_guarded(|| ExitCode::Findings), ExitCode::Findings);
    }
}
