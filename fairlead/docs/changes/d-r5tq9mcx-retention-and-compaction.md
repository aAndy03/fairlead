+++
id = "d-r5tq9mcx"
kind = "change"
status = "draft"
impact = "add"
scope = "root"
title = "Retention and compaction"
covers = ["crates/fairlead-core/src/**", "crates/fairlead/src/**"]
created = "0.2.3"
+++

# Retention and compaction

Old records stop piling up once the root declares `[retain]`. Change docs and requests now carry `sum`, `out` and `why` from creation; when a sealed or closed record has aged past its retention (8 versions and 90 days by default, set per kind, and a child scope may only keep records longer), the next `seal` replaces it with one summary line in the scope's ledger and removes the file (git keeps it). The seal refuses, listing every record, while a due record has no summary, and `check` warns earlier. Old summary lines later fold into quarterly count lines, and ordinals are never reused. A compacted record's ID still works: `explain` prints its summary and the commit that last held it, and the variation protocol still resolves it.

The ledger gained upkeep: `sync` repairs an open year after a union merge, `sync --purge <id>` erases a record's text but keeps the chain valid, a hand edit of a ledger line is now detected, a sealed record deleted by hand gets a tombstone, and `seal --compact-only` catches up without a new version. Merging or demoting a scope with sealed history now carries its ledger lines into the receiving scope.
