//! The command handlers of contract/commands.md.
// fairlead: def ctr.backend path="backend/**"
// fairlead: def un.commands layer=app.command container=ctr.backend sum="the backend's command handlers"
use std::collections::BTreeMap;

use crate::notes::Store;
use crate::wire::encode;

fn arg<'a>(args: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    args.get(key).map(String::as_str).ok_or_else(|| format!("missing-arg-{key}"))
}

fn id_arg(args: &BTreeMap<String, String>) -> Result<u64, String> {
    arg(args, "id")?.parse().map_err(|_| "bad-id".to_owned())
}

/// Runs one command. A payload is percent-encoded `key=value` fields joined by `&`; a list joins
/// its records with `;`.
pub fn handle(store: &mut Store, cmd: &str, args: &BTreeMap<String, String>) -> Result<String, String> {
    match cmd {
        // fairlead: def brg.notes-list side=rs ver=1
        "notes.list" => Ok(store
            .list()
            .iter()
            .map(|n| format!("id={}&title={}", n.id, encode(&n.title)))
            .collect::<Vec<_>>()
            .join(";")),
        // fairlead: def brg.notes-get side=rs ver=1
        "notes.get" => {
            let note = store.get(id_arg(args)?).ok_or("not-found")?;
            Ok(format!("id={}&title={}&body={}", note.id, encode(&note.title), encode(&note.body)))
        }
        // fairlead: def brg.notes-create side=rs ver=1
        "notes.create" => {
            let note = store.create(arg(args, "title")?, args.get("body").map_or("", String::as_str))?;
            Ok(format!("id={}", note.id))
        }
        // fairlead: def brg.notes-delete side=rs ver=1
        "notes.delete" => {
            if store.delete(id_arg(args)?) {
                Ok(String::new())
            } else {
                Err("not-found".into())
            }
        }
        other => Err(format!("unknown-command-{other}")),
    }
}
