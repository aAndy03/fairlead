// fairlead: def ctr.ui path="src/**"
// fairlead: def un.sync-button layer=app.view container=ctr.ui
// fairlead: use brg.sync-run
// fairlead: def ent.click etype=user-action unit=un.sync-button
import { syncNow } from '../api/sync';
export function SyncButton() {
  return syncNow;
}
