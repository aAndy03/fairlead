# fairlead 0.1 Implementation Plan (master)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This master plan sequences ten phase plans; read it, then [01-contract.md](01-contract.md), then [02-contract-amendments.md](02-contract-amendments.md), then your phase plan.

**Goal:** Build release 0.1 ("declare, check, brief") of fairlead: a Rust CLI and two libraries that let a codebase declare intent, report drift against it, and give agents a token-budgeted brief.

**Architecture:** Three crates in one Cargo workspace: `fairlead-syntax` (pure grammar and extraction), `fairlead-core` (walk, extract, resolve, check, render, context, write), `fairlead` (thin CLI). The pipeline is walk -> extract -> resolve -> check -> render; only discovery, walking, extraction, facts and the write transaction touch the disk or git. Authored intent is resolved at read time down the scope chain and never copied.

**Tech Stack:** Rust 2024 edition, MSRV 1.88; `winnow`, `toml`/`toml_edit`, `ignore`, `globset`, `blake3`, `rayon`, `git2`, `serde`/`serde_json`, `jiff`, `similar`, `clap`, `insta`, `trycmd`, `proptest`, `cargo-fuzz` (versions in [01-contract.md](01-contract.md) section 2).

**Spec:** [../../README.md](../../README.md) (index), [../../decisions.md](../../decisions.md) (D-01 to D-54, wins over any plan), [../../50-architecture.md](../../50-architecture.md), [../../60-roadmap.md](../../60-roadmap.md) sections 3.1 and 4.1 (scope and exit criteria E1 to E10).

## Global Constraints

Every task in every phase plan inherits these (values copied from the spec).

- Rust edition 2024, MSRV 1.88, workspace resolver 3; `#![forbid(unsafe_code)]` in `fairlead-syntax` and `fairlead-core`, `deny` in `fairlead`.
- No YAML crate, no `bincode`, no HTTP/TLS client, no network stack in 0.1 (R-ARCH-123). No process is spawned except the built `fairlead` binary and `git` in tests (R-ARCH-005; D-48).
- Checks and renderers are pure functions; output is byte-deterministic: LF only, forward-slash repo-relative paths, bytewise-sorted collections, no timestamps, no absolute paths, no locale formatting (R-ARCH-052, R-ARCH-060, R-ARCH-091).
- `check`, `context` and `explain` are read-only (they write only under `.fairlead/`) and never take the writer lock. The only write path is `write::Transaction` (R-ARCH-140).
- Exit codes 0 to 5 are defined only in [15-cli.md](../../15-cli.md) R-CLI-003; diagnostic codes only in [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md). No string literal `"HN-..."` outside the code registry.
- The hash is blake3; fingerprints and digests are lowercase Crockford base32 (C-7). The clock is read only through `AsOf` and the `--budget` deadline.
- Product name `fairlead`, folder `fairlead/`, marker `fairlead:` are placeholders (D-47); take them from configuration, never hard-code them outside the config defaults.
- Test names start with the snake-case rule ID. CI runs with `INSTA_UPDATE=no`. A bug fix adds the regression fixture first.

## Document map

| File | Role | Tasks | Words |
|---|---|---|---|
| [01-contract.md](01-contract.md) | Names, signatures, seams, phase ownership, test conventions, decisions C-1..C-40. Law for NAMES. | - | 11k |
| [02-contract-amendments.md](02-contract-amendments.md) | Consolidated resolutions of every cross-plan issue, the cross-plan task order. Wins over the plans and the contract. | - | 9k |
| [p1-bootstrap.md](p1-bootstrap.md) | Workspace, lints, CI, xtask, repo's own `fairlead/` folder | 13 | 12k |
| [p2-syntax.md](p2-syntax.md) | `fairlead-syntax`: ids, lexer, directives, frontmatter, attachment, extract | 23 | 30k |
| [p3-core-foundation.md](p3-core-foundation.md) | errors, exit codes, hashing, discover, walk, classify, extract, config, cache, git facts | 26 | 40k |
| [p4-registry.md](p4-registry.md) | scopes, items, overrides, grants, views, ledger, resolution | 15 | 24k |
| [p5a-check-engine.md](p5a-check-engine.md) | code registry, ramp, fingerprints, floor rules, 13 built-in checks, literal-leak, outcome | 30 | 25k |
| [p5b-baseline-render.md](p5b-baseline-render.md) | baseline, generated files, sanitising, secrets, human/terse/JSON/SARIF output | 20 | 23k |
| [p6-context.md](p6-context.md) | `fairlead context`: tiers, packing, estimator, digests, brief | 19 | 24k |
| [p7-write-verbs.md](p7-write-verbs.md) | write transaction, lock, `init`, `new`, `sync`, `approve`, `explain` | 19 | 26k |
| [p8-cli-integration.md](p8-cli-integration.md) | pipeline, `fairlead` binary, clap, exit mapping, trycmd | 17 | 15k |
| [p9-quality.md](p9-quality.md) | golden repos, determinism, fuzz, benchmarks, SARIF schema, dogfood, ablation harness | 27 | 19k |

209 tasks, about 250,000 words, nearly all of it test and implementation code. Nothing in them has been compiled; every uncertain library call carries a `Verify:` note that the executor resolves on the first compile of that task.

## Phase graph

