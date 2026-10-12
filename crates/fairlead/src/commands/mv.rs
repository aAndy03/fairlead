//! `fairlead move` (R-CLI-040, R-CLI-044, R-CLI-047). Plan-only without `--yes`; `--dry-run`
//! prints the plan and exits 0. Applying writes the move, then regenerates the generated files.

use fairlead_core::error::Error;
use fairlead_core::outcome::ExitCode;
use fairlead_core::pipeline::{
    MoveRequest, ScopeOp, WriteSession, commit_plan, load, merge_gate, move_gate, names_containers,
    plan_detach, plan_merge, plan_merge_containers, plan_move, plan_scope_op, plan_sync,
    resolve_loaded, to_root_relative,
};
use fairlead_core::write::{BaselineMode, SyncRequest, SysProbe};

use crate::cli::{GlobalArgs, MoveArgs};
use crate::commands::check::default_options;
use crate::commands::sync::changed_list;
use crate::commands::{CommandResult, as_of, discover_request, load_request, working_dir};

pub fn run(global: &GlobalArgs, args: &MoveArgs) -> CommandResult {
    match mv(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn mv(global: &GlobalArgs, args: &MoveArgs) -> Result<CommandResult, Error> {
    let session = if args.yes && !args.dry_run {
        Some(WriteSession::open_allocating(
            &discover_request(global, true)?,
            &SysProbe,
        )?)
    } else {
        None
    };
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let cwd = working_dir(global)?;
    let src = to_root_relative(&loaded.root, &cwd, &args.src)?;
    let scope_op = if args.delete {
        Some(ScopeOp::Delete { force: args.force })
    } else if args.demote {
        Some(ScopeOp::Demote)
    } else {
        None
    };
    let out = match scope_op {
        None if args.adopt => {
            if args.dst.is_some() {
                return Err(Error::Usage(
                    "--adopt takes the mount's scope id and no DST".into(),
                ));
            }
            fairlead_core::pipeline::plan_adopt(&loaded, &registry, &args.src)?
        }
        None if args.attach => {
            if args.dst.is_some() {
                return Err(Error::Usage(
                    "--attach takes the arrived scope's directory and no DST".into(),
                ));
            }
            fairlead_core::pipeline::plan_attach(&loaded, &registry, &src)?
        }
        None if args.detach => {
            if args.dst.is_some() {
                return Err(Error::Usage("--detach takes no DST".into()));
            }
            plan_detach(&loaded, &registry, &src)?
        }
        Some(op) => {
            if args.dst.is_some() {
                return Err(Error::Usage("--delete and --demote take no DST".into()));
            }
            plan_scope_op(&loaded, &registry, &src, op)?
        }
        None => {
            let Some(dst) = &args.dst else {
                return Err(Error::Usage("move SRC DST: DST is missing".into()));
            };
            // R-HIER-100: `--merge` of two container IDs is a hierarchy refactor, not a path merge.
            if args.merge && names_containers(&registry, &args.src, dst) {
                plan_merge_containers(&loaded, &registry, &args.src, dst)?
            } else {
                let dst = to_root_relative(&loaded.root, &cwd, dst)?;
                if args.merge {
                    plan_merge(&loaded, &registry, &src, &dst)?
                } else if args.copy {
                    fairlead_core::pipeline::plan_copy(&loaded, &registry, &src, &dst)?
                } else if args.cut {
                    fairlead_core::pipeline::plan_cut(
                        &loaded,
                        &registry,
                        &MoveRequest {
                            src,
                            dst,
                            allow_dirty: args.allow_dirty,
                        },
                    )?
                } else {
                    plan_move(
                        &loaded,
                        &registry,
                        &MoveRequest {
                            src,
                            dst,
                            allow_dirty: args.allow_dirty,
                        },
                    )?
                }
            }
        }
    };
    let mut head = format!(
        "move: plan {} ({} files)\n",
        out.hash,
        out.plan.set.writes.len()
    );
    head.push_str("obligations:\n");
    if out.obligations.is_empty() {
        head.push_str("  none\n");
    }
    for o in &out.obligations {
        head.push_str(&format!("  {o}\n"));
    }
    for c in &out.code_sites {
        head.push_str(&format!("{c}\n"));
    }
    let Some(session) = session else {
        let mut result = CommandResult::text(format!(
            "{head}{}{}",
            out.plan.render_text("move plan"),
            if args.dry_run {
                ""
            } else {
                "not applied; re-run with --yes\n"
            }
        ));
        if !args.dry_run {
            result.outcome.signal(ExitCode::Refused);
        }
        return Ok(result);
    };
    merge_gate(&loaded, "move", false)?;
    if args.adopt {
        fairlead_core::pipeline::adopt_gate(&out)?;
    }
    move_gate(&loaded, &out, args.allow_dirty)?;
    let moved = commit_plan(&session, out.plan)?;
    let after = load(&load_request(global, false, None)?)?;
    let reg = resolve_loaded(&after, as_of(global));
    let sync = plan_sync(
        &after,
        &reg,
        &default_options(as_of(global)),
        &SyncRequest {
            baseline: BaselineMode::Off,
            accept_vendor: false,
            force: false,
            scope: None,
        },
    )?;
    let synced = commit_plan(&session, sync)?;
    Ok(CommandResult::text(format!(
        "{head}{}{}code sites are listed, never edited; run fairlead check\n",
        changed_list(&moved.report),
        if synced.report.written.is_empty() {
            String::new()
        } else {
            changed_list(&synced.report)
        }
    )))
}
