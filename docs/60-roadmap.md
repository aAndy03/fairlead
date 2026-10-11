# Roadmap

> Status: draft · Milestone: mixed · Decisions: D-01, D-06, D-13, D-14, D-17, D-26, D-27, D-28, D-30, D-31, D-33, D-34, D-36, D-37, D-38, D-39, D-41, D-42, D-43, D-44, D-46, D-47, D-48, D-49 · Related: 00-vision.md, 13-checks-and-diagnostics.md, 14-context.md, 15-cli.md, 20-versions-and-seal.md, 21-environments-maturity-lifecycle.md, 22-editions-and-upgrade.md, 23-git-integration.md, 30-hierarchy-and-engines.md, 31-variation-protocol.md, 32-portability-and-restructure.md, 33-retention-and-compaction.md, 50-architecture.md, 70-open-questions.md

## 1. Purpose

This chapter fixes what each milestone contains, when a milestone is done, how the product's central claims will be measured, which findings would end or reshape the project, how releases are published and maintained on crates.io, and what must be true before the first public release. The documentation specifies the whole model and tags every rule with a milestone (D-06); this chapter is the single place where those tags are summed into releases. The 0.1 exit criteria and the kill assumptions are final as of 2026-10-09 (D-46) and are pre-registered by R-ROAD-014.

## 2. Concepts

| Term | Meaning |
|---|---|
| **Milestone** | A release line: 0.1, 0.2, 0.3. Rules carry exactly one tag. |
| **Exit criterion** | A measurable condition that must hold before a milestone is declared complete. |
| **Kill assumption** | An assumption whose falsification stops or re-scopes the project. |
| **Gate** | A decision point where a kill assumption is tested before further investment. |
| **Ablation** | An experiment comparing agent behaviour with and without a fairlead artifact. |
| **Pre-registration** | Committing tasks, metrics and thresholds before the first measured run (R-ROAD-014). |
| **Release test** | The CI and benchmark suite that must pass on all three platforms before a release (4.1, E5 and E6). |
| **Pre-publish checklist** | The conditions of Section 9.5 that must hold before anything is published. |

## 3. Milestones and exact scope (D-06)

### 3.1 Release 0.1 - "declare, check, brief"

Alpha. Proves the wedge loop: declaration, check, agent context, recorded exception. D-06 lists the 0.1 features: nested scopes, inheritance, `ovr`/`allow`, `def`/`use`, `check`, `context`, `init`, `new`, `explain`, `sync`, `approve`, baseline. `use` lines exist in 0.1 (Q-PROD-9), and so does the literal-leak check (D-43).

| Area | In scope |
|---|---|
| Structure | Root-only first, then nested scopes (D-38): stored scope IDs, eligibility hints, `fairlead new scope` (advisory warning below depth 3), `--hoist`, promotion and demotion rules with tombstones, `[[convention]]` (`transparent`, `class`), `home=` checks, root found by `--root`, `FAIRLEAD_ROOT` or an upward search for `root = true` (D-31), fail-closed control file, scopes absent from the working tree or a sparse checkout reported unknown and `check` exiting 5 (D-56; [10-scopes-and-inheritance.md](10-scopes-and-inheritance.md), [23-git-integration.md](23-git-integration.md) R-GIT-007) |
| Inheritance | Read-time resolution down the chain; per-key override policy `open`, `needs-exception`, `sealed`; vocabulary keys root-only; child control files add facts only (no `[delegate]` until 0.2); `ovr` with `why`, `owner` (a handle in `owners`) and date expiry (`exp=`, checked against `--as-of`, D-37) capped by `exceptions.max-days`; `[[allow]]` grants (not re-delegable); `[[req]]` escalations; approvals in `fairlead/approvals.toml` (D-34); exception ledger computed on read |
| Syntax | Six source verbs in the frozen 0.1 grammar; 0.1 behaviour covers `def`, `use`, `ovr` and `end` (`flag` and `env` syntax only, behaviour 0.2); `!` must-understand and `x-` keys; `def foreign.<name>` regions and `gen=` on `def` (D-35); IDs `[scope:]kind.name`; TOML frontmatter `directives`; user-defined kinds; carriers: source comments, Markdown body comments and frontmatter keys (D-32), never the control file; per-host placement table and golden-fixture conformance suite ([12-directive-syntax.md](12-directive-syntax.md)) |
| Checks | Syntax diagnostics; duplicate and shadowed IDs; dangling and undeclared `use`; inert, expired, unapproved, stale and overlapping `ovr`; unknown owner; literal leaks of declared values by declared value plus property context (`applies=`, minimum length; the 0.1 proof of the wedge, D-43); token contract checks (`TOK-MISSING`, `TOK-UNDECLARED`, `OVR-MISSING`); vendor-owned file drift; stale and hand-edited generated files; floor rules; baseline; severity ramp observe, warn, enforce; "0 rules declared" notice; `check --changed` (D-43); scope-move detection (`GLOB-DEAD`, `SCOPE-REPARENTED`, `SCOPE-ID-CHANGED`); non-UTF-8 files listed, never silently green ([13-checks-and-diagnostics.md](13-checks-and-diagnostics.md)) |
| Verbs | `init`, `check`, `sync` (scope index shards; `sync --baseline` writes the baseline, D-38; `--accept-new --why`, `--accept-vendor`), `new` (scope, rule, token, any declared doc kind, escalation, handoff), `context`, `explain`, `approve`. `init` offers hooks; `--hooks` installs them only when asked, never by default (D-38). Exit codes 0 to 5 as defined in [15-cli.md](15-cli.md) R-CLI-003 |
| Context | Terse `ID k=v` lines, tiers L0 and L1 with `explain` as L2, `--budget` (defaults 500 and 700), token estimate with a configurable ratio and opt-in `--exact-tokens`, ancestor deduplication, fenced provenance for authored prose, newest handoff line; path and ID queries only, free text exits 2 ([14-context.md](14-context.md)) |
| Files | Authored, generated, reference, vendor-owned classes; external-generated files ignored; generated files sorted, sharded and written as terse `ID k=v` lines (D-32); reference files carry a header and prose, never directives (D-36); a once-only `created` stamp on new docs (D-13); the only generated artifacts are the baseline and scope index shards (D-10); writer lock `.fairlead/lock` |
| Output | Text (`human`, `terse`), JSON and SARIF 2.1.0 (validated against the official schema in CI); stable rule IDs and diagnostic codes |
| Platform | Rust, MSRV 1.88, Linux, Windows, macOS from the first commit; prebuilt binaries; git reads through `git2` (D-48); Cargo TOML manifests are the only manifests parsed; JSON cache keyed by fairlead version, grammar, config hash and `--as-of` |

