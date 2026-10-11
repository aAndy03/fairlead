+++
id = "d-ptn7sp0m"
kind = "change"
status = "ready"
impact = "add"
scope = "root"
title = "Lifecycle and the graveyard"
covers = ["crates/fairlead-core/src/lifecycle.rs", "crates/fairlead-core/src/graveyard.rs", "crates/fairlead-core/src/check/builtin/plans.rs", "crates/fairlead-core/src/check/builtin/grave.rs", "crates/fairlead-core/src/config/vocab.rs"]
created = "0.2.0"
+++

# Lifecycle and the graveyard

Adds the lifecycle of items: environment and maturity vocabularies, remove, replace and retarget plans that come due at a seal, tombstones that reserve retired IDs, revival, and the graveyard of rejected approaches with fingerprints.
