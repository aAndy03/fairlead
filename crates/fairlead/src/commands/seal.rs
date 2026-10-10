//! `fairlead seal` (R-CLI-041, R-VER-050). Plan-only without `--yes` (R-CLI-002).

use fairlead_core::error::Error;
use fairlead_core::git::GitRepo;
use fairlead_core::outcome::ExitCode;
use fairlead_core::pipeline::{
    SealRequest, WriteSession, commit_plan, load, plan_seal, resolve_loaded,
};
use fairlead_core::write::SysProbe;

use crate::cli::{GlobalArgs, SealArgs};
use crate::commands::check::default_options;
use crate::commands::sync::changed_list;
use crate::commands::{CommandResult, as_of, discover_request, load_request};

pub fn run(global: &GlobalArgs, args: &SealArgs) -> CommandResult {
    match seal(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn seal(global: &GlobalArgs, args: &SealArgs) -> Result<CommandResult, Error> {
    let session = WriteSession::open(&discover_request(global, true)?, &SysProbe)?;
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let options = default_options(as_of(global));
    let request = SealRequest {
        allow_dirty: args.allow_dirty,
        seal_anywhere: args.anywhere,
        integration_branch: "main".to_owned(),
    };
    let (seal_plan, plan) = plan_seal(&loaded, &registry, &options, &request)?;
    let mut head = format!(
        "bump: {} -> {} ({} docs, {} carried)\n",
        seal_plan
            .prev
            .map_or_else(|| "none".to_owned(), |v| v.to_string()),
        seal_plan.open,
        seal_plan.sealing.len(),
        seal_plan.carried.len()
    );
    for s in &seal_plan.sweep {
        head.push_str(&format!("sweep: {s}\n"));
    }
    if !args.yes || args.dry_run {
        let mut result = CommandResult::text(format!(
            "{head}{}not applied; re-run with --yes to seal\n",
            plan.render_text("seal plan")
        ));
        if !args.dry_run {
            result.outcome.signal(ExitCode::Refused);
        }
        return Ok(result);
    }
    let outcome = commit_plan(&session, plan)?;
    let version = seal_plan.open.to_string();
    let tail = if args.commit {
        GitRepo::commit_paths(&loaded.root, &outcome.report.written, &version)?;
        if args.tag {
            GitRepo::tag(&loaded.root, &version)?;
            format!("sealed {version}; committed and tagged fairlead/{version}\n")
        } else {
            format!("sealed {version}; committed\n")
        }
    } else {
        format!("sealed {version}; run: git add -A && git commit   (or re-run with --commit)\n")
    };
    Ok(CommandResult::text(format!(
        "{head}{}{tail}",
        changed_list(&outcome.report)
    )))
}