Not in 0.1: seal and versions, environments, maturity, lifecycle flags, graveyard and item tombstones, edition migration (`upgrade`), the `git` subprocess (`seal --commit`, `--tag`, signed-record verification), docs-first coverage, `move` (0.2, D-38), `--since`, `--staged`, `--base`, task queries, hotfix debts, revive docs, import graphs, hierarchy, variation protocol, portability beyond move detection, retention, bridges, MCP server.

### 3.2 Release 0.2 - "versions, lifecycle, git"

| Area | In scope |
|---|---|
| Versions | `seal`: open and sealed versions (`MAJOR.MINOR.PATCH`, derived, D-42), all-or-nothing, commit point last, optional `--commit` and `--tag` through the `git` executable (D-48), collision repair, ledger shards with checkpoints, rollups at seal, virtual file versions, draft IDs, ordinals and the alias map, escalation grace, sweep report, exception version cap ([20-versions-and-seal.md](20-versions-and-seal.md)) |
| Environments, maturity, lifecycle | Three orthogonal axes with root-owned vocabularies; `env` and `flag` behaviour (`target=`, D-49); bindings cross-checked for existence; tombstones as `[[tomb]]` tables, graveyard with fingerprint matching; replacement chains; due-date logic; revive docs; optional `authority.signed-records` ([21-environments-maturity-lifecycle.md](21-environments-maturity-lifecycle.md)) |
| Editions | Edition contract, two-step `upgrade` (draft upgrade doc, then approved apply), must-understand behaviour, unknown class error, unknown key warning, compatibility matrix, mixed editions for sealed sub-roots, `upgrade --fetch` as the only network use ([22-editions-and-upgrade.md](22-editions-and-upgrade.md)) |
| Git | Full git-case matrix, shallow-clone handling, `merge=union` ledgers, allocation lock, `--base`, `--staged`, `--since`, `--against`, CI mode (`check --ci`). Hook installation stays the 0.1 `init --hooks` offer (D-38) ([23-git-integration.md](23-git-integration.md)) |
| Docs-first | Path-coverage check with observe, warn, enforce ramp, rolling chore docs, exempt globs, hotfix backfill debts, restructure docs written by `move` |
| Scopes and syntax | Delegation of child control keys (`[delegate]`), `[[attach]]` for comment-free files, the `own` convention and `provides` enforcement (`NOT-PROVIDED`), `SCOPE-AMBIG`, control-file semantic merge checks |
| Styles | Tailwind v4 caveat checks (`TOK-DERIVED` graph, layer order, bare overrides, `FOREIGN-LOST`) and class safelists (`classes=`); contract-declared derivations stay 0.1 |
| Authority | Owner `kind = "agent"` and `APPROVER-AGENT`, escalation resolution by the nearest ancestor owner, PR-template text via `explain` |
| Other | Version-based expiry (`until=<version>`) as the default, with dates still allowed (D-37); `move` for scope renames, merge, demote, delete and detach (0.2, D-38); `--since` deltas by version or git revision; ledger checkpoint continuity check |

### 3.3 Release 0.3 - "hierarchy, variation, portability, retention"

| Area | In scope |
|---|---|
| Hierarchy | Ladders, ranks, layers, containers, units, flows, entries, ports; reachability audits (orphan, stale, dormant, forgotten); evidence-grep; adoption ramp L0-L3 ([30-hierarchy-and-engines.md](30-hierarchy-and-engines.md)) |
| Variation | Home scope, visibility, variation request with four outcomes, variants, declared forks (detection ships off), richer hardcoded-token rules on top of the 0.1 check (near colours, `rem`, arbitrary values, `var()` chains) ([31-variation-protocol.md](31-variation-protocol.md)) |
| Portability | Sealed sub-roots, `adopt`, attach, cut and copy, digest-only lock with `portable` snapshots, requires and provides ([32-portability-and-restructure.md](32-portability-and-restructure.md)) |
| Retention | Version-based TTL with a calendar floor (minimum age 90 days by default, D-39); compaction inside `seal`; `sum:` authored at creation and `out:` filled at close (D-39); append-only ledger with hash chain and periodic checkpoints ([33-retention-and-compaction.md](33-retention-and-compaction.md)) |
| Bridge | TS to Rust bridge declarations with presence, wire-literal and version-pin checks (D-29) |
| Context and authority | BM25 task queries, `--dense`, general signed-commit verification of authority records, the MCP server as a separate crate (`check`, `context`, `explain`; D-62) |
| Later in the line | Import-graph extraction (tree-sitter, `syn`). TypeScript parsing through tree-sitter is adopted only after a CI compatibility test of the grammar against the pinned core passes ([30-hierarchy-and-engines.md](30-hierarchy-and-engines.md) R-HIER-035); until then import evidence is the regex pass only (R-HIER-033) |

