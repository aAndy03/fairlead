//! Command modules and the glue they share: `CommandResult`, request builders, threads and the as-of date.
//! Environment reads here are limited to FAIRLEAD_ROOT and FAIRLEAD_THREADS (constraint 9).

pub mod approve;
pub mod check;
pub mod context;
pub mod explain;
pub mod init;
pub mod new;
pub mod reserved;
pub mod seal;
pub mod sync;
pub mod upgrade;

use std::collections::BTreeSet;
use std::ffi::OsString;

use camino::Utf8PathBuf;
use fairlead_core::discover::DiscoverRequest;
use fairlead_core::error::Error;
use fairlead_core::outcome::{ExitCode, VerbOutcome};
use fairlead_core::path::RepoPath;
use fairlead_core::pipeline::LoadRequest;
use fairlead_core::time::AsOf;

use crate::cli::GlobalArgs;
use crate::exit::exit_for_error;

/// What a verb hands to `output::write_text`, plus the exit code it signalled.
#[derive(Default)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub outcome: VerbOutcome,
}

impl CommandResult {
    pub fn text(stdout: String) -> Self {
        Self {
            stdout,
            ..Self::default()
        }
    }

    /// An error stops the verb: its Display, then the diagnostics it carries (R-ARCH-103), go to
    /// stderr and its class sets the exit code.
    pub fn failed(error: &Error) -> Self {
        let mut outcome = VerbOutcome::ok();
        outcome.signal(exit_for_error(error));
        let mut stderr = format!("{error}\n");
        if !error.diagnostics().is_empty() {
            stderr.push_str(&fairlead_core::render::diagnostic_lines(
                error.diagnostics(),
            ));
        }
        Self {
            stdout: String::new(),
            stderr,
            outcome,
        }
    }

    pub fn exit(&self) -> ExitCode {
        self.outcome.exit_code()
    }
}

/// `-C` joined onto the working directory when relative; discover does the cleaning and the existence check (C-10).
pub fn working_dir(global: &GlobalArgs) -> Result<Utf8PathBuf, Error> {
    let raw = std::env::current_dir()
        .map_err(|e| Error::Internal(format!("cannot read the working directory: {e}")))?;
    let base = Utf8PathBuf::from_path_buf(raw)
        .map_err(|p| Error::Usage(format!("working directory is not UTF-8: {}", p.display())))?;
    Ok(match &global.cwd {
        None => base,
        Some(dir) if dir.is_absolute() => dir.clone(),
        Some(dir) => base.join(dir),
    })
}

pub fn binary_version() -> Result<semver::Version, Error> {
    semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|e| Error::Internal(format!("binary version is not semver: {e}")))
}

/// FAIRLEAD_ROOT as the discover request's `env_root`; an empty value counts as unset (R-CLI-004).
pub fn env_root_from(value: Option<OsString>) -> Result<Option<Utf8PathBuf>, Error> {
    match value {
        None => Ok(None),
        Some(text) if text.is_empty() => Ok(None),
        Some(text) => Utf8PathBuf::from_path_buf(std::path::PathBuf::from(text))
            .map(Some)
            .map_err(|p| Error::Usage(format!("FAIRLEAD_ROOT is not UTF-8: {}", p.display()))),
    }
}

/// The edition this binary writes; a `test-edition` build honours `FAIRLEAD_TEST_EDITION` (Q-1).
pub fn current_edition() -> fairlead_core::discover::Edition {
    #[allow(clippy::disallowed_methods)]
    // reason: the bin reads the environment (P8 Global Constraint 9)
    let over = std::env::var("FAIRLEAD_TEST_EDITION").ok();
    fairlead_core::edition::current(over.as_deref())
}

pub fn discover_request(global: &GlobalArgs, require_root: bool) -> Result<DiscoverRequest, Error> {
    Ok(DiscoverRequest {
        root_flag: global.root.clone(),
        #[allow(clippy::disallowed_methods)] // reason: the bin reads the environment (P8 Global Constraint 9)
        env_root: env_root_from(std::env::var_os("FAIRLEAD_ROOT"))?,
        cwd: working_dir(global)?,
        binary_version: binary_version()?,
        require_root,
        edition: current_edition(),
        upgrade: false,
    })
}

/// D-54: a flag wins; `0` or a non-integer is a usage error (exit 2).
pub fn resolve_threads(flag: Option<usize>, env: Option<&str>) -> Result<Option<usize>, Error> {
    let requested = match (flag, env) {
        (Some(n), _) => n,
        (None, Some(text)) => text.trim().parse::<usize>().map_err(|_| {
            Error::Usage(format!(
                "FAIRLEAD_THREADS must be a whole number of at least 1, got {text:?}"
            ))
        })?,
        (None, None) => return Ok(None),
    };
    if requested == 0 {
        return Err(Error::Usage(
            "the thread count must be at least 1".to_string(),
        ));
    }
    Ok(Some(requested))
}

fn env_threads() -> Result<Option<String>, Error> {
    #[allow(clippy::disallowed_methods)]
    // reason: the bin reads the environment (P8 Global Constraint 9)
    match std::env::var("FAIRLEAD_THREADS") {
        Ok(text) => Ok(Some(text)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::Usage(
            "FAIRLEAD_THREADS is not valid UTF-8".to_string(),
        )),
    }
}

/// The read-path request. `FAIRLEAD_THREADS` is read only when `--threads` is absent (flag wins).
pub fn load_request(
    global: &GlobalArgs,
    want_changed: bool,
    only: Option<BTreeSet<RepoPath>>,
) -> Result<LoadRequest, Error> {
    let env = if global.threads.is_none() {
        env_threads()?
    } else {
        None
    };
    let threads = resolve_threads(global.threads, env.as_deref())?;
    Ok(LoadRequest {
        discover: discover_request(global, true)?,
        use_cache: !global.no_cache,
        threads,
        want_changed,
        only,
    })
}

/// `--as-of`, or UTC today when absent (the only clock read in the bin, through the library type).
pub fn as_of(global: &GlobalArgs) -> AsOf {
    global.as_of.unwrap_or_else(AsOf::today_utc)
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use fairlead_core::error::ErrorClass;

    #[test]
    fn r_arch_082__the_flag_wins_over_the_environment() {
        assert!(matches!(resolve_threads(Some(3), Some("8")), Ok(Some(3))));
    }

    #[test]
    fn r_arch_082__the_environment_is_used_when_the_flag_is_absent() {
        assert!(matches!(resolve_threads(None, Some(" 8 ")), Ok(Some(8))));
        assert!(matches!(resolve_threads(None, None), Ok(None)));
    }

    #[test]
    fn r_arch_082__zero_and_non_integers_are_usage_errors() {
        let results = [
            resolve_threads(Some(0), None),
            resolve_threads(None, Some("0")),
            resolve_threads(None, Some("many")),
        ];
        for result in results {
            assert!(matches!(
                result.err().map(|e| e.class()),
                Some(ErrorClass::Usage)
            ));
        }
    }

    #[test]
    fn r_cli_004__an_empty_fairlead_root_counts_as_unset() {
        assert!(env_root_from(Some(OsString::new())).unwrap().is_none());
        assert!(env_root_from(None).unwrap().is_none());
    }

    #[test]
    fn r_cli_003__a_failed_command_result_carries_the_class_exit_code() {
        let result = CommandResult::failed(&Error::Internal("boom".to_string()));
        assert_eq!(result.exit(), ExitCode::Internal);
        assert_eq!(result.stderr, "internal error: boom\n");
    }
}
