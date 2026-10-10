//! `fairlead check` (R-CHK-040, R-CLI-021). Reads and renders only; the exit code is the report's outcome.

use std::collections::BTreeSet;
use std::time::Duration;

use camino::Utf8Path;
use fairlead_core::check::options::Ramp;
use fairlead_core::check::{CheckOptions, MaxFindings, all_specs};
use fairlead_core::diag::DiagCode;
use fairlead_core::error::Error;
use fairlead_core::outcome::VerbOutcome;
use fairlead_core::path::RepoPath;
use fairlead_core::pipeline::{
    Loaded, check_loaded, load, resolve_loaded, to_root_relative, verify_authority,
};
use fairlead_core::render::{OutputFormat, RenderOptions, renderer};
use fairlead_core::time::AsOf;

use crate::cli::{CheckArgs, GlobalArgs};
use crate::commands::{CommandResult, as_of, load_request, working_dir};
use crate::output::{colour_enabled, default_format, no_color_set, stdout_is_tty};

pub fn run(global: &GlobalArgs, args: &CheckArgs) -> CommandResult {
    match check(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

/// R-GIT-054: `FAIRLEAD_SKIP` (set by a person who must commit past the hook) skips the run,
/// except under `CI`, where it is ignored and its use is listed in the summary.
fn skip_state() -> (bool, Option<String>) {
    #[allow(clippy::disallowed_methods)]
    // reason: the bin reads the environment (P8 Global Constraint 9)
    let skip = std::env::var("FAIRLEAD_SKIP")
        .ok()
        .filter(|v| !v.is_empty());
    #[allow(clippy::disallowed_methods)]
    // reason: the bin reads the environment (P8 Global Constraint 9)
    let ci = std::env::var_os("CI").is_some_and(|v| !v.is_empty());
    match (skip, ci) {
        (None, _) => (false, None),
        (Some(_), false) => (true, None),
        (Some(v), true) => (
            false,
            Some(format!(
                "skip: FAIRLEAD_SKIP={v} is set and ignored under CI (R-GIT-054)\n"
            )),
        ),
    }
}

fn check(global: &GlobalArgs, args: &CheckArgs) -> Result<CommandResult, Error> {
    let (skipped, ci_note) = skip_state();
    if skipped {
        return Ok(CommandResult {
            stdout: String::new(),
            stderr: "check skipped: FAIRLEAD_SKIP is set (ignored under CI)\n".into(),
            outcome: VerbOutcome::default(),
        });
    }
    let tty = stdout_is_tty();
    let format = check_format(global.format, tty, args.for_agent);
    let cwd = working_dir(global)?;
    let mut loaded = load(&load_request(global, args.changed, None)?)?;
    if args.ci {
        verify_authority(&mut loaded)?;
    }
    if let Some(base) = &args.base {
        fairlead_core::pipeline::apply_base(&mut loaded, base)?;
    }
    let registry = resolve_loaded(&loaded, as_of(global));
    let mut options = check_options(global, args, &loaded, &cwd)?;
    if args.staged {
        options.files_from = Some(fairlead_core::pipeline::staged_paths(&loaded)?);
    }
    let report = check_loaded(&loaded, &registry, &options)?;
    let outcome: VerbOutcome = report.outcome(&options);
    let render = RenderOptions {
        format,
        for_agent: args.for_agent,
        max: max_findings(args.max),
        show_observe: args.show_observe,
        color: colour_enabled(global.color, tty, no_color_set()),
        quiet: global.quiet,
    };
    let mut stdout = renderer(render.format).check(&report, &render);
    if let Some(rev) = &args.since {
        for l in fairlead_core::pipeline::must_review(&loaded, rev)? {
            stdout.push_str(&l);
            stdout.push('\n');
        }
    }
    if let Some(rev) = &args.against {
        for l in fairlead_core::pipeline::against(&loaded, rev)? {
            stdout.push_str(&l);
            stdout.push('\n');
        }
    }
    if let Some(n) = ci_note {
        stdout.push_str(&n);
    }
    Ok(CommandResult {
        stdout,
        stderr: String::new(),
        outcome,
    })
}

fn check_options(
    global: &GlobalArgs,
    args: &CheckArgs,
    loaded: &Loaded,
    cwd: &Utf8Path,
) -> Result<CheckOptions, Error> {
    let mut options = default_options(as_of(global));
    options.changed = args.changed;
    options.files_from = match &args.files_from {
        Some(spec) => Some(read_files_from(spec)?),
        None => None,
    };
    options.paths = args
        .paths
        .iter()
        .map(|path| to_root_relative(&loaded.root, cwd, path))
        .collect::<Result<Vec<RepoPath>, Error>>()?;
    options.rule_filter = rule_filter(&args.rule)?;
    options.max = max_findings(args.max);
    options.ramp = Ramp {
        show_observe: args.show_observe,
        deny_warnings: args.deny_warnings,
        nudge_days: [60, 120],
    };
    options.for_agent = args.for_agent;
    options.no_baseline = args.no_baseline;
    options.require_declarations = args.require_declarations;
    options.budget = args.budget.as_deref().map(parse_budget).transpose()?;
    Ok(options)
}

/// The defaults shared with `context`: nothing filtered, every finding kept, baseline honoured.
pub fn default_options(as_of: AsOf) -> CheckOptions {
    CheckOptions {
        as_of,
        changed: false,
        files_from: None,
        paths: Vec::new(),
        rule_filter: None,
        max: MaxFindings::Default,
        ramp: Ramp {
            show_observe: false,
            deny_warnings: false,
            nudge_days: [60, 120],
        },
        for_agent: false,
        no_baseline: false,
        require_declarations: false,
        budget: None,
    }
}

/// C-14: an integer plus `ms`, `s` or `m`. Overflow and anything else is a usage error.
pub fn parse_budget(text: &str) -> Result<Duration, Error> {
    let bad = || {
        Error::Usage(format!(
            "--budget takes an integer with ms, s or m (90s, 2m); got {text:?}"
        ))
    };
    let (digits, unit) = if let Some(d) = text.strip_suffix("ms") {
        (d, "ms")
    } else if let Some(d) = text.strip_suffix('s') {
        (d, "s")
    } else if let Some(d) = text.strip_suffix('m') {
        (d, "m")
    } else {
        return Err(bad());
    };
    let value: u64 = digits.parse().map_err(|_| bad())?;
    match unit {
        "ms" => Ok(Duration::from_millis(value)),
        "s" => Ok(Duration::from_secs(value)),
        _ => Ok(Duration::from_secs(value.checked_mul(60).ok_or_else(bad)?)),
    }
}

pub fn max_findings(value: Option<u32>) -> MaxFindings {
    match value {
        None => MaxFindings::Default,
        Some(0) => MaxFindings::All,
        Some(n) => MaxFindings::Limit(n),
    }
}

/// `--for-agent` implies terse (R-CHK-039). Otherwise an explicit format wins, then human on a TTY, terse when piped.
pub fn check_format(explicit: Option<OutputFormat>, tty: bool, for_agent: bool) -> OutputFormat {
    if for_agent {
        return OutputFormat::Terse;
    }
    default_format(explicit, tty)
}

/// `--rule CODE`: `DiagCode` holds a static string, so the code is looked up in the spec table, never built from input.
fn rule_filter(text: &Option<String>) -> Result<Option<DiagCode>, Error> {
    match text {
        None => Ok(None),
        Some(code) => all_specs()
            .iter()
            .find(|spec| spec.code.0 == code.as_str())
            .map(|spec| Some(spec.code))
            .ok_or_else(|| Error::Usage(format!("unknown diagnostic code {code}"))),
    }
}

/// `--files-from FILE` or `-` (stdin): one repo-relative path per line; blank lines are skipped (R-CLI-022).
fn read_files_from(spec: &str) -> Result<BTreeSet<RepoPath>, Error> {
    let text = if spec == "-" {
        std::io::read_to_string(std::io::stdin()).map_err(|e| Error::Internal(e.to_string()))?
    } else {
        std::fs::read_to_string(spec).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Usage(format!("--files-from: no such file {spec}"))
            } else {
                Error::Internal(e.to_string())
            }
        })?
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            RepoPath::new(line)
                .map_err(|_| Error::Usage(format!("--files-from line is not a repo path: {line}")))
        })
        .collect()
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn r_cli_021__budget_takes_ms_s_or_m_and_rejects_the_rest() {
        assert_eq!(parse_budget("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_budget("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_budget("500ms").unwrap(), Duration::from_millis(500));
        for bad in ["1h", "", "abc", "s", "-5s"] {
            assert!(parse_budget(bad).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn r_cli_021__max_zero_prints_all_and_absent_is_the_default() {
        assert!(matches!(max_findings(None), MaxFindings::Default));
        assert!(matches!(max_findings(Some(0)), MaxFindings::All));
        assert!(matches!(max_findings(Some(5)), MaxFindings::Limit(5)));
    }

    #[test]
    fn r_chk_040__an_unknown_rule_code_is_a_usage_error() {
        assert!(rule_filter(&Some(["HN", "-NOPE-999"].concat())).is_err());
        assert!(matches!(
            rule_filter(&Some(["HN", "-S001"].concat())),
            Ok(Some(_))
        ));
    }

    #[test]
    fn r_cli_021__for_agent_implies_terse() {
        assert!(matches!(
            check_format(None, true, true),
            OutputFormat::Terse
        ));
        assert!(matches!(
            check_format(Some(OutputFormat::Json), true, false),
            OutputFormat::Json
        ));
    }
}
