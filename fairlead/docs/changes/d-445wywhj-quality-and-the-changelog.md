+++
id = "d-445wywhj"
kind = "change"
status = "ready"
impact = "add"
scope = "root"
title = "Quality and the changelog"
covers = ["tests/**", "xtask/**", ".github/workflows/**", "crates/fairlead-core/src/generate/**", "crates/fairlead-core/src/classify.rs"]
created = "0.2.0"
+++

# Quality and the changelog

Adds the 0.2 quality evidence: six git-case goldens, the merge simulator, edition upgrades of every golden with an interrupted apply, the coverage replay, the Merkle and ledger benchmark with a cache fix for generated and ledger files, and the `[changelog]` aggregate this file list is generated into.
