+++
id = "d-v45h978a"
kind = "change"
status = "sealed"
impact = "fix"
scope = "root"
title = "a removed rule ID cannot be declared again"
covers = ["crates/fairlead-core/src/write/new.rs", "crates/fairlead-core/src/check/builtin/refs.rs"]
created = "0.2.2"
sealed = "0.2.2"
body-hash = "b3:8n0f1yp6bq7rd5rec5bk"
ordinal = 22
+++

# a removed rule ID cannot be declared again

A removed ID is reserved forever. `fairlead new rule` now refuses a name that has a tombstone, and `fairlead check` reports a `[rules.X]` or `[[file]]` table that reuses a tombstoned ID as `HN-D003`, as it already did for a `def` in code or docs.