### 3.3a Path to 1.0 (D-53)

The v1 API (CLI surface, directive grammar per edition, output formats) is frozen at the end of 0.4 (D-64; it was the end of 0.3 under D-53). Releases 0.5 to 0.9 close the gaps found after the freeze, without breaking the frozen surface except through an edition. 1.0 follows when the gap list is empty. Gaps that remain open in this specification until then are expected, not defects.

### 3.4 Rule counts by milestone

Counted by grep of rule tags in chapters 10 to 40 on 2026-10-09 (unique rule IDs by their first tag). Recount before each milestone review; a rule keeps its tag when a milestone slips (edge case 8).

| Chapter | [0.1] | [0.2] | [0.3] |
|---|---|---|---|
| 10 scopes | 59 | 3 | 1 |
| 11 files | 43 | 5 | 0 |
| 12 syntax | 47 | 3 | 2 |
| 13 checks | 49 | 8 | 1 |
| 14 context | 29 | 6 | 7 |
| 15 CLI | 41 | 6 | 0 |
| 20 versions | 0 | 50 | 0 |
| 21 lifecycle | 0 | 53 | 0 |
| 22 editions | 1 | 34 | 1 |
| 23 git | 2 | 40 | 0 |
| 30 hierarchy | 0 | 0 | 58 |
| 31 variation | 1 | 0 | 45 |
| 32 portability | 9 | 7 | 27 |
| 33 retention | 0 | 5 | 32 |
| 40 trust | 27 | 6 | 1 |
| **Total** | **308** | **226** | **175** |

## 4. Exit criteria

### 4.1 Release 0.1 (final)

0.1 is complete when every row below holds. The rows and their thresholds are fixed before the controlled test is built (R-ROAD-014, Q-PROD-4) and may change only by a dated amendment.

| # | Exit criterion | Measure | Source |
|---|---|---|---|
| E1 | All nine success criteria hold on the benchmark corpus and fixtures | Section 5, R-ROAD-001 to R-ROAD-009 | Section 5 |
| E2 | Controlled test (D-06): agents change behaviour after reading `context` | 20 tasks (10 UI, 10 backend), 3 runs per task per arm, arm C (`context` block) against arm A (nothing): hardcoded-declared-value count in the final diff falls by at least 50%; under 20% is weak and fires kill assumption 2. Arm B (static `AGENTS.md`) is reported, not a gate (R-ROAD-018) | R-ROAD-005, 7.1 |
| E3 | The lead claim has its own measurement | Exception hygiene: in at least 80% of tasks that conflict with a sealed or waivable rule, the agent ends with a declared, owned, expiring `ovr`, a refusal or an escalation, against arm A | R-ROAD-009, 7.2 |
| E4 | Gates passed | G0 (assumptions 1 and 9, replay) before the 0.1 feature set is frozen; G1 (assumptions 2, 4 and 10, alpha) before any announcement | R-ROAD-010, R-ROAD-011 |
| E5 | Release test green on Linux, Windows and macOS | Byte-identical output cold, warm and deleted cache, 1 and N threads, LF and CRLF checkouts, two working directories; clean `git status` after `check`, `context` and `explain` in three real repositories; 50 of 50 seeded secret-like values rejected; SARIF output validates against the official 2.1.0 schema; fuzz targets run for their bounded time; every golden repository passes; MSRV build and `cargo audit` pass | R-ROAD-008, [50-architecture.md](50-architecture.md) R-ARCH-071, R-ARCH-080, R-ARCH-110, R-ARCH-122, R-ARCH-150 |
| E6 | Benchmarks measured, not assumed | 100,000-file tree: cold `check` at most 60 s, warm at most 5 s, `--changed` at most 1 s, peak RSS at most 500 MB. The same run counts tokens of sample `context` slices with the target model's tokenizer and records the measured bytes-per-token ratio in this chapter, and measures Merkle digest and ledger cost at 100,000 files | R-ROAD-001, [14-context.md](14-context.md) R-CTX-072, R-ARCH-130 |
| E7 | Dogfooding | The fairlead repository passes its own `check` in CI with the previously released binary as reference (the in-tree build alone until a release exists) | R-ARCH-160 to R-ARCH-162 |
| E8 | Humans and agents can write the syntax | Adoption trial: first green on a legacy repository in at most 15 minutes. Syntax-card trial: at most 5% invalid directives in 100 written by fresh agent sessions | R-ROAD-006, kill assumption 10 |
| E9 | The evidence is not Rust-only | The test fixtures, the precision corpus and the ablation include a TypeScript/React repository | R-ROAD-015 |
| E10 | No open question remains tagged 0.1 | Every register question for 0.1 is resolved (D-46); new questions need owner acceptance first | [70-open-questions.md](70-open-questions.md) |

### 4.2 Release 0.2 (thresholds provisional until R-ROAD-020)

