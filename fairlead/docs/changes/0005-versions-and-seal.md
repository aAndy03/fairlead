+++
id = "d-mnyjssq7"
kind = "change"
status = "sealed"
impact = "add"
scope = "root"
title = "Versions and seal"
covers = ["crates/fairlead-core/src/seal.rs", "crates/fairlead-core/src/version.rs", "crates/fairlead-core/src/docs.rs", "crates/fairlead-core/src/hist.rs", "crates/fairlead-core/src/merkle.rs", "crates/fairlead/src/commands/seal.rs"]
created = "0.2.0"
sealed = "0.2.0"
body-hash = "b3:8ycmkpsj818rtfdfhbhp"
ordinal = 5
+++

# Versions and seal

Adds versions to a fairlead tree: change docs with draft IDs and ordinals, `fairlead seal` as one transaction (version file, ledger shards and checkpoints, rollups, alias maps, `scope.lock`), Merkle scope digests, `seal --reseal` for an unintegrated seal, `new --reid`, and the version-line checks (`H-VER-COLLISION`, `H-VER-GAP`, `H-SEAL-TAMPER`, `H-TX-INCOMPLETE`).
