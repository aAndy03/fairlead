//! `fairlead explain` (R-CLI-035). Target classification is in the pipeline (C-21); this module renders.

use fairlead_core::error::Error;
use fairlead_core::explain::{ExplainTarget, explain as explain_text};
use fairlead_core::pipeline::{check_loaded, explain_target, load, resolve_loaded};
use fairlead_core::render::renderer;

use crate::cli::{ExplainArgs, GlobalArgs};
use crate::commands::check::default_options;
use crate::commands::{CommandResult, as_of, load_request, working_dir};
use crate::output::{default_format, stdout_is_tty};

pub fn run(global: &GlobalArgs, args: &ExplainArgs) -> CommandResult {
    match explain(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn explain(global: &GlobalArgs, args: &ExplainArgs) -> Result<CommandResult, Error> {
    let cwd = working_dir(global)?;
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let target = explain_target(&loaded.root, &cwd, &args.target)?;
    let report = match &target {
        ExplainTarget::File(_) => Some(check_loaded(
            &loaded,
            &registry,
            &default_options(as_of(global)),
        )?),
        _ => None,
    };
    let output = explain_text(&target, &registry, report.as_ref())?;
    let format = default_format(global.format, stdout_is_tty());
    let stdout = renderer(format).explain(&output).ok_or_else(|| {
        Error::Usage("explain has no output in this format; use human or terse".to_string())
    })?;
    Ok(CommandResult::text(stdout))
}
