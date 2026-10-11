+++
id = "d-a23srma6"
kind = "change"
status = "draft"
impact = "add"
scope = "root"
title = "Hierarchy: entries, reachability and bridges"
covers = ["crates/fairlead-core/src/**", "crates/fairlead/src/**"]
created = "0.2.3"
+++

# Hierarchy: entries, reachability and bridges

Hierarchy level 3. Entries (`etype=`, `unit=`, `reaches=`) start reachability; tests, stories and benchmarks are automatic entries. A unit no entry reaches is stale; at level 3 every seal records each unit's digest, so a unit unchanged for `[hier] dormant-seals` seals is dormant, and stale plus dormant is forgotten. The seal records also drive two more suggestions: merging a container that has held one unit for several seals, and demoting a unit with one consumer.

TypeScript-to-Rust bridges (`def brg.* proto= wire= ver= ts= rs=`) are checked by text: each end's marker and symbol, the wire name on both sides, the version on end markers and on consumers' `pin=`, a dynamic `invoke(name)` reported as unverifiable, contract changes that need a change doc, and generated bindings not regenerated after one. `explain` shows an entry's expected chain, and says what a bridge cannot verify. Change docs may now cover work by ID or scope, a new unit or bridge needs a doc that lists it, and `fairlead new change <title> --from-diff` writes a rolling chore doc for small changes. `seal --commit` no longer fails when a draft doc was sealed before its first commit.
