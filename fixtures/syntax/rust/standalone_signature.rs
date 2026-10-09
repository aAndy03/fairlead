use std::io;

// fairlead: def cmd.sync-now sym=sync_now
#[tauri::command]
pub async fn sync_now(
    id: u32,
) -> Result<(), Error>
{
    run();
}

// fairlead: ovr rule.no-unwrap why="startup invariant" owner=@platform exp=2026-12-31
let cfg = load().unwrap();
