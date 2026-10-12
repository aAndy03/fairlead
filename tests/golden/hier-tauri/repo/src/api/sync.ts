// fairlead: def brg.sync-run side=ts ver=1
import { invoke } from '@tauri-apps/api/core';
export function syncNow() {
  return invoke('sync_run');
}
