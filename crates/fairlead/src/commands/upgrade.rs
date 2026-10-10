//! `fairlead upgrade` (R-CLI-042, R-EDN-030 to R-EDN-039). Plan-only without `--yes`. With
//! `--yes`: the first run writes only the draft upgrade doc; once that doc is ready and approved
//! for the same plan hash, the edits are applied with the control file last, then `check` runs.

use fairlead_core::error::Error;
use fairlead_core::outcome::ExitCode;
use fairlead_core::pipeline::{
    UpgradeGate, WriteSession, check_loaded, commit_plan, load, plan_rename, plan_upgrade,
    resolve_loaded, upgrade_gate,
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
        Some(WriteSession::open_allocating(&discover, &SysProbe)?)
    } else {
        None
    };
    let mut request = load_request(global, false, None)?;
    request.discover.upgrade = true;
    let loaded = load(&request)?;
    let to = current_edition();
    let rename = args.rename_marker.is_some() || args.rename_folder.is_some();
    if loaded.root.edition == to && !rename {
        return Ok(CommandResult::text(format!(
            "already on edition {}; nothing to upgrade\n",
            to.0
        )));
    }
    let registry = resolve_loaded(&loaded, as_of(global));
    let options = default_options(as_of(global));
    let out = if rename {
        if loaded.root.edition != to {
            return Err(Error::Usage(
                "upgrade the edition first; a rename runs on the current edition".into(),
            ));
        }
        plan_rename(
            &loaded,
            args.rename_marker.as_deref(),
            args.rename_folder.as_deref(),
        )?
    } else {
        plan_upgrade(&loaded, &registry, &options, to)?
    };
    let mut head = if rename {
        format!(
            "upgrade: rename, plan {} ({} edits)\n",
            out.hash,
            out.edits.len()
        )
    } else {
        format!(
            "upgrade: edition {} -> {}, plan {} ({} edits)\n",
            out.from.0,
            out.to.0,
            out.hash,
            out.edits.len()
        )
    };
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
            // A renamed folder is no longer the default one the upward search probes: reload
            // from the known root directory.
            let mut again = load_request(global, false, None)?;
            again.discover.root_flag = Some(loaded.root.dir.clone());
            let after = load(&again)?;
            let reg = resolve_loaded(&after, as_of(global));
            let report = check_loaded(&after, &reg, &default_options(as_of(global)))?;
            let mut result = CommandResult::text(format!(
                "{head}{}{} edition {}; check: {} errors, {} warnings\n",
                changed_list(&outcome.report),
                if rename { "renamed on" } else { "upgraded to" },
                out.to.0,
                report.summary.errors,
                report.summary.warns
            ));
            result.outcome = report.outcome(&default_options(as_of(global)));
            Ok(result)
        }
    }
}