```text
P1 bootstrap
 |-- P2 fairlead-syntax --\
 |                      >-- P4 registry --\
 |-- P3 core foundation /                  >-- P5a check engine --\
                                           |-- P5b baseline/render -+-- P8 CLI integration -- P9 quality (completes last)
                                           |-- P6 context ---------/
                                           |-- P7 write layer + verbs
```

- P1 first. P2 and P3 in parallel (they meet at the types of contract section 4.1 to 4.2, which exist as signatures from P1's stubs). P4 needs both. P5a, P5b, P6, P7 run in parallel after P4; P5b tasks 0 to 10 need only P3/P4. P8 needs the types of P5 to P7. P9's harnesses start in P1 and finish last.
- The exact task-level order and the symbols passed between tasks are in [02-contract-amendments.md](02-contract-amendments.md), section "Cross-plan task order".

## Execution protocol

1. **Workspace.** One Git repository at the `harness/` directory (P1 Task 1 runs `git init`). One worktree per phase when phases run in parallel; merge phase branches to `main` in the order of the phase graph.
2. **Per task.** A fresh implementer subagent receives only: the task text, [01-contract.md](01-contract.md), [02-contract-amendments.md](02-contract-amendments.md), the global constraints above. Two-stage review follows: spec-compliance review against the rule IDs the task cites, then code-quality review. A task is done when its listed tests pass, `cargo clippy -p <crate> --all-targets -- -D warnings` and `cargo fmt --check` pass, and the commit exists.
3. **Compile-first.** The first run of any task is the compile check named in its steps. Where a `Verify:` note exists, the implementer confirms the API against the crate's real docs (docs.rs) and, if different, adapts the call and records the change in the task's commit message. Plans are not edited by executors; discrepancies go to a `plans/0.1/errata.md` file.
4. **Resource limits.** Cargo runs are heavy. Use `CARGO_BUILD_JOBS=2`, run `cargo test -p <crate>` rather than `--workspace` during a phase, run `clippy` and `fmt` once per task, and do not run two cargo sessions on the same machine at the same time. The full workspace test, benchmarks and fuzz runs belong to CI and to P9.
5. **Phase gate.** A phase is merged when: every task in it is checked, the phase's traceability table is fully green (each rule has a passing test named after its rule ID), and `cargo test -p <crate>` passes on Linux and Windows. P5 to P8 also pass the integration tests that exist at that point.
6. **Stubs.** Where a later phase supplies a symbol an earlier task needs, the plan names the local stub; the stub is replaced in the named task and a test fails if it is not (see the amendments, section D).

## Human-only steps and decisions

These cannot be automated and are listed so they are not forgotten.

| When | Step |
|---|---|
| Before P1 | Choose the licence (the contract uses a placeholder; R-REL-007 expects one before publishing). Confirm the repository root is the `harness/` directory. Run the trademark check for `fairlead` if the rename is wanted before the first publish (D-47). |
| Before each phase | Resolve the `UNVERIFIED` dependency versions and feature names listed in contract section 2 on the machine that runs cargo. |
| P9 | Review every golden expected output line by line against the spec before the first `cargo xtask golden-bless` (they are spec-derived). Run `fairlead approve` for the dogfood waiver. Keep the results log of the controlled test (E2, E3) and the adoption and syntax-card trials (E8). |
| Release | Pass the pre-publish checklist of [60-roadmap.md](../../60-roadmap.md) 9.5 (trademark, prior-art verification, crates.io name claim). |

## Definition of done for 0.1

0.1 is done when exit criteria E1 to E10 of [60-roadmap.md](../../60-roadmap.md) section 4.1 hold. P9's traceability table maps each criterion to the task that verifies it. In short: the nine success criteria on the benchmark corpus; the controlled test (agents change behaviour after reading `context`); the exception-hygiene measurement; gates G0 and G1; the release test on Linux, Windows and macOS; measured benchmarks on a 100,000-file tree; dogfooding in CI; the adoption and syntax-card trials; a TypeScript/React repository in the evidence; no open 0.1 question.

After 0.1: 0.2 and 0.3 are specified in the chapters and get their own implementation plans; the v1 API freeze is at the end of 0.3 and gaps are closed in 0.4 to 0.9 (D-53).

## Risks specific to this plan

| Risk | Mitigation |
|---|---|
| Code in the plans was never compiled. | Compile-first rule; `Verify:` notes; errata file; each task is small enough to fix in place. |
| Ten plans written in parallel may drift from each other. | The contract and the amendments file are the single source of names; the integrator pass in `02` resolves known conflicts; the phase gate runs integration tests. |
| Plan size (about 250,000 words) is more than one agent session holds. | Executors receive one task at a time plus the contract and amendments, never the whole plan set. |
| Spec-derived golden outputs may encode a spec error. | Human line-by-line review before the first bless (P9); spec fixes go through a new D-xx. |
| Unverified crate versions or features. | Pre-phase dependency check (human-only table above); `cargo deny` and `xtask check-deps` in P1 fail the build on forbidden crates. |

## Execution handoff

Plan set complete and saved under `docs/plans/0.1/`. Recommended: **subagent-driven development** (one fresh subagent per task, two-stage review), starting with P1. Inline execution with checkpoints works for P1 and P8, which are small.
