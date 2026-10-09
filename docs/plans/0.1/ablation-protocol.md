# 0.1 controlled-test protocol (pre-registered)

> Status: pre-registered · Committed before the fixture is built (R-ROAD-014, Q-PROD-4) · Thresholds: 60-roadmap 4.1, 5, 7.1, 7.2 · Owner: maintainer · Amendments: dated, appended at the end, invalidate earlier runs

## Thresholds (machine-read by xtask; do not reword the keys)

threshold.pass_reduction_pct = 50
threshold.weak_below_pct = 20
threshold.hygiene_min_pct = 80
threshold.precision_sample = 200
threshold.precision_max_false_pct = 2
threshold.replay_introducing = 50
threshold.replay_recall_min_pct = 70
threshold.replay_unrelated = 20
threshold.replay_fp_max_pct = 5
threshold.syntax_card_sample = 100
threshold.syntax_card_max_invalid_pct = 5
threshold.adoption_max_minutes = 15
threshold.e6_cold_check_max_s = 60
threshold.e6_warm_check_max_s = 5
threshold.e6_changed_max_s = 1
threshold.e6_peak_rss_max_mb = 500
threshold.ui_tasks = 10
threshold.backend_tasks = 10
threshold.runs_per_task = 3

## E2: wedge effect (ablation, R-ROAD-005, R-ROAD-018)

- Tasks: 20 (10 UI on the `ts-react` fixture, 10 backend on the `fullstack-tauri` and `rust-workspace` fixtures). Each task has a natural path to a hardcoded declared value (`ablation/tasks.toml`).
- Arms: A, no fairlead artifact. B, a hand-written static AGENTS.md with the same declarations in prose (`ablation/arms/b-agents.md`). C, the output of `fairlead context <path>` inserted at session start.
- Runs: 3 per task per arm, fresh session each, model and settings recorded per run, run order randomised with a recorded seed.
- Primary metric: count of hardcoded declared values in the added lines of the final diff, found by `cargo xtask ablation-score`. The scorer does not call `fairlead check` (R-ROAD-016).
- Decision: reduction of C against A. At least 50% passes. Below 20% is weak and fires kill assumption 2. Arm B is reported, never gated (R-ROAD-018).
- Blinding: the scorer receives diffs under opaque file names. The mapping to arm and task is written after scoring.
- Literal-leak check and `use` lines are enabled in the fixture (R-ROAD-016).
- Threats recorded with each result: model drift between runs, fixture ease, scorer bias, agents reading `fairlead/` directly.

## E3: exception hygiene (R-ROAD-009, roadmap 7.2)

- Fixture: three nested scopes, one sealed key, one waivable rule, one open key. Twelve tasks, each requiring a deviation.
- Per task, one of: (a) avoided by an allowed route, (b) declared `ovr` with `why`, `owner` and expiry, (c) silent deviation, (d) attempt against a sealed key, with the result inert and an escalation produced.
- Pass: at least 80% of the conflicting tasks end in (b), a refusal, or an escalation, in arm C, against arm A.
- A reviewer given only the diff and the ledger lists every deviation. Target: all.

## R-ROAD-003 and R-CHK-051: precision

- Blind review of 200 findings drawn by seeded random sample from the terse output of three real repositories (TypeScript/React, Rust, mixed). The reviewer sees the finding text without its code.
- At most 2% judged false. Zero false dangling-reference reports by automated test (`r_chk_051__no_false_dangling_references`).

## R-ROAD-004: recall on history (replay)

- 50 introducing commits and 20 unrelated commits, from at least three repositories, one TypeScript/React.
- Recall: a literal-leak finding at the introducing commit that did not exist in the parent. At least 70% of the 50.
- False positives: an unrelated commit counts as a false positive when it adds a new literal-leak finding. At most 5% of the 20 (one commit).

## E8: humans and agents write the syntax

- Adoption trial: a developer who has not seen the specification installs the binary, runs `init`, writes a baseline and reaches the first green `check`. Elapsed minutes and the number of documents consulted are recorded. Limit: 15 minutes.
- Syntax-card trial: fresh agent sessions given only the syntax card (`fairlead explain syntax`) write 100 directives. Scored by `cargo xtask syntax-card-score`. Limit: at most 5% invalid.

## E6 and R-ROAD-017: benchmarks

- 100,000-file generated tree (`cargo xtask bench-corpus generate`). Cold check at most 60 s, warm at most 5 s, `--changed` at most 1 s, peak RSS at most 500 MB. `--files-from` is reported against the R-CHK-016 goal of about one second, not gated.
- The bytes-per-token ratio is measured with the target model's tokenizer on sample `context` slices. The result is recorded in 60-roadmap section 11.

## Results log

Runs made under this text are recorded in `docs/plans/0.1/results/` by the maintainer. An amendment is appended below with a date, the changed key and the reason. Runs made before an amendment are invalid for the gate.