| # | Criterion |
|---|---|
| 1 | Across 50 simulated parallel merges with seals on branches, zero unresolved conflicts after regeneration with `sync`; collisions are detected, never silent. |
| 2 | Edition migration on every golden repository is idempotent: plan, apply, re-plan yields an empty plan; interrupted apply followed by re-run finishes. |
| 3 | Docs-first coverage: at most 5% of changed files in a 100-commit replay are flagged incorrectly after exempt globs are tuned once; severity stays at warn by default. |
| 4 | An old-edition checkout can be checked by the new binary, and a too-new edition is refused with the exit code that [15-cli.md](15-cli.md) assigns (D-41). |
| 5 | Shallow clone, rebase, squash, cherry-pick and revert each have a passing golden scenario ([23-git-integration.md](23-git-integration.md)). |
| 6 | An eight-week pilot on at least two real repositories, at least one of them external, ends with at most 30% of recorded exceptions stale or expired (kill assumption 3). |
| 7 | All 0.1 criteria still hold. |
| 8 | The Merkle scope digest and the ledger shard layout are benchmarked on a 100,000-file fixture before the 0.2 freeze ([20-versions-and-seal.md](20-versions-and-seal.md) R-VER-016); the result is recorded here. Result (2026-10-11, `cargo xtask bench-corpus merkle`, 100,000 files, 50 scopes, 20 seals, a debug build on Windows): after a one-file change the check reads 1 file, a warm check reads 0; the 51 ledger files hold 14,910 bytes; a check that recomputes every scope digest and verifies the ledger chain takes 15.7 s warm and 16.4 s after the change, in the debug build. The weekly `bench.yml` job `merkle` repeats it with a release build and fails unless the change reads exactly 1 file. |
| 9 | After 50 simulated `merge=union` merges of ledger files, the checkpoint continuity check passes or reports a repairable break ([33-retention-and-compaction.md](33-retention-and-compaction.md) R-RET-036). |

### 4.3 Release 0.3 (thresholds provisional until R-ROAD-020)

| # | Criterion |
|---|---|
| 1 | Hierarchy findings reviewed blind on three repositories: at most 2% judged false. With zero entries declared, `stale` is skipped. |
| 2 | Retention: a simulated five-year history compacts idempotently; two machines produce byte-identical compaction; `context` size at a leaf stays within the 0.1 budget independent of history length. |
| 3 | Portability: cut, copy, mount, submodule and vendored-subtree scenarios pass golden tests; no silent rank guessing at attach. |
| 4 | Variation: a scripted request-to-variant cycle completes with every consumer reference migrated and no undeclared fork flagged in the fixture. |
| 5 | Bridge: removal of either end, or a changed contract, is detected in the fixture. |
| 6 | All 0.1 and 0.2 criteria still hold. |
| 7 | TypeScript parsing through tree-sitter is used only if the grammar compatibility test passes in CI; otherwise the regex evidence pass stays the only source and the criterion is met without it. |

## 5. Success criteria for 0.1 (measurable)

| ID | Criterion | Method | Milestone |
|---|---|---|---|
| R-ROAD-001 | **Speed.** 100,000-file tree, Windows 11, Defender on: cold `check` at most 60 s; warm and unchanged at most 5 s; `--changed` at most 1 s; peak RSS at most 500 MB. | Benchmark corpus ([50-architecture.md](50-architecture.md), R-ARCH-130). | [0.1] |
| R-ROAD-002 | **Context budget.** `context` on a leaf path at most 800 tokens at p90 and at most 1,500 at depth 3 (bytes divided by 4, labelled an estimate); byte-identical across two runs. | Fixture repositories at several depths. | [0.1] |
| R-ROAD-003 | **Precision.** Blind review of 200 findings drawn from three real repositories (TS/React, Rust, mixed): at most 2% judged false. Dangling-reference false positives: zero by test. Same gate as [13-checks-and-diagnostics.md](13-checks-and-diagnostics.md) R-CHK-051. | Reviewer not told which checks produced which findings. | [0.1] |
| R-ROAD-004 | **Recall on history.** 50 commits found with `git log -S` that introduced a hardcoded declared value: `check` flags at least 70% at the introducing commit; at most 5% new findings on commits that add no hardcoded value. | Replay harness. | [0.1] |
| R-ROAD-005 | **Wedge effect (ablation).** 20 UI and backend tasks, 3 runs each: hardcoded-declared-value rate falls at least 50% with the `context` block. Under 20% means the wedge is weak. | Section 7.1. | [0.1] |
| R-ROAD-006 | **Adoption.** Install to first green `check` on a legacy repository in at most 15 minutes using the baseline; `init` creates at most 3 files; zero new failures across 20 replayed unrelated commits. | Timed trial with a person who did not write fairlead. | [0.1] |
| R-ROAD-007 | **Ceremony and conflicts.** A typical 3-file, one-scope change needs at most 2 fairlead commands and at most 6 fairlead-authored lines (median of 10 scripted changes). Zero conflicts across 50 simulated parallel merges. | Scripted changes. | [0.1] |
| R-ROAD-008 | **Cross-platform and safety.** Identical `check` output on Linux and Windows checkouts (LF and CRLF) across 20 repository states. 100% of 50 seeded secret-like values rejected. `check`, `context` and `explain` leave `git status` clean in three repositories. SARIF output validates against the official schema. | CI and fixtures. | [0.1] |
| R-ROAD-009 | **Wedge-specific effect.** In a nested-scope fixture, agents asked to complete tasks that conflict with a sealed or waivable rule end with a declared, owned, expiring `ovr` (or a refusal/escalation) in at least 80% of such tasks, versus the same tasks without fairlead. | Section 7.2. | [0.1] |

R-ROAD-009 is new relative to the red-team criteria: criteria 1 to 8 measure the check-and-context loop, which a ripgrep-plus-CI setup could imitate. D-01 makes inheritance, override policy and the exception ledger the lead claim, so the lead claim needs its own measurement. The 80% threshold is final for 0.1 (R-ROAD-014).

