//! `fairlead init` (R-CLI-012, R-CLI-002, R-CLI-043): plan first. Writes only when `--yes` is given.

use fairlead_core::error::Error;
use fairlead_core::outcome::{ExitCode, VerbOutcome};
use fairlead_core::pipeline::{
    WriteSession, commit_plan, init_first_scan, load, plan_sync, resolve_loaded,
};
use fairlead_core::write::{
    BaselineMode, InitRequest, Plan, Preset, SyncRequest, SysProbe, detect_init,
    init_control_bytes, init_plan, init_root,
};

use crate::cli::{GlobalArgs, InitArgs, PresetArg};
use crate::commands::check::default_options;
use crate::commands::sync::changed_list;
use crate::commands::{CommandResult, as_of, discover_request, load_request};

const TITLE: &str = "init plan";

pub fn run(global: &GlobalArgs, args: &InitArgs) -> CommandResult {
    match init(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Gate {
    DryRun,
    Apply,
    NotApplied,
}

/// `--dry-run` prints the plan and exits 0; without `--yes` the plan is printed and nothing applied (exit 4).
pub fn gate(dry_run: bool, yes: bool) -> Gate {
    if dry_run {
        Gate::DryRun
    } else if yes {
        Gate::Apply
    } else {
        Gate::NotApplied
    }
}

fn init(global: &GlobalArgs, args: &InitArgs) -> Result<CommandResult, Error> {
    let req = discover_request(global, false)?;
    let dir = req.root_flag.clone().unwrap_or_else(|| req.cwd.clone());
    let request = InitRequest {
        preset: preset(args.preset),
        no_baseline: args.no_baseline,
        hooks: args.hooks,
        codeowners: args.codeowners,
        agents: args.agents.clone(),
        yes: args.yes,
    };
    let detected = detect_init(&dir, &request, req.binary_version.clone())?;
    // A-10: the scan sees the control file init writes, or the one already there.
    let control_bytes = detected
        .existing_control
        .clone()
        .unwrap_or_else(|| init_control_bytes(&request, &detected));
    let root = init_root(dir.clone(), &detected.folder);
    let options = default_options(as_of(global));
    let first_scan = init_first_scan(&root, &control_bytes, global.threads, &options)?;
    let plan = init_plan(&request, &detected, &first_scan)?;
    Ok(match gate(args.dry_run, args.yes) {
        Gate::DryRun => CommandResult::text(plan.render_text(TITLE)),
        Gate::NotApplied => not_applied(&plan),
        Gate::Apply => {
            let session = WriteSession::open_at(root, &SysProbe)?;
            let mut text = changed_list(&commit_plan(&session, plan)?.report);
            // E-27: the generated shards `sync` writes, so the first `check` after init only fails on
            // new findings (R-CLI-019).
            let mut global = global.clone();
            global.root = Some(dir);
            let loaded = load(&load_request(&global, false, None)?)?;
            let registry = resolve_loaded(&loaded, options.as_of);
            let sync = SyncRequest {
                baseline: BaselineMode::Off,
                accept_vendor: false,
                force: false,
                scope: None,
            };
            let generated = plan_sync(&loaded, &registry, &options, &sync)?;
            let written = commit_plan(&session, generated)?.report;
            if !written.written.is_empty() {
                text.push_str(&changed_list(&written));
            }
            CommandResult::text(text)
        }
    })
}

fn not_applied(plan: &Plan) -> CommandResult {
    let mut outcome = VerbOutcome::ok();
    outcome.signal(ExitCode::Refused);
    CommandResult {
        stdout: format!(
            "{}not applied; apply with: fairlead init --yes\n",
            plan.render_text(TITLE)
        ),
        stderr: String::new(),
        outcome,
    }
}

fn preset(arg: PresetArg) -> Preset {
    match arg {
        PresetArg::NoPreset => Preset::None,
        PresetArg::Rust => Preset::Rust,
        PresetArg::Ts => Preset::Ts,
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_cli_002__dry_run_wins_and_without_yes_nothing_is_applied() {
        assert_eq!(gate(true, true), Gate::DryRun);
        assert_eq!(gate(false, false), Gate::NotApplied);
        assert_eq!(gate(false, true), Gate::Apply);
    }

    #[test]
    fn r_cli_043__init_needs_yes_to_apply() {
        assert_ne!(gate(false, false), Gate::Apply);
    }
}
