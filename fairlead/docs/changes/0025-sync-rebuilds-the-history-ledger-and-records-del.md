+++
id = "d-dnfnp0bq"
kind = "change"
status = "sealed"
impact = "break"
sum = "sync now rebuilds an out-of-order open ledger year, purges records on request and tombstones deleted sealed docs"
out = "shipped"
why = "merged branches left a ledger that only seal --reseal repaired, and hand edits to history were silently kept"
scope = "root"
title = "sync rebuilds the history ledger and records deleted docs"
covers = ["crates/fairlead-core/src/hist.rs", "crates/fairlead-core/src/write/sync.rs"]
created = "0.2.3"
sealed = "0.3.0"
body-hash = "b3:mnavgbwth6ccqh9kmxby"
ordinal = 25
+++

# sync rebuilds the history ledger and records deleted docs

`sync` now writes to the history ledger, which in 0.2 it never touched. When the open year of `ledger/hist/` has records out of order or checkpoints that no longer add up (two branches' seals merged with `merge=union`), `sync` rebuilds that year in order; a closed year is never rewritten. After a rebase, `check` is therefore clean before `seal --reseal`, which still renumbers the version. A record is removed from history only with `sync --purge <draft>`, which keeps its digest; editing a ledger line by hand is reported, not kept. When a sealed change doc's file is deleted by hand, `sync` adds a `[[tomb]]` with `deleted-by = "user"`. Scripts that diffed the ledger after `sync`, or relied on editing it in place, need updating.
