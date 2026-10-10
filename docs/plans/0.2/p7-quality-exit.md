# P7: quality and the 0.2 exit

> [60-roadmap.md](../../60-roadmap.md) 4.2 (exit criteria), R-ROAD-012, R-ROAD-019, R-ROAD-020, R-REL-025; [22-editions-and-upgrade.md](../../22-editions-and-upgrade.md) R-EDN-050 to R-EDN-053; [23-git-integration.md](../../23-git-integration.md) case matrix; [20-versions-and-seal.md](../../20-versions-and-seal.md) R-VER-016 benchmark.

## Goal

Each 0.2 exit criterion has a test or a recorded human run. The 0.1 suites stay green, and fairlead's own repository uses seals, change docs and a generated changelog.

## Tasks

**T1. Git-case goldens.** Exit criterion 5, R-GIT-030 to R-GIT-036, R-GIT-040:
- New golden repositories, each driven by a scripted git history in its `golden.toml`. The 0.1 runner already supports `git` commands per case (A-38).
- Each repository runs the full verb set and blesses the expected output after review, as in 0.1.

| Golden | What it exercises |
|---|---|
| `shallow-clone` | depth 1, coverage unavailable, exit not 0 |
| `rebase-after-seal` | a seal on a branch, then a rebase onto a newer seal: `H-VER-COLLISION`, then `seal --reseal` |
| `squash-merge` | change docs survive a squash; coverage by file |
| `cherry-pick-seal` | a seal cherry-picked alone: `prev` continuity fails |
| `revert-seal` | a revert un-seals; `O` returns |
| `merge-conflict-gen` | conflict markers in `gen/`: `sync` regenerates; authored conflict refused |

**T2. Merge simulator.** Exit criteria 1 and 9:
- `cargo xtask merge-sim --branches 50 --seed N` builds a temp repository, creates 50 branches that each declare, seal (with `seal-where = "any"`) and edit, then merges them in seeded order with `merge=union` for ledgers.
- After each merge it runs `sync`, then `check --ci`.

Pass requires both:
- zero unresolved conflicts after regeneration, with every collision reported (never silent);
- checkpoint continuity passes or reports a repairable break.

Runs in the weekly workflow, not on push (E-18).

**T3. Edition migration idempotence.** Exit criteria 2 and 4, R-EDN-051, R-EDN-052. For every golden repository, under the `test-edition` feature: plan, apply, re-plan gives an empty plan; an interrupted apply followed by a re-run finishes; an old-edition checkout is refused with the bridging line, then upgraded and checked.

**T4. Coverage replay.** Exit criterion 3: `cargo xtask coverage-replay --repo <path> --commits 100`:
- For each commit, compute the changed set and the coverage result against the change docs present at that commit.
- Write the flagged files to a CSV for human labelling.
- After the exempt globs are tuned once, the share of wrong flags must be at most 5%.

The labelling is agent protocol A-2 of [../agent-runbook.md](../agent-runbook.md). The single tuning of exempt globs is done by the reviewer agent's proposal and recorded. The tool and its unit tests are code.

**T5. Merkle and ledger benchmark.** Exit criterion 8, R-VER-016:
- Extend `bench-corpus` with a seal history: 20 seals across 50 scopes on the 100,000-file corpus.
- Measure the scope-digest recomputation after a one-file change. It must read only the changed file (counted through the cache).
- Measure the ledger size and the time to verify continuity.
- Record the results in 60-roadmap 4.2 row 8.

**T6. Dogfood 0.2.**
- R-EDN-050 and R-REL-025: this repository uses change docs from the first 0.2 commit onwards.
- Each release seals (`fairlead seal --yes --commit` on `main`).
- `CHANGELOG.md` is the generated aggregate (`[changelog] file`). The human review before tagging is a release step.
- `dogfood.yml` runs `check --ci` with the in-tree binary and with the last released binary (`FAIRLEAD_REFERENCE_VERSION`, already wired in 0.1).

**T7. Codes complete.** `PENDING_0_2` is empty and `tests/codes.rs` passes. Every M02 code has an `explain` card (R-CLI-035), which the existing explain test checks.

**T8. Regression of 0.1.** The 11 goldens of 0.1, the determinism matrix, read-only, SARIF validation, secrets and the platform tests still pass unchanged (exit criterion 7). Any changed expected output needs an errata entry that names the 0.2 rule that changed it.

**T9. Agent-run evidence and owner acts (D-59).** Each row follows [../agent-runbook.md](../agent-runbook.md). Build each runner script `xtask/agent/<protocol>.ps1` in this task.

| Step | Rule |
|---|---|
| Agent protocol A-1: simulated eight-week pilot on three public repositories; stale or expired exceptions at most 30% | R-ROAD-019, R-ROAD-012, gate G2, exit criterion 6 |
| Agent protocol A-3: threshold amendment draft | R-ROAD-020 |
| Agent protocol A-2: label the coverage-replay CSV (T4) | exit criterion 3 |
| Agent protocol A-4: changelog review before tagging | R-REL-025 |
| Agent protocol A-5: golden review before the first bless (T1) | 0.1 practice (E-34) |
| Owner: `fairlead approve` of the version-based dogfood waiver (P6-T10), and tagging `v0.2.0` | R-AGT-026 |
