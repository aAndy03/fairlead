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
    if args.abort {
        let session = WriteSession::open_abort(&discover_request(global, true)?, &SysProbe)?;
        return Ok(CommandResult::text(match session.recovery {
            fairlead_core::write::Recovery::Clean => "nothing to abort\n".to_owned(),
            _ => "interrupted seal rolled back\n".to_owned(),
        }));
    }
    let session = WriteSession::open_allocating(&discover_request(global, true)?, &SysProbe)?;
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let options = default_options(as_of(global));
    let mut request = SealRequest {
        allow_dirty: args.allow_dirty,
        seal_anywhere: args.anywhere,
        also_sealed: Vec::new(),
        own_outputs: Default::default(),
        also_docs: Vec::new(),
    };
    let mut loaded = loaded;
    let mut registry = registry;
    let mut unsealed = String::new();
    if args.reseal {
        // R-GIT-031: undo the own seal, then seal again after the integration line's numbers.
        let fairlead_core::pipeline::Unseal {
            plan: undo,
            own,
            theirs,
            their_docs,
        } = fairlead_core::pipeline::plan_unseal(&loaded)?;
        // 0.2.2-T4: the tag of the undone seal is left in place (deleting a tag is the owner's
        // call); the field test found it pointing at the undone seal.
        let tag_note = if GitRepo::tag_exists(&loaded.root, &own.to_string()) {
            format!(
                "note: tag fairlead/{own} still points at the undone seal; delete it with: git tag -d fairlead/{own}   (and on a remote that has it: git push --delete origin fairlead/{own})\n"
            )
        } else {
            String::new()
        };
        if !args.yes || args.dry_run {
            let mut r = CommandResult::text(format!(
                "reseal: {own} is unintegrated; it is renumbered after {}\n{}{tag_note}not applied; re-run with --yes\n",
                theirs
                    .iter()
                    .max()
                    .map_or_else(|| "none".to_owned(), |v| v.to_string()),
                undo.render_text("unseal plan")
            ));
            if !args.dry_run {
                r.outcome.signal(ExitCode::Refused);
            }
            return Ok(r);
        }
        let undone = commit_plan(&session, undo)?;
        request.own_outputs = undone.report.written.iter().cloned().collect();
        unsealed = format!("unsealed {own}\n{tag_note}");
        loaded = load(&load_request(global, false, None)?)?;
        registry = resolve_loaded(&loaded, as_of(global));
        request.also_sealed = theirs;
        request.also_docs = their_docs;
        request.seal_anywhere = true;
    }
    let (seal_plan, plan) = plan_seal(&loaded, &registry, &options, &request)?;
    let mut head = format!(
        "{unsealed}bump: {} -> {} ({} docs, {} carried)\n",
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
