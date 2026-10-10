//! `fairlead new` (R-CLI-028 to R-CLI-031). Never overwrites (the planner refuses, exit 4); applies directly (R-CLI-043).

use std::collections::BTreeMap;

use fairlead_core::error::Error;
use fairlead_core::path::RepoPath;
use fairlead_core::pipeline::{WriteSession, commit_plan, load, plan_new, resolve_loaded};
use fairlead_core::write::{NewRequest, SysProbe};

use crate::cli::{GlobalArgs, NewArgs};
use crate::commands::context::scope_pin;
use crate::commands::sync::changed_list;
use crate::commands::{CommandResult, as_of, discover_request, load_request};

pub fn run(global: &GlobalArgs, args: &NewArgs) -> CommandResult {
    match new(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn new(global: &GlobalArgs, args: &NewArgs) -> Result<CommandResult, Error> {
    let session = WriteSession::open(&discover_request(global, true)?, &SysProbe)?;
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let request = NewRequest {
        kind: args.kind.clone(),
        name: args.name.clone(),
        scope: scope_pin(args.scope.as_deref())?,
        fields: {
            let mut f = parse_fields(&args.fields)?;
            if args.hotfix {
                f.insert("hotfix".to_owned(), "true".to_owned());
            }
            f
        },
        print: args.print,
        at: optional_path(args.at.as_deref())?,
        hoist: args.hoist,
    };
    let plan = plan_new(&loaded, &registry, &request, as_of(global))?;
    if args.print {
        // The planner puts the scaffold text in `notes` for `--print` and writes nothing (Contract issues CI-8).
        return Ok(CommandResult::text(plan.notes.join("\n")));
    }
    if args.dry_run {
        return Ok(CommandResult::text(plan.render_text("new plan")));
    }
    let outcome = commit_plan(&session, plan)?;
    Ok(CommandResult::text(changed_list(&outcome.report)))
}

/// `--field K=V`, repeatable. The first `=` splits; a repeated key is a usage error.
pub fn parse_fields(items: &[String]) -> Result<BTreeMap<String, String>, Error> {
    let mut fields = BTreeMap::new();
    for item in items {
        let (key, value) = item
            .split_once('=')
            .filter(|(key, _)| !key.is_empty())
            .ok_or_else(|| Error::Usage(format!("--field takes K=V, got {item:?}")))?;
        if fields.insert(key.to_string(), value.to_string()).is_some() {
            return Err(Error::Usage(format!("--field {key} is given twice")));
        }
    }
    Ok(fields)
}

fn optional_path(text: Option<&str>) -> Result<Option<RepoPath>, Error> {
    match text {
        None => Ok(None),
        Some(path) => RepoPath::new(path)
            .map(Some)
            .map_err(|_| Error::Usage(format!("--at takes a repo-relative folder, got {path:?}"))),
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_cli_028__fields_are_k_equals_v_and_repeats_are_refused() {
        let fields =
            parse_fields(&["title=Progress bar".to_string(), "expr=a=b".to_string()]).unwrap();
        assert_eq!(
            fields.get("title").map(String::as_str),
            Some("Progress bar")
        );
        assert_eq!(fields.get("expr").map(String::as_str), Some("a=b"));
        assert!(parse_fields(&["no-equals".to_string()]).is_err());
        assert!(parse_fields(&["k=1".to_string(), "k=2".to_string()]).is_err());
    }
}