| ID | Rule | Milestone |
|---|---|---|
| R-ROAD-014 | The 0.1 exit criteria (4.1), the thresholds of Section 5, the ablation design (7.1) and the exception-hygiene design (7.2) MUST be committed to this repository before the controlled test fixture is built and before the first measured run. A later change MUST be a dated amendment recorded in [70-open-questions.md](70-open-questions.md) and invalidates runs made under the old text. | [0.1] |
| R-ROAD-015 | The controlled test, the precision corpus and the replay MUST include a TypeScript/React repository (the golden `ts-react` tree of [50-architecture.md](50-architecture.md) R-ARCH-153 and a real repository), in addition to fairlead's own Rust repository. A 0.1 claim made on Rust-only evidence is invalid. | [0.1] |
| R-ROAD-016 | Precision (R-ROAD-003), recall (R-ROAD-004) and the ablation MUST be run with the literal-leak check (D-43) and `use` lines enabled, since they are the checks the wedge proof rests on. The independent scoring script MUST NOT call `fairlead check`. | [0.1] |
| R-ROAD-017 | A bytes-per-token benchmark with the target model's tokenizer and a Merkle digest benchmark at 100,000 files MUST be part of the 0.1 release test; the measured ratio is recorded in this chapter, and the default ratio changes only with an edition ([14-context.md](14-context.md) R-CTX-045, R-CTX-072). | [0.1] |
| R-ROAD-018 | Arm B (a hand-written static `AGENTS.md`) of the ablation is reported and informs positioning; it is not part of the pass/fail gate. The gate is arm C against arm A (D-06). | [0.1] |
| R-ROAD-019 | The eight-week pilot MUST include at least one external repository before 0.2 is declared complete; the owner's own projects are the others. | [0.2] |
| R-ROAD-020 | The 0.2 and 0.3 thresholds in 4.2 and 4.3 are reviewed once, after the 0.1 alpha ablation has produced data, and then frozen by a dated amendment. The 0.1 thresholds are final and are not reviewed after the fact. | [0.2] |
| R-ROAD-021 | If 0.1 slips, features are cut in the order of 8.1; the never-cut set is not negotiable. Nested scopes are not cut: D-06 puts them in 0.1 and D-38 only orders the build. | [0.1] |

## 6. Kill assumptions and gates

Ten assumptions, each with the cheapest falsifier. "Action" is the response when the falsifier fires; the table is final (D-46).

| # | Assumption | Falsifier (cheapest to run) | Action if falsified |
|---|---|---|---|
| 1 | Hardcoded declared values are common and costly. | Replay finds fewer than one introducing commit per 100 on real repositories. | Re-scope: the loop has no market. Narrow to the wedge only (inheritance and ledger) or stop. |
| 2 | Agents read `context` and change behaviour. | Ablation below 20% reduction. | Stop agent-facing claims; evaluate whether human-only drift tooling is worth shipping, given substitutes. |
| 3 | Humans keep declarations current. | After eight weeks, more than 30% of declarations stale or waived. | Add automated staleness reporting; revisit retention and ramp defaults; do not extend scope. |
| 4 | Literal checks reach at most 2% false positives on real code. | Blind review above 5%. | Remove literal checks from default rules; keep opt-in only. |
| 5 | A baseline makes legacy adoption painless. | First green takes over an hour of triage on 100,000 files. | Redesign baseline granularity (per rule, per directory) before 0.2. |
| 6 | Teams run `check` in CI or pre-commit. | Pilot teams never run it, so agents skip it too. | Move effort to hooks and agent integration; reconsider pure CLI shape. |
| 7 | Ceremony is small enough not to bypass. | `--no-verify` or waiver rate above 10% of pilot commits. | Cut rules and scaffolding; enforce the "at most 10 rules at start" guidance. |
| 8 | Users accept "declared intent is checked", not "the codebase is consistent". | "Fairlead missed X" tickets for undeclared drift outnumber catches. | Fix positioning (R-VIS-001), not features. |
| 9 | Substitutes do not cover the wedge: Semgrep or ast-grep rules, dependency-cruiser or Nx boundaries, ESLint rules, knip, jscpd, Changesets, ADR tools, nested CLAUDE.md or Cursor glob rules. | Substitutes capture at least 80% of replay findings and path-scoped context. | Reduce to the one part they cannot do (override policy, sealed keys, exception ledger) or stop. |
| 10 | Humans and agents write valid declarations from an 800-token syntax card, and the grammar survives editions. | Over 5% invalid annotations after two weeks, or a breaking grammar change before 0.2. | Simplify the grammar before freezing edition 1. |

| ID | Rule | Milestone |
|---|---|---|
| R-ROAD-010 | Assumptions 1 and 9 MUST be tested by replay on real repositories before the 0.1 feature set is frozen (Gate G0). | [0.1] |
| R-ROAD-011 | Assumptions 2, 4 and 10 MUST be tested on the 0.1 alpha before any public announcement (Gate G1). | [0.1] |
| R-ROAD-012 | Assumptions 3, 5, 6 and 7 MUST be tested in the eight-week pilot before 0.2 is declared complete (Gate G2). | [0.2] |
| R-ROAD-013 | A falsified assumption MUST be recorded in [70-open-questions.md](70-open-questions.md) with the measured value and the chosen action before work continues. | [0.1] |

## 7. Validation plan

### 7.1 Ablation: does `context` change agent behaviour (R-ROAD-005)

| Item | Design |
|---|---|
| Tasks | 20 tasks (10 UI, 10 backend) on a fixture monorepo with a TypeScript/React app and a Rust crate (golden `ts-react` and `fullstack-tauri`, R-ROAD-015) and 30 declared items including tokens, constants and rules. Each task has a natural path to a hardcoded declared value. |
| Arms | **A** no fairlead artifact. **B** a hand-written static `AGENTS.md` containing the same declarations in prose (the status quo). **C** the `fairlead context <path>` output inserted at session start. B is the honest comparison for the claim that dynamic context beats static files; it is reported, not gated (R-ROAD-018). |
| Runs | 3 runs per task per arm, fresh session each, fixed model and settings recorded, order randomised. |
| Primary metric | Count of hardcoded declared values in the final diff, found by an independent script (not by `fairlead check`, to avoid circularity, R-ROAD-016). |
| Secondary | Rule violations found by `check`; tokens consumed; number of tool calls; whether the agent created an `ovr` and whether the reason is meaningful. |
| Blinding | Scorer sees diffs without arm labels. |
| Decision rule | Arm C versus A: reduction of at least 50% passes; under 20% is weak (kill assumption 2). Arm C versus B is reported but not a gate. |
| Pre-registration | Tasks, metric and thresholds committed before the first run (R-ROAD-014). |
| Threats | Model drift between runs; fixture too easy; scorer script bias; agents with tool access reading `fairlead/` directly. Each is recorded with the result. |

