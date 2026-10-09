# P2: docs-first coverage

> Chapters: [20-versions-and-seal.md](../../20-versions-and-seal.md) R-VER-030 to R-VER-039, R-VER-047, R-VER-048, R-VER-060; [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) R-CHK-034, R-CHK-044, R-CHK-045; [40-agent-integration-and-trust.md](../../40-agent-integration-and-trust.md) R-AGT-031; [21-environments-maturity-lifecycle.md](../../21-environments-maturity-lifecycle.md) R-ENV-006. Decision D-15.

## Goal

Every changed, non-exempt file is covered by the `covers` of at least one non-abandoned change doc. This proves that docs are present, not that they were written first (R-VER-039). Severity ramps `observe`, then `warn`, then `enforce`. The default is `warn`, and a scope can opt in to `enforce`.

## Design

- `coverage::changed_set(repo, base) -> ChangedSet` returns the files that differ between the base tree and the working directory, index included. Untracked files are included and gitignored ones excluded (R-GIT-075, R-VER-033).
- The default base is the commit that introduced `versions/<S>.md`, found through git2 history of that path. `--base <ref>` overrides it.
- Without git, or in a shallow clone missing the base, the result is `Unavailable`. It is reported as `H-GIT-SHALLOW` (HN-V010) and never passes silently (R-GIT-040, R-GIT-006).
- `coverage::check(changed, docs, level_by_scope) -> Vec<Diagnostic>`, pure.
- The coverage unit is the file. In 0.2 only globs count; IDs in `covers` are validated, not counted (R-CHK-045).
- The code is `HN-D006` (`doc.uncovered`, M02). Its severity comes from the scope's `docs-first.level`, or from the scope's default maturity when no level is set (R-ENV-006).

## Tasks

**T1. Changed set.** Rules R-VER-033 and R-GIT-075. Generated files, `.fairlead/`, gitignored output and `docs-first.exempt` globs are exempt. Rename detection is used only to de-duplicate. Tests:
- `r_ver_033__default_base_is_the_commit_of_the_last_version_file`
- `r_ver_033__exempt_globs_and_generated_files_are_skipped`
- `r_git_075__untracked_included_ignored_excluded`

**T2. Coverage check and ramp.** R-VER-032, R-VER-034, R-CHK-034, R-CHK-045, R-ENV-006:
- A file counts as covered when it matches a `covers` glob of a non-abandoned doc: `draft`, `ready`, or sealed in `O`.
- A child may raise the level but not lower an ancestor's without an `ovr`.

Tests:
- `r_ver_032__one_doc_covers_a_shared_file`
- `r_ver_034__child_cannot_lower_level_without_ovr`
- `r_chk_045__ids_in_covers_are_validated_not_counted`
- `r_env_006__lowest_maturity_scope_defaults_to_observe`

**T3. Wire coverage into seal preflight and `check`.** Replaces the P1-T8 stub. The coverage summary goes in the seal output (`coverage 41/41`). Remove `HN-D006` and `HN-V010` from `PENDING_0_2`. Tests: `r_ver_050__seal_refuses_on_uncovered_ready_docs`, `r_git_040__shallow_coverage_never_exits_0`.

**T4. Special doc kinds.** R-VER-035 to R-VER-038:
- A rolling `chore` doc covers recurring changes. At seal it is recorded in the version and a successor draft with the same `covers` is created.
- `kind = "mechanical"` with `tool=` covers only files whose whitespace- and comment-normalised hash is unchanged. The normaliser is the 0.1 lexer's `comment_spans`.
- Covers broader than the home subtree give `H-DOC-BROAD` (HN-V008).
- `**` at the root needs `kind = "bulk"` with a reason.
- `kind = "revert"` with `of=` is auto-accepted.

Tests: one per rule.

**T5. Move and rename coverage.** R-CHK-044, R-VER-032. The restructure doc written by `move` (P5-T9) covers the moved paths. Until P5 lands, test it with a fixture restructure doc: `r_chk_044__restructure_doc_covers_moved_paths`.

**T6. Hotfix backfill debt.** R-VER-047, R-VER-048:
- `new change --hotfix`, only when `docs-first.hotfix = true`, writes a draft with `hotfix = true`.
- The deadline is `created` date + `hotfix-window` whole UTC days, evaluated against `--as-of`.
- `H-HOTFIX-DEBT` (HN-V015) warns while open and is an error after the deadline.
- `context` lists open debts with their deadline.
- There is no bypass variable, and `strict-order` is hook-only.

Tests: `r_ver_047__debt_warns_then_errors_after_window`, `r_ver_047__context_lists_open_debts`, `r_ver_048__check_never_enforces_order`.

**T7. Draft `def`s do not satisfy references.** R-VER-060. A `def` only in a `draft`, `ready` or `abandoned` doc leaves the reference dangling (`HN-R001`), with a note naming the draft. Test: `r_ver_060__draft_def_is_not_a_declaration`.

**T8. Shipped wording and the PR template.** R-VER-039, R-AGT-031:
- `explain docs-first` states "coverage proves docs-present, not docs-first".
- `explain pr-template` prints the PR template text. `init` does not write it.

trycmd: `explain-docs-first`, `explain-pr-template`.
