# fairlead 0.2 implementation plan (master)

> Release 0.2, "versions, lifecycle, git". Scope: [60-roadmap.md](../../60-roadmap.md) section 3.2. Exit criteria: section 4.2. Rules: every rule tagged `[0.2]` in chapters 10 to 50 (226 by the roadmap count; 279 tagged lines).
>
> **Level of detail.** This plan is written at the level of tasks, interfaces, file formats and test names, not full code; the 0.1 plans spelled out the code. Each task names the rules it implements, the modules it touches, the tests it adds (named after the rule ID, as in 0.1) and when it is done. The executor writes the code against the spec chapter the task cites. Where this plan and a chapter disagree, the chapter and [decisions.md](../../decisions.md) win, and the difference goes into `plans/0.2/errata.md`.

## Starting point

- 0.1 is code-complete: P1 to P9 are merged (see [../0.1/errata.md](../0.1/errata.md), E-1 to E-45). Its exit criteria still need the owner's runs ([../0.1/owner-runbook.md](../0.1/owner-runbook.md)): the controlled test (E2, E3), gates G0 and G1 (E4), the benchmark baseline (E6), the trials (E8), and the pre-publish checklist (60-roadmap 9.5).
- **Tag `v0.1.0` before 0.2 code lands on `main`.** 0.2 freezes edition 1 (R-REL-002): after the tag, generated output of edition 1 is byte-stable (R-EDN-002), so the first 0.2 task records the edition-1 goldens as the stability fixture (R-EDN-051).
- The binary already reserves `move`, `seal` and `upgrade` (exit 2, `crates/fairlead/src/commands/reserved.rs`), and the code table already registers every 0.2 code at milestone `M02` (`crates/fairlead-core/src/check/codes.rs`, the `late!` rows). `check/apply.rs` drops inactive codes, so a check can land before its code is active.

## Global constraints (inherited from 0.1, plus four)

All constraints of [../0.1/00-master.md](../0.1/00-master.md) still hold: Rust 2024, MSRV 1.88, pure checks and renderers, byte-deterministic output, one write path (`write::Transaction`), exit codes only from 15-cli.md R-CLI-003, codes only from the code table, blake3 plus Crockford base32, test names start with the rule ID, `cargo` with two jobs.

New for 0.2:

1. **Git writes only in `seal --commit` and `--tag`**, through the `git` executable as a foreground subprocess, in the same module as the git2 surface (`crates/fairlead-core/src/git.rs`, R-GIT-070, R-GIT-076, D-48). `check --ci` may run `git verify-commit` only when `authority.signed-records = true` (R-LC-040). Nothing else spawns a process.
2. **No network** except `upgrade --fetch`, in an isolated `net` module behind a Cargo feature that is off by default (R-ARCH-124, R-EDN-025). See owner question Q-2.
3. **No stored version state.** `S` and `O` are derived on every read (R-VER-001, R-VER-002); no pointer file, no counter, no SHA of unsealed state (R-GIT-002, R-GIT-004).
4. **Activation.** The first task sets `ACTIVE_MILESTONE = M02` and adds a `PENDING_0_2` list to `tests/codes.rs`. A code on that list is exempt from the "asserted somewhere" test (R-ARCH-151). Each task that lands a check removes its codes from the list; P7's last task requires the list to be empty.

## Document map