### 7.2 Exception hygiene: does the wedge work (R-ROAD-009)

Fixture: three nested scopes, one sealed key, one waivable rule, one open key. Twelve tasks, each requiring a deviation. Measure, per task: whether the deviation was (a) avoided by an allowed route, (b) made as a declared `ovr` with `why`, `owner` and expiry, (c) made silently, (d) attempted against a sealed key, and whether in case (d) the result stayed inert and produced an escalation. Compare arm C with arm A. Also record whether a reviewer, given only the diff and the ledger, could list every deviation correctly (target: all).

### 7.3 Replay, adoption and pilot

| Experiment | Purpose | Design |
|---|---|---|
| Replay | Assumptions 1, 4, 9; R-ROAD-003, R-ROAD-004 | 50 introducing commits and 20 unrelated commits from at least three repositories, one of them TypeScript/React; compare against the substitutes in assumption 9 run over the same commits. |
| Adoption trial | R-ROAD-006, assumption 5 | A developer who has not seen the spec installs, runs `init`, writes a baseline and reaches green; time and number of docs consulted recorded. |
| Eight-week pilot | Assumptions 3, 6, 7 | At least two repositories, at least one external (R-ROAD-019), with at least one agent user each. Waiver growth is measured from git history (`git log -S "fairlead: ovr"`), not from telemetry (R-VIS-007). |
| Syntax card trial | Assumption 10 | Fresh agent sessions given only the syntax card write 100 directives; invalid rate recorded. |

## 8. Risks

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| 1 | Scope: the specification bundles six products; the work does not finish | High | High | Milestone tags; cut order in 8.1; each milestone ships standalone value. |
| 2 | Substitutes already cover the 0.1 checks | Medium | High | Gate G0; wedge-specific criterion R-ROAD-009. |
| 3 | Nested scopes in 0.1 (D-06) add complexity before the root-only path is proven | Medium | Medium | Ship root-only dogfooding first within 0.1 (build order fixed by D-38); explicit promotion only; advisory warning beyond depth 3 ([10-scopes-and-inheritance.md](10-scopes-and-inheritance.md) R-SCOPE-010). |
| 4 | Prior art (Archkeel and others) moves into the wedge | Medium | High | Re-run the survey at each milestone; keep the claim narrow (R-VIS-001). |
| 5 | Directive grammar proves wrong after users depend on it | Medium | High | Syntax-card trial before freezing edition 1; unknown-class error and `!` marker (D-17). |
| 6 | Name collision or rename disruption | Medium | Low | Marker and folder name are configuration; trademark check and rename before publish ([00-vision.md](00-vision.md), Appendix A; R-REL-040). |
| 7 | False positives destroy trust | Medium | High | Opt-in literal checks with minimum length, precision criterion, baseline. |
| 8 | Windows-specific failures (paths, locks, antivirus) | Medium | Medium | Windows CI from the first commit; bounded retry; case-folded keys. |
| 9 | Upstream churn (`rmcp`, tree-sitter grammars, `gix`) | Medium | Low | Not in 0.1 binary; pin exact minors when adopted. |
| 10 | Supply-chain concern for a tool that edits instruction files | Low | High | `init` prints a diff; no self-update; checksums; floor rules; `--locked` builds. |
| 11 | Rule fatigue: users declare too much | Medium | Medium | Guidance: at most 10 rules at start, each with an owner and a one-line reason; a rule with over 20% of hits waived goes to review. |
| 12 | Single maintainer | High | Medium | Dogfooding keeps decisions in repo; stable, documented formats. |
| 13 | Evidence gathered on fairlead's own Rust repository only | Medium | High | R-ROAD-015: a TypeScript/React repository is mandatory in every 0.1 measurement. |
| 14 | A prior-art citation turns out false after publication | Medium | Medium | R-REL-041: verify or remove before the first release. |

### 8.1 Cut order if 0.1 slips

Never cut: baseline, `check` core, the literal-leak check (D-43), `context`, `ovr` with ledger, determinism and read-only guarantees, floor rules. Cut in this order: (1) `new` kinds beyond scope, rule, token and escalation (handoff and other doc kinds are scaffolded by hand); (2) breadth of `explain`; (3) literal-leak checks beyond colours and strings; (4) `approve` simplified to a documented CODEOWNERS convention plus the record file (the record in `approvals.toml` stays, D-03). Cutting `approve` entirely and cutting nested scopes back to root-only are not options (D-03, D-06). This ordering is final (R-ROAD-021).

## 9. Release and maintenance plan

### 9.1 Versioning

