# P7: quality, exit and the v1 API freeze

> [60-roadmap.md](../../60-roadmap.md) 3.3a, 4.3, R-REL-034; [50-architecture.md](../../50-architecture.md) R-ARCH-003. Decision D-53.

## Tasks

**T1. Hierarchy goldens.** Exit criterion 1, structural part. Add three golden repositories:
- `hier-ui`: the UI ladder from atoms to pages, with a rank violation, a container cycle and a visibility error.
- `hier-core`: the backend ladder with Cargo edges, an entry-only shell and a stale engine.
- `hier-tauri`: the "sync with server" worked example of 30-hierarchy, with bridges.

The expected outputs are reviewed by the owner before the first bless, as in 0.1.

**T2. Five-year retention simulation.** Exit criterion 2:
- `cargo xtask retention-sim --years 5 --seals-per-year 12 --seed N` generates records, seals and closures in a temp repository.
- It runs the history twice, on Linux and Windows in CI, and compares the bytes of every ledger file.
- It asserts that a second `seal --compact-only` is a no-op.
- It measures the leaf `context` size at years 1 and 5; the two must be equal within the budget.

**T3. Portability scenarios.** Exit criterion 3: goldens for cut, copy, mount, submodule and vendored subtree. Each uses real `git` in temp repositories, two repositories where needed. Assert that attach never guesses ranks: a missing `requires` is `UNSATISFIED`, never inferred.

**T4. Variation cycle.** Exit criterion 4: a scripted test runs the whole cycle, then asserts every consumer reference migrated and no `var.fork-suspect`.
1. A consumer files a request.
2. An interim wrapper with `awaiting`.
3. A home answer with `type = variant` and an approval.
4. Seal.
5. The consumer migrates.
6. The wrapper is removed.
7. The request is `closed`.

**T5. Bridge detection.** Exit criterion 5: in `hier-tauri`, deleting the TS end, deleting the Rust end and editing the contract file each produce the expected finding.

**T6. Regression.** Exit criterion 6: every 0.1 and 0.2 suite passes unchanged, including the edition byte-stability fixtures. `PENDING_0_3` is empty.

**T7. v1 API freeze.** D-53, R-ARCH-003:
- Write `docs/v1-api.md` listing the frozen surface:
  - CLI verbs and documented flags;
  - exit codes;
  - the directive grammar per edition;
  - the JSON and SARIF output shapes;
  - generated-file formats;
  - diagnostic and rule IDs.
- Add `tests/v1_api.rs`, which snapshots `--help` of every verb, the JSON schema of `check --format json` and `context --format json`, and the code table. Any change fails until the snapshot is updated together with an edition or a recorded gap decision.
- Open `docs/plans/1.0-gaps.md` for the gap list of 0.4 to 0.9.

**T8. Human steps.**

| Step | Rule |
|---|---|
| Blind review of hierarchy findings on three repositories (at most 2% false) | 4.3 criterion 1 |
| Label the BM25 query set (about 100 prompts) | R-CTX-063 |
| Decide edition 2 for 0.3 (master Q-1), tree-sitter (Q-2), MCP (Q-3) | owner questions |
| Review `docs/v1-api.md` before tagging `v0.3.0` | D-53 |
| Track 1.0 preconditions: edition 1 stable for two minors, one external repository for six months | R-REL-034 |
