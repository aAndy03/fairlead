+++
directives = ['def brg.sync-run proto=tauri-command dir="ts->rs" wire=sync_run ver=1 params=req ts="src/api/sync.ts#syncNow" rs="src-tauri/src/commands.rs#sync_run" reaches=un.sync-cmd contract="docs/sync-contract.md"']
+++
# Bridges

The sync button calls the `sync_run` Tauri command.
