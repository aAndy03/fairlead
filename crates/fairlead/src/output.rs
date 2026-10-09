//! The only module that writes to stdout and stderr (contract Section 3). Output is LF-only (R-CLI-006).

use std::io::{IsTerminal, Write};

use fairlead_core::render::OutputFormat;

use crate::cli::ColorArg;

/// R-CLI-006 and C-28: an explicit `--format` wins; otherwise human on a TTY, terse when piped.
pub fn default_format(explicit: Option<OutputFormat>, stdout_is_tty: bool) -> OutputFormat {
    match explicit {
        Some(format) => format,
        None if stdout_is_tty => OutputFormat::Human,
        None => OutputFormat::Terse,
    }
}

/// R-CLI-006: colour is off when not a TTY or when NO_COLOR is set; `--color never` always wins.
pub fn colour_enabled(color: ColorArg, stdout_is_tty: bool, no_color_set: bool) -> bool {
    match color {
        ColorArg::Never => false,
        ColorArg::Auto => stdout_is_tty && !no_color_set,
    }
}

/// NO_COLOR counts when present and not empty (no-color.org).
pub fn no_color_set() -> bool {
    #[allow(clippy::disallowed_methods)]
    // reason: the bin reads the environment (P8 Global Constraint 9)
    std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
}

pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// R-CLI-006, R-ARCH-052: LF-only text with exactly one trailing newline when non-empty.
pub fn lf_only(text: &str) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    if unified.is_empty() {
        return unified;
    }
    let mut out = String::with_capacity(unified.len() + 1);
    out.push_str(unified.trim_end_matches('\n'));
    out.push('\n');
    out
}

/// Writes a verb's stdout and stderr text. Every byte goes through `lf_only`; a failed write is an error the caller maps to exit 5.
pub fn write_text(stdout: &str, stderr: &str) -> std::io::Result<()> {
    if !stdout.is_empty() {
        let mut out = std::io::stdout().lock();
        out.write_all(lf_only(stdout).as_bytes())?;
        out.flush()?;
    }
    if !stderr.is_empty() {
        let mut err = std::io::stderr().lock();
        err.write_all(lf_only(stderr).as_bytes())?;
        err.flush()?;
    }
    Ok(())
}

/// The one line a caught panic or a failed write produces (R-ARCH-101).
pub fn write_internal_error(detail: &str) {
    let line = format!("fairlead: internal error (exit 5): {detail}; no result is reported\n");
    let mut err = std::io::stderr().lock();
    // A failed write to a closed stderr has nowhere left to be reported.
    let _ = err.write_all(line.as_bytes());
    let _ = err.flush();
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::cli::ColorArg;

    #[test]
    fn r_cli_006__format_defaults_to_human_on_a_tty_and_terse_when_piped() {
        assert_eq!(default_format(None, true), OutputFormat::Human);
        assert_eq!(default_format(None, false), OutputFormat::Terse);
    }

    #[test]
    fn r_cli_006__an_explicit_format_always_wins() {
        assert_eq!(
            default_format(Some(OutputFormat::Json), true),
            OutputFormat::Json
        );
        assert_eq!(
            default_format(Some(OutputFormat::Sarif), false),
            OutputFormat::Sarif
        );
    }

    #[test]
    fn r_cli_006__colour_is_off_when_not_a_tty_or_no_color_is_set() {
        assert!(!colour_enabled(ColorArg::Auto, false, false));
        assert!(!colour_enabled(ColorArg::Auto, true, true));
        assert!(colour_enabled(ColorArg::Auto, true, false));
        assert!(!colour_enabled(ColorArg::Never, true, false));
    }

    #[test]
    fn r_cli_006__output_is_lf_only_with_one_trailing_newline() {
        assert_eq!(lf_only("a\r\nb\r\n\r\n"), "a\nb\n");
        assert_eq!(lf_only("x\r"), "x\n");
        assert_eq!(lf_only(""), "");
        assert!(!lf_only("a\r\n").contains('\r'));
    }
}
