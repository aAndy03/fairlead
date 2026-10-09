//! `fairlead approve` (R-CLI-037, R-CLI-039, R-CLI-045). A human records an approval at a terminal. Its own verb, never a `new` target.

use std::io::IsTerminal;

use fairlead_core::error::{Error, Refusal, RefusalKind};
use fairlead_core::hash::Fingerprint;
use fairlead_core::pipeline::{
    WriteSession, approve_preview_loaded, commit_plan, load, plan_approve_loaded, resolve_loaded,
};
use fairlead_core::write::{ApproveRequest, SysProbe};

use crate::cli::{ApproveArgs, GlobalArgs};
use crate::commands::sync::changed_list;
use crate::commands::{CommandResult, as_of, discover_request, load_request};
use crate::output::write_text;

pub fn run(global: &GlobalArgs, args: &ApproveArgs) -> CommandResult {
    match approve(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn approve(global: &GlobalArgs, args: &ApproveArgs) -> Result<CommandResult, Error> {
    let session = WriteSession::open(&discover_request(global, true)?, &SysProbe)?;
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    if args.list {
        // Listing is read-only and needs no terminal (R-CLI-037).
        return Ok(CommandResult::text(list_text(&loaded)));
    }
    #[allow(clippy::disallowed_methods)]
    // reason: the bin reads the environment (P8 Global Constraint 9)
    let ci_set = std::env::var_os("CI").is_some_and(|value| !value.is_empty());
    terminal_gate(
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
        ci_set,
    )?;
    let request = ApproveRequest {
        target: args.target.clone().unwrap_or_default(),
        as_handle: args.as_handle.clone(),
        list: false,
        revoke: args
            .revoke
            .as_deref()
            .map(|t| {
                Fingerprint::parse(t).map_err(|_| {
                    Error::Usage(format!("--revoke: {t:?} is not a 12-character fingerprint"))
                })
            })
            .transpose()?,
    };
    let preview = approve_preview_loaded(&loaded, &registry, &request)?;
    write_text(&confirm_prompt(&preview), "").map_err(|e| Error::Internal(e.to_string()))?;
    let mut typed = String::new();
    std::io::stdin()
        .read_line(&mut typed)
        .map_err(|e| Error::Internal(e.to_string()))?;
    if !retype_matches(&typed, &preview.confirm_prefix) {
        return Err(Error::Refused(Refusal {
            kind: RefusalKind::PlanNotConfirmed,
            message: "the confirmation did not match; nothing was recorded".to_string(),
            hint: "retype the first 4 characters of the fingerprint exactly".to_string(),
        }));
    }
    let approver = args
        .as_handle
        .clone()
        .ok_or_else(|| Error::Usage("approve needs --as @handle".to_string()))?;
    let plan = plan_approve_loaded(&loaded, &preview, &approver, as_of(global))?;
    let outcome = commit_plan(&session, plan)?;
    Ok(CommandResult::text(changed_list(&outcome.report)))
}

/// R-CLI-037: refused (exit 4) when stdin or stdout is not a TTY, or when `CI` is set, so an unattended agent cannot self-approve.
pub fn terminal_gate(stdin_tty: bool, stdout_tty: bool, ci_set: bool) -> Result<(), Error> {
    if ci_set {
        return Err(Error::Refused(Refusal {
            kind: RefusalKind::CiSet,
            message: "approve is refused when CI is set".to_string(),
            hint: "approve at a terminal, outside CI".to_string(),
        }));
    }
    if !(stdin_tty && stdout_tty) {
        return Err(Error::Refused(Refusal {
            kind: RefusalKind::NotATty,
            message: "approve needs a terminal on stdin and stdout".to_string(),
            hint: "run fairlead approve in an interactive terminal".to_string(),
        }));
    }
    Ok(())
}

/// The ovr line, its fingerprint, scope and expiry, then the retype question (R-CLI-037).
fn confirm_prompt(preview: &fairlead_core::write::ApprovePreview) -> String {
    let exp = preview
        .exp
        .map_or_else(|| "none".to_string(), |d| d.to_string());
    format!(
        "{}\nfingerprint: {}\nscope: {}\nexpires: {}\nType the first 4 characters of the fingerprint to confirm: ",
        preview.ovr_line, preview.fp, preview.scope.0, exp
    )
}

/// The reviewer's typed confirmation must equal the preview's 4-character prefix (surrounding whitespace ignored).
pub fn retype_matches(typed: &str, prefix: &str) -> bool {
    typed.trim() == prefix
}

fn list_text(loaded: &fairlead_core::pipeline::Loaded) -> String {
    loaded
        .facts
        .approvals
        .approvals
        .iter()
        .map(|a| format!("{} {} {} {}\n", a.fp, a.target, a.approver, a.date))
        .collect()
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_cli_037__the_terminal_gate_refuses_a_pipe_and_ci() {
        assert!(matches!(
            terminal_gate(false, true, false).err(),
            Some(Error::Refused(_))
        ));
        assert!(matches!(
            terminal_gate(true, false, false).err(),
            Some(Error::Refused(_))
        ));
        assert!(matches!(
            terminal_gate(true, true, true).err(),
            Some(Error::Refused(_))
        ));
        assert!(terminal_gate(true, true, false).is_ok());
    }

    #[test]
    fn r_cli_037__the_reviewer_retypes_the_first_four_characters() {
        assert!(retype_matches("ab12", "ab12"));
        assert!(retype_matches(" ab12 \n", "ab12"));
        assert!(!retype_matches("ab1", "ab12"));
        assert!(!retype_matches("", "ab12"));
    }
}