| ID | Rule | Milestone |
|---|---|---|
| R-REL-001 | The binary and crates MUST follow semantic versioning. Pre-1.0, a minor release MAY change CLI flags and output; it MUST NOT change the directive grammar or generated-file bytes within a frozen edition. | [0.1] |
| R-REL-002 | 0.1 is an alpha with no migration promise: edition 1 is not frozen. Edition 1 MUST be frozen no earlier than the 0.2 release, when `upgrade` exists. | [0.1] |
| R-REL-003 | From the freeze, within an edition: grammar and generated output are byte-stable across binary minors and patches (D-17). New rules ship at `off` or `observe`. Bug fixes may change check results; baselines absorb them. | [0.2] |
| R-REL-004 | The stability surface is: CLI verbs and documented flags, exit codes ([15-cli.md](15-cli.md) R-CLI-003), directive grammar per edition, diagnostic and rule IDs, JSON and SARIF output shapes, generated-file formats. The `fairlead-core` and `fairlead-syntax` library APIs are NOT part of it before 1.0. | [0.1] |
| R-REL-005 | Rule IDs and diagnostic codes MUST NOT be reused after retirement. | [0.1] |
| R-REL-006 | The MSRV floor is 1.88. It MAY rise only in a minor release, MUST be announced in the changelog, and MUST be tested in CI. It rises to 1.90 only when `tree-sitter` is adopted. A patch release MUST NOT raise it. | [0.1] |
| R-REL-007 | The project and its fixtures MUST be licensed GPL-3.0-or-later from 0.2.0, with the full licence text in `LICENSE` (D-60). Release 0.1.0 was published under MIT and stays MIT. | [0.1] [0.2] |

### 9.2 Editions and deprecation

| ID | Rule | Milestone |
|---|---|---|
| R-REL-010 | A new edition MAY change grammar or generated output and MUST ship with `fairlead upgrade` support from the previous edition. | [0.2] |
| R-REL-011 | A binary MUST read the current edition and refuse older ones with the exact bridging version to install; only `upgrade` parses the previous edition ([22-editions-and-upgrade.md](22-editions-and-upgrade.md) R-EDN-010). | [0.2] |
| R-REL-012 | A CLI flag, config key, rule or verb MUST be deprecated for at least one minor release (a warning naming the replacement) before removal. Grammar removals happen only at an edition boundary. | [0.1] |
| R-REL-013 | The release that deprecates something MUST say so in the changelog and in `explain`. | [0.1] |

### 9.3 Publishing to crates.io

| ID | Rule | Milestone |
|---|---|---|
| R-REL-020 | Crates MUST be published in dependency order: the syntax crate, the core crate, then the binary crate (`fairlead-syntax`, `fairlead-core`, `fairlead` while the placeholder name stands). Each release MUST pass `cargo publish --dry-run` and the full CI matrix. | [0.1] |
| R-REL-021 | The chosen crate names MUST be re-checked for availability immediately before the first publish. Placeholder or empty crates MUST NOT be published to reserve names; the first publish is a working alpha. | [0.1] |
| R-REL-022 | The name decision (D-30, D-47) MUST be made before the first publish; renaming after publish requires publishing new crates and yanking or marking old ones deprecated. | [0.1] |
| R-REL-023 | Prebuilt binaries for Linux, Windows and macOS with published BLAKE3 checksums (`b3sum`) MUST accompany each release, because `cargo install` alone is slow on Windows and unavailable on CI images without a toolchain. `cargo install <name> --locked` MUST be documented. | [0.1] |
| R-REL-024 | A release MUST be yanked only if it destroys data, produces wrong results silently, or has a security flaw. The fix ships as a patch release. | [0.1] |
| R-REL-025 | The release changelog MUST be generated from fairlead's own change docs (dogfooding), reviewed by a human before tagging. | [0.2] |
| R-REL-026 | `fairlead upgrade` MUST NOT replace the binary. Binary updates are the user's package-manager action. | [0.2] |

### 9.4 Maintenance

| ID | Rule | Milestone |
|---|---|---|
| R-REL-030 | Pre-1.0, only the latest minor line is supported with fixes. After 1.0, the latest minor of the current and previous edition lines is supported. | [0.1] |
| R-REL-031 | Security reports MUST have a documented private channel and a stated response target in the repository. | [0.1] |
| R-REL-032 | The survey of prior art ([00-vision.md](00-vision.md), 4.1) MUST be refreshed at the start of each milestone and any change to the wedge claim recorded. | [0.1] |
| R-REL-033 | A dependency with a RustSec advisory MUST be replaced or pinned away before the next release; the dependency table ([50-architecture.md](50-architecture.md), R-ARCH-120) MUST be updated. | [0.1] |
| R-REL-034 | 1.0 MUST NOT be declared until all three milestones are complete, edition 1 has been stable for at least two minor releases, and at least one external repository has used it for six months. | [0.3] |

### 9.5 Pre-publish checklist

Nothing is published until every item holds (R-VIS-006). Each item is recorded in this repository with its date.

| ID | Rule | Milestone |
|---|---|---|
| R-REL-040 | **Trademark check.** The target name (`fairlead`, D-47) MUST be checked for conflicting software marks and live products in the relevant registers and search engines, in addition to the crates.io, GitHub and docs.rs checks of [00-vision.md](00-vision.md) Appendix A; the date, sources and outcome are recorded. A name that fails is dropped and the next candidate (`gusset`, `lashing`) goes through the same check. | [0.1] |
| R-REL-041 | **Prior-art verification.** Every citation tagged S or UNVERIFIED in [00-vision.md](00-vision.md) 4.1 MUST be verified on a primary page or removed; positioning against `charter`, `corbel`, `clew` and `dowel` MUST be written; the survey is refreshed (R-REL-032). | [0.1] |
| R-REL-042 | **Crate name claim.** Availability of `<name>`, `<name>-core` and `<name>-syntax` MUST be re-checked immediately before publishing (R-REL-021); the three are then claimed by publishing the first working alpha in dependency order (R-REL-020), never as placeholders. | [0.1] |
| R-REL-043 | **Rename executed.** The specification, fixtures and the fairlead repository's own control file MUST be switched from `fairlead` to the target name (marker, folder, crate and binary names) by the mechanical rename of [22-editions-and-upgrade.md](22-editions-and-upgrade.md) R-EDN-007 (`upgrade --rename-marker X --rename-folder Y`), and `check` MUST pass afterwards. | [0.1] |
| R-REL-044 | **Gate evidence.** Gate G1 (R-ROAD-011) has passed and its measured values are recorded; public copy states the measured effect and the limits only (R-VIS-005, R-VIS-001). | [0.1] |
| R-REL-045 | **Repository hygiene.** Licence files (R-REL-007), the private security channel (R-REL-031), a changelog entry, the BLAKE3-checksummed prebuilt binaries (R-REL-023), and the README statements on declared-only verification (R-VIS-001) and no telemetry (R-VIS-007) are present. | [0.1] |
| R-REL-046 | **Release build.** CI is green on Linux, Windows and macOS; `cargo publish --dry-run` passes for all three crates; the release is built with `--locked`; `cargo audit` is clean; the MSRV job passes. | [0.1] |