| File | Phase | Covers | Tasks |
|---|---|---|---|
| [p1-versions-seal.md](p1-versions-seal.md) | P1 | version model, change docs, draft IDs, ordinals, ledger shards, `scope.lock`, Merkle digests, `seal` | 16 |
| [p2-docs-first.md](p2-docs-first.md) | P2 | coverage check, changed set, rolling and mechanical docs, hotfix debt, draft `def`s | 8 |
| [p3-lifecycle.md](p3-lifecycle.md) | P3 | env, maturity and lifecycle vocabulary, plan flags and due dates, tombstones, graveyard, bindings, signed records | 14 |
| [p4-editions-upgrade.md](p4-editions-upgrade.md) | P4 | edition contract, pins, two-step `upgrade`, sealed sub-root editions, `--fetch` | 9 |
| [p5-git-move.md](p5-git-move.md) | P5 | git case matrix, `--base`, `--staged`, `--since`, `--against`, `check --ci`, dirty-tree rules, `move` | 13 |
| [p6-scopes-styles-authority.md](p6-scopes-styles-authority.md) | P6 | `[delegate]`, `own`, `provides`, `[[attach]]`, Tailwind cascade checks, safelists, agent owners, escalations, version-based expiry, `context --since` | 12 |
| [p7-quality-exit.md](p7-quality-exit.md) | P7 | goldens for the git matrix, merge simulations, migration idempotence, coverage replay, Merkle benchmark, dogfood changelog, exit criteria | 9 |

81 tasks.

## Phase graph

```text
P1 versions + seal -----+--> P2 docs-first ------------\
  (version model,       +--> P3 lifecycle -------------+--> P7 quality + exit
   ledger, seal tx)     +--> P4 editions + upgrade ----+
                        +--> P5 git + move ------------+
P6 scopes/styles/authority (independent of P1 except tasks 10 to 12) --/
```

- P1 comes first: change docs, the ledger and `seal` are what P2 to P5 build on.
- P2, P3, P4 and P5 can run in parallel after P1 task 9 (the seal transaction). P3's tombstones need the seal write-set; P5's `move` needs P3's tombstones (`move --merge`, R-PORT-011).
- P6 tasks 1 to 9 depend only on 0.1. Tasks 10 to 12 need P1.
- P7 runs last. Its harnesses (merge simulator, replay corpus) may start early.

## Module map (new and changed)

| Module (`crates/fairlead-core/src/`) | New in 0.2 | Phase |
|---|---|---|
| `version.rs` | `Version`, `Impact`, `bump`, `VersionLine { sealed, open }`, `derive_line(versions_dir, docs)` | P1 |
| `docs/` (new) | change-doc model: frontmatter schema, status ladder, draft IDs, ordinals, alias map, `covers` | P1, P2 |
| `ledger/` (new) | shard line format, checkpoint lines, continuity check, year files | P1 |
| `seal/` (new) | preflight, compute, due gate, stage, apply, commit point, `--resume`/`--abort`/`--reseal`, git commit/tag | P1 |
| `coverage.rs` | changed set and coverage check | P2 |
| `lifecycle/` (new) | vocabulary, env/maturity resolution, plan flags, due computation, tombstones, graveyard, fingerprints | P3 |
| `edition/` (new) | edition registry, upgrade plan, plan hash, apply | P4 |
| `git.rs` (extended) | base tree diff, shallow flag, common dir, allocation lock, `verify-commit`, commit/tag subprocess | P1, P3, P5 |
| `restructure/` (new) | `move` planner, obligations diff, restructure doc | P5 |
| `check/builtin/` | new built-ins: `versions.rs`, `coverage.rs`, `lifecycle.rs`, `env.rs`, `cascade.rs`, `authority.rs` | P1 to P6 |
| `net/` (new, feature `fetch`) | `upgrade --fetch` client only | P4 |

The CLI gains `seal`, `move` and `upgrade` (replacing `reserved.rs`), plus flags on `check` (`--base`, `--staged`, `--since`, `--against`, `--ci`), `context` (`--since`), `sync` (`--migrate-vocab`, `--purge` is 0.3), `new` (`--hotfix`, `--reid`) and `explain` (file version, `pr-template`).

## Owner questions (answer before the phase that needs them)

