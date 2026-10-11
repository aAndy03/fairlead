# P7: quality, exit and the v1 API freeze

> [60-roadmap.md](../../60-roadmap.md) 3.3a, 4.3, R-REL-034; [50-architecture.md](../../50-architecture.md) R-ARCH-003. Decision D-53.

## Tasks

**T1. Hierarchy goldens.** Exit criterion 1, structural part. Add three golden repositories:
- `hier-ui`: the UI ladder from atoms to pages, with a rank violation, a container cycle and a visibility error.
- `hier-core`: the backend ladder with Cargo edges, an entry-only shell and a stale engine.
- `hier-tauri`: the "sync with server" worked example of 30-hierarchy, with bridges.

Before the first bless, a reviewer agent reviews the expected outputs (agent protocol A-5 of [../agent-runbook.md](../agent-runbook.md)).

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

**T8. Agent-run evidence and owner acts (D-59).** Runners follow [../agent-runbook.md](../agent-runbook.md).

| Step | Rule |
|---|---|
| Agent protocol B-1: blind review of hierarchy findings on three repositories (at most 2% false) | 4.3 criterion 1 |
| Agent protocol B-2: BM25 query set, written and labelled by separate agents | R-CTX-063 |
| Agent protocol B-3: variation cycle played by a consumer agent | 4.3 criterion 4 |
| Agent protocol B-4: review of `docs/v1-api.md` before the freeze | D-53 |
| Agent protocols B-5 and B-6: adoption and syntax-card regressions | E8 (agent-run) |
| Defaults for master Q-1 to Q-3 apply unless the owner overrides them; record which in the errata | owner questions |
| Owner: `fairlead approve` where a protocol needs it, and tagging `v0.3.0` | R-AGT-026 |
| 1.0 preconditions stay time-based: edition 1 stable for two minors, one external repository for six months | R-REL-034 |

**T9. Names reserved for the 0.4 content model (D-63).** Added 2026-10-11. The v1 freeze adds nothing within edition 1, so the names 0.4 needs are reserved before the freeze. Each is recognised and refused with `<name> arrives in 0.4` (exit 2), like the reserved verbs of R-CLI-001, and `docs/v1-api.md` lists them as reserved:
- verbs `map` (the object graph of a scope or the tree) and `config` (governed config editing of D-62: re-glob or retire a rule, raise a pin, add a table, each as an approvable plan);
- the built-in kind `const` (a declared constant, bound to the symbol below the directive);
- rule check types `check = "query"` (a rule over content objects) and `check = "require"` (a positive check: something must exist);
- root control tables `[content]` (languages to parse, object limits, cache) and `[agent-mode]` (scopes, version range, on or off);
- the approval record value `approver = "agent-mode"`.

A test asserts each reserved name gives the "arrives in 0.4" message and changes nothing on disk.