## 10. Examples

Milestone tags as they appear in rules; and the headers that `init` writes. The edition is the pin, and there is no separate pin file (D-33).

```toml
# fairlead/control.toml (root)
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
```

```text
0.1 --- 0.2 --- 0.3 --- 1.0
 |       |       |
 |       |       +-- hierarchy, variation, portability, retention, bridge
 |       +---------- seal, versions, env/maturity/lifecycle, editions, git, coverage
 +------------------ nested scopes, inheritance, ovr/allow, def/use, check, context, baseline
Gates:  G0 before freeze   G1 before announcement   G2 after pilot
Publish: checklist 9.5 after G1
```

## 11. Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | The ablation passes but R-ROAD-009 fails | The wedge claim is withdrawn from public copy; the product ships as a drift and context tool (R-VIS-005). | R-ROAD-013 |
| 2 | The ablation arm B (static AGENTS.md) performs as well as arm C | Dynamic context is not justified by this fixture; test on a larger repository before any claim. Arm B is not a gate. | Section 7.1, R-ROAD-018 |
| 3 | A crates.io name is taken between decision and publish | Choose the next candidate (Appendix A of [00-vision.md](00-vision.md)); marker and folder are configuration; the trademark check is repeated for it. | R-REL-021, R-REL-040 |
| 4 | A bug fix changes check results on an edition-frozen repository | Allowed in a patch release; baselines absorb it; the changelog says which rule changed. | R-REL-003 |
| 5 | A security fix needs a breaking CLI change | Ship as a minor release with a deprecation shim where possible; yank only the vulnerable release. | R-REL-024 |
| 6 | MSRV must rise because a dependency requires it | Minor release only, announced. | R-REL-006 |
| 7 | Users pinned to an alpha-era grammar after the edition freeze | `upgrade` migrates; alpha-era files without an `[fairlead]` edition are refused with instructions. | R-REL-002 |
| 8 | A milestone slips and rules tagged for it are already implemented | Rules keep their tag; the feature is gated off until the milestone ships. | Section 3 |
| 9 | The trademark check eliminates `fairlead` | `gusset`, then `lashing`, each with the same check; nothing is published under `fairlead`. | R-REL-040, R-VIS-006 |
| 10 | A prior-art citation cannot be verified | It is removed from the spec and from public copy; the wedge claim is unchanged because it does not rely on it. | R-REL-041, R-VIS-008 |
| 11 | The ablation fixture is Rust only | The result is invalid for the exit gate; rerun with a TypeScript/React repository. | R-ROAD-015 |
| 12 | A 0.1 threshold looks wrong after the first run | It is not changed in place; a dated amendment invalidates the runs made under the old text. | R-ROAD-014 |
| 13 | The measured bytes-per-token ratio differs from 4 | Recorded here; the default ratio changes only with a new edition. | R-ROAD-017 |

## 12. Resolved questions

All questions were accepted by their stated leaning on 2026-10-09 (D-46).

- Q-ROAD-1 (resolved 2026-10-09): edition 1 is frozen at 0.2, when `upgrade` exists; see R-REL-002
- Q-ROAD-2 (resolved 2026-10-09): arm B is report-only, the gate is C against A; see R-ROAD-018
- Q-ROAD-3 (resolved 2026-10-09): the cut order of 8.1 stands; D-38 orders the build but nested scopes and `approve` records are not cut; see R-ROAD-021
- Q-ROAD-4 (resolved 2026-10-09): the pilot includes at least one external repository before 0.2 is complete; see R-ROAD-019
- Q-ROAD-5 (resolved 2026-10-09): the 0.1 thresholds, including R-ROAD-009's 80%, are final and pre-registered (Q-PROD-4); the 0.2 and 0.3 thresholds are reviewed once after the 0.1 ablation; see R-ROAD-014, R-ROAD-020
- Q-ROAD-6 (resolved 2026-10-09; changed 2026-10-11 by D-60): the project and its fixtures are GPL-3.0-or-later from 0.2.0; 0.1.0 was MIT; see R-REL-007
- Q-ROAD-7 (resolved 2026-10-09): rename first, to `fairlead` after the trademark check (D-47); see R-REL-040, R-REL-043
- Q-PROD-3 (resolved 2026-10-09): the literal-leak check is part of 0.1 (D-43); see 3.1, R-ROAD-016
- Q-PROD-4 (resolved 2026-10-09): the measurable 0.1 exit criteria are written before the test is built; see 4.1, R-ROAD-014
- Q-PROD-5 (resolved 2026-10-09): a TypeScript fixture repository is part of the 0.1 exit test; see R-ROAD-015
- Q-PROD-9 (resolved 2026-10-09): `use` lines exist in 0.1, as D-06 lists them; see 3.1
- Q-PROD-1, Q-PROD-2, Q-PROD-6, Q-PROD-7, Q-PROD-8 (resolved 2026-10-09): recorded in [00-vision.md](00-vision.md) Section 11; the publish-side conditions are R-REL-040 to R-REL-046
