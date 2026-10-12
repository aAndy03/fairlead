//! `fairlead sync` (R-CLI-025, R-CLI-046, R-CLI-043). Applies directly; `--yes` is accepted and has no effect.

use fairlead_core::error::Error;
use fairlead_core::pipeline::{WriteSession, commit_plan, load, plan_sync, resolve_loaded};
use fairlead_core::write::{BaselineMode, CommitReport, SyncRequest, SysProbe};

use crate::cli::{GlobalArgs, SyncArgs};
use crate::commands::check::default_options;
use crate::commands::context::scope_pin;
use crate::commands::{CommandResult, as_of, discover_request, load_request};

pub fn run(global: &GlobalArgs, args: &SyncArgs) -> CommandResult {
    match sync(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn sync(global: &GlobalArgs, args: &SyncArgs) -> Result<CommandResult, Error> {
    let session = WriteSession::open(&discover_request(global, true)?, &SysProbe)?;
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let options = default_options(as_of(global));
    let request = SyncRequest {
        baseline: baseline_mode(args)?,
        accept_vendor: args.accept_vendor,
        force: args.force,
        scope: scope_pin(args.scope.as_deref())?,
        purge: args.purge.clone(),
    };
    fairlead_core::pipeline::merge_gate(&loaded, "sync", true)?;
    let plan = plan_sync(&loaded, &registry, &options, &request)?;
    if args.dry_run {
        return Ok(CommandResult::text(plan.render_text("sync plan")));
    }
    let outcome = commit_plan(&session, plan)?;
    Ok(CommandResult::text(changed_list(&outcome.report)))
}

/// `--baseline` alone prunes; `--accept-new` needs `--why` (R-CHK-032); no `--baseline` leaves the baseline alone.
pub fn baseline_mode(args: &SyncArgs) -> Result<BaselineMode, Error> {
    if !args.baseline {
        return Ok(BaselineMode::Off);
    }
    match (args.accept_new, args.why.as_deref()) {
        (true, Some(why)) => Ok(BaselineMode::AcceptNew {
            why: why.to_string(),
        }),
        (true, None) => Err(Error::Usage("--accept-new needs --why TEXT".to_string())),
        (false, _) => Ok(BaselineMode::Prune),
    }
}

/// One line per written file, sorted by the library; "no changes" when nothing was written (R-CLI-027).
pub fn changed_list(report: &CommitReport) -> String {
    if report.written.is_empty() {
        return "no changes\n".to_string();
    }
    report
        .written
        .iter()
        .map(|path| {
            let verb = if report.removed.contains(path) {
                "removed"
            } else {
                "wrote"
            };
            format!("{verb} {}\n", path.as_str())
        })
        .collect()
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    fn args(baseline: bool, accept_new: bool, why: Option<&str>) -> SyncArgs {
        SyncArgs {
            dry_run: false,
            yes: false,
            baseline,
            accept_new,
            why: why.map(str::to_string),
            accept_vendor: false,
            force: false,
            scope: None,
            purge: None,
        }
    }

    #[test]
    fn r_cli_025__baseline_mode_follows_the_flags() {
        assert!(matches!(
            baseline_mode(&args(false, false, None)),
            Ok(BaselineMode::Off)
        ));
        assert!(matches!(
            baseline_mode(&args(true, false, None)),
            Ok(BaselineMode::Prune)
        ));
        assert!(matches!(
            baseline_mode(&args(true, true, Some("legacy"))),
            Ok(BaselineMode::AcceptNew { .. })
        ));
        assert!(baseline_mode(&args(true, true, None)).is_err());
    }
}
