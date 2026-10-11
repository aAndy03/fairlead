//! `fairlead context` (R-CLI-032 to R-CLI-034). Builds the brief through the pipeline; text and JSON are two renderings of one structure (C-15).

use fairlead_core::check::CheckOptions;
use fairlead_core::context::{Budget, ContextRequest, ContextStatus};
use fairlead_core::error::Error;
use fairlead_core::outcome::{ExitCode, VerbOutcome};
use fairlead_core::pipeline::{check_loaded, context_loaded, context_target, load, resolve_loaded};
use fairlead_core::registry::ScopeId;
use fairlead_core::render::{OutputFormat, renderer};
use fairlead_syntax::ids::ScopeSlug;

use crate::cli::{ContextArgs, GlobalArgs};
use crate::commands::check::default_options;
use crate::commands::{CommandResult, as_of, load_request, working_dir};
use crate::output::{default_format, stdout_is_tty};

pub fn run(global: &GlobalArgs, args: &ContextArgs) -> CommandResult {
    match context(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn context(global: &GlobalArgs, args: &ContextArgs) -> Result<CommandResult, Error> {
    let cwd = working_dir(global)?;
    let loaded = load(&load_request(global, false, None)?)?;
    let target = context_target(&loaded.root, &cwd, args.target.as_deref())?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let mut options: CheckOptions = default_options(as_of(global));
    if let fairlead_core::context::ContextTarget::Path(path) = &target {
        options.paths = vec![path.clone()];
    }
    let report = check_loaded(&loaded, &registry, &options)?;
    let request = ContextRequest {
        target,
        budget: budget(args.budget),
        scope_pin: scope_pin(args.scope.as_deref())?,
        full: args.full,
        depth: args.depth,
        digest_only: args.digest,
        exact_tokens: args.exact_tokens,
        as_of: as_of(global),
        since: None,
    };
    let mut output = context_loaded(&loaded, &registry, &report, &request)?;
    if let Some(arg) = &args.since {
        // R-CTX-051: the delta is recomputed on the current tree against the given point.
        let since =
            fairlead_core::pipeline::context_since(&loaded, &output.brief.header.digest, arg)?;
        let request = ContextRequest {
            since: Some(since),
            ..request
        };
        output = context_loaded(&loaded, &registry, &report, &request)?;
    }
    let format = context_format(global.format, stdout_is_tty());
    let stdout = renderer(format)
        .context(&output)
        .ok_or_else(|| Error::Internal("the renderer produced no context text".to_string()))?;
    Ok(CommandResult {
        stdout,
        stderr: String::new(),
        outcome: status_outcome(&output.status),
    })
}

/// `--budget N`: 0 is unlimited (R-CTX-044); absent is the configured default.
pub fn budget(value: Option<u32>) -> Budget {
    match value {
        None => Budget::Default,
        Some(0) => Budget::Unlimited,
        Some(n) => Budget::Tokens(n),
    }
}

/// C-28: context renders terse lines or JSON only; `human` behaves as terse.
pub fn context_format(explicit: Option<OutputFormat>, tty: bool) -> OutputFormat {
    match default_format(explicit, tty) {
        OutputFormat::Json => OutputFormat::Json,
        _ => OutputFormat::Terse,
    }
}

/// R-CLI-033, R-CTX-041: mandatory content that does not fit the budget exits 2.
pub fn status_outcome(status: &ContextStatus) -> VerbOutcome {
    let mut outcome = VerbOutcome::ok();
    if matches!(status, ContextStatus::TruncatedMandatory { .. }) {
        outcome.signal(ExitCode::Usage);
    }
    outcome
}

pub fn scope_pin(text: Option<&str>) -> Result<Option<ScopeId>, Error> {
    match text {
        None => Ok(None),
        Some(slug) => ScopeSlug::parse(slug)
            .map(|parsed| Some(ScopeId(parsed)))
            .map_err(|_| Error::Usage(format!("--scope takes a scope slug, got {slug:?}"))),
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use fairlead_core::outcome::ExitCode;

    #[test]
    fn r_cli_032__budget_zero_is_unlimited_and_absent_is_the_default() {
        assert!(matches!(budget(None), Budget::Default));
        assert!(matches!(budget(Some(0)), Budget::Unlimited));
        assert!(matches!(budget(Some(700)), Budget::Tokens(700)));
    }

    #[test]
    fn r_cli_032__human_behaves_as_terse_and_json_is_kept() {
        assert!(matches!(context_format(None, true), OutputFormat::Terse));
        assert!(matches!(
            context_format(Some(OutputFormat::Human), true),
            OutputFormat::Terse
        ));
        assert!(matches!(
            context_format(Some(OutputFormat::Json), false),
            OutputFormat::Json
        ));
    }

    #[test]
    fn r_cli_033__truncated_mandatory_content_exits_2() {
        let outcome = status_outcome(&ContextStatus::TruncatedMandatory { min_budget: 40 });
        assert_eq!(outcome.exit_code(), ExitCode::Usage);
        assert_eq!(status_outcome(&ContextStatus::Ok).exit_code(), ExitCode::Ok);
    }
}
