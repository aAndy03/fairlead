//! `fairlead upgrade` (R-CLI-042, R-EDN-030 to R-EDN-039). Plan-only without `--yes`. With
//! `--yes`: the first run writes only the draft upgrade doc; once that doc is ready and approved
//! for the same plan hash, the edits are applied with the control file last, then `check` runs.

use fairlead_core::error::Error;
use fairlead_core::outcome::ExitCode;
use fairlead_core::pipeline::{
    UpgradeGate, WriteSession, check_loaded, commit_plan, load, plan_upgrade, resolve_loaded,
    upgrade_gate,
};
use fairlead_core::write::SysProbe;

use crate::cli::{GlobalArgs, UpgradeArgs};
use crate::commands::check::default_options;
use crate::commands::sync::changed_list;
use crate::commands::{CommandResult, as_of, current_edition, discover_request, load_request};

/// Q-2: no client and no published migration data in 0.2; every supported migration is embedded.
pub const NO_REMOTE_DATA: &str = "upgrade --fetch: no remote migration data is published for this binary; every supported migration is embedded (R-EDN-025)";

pub fn run(global: &GlobalArgs, args: &UpgradeArgs) -> CommandResult {
    match upgrade(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn upgrade(global: &GlobalArgs, args: &UpgradeArgs) -> Result<CommandResult, Error> {
    if args.fetch {
        return Err(Error::Usage(NO_REMOTE_DATA.into()));
    }
    if args.scope.is_some() {
        return Err(Error::Usage(
            "upgrade --scope needs a registered sealed sub-root; registration arrives with mounts (0.3)"
                .into(),
        ));
    }
    let mut discover = discover_request(global, true)?;
    discover.upgrade = true;
    let session = if args.yes {
        Some(WriteSession::open(&discover, &SysProbe)?)
    } else {
        None
    };
    let mut request = load_request(global, false, None)?;
    request.discover.upgrade = true;
    let loaded = load(&request)?;
    let to = current_edition();
    if loaded.root.edition == to {
        return Ok(CommandResult::text(format!(
            "already on edition {}; nothing to upgrade\n",
            to.0
        )));
    }
    let registry = resolve_loaded(&loaded, as_of(global));
    let options = default_options(as_of(global));
    let out = plan_upgrade(&loaded, &registry, &options, to)?;
    let mut head = format!(
        "upgrade: edition {} -> {}, plan {} ({} edits)\n",
        out.from.0,
        out.to.0,
        out.hash,
        out.edits.len()
    );
    for n in &out.needs {
        head.push_str(&format!(
            "needs-human: {} in {}: {}\n",
            n.field,
            n.path.as_str(),
            n.reason
        ));
    }
    let Some(session) = session else {
        let mut result = CommandResult::text(format!(
            "{head}{}not applied; re-run with --yes\n",
            out.plan.render_text("upgrade plan")
        ));
        result.outcome.signal(ExitCode::Refused);
        return Ok(result);
    };
    match upgrade_gate(&loaded, &out, args.allow_dirty)? {
        UpgradeGate::Draft { id, plan } => {
            let outcome = commit_plan(&session, plan)?;
            let mut result = CommandResult::text(format!(
                "{head}{}not applied: upgrade doc {id} awaits approval; set it ready, then a human runs `fairlead approve {id}`\n",
                changed_list(&outcome.report)
            ));
            result.outcome.signal(ExitCode::Refused);
            Ok(result)
        }
        UpgradeGate::Apply => {
            let outcome = commit_plan(&session, out.plan)?;
            drop(session);
            let after = load(&load_request(global, false, None)?)?;
            let reg = resolve_loaded(&after, as_of(global));
            let report = check_loaded(&after, &reg, &default_options(as_of(global)))?;
            let mut result = CommandResult::text(format!(
                "{head}{}upgraded to edition {}; check: {} errors, {} warnings\n",
                changed_list(&outcome.report),
                out.to.0,
                report.summary.errors,
                report.summary.warns
            ));
            result.outcome = report.outcome(&default_options(as_of(global)));
            Ok(result)
        }
    }
}
