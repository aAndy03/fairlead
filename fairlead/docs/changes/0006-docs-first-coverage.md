+++
id = "d-pskjh5xj"
kind = "change"
status = "sealed"
impact = "add"
scope = "root"
title = "Docs-first coverage"
covers = ["crates/fairlead-core/src/check/builtin/coverage.rs", "crates/fairlead-core/src/check/builtin/versions.rs", "crates/fairlead/src/commands/check.rs", "crates/fairlead/src/commands/new.rs"]
created = "0.2.0"
sealed = "0.2.0"
body-hash = "b3:ep8z77wcjensaxgkdy1z"
ordinal = 6
+++

# Docs-first coverage

Adds docs-first coverage: every file changed since the last seal must be covered by an open change doc (`HN-D006`), with exempt globs, hotfix docs and their backfill deadline, `check --base`, `--staged`, `--since` and `--against`, and `FAIRLEAD_SKIP` outside CI.
