// fairlead: def un.sync-cmd layer=core.command container=ctr.cmds
// fairlead: use un.engine
// fairlead: def brg.sync-run side=rs ver=1
#[tauri::command]
pub fn sync_run() {}