| # | Question | Needed by | Recommendation |
|---|---|---|---|
| Q-1 | 0.2 ships the `upgrade` machinery, but edition 2 does not exist yet. How is it tested? | P4 | Ship 0.2 on edition 1 (now frozen). Test the machinery against a test-only edition 2 behind a `test-edition` Cargo feature, with one mechanical change (for example a renamed generated header key). The first real edition 2 is a later decision. |
| Q-2 | `upgrade --fetch` needs an HTTP client (R-ARCH-124) and a place to publish migration data. | P4 | Defer the client: in 0.2 the flag parses and exits 2 with "no remote migration data is published for this binary". Every supported migration is embedded (R-EDN-025). Revisit when edition 2 exists. |
| Q-3 | Integration branch for `seal-where = "integration"`. | P1 | Keep the spec default `integration-branch = "main"` (20-versions sample). Detect it by the current branch name through git2. |
| Q-4 | The eight-week pilot with at least one external repository (R-ROAD-019, gate G2). | P7 | Human: choose the repositories and start the pilot when P1 to P3 are usable. |
| Q-5 | Review of the 0.2 and 0.3 thresholds after the 0.1 ablation (R-ROAD-020). | P7 | Human: after E2 data exists, freeze them by a dated amendment to 60-roadmap. |
| Q-6 | Signed records (`authority.signed-records`) need GPG or SSH signing on the CI runner for tests. | P3 | Unit-test the parser of `git verify-commit` output. Run one integration test with an SSH-signed commit made in the test, and skip it when `ssh-keygen` is missing. |

## Definition of done for 0.2

All rows of [60-roadmap.md](../../60-roadmap.md) 4.2 hold. P7 maps each row to a test or a human run:

| # | Criterion (short) | Verified by |
|---|---|---|
| 1 | 50 simulated parallel merges with seals: no unresolved conflict after `sync` | P7-T2 |
| 2 | Edition migration idempotent on every golden repository | P7-T3 |
| 3 | Docs-first coverage: at most 5% wrong flags on a 100-commit replay | P7-T4 |
| 4 | Old-edition checkout checkable by the new binary (through `upgrade`), too-new refused | P4-T2, P7-T3 |
| 5 | Shallow clone, rebase, squash, cherry-pick, revert goldens | P7-T1 |
| 6 | Eight-week pilot, at most 30% stale or expired exceptions | owner (Q-4) |
| 7 | All 0.1 criteria still hold | P7-T8 (0.1 suites unchanged and green) |
| 8 | Merkle digest and ledger layout benchmarked at 100,000 files | P7-T5 |
| 9 | 50 `merge=union` merges: checkpoint continuity passes or is repairable | P7-T2 |

Also: `PENDING_0_2` is empty, dogfood passes with version-based waivers, and the release changelog is generated from fairlead's own change docs (R-REL-025).

## Risks

| Risk | Mitigation |
|---|---|
| `seal` is the first multi-scope, multi-file transaction with a commit point. A bug corrupts history. | Reuse the 0.1 `write::Transaction` and journal (`write/tx.rs`, `write/journal.rs`). Add fault injection at every rename, as the 0.1 tx tests do. `seal --resume`/`--abort` are tested from each injected failure. |
| Version derivation and collisions across branches are subtle. | P7's merge simulator runs real `git` in temp repos with seeded branch histories. Collision codes `HN-V001` to `HN-V007` each get a golden. |
| Graveyard fingerprints (min-hash) are nondeterministic if seeded badly. | A fixed hash family from blake3 with constant keys, and a proptest asserting the same bytes give the same fingerprint on every platform. |
| `move` edits authored text. A wrong rewrite is a silent corruption. | Plan-then-apply, like `init`. The obligations diff is always printed. Rewrites are limited to fairlead's own text (R-CLI-044), and every rewritten directive is re-extracted and compared before commit. |
| Scope creep: 0.2 is 226 rules, against 308 for 0.1. | The phases are cut by chapter. P6 holds everything that can slip to 0.2.x without breaking the edition (the style checks), in the roadmap's cut order (8.1). |
