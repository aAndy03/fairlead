# P5: git integration and `move`

> Chapters: [23-git-integration.md](../../23-git-integration.md) (all [0.2] rules); [15-cli.md](../../15-cli.md) R-CLI-022 (git-aware flags), R-CLI-040, R-CLI-044, R-CLI-047; [32-portability-and-restructure.md](../../32-portability-and-restructure.md) R-PORT-004 to R-PORT-006, R-PORT-010 to R-PORT-013 and the operation table; [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) R-CHK-058; [40-agent-integration-and-trust.md](../../40-agent-integration-and-trust.md) R-AGT-020. Decisions D-11, D-25, D-38, D-48, D-50, D-52.

## Goal

The git case matrix behaves as specified: dirty trees, merges, rebases, shallow clones, worktrees and submodules. `check` gains `--base`, `--staged`, `--since <rev>`, `--against` and `--ci`. `move` restructures scopes and folders transactionally, prints an obligations diff, and rewrites only fairlead's own text.

## Design

- All git reads stay in `git.rs` through git2. Additions: `diff_base_to_workdir(base)`, `is_shallow()`, `common_dir()`, `unmerged_paths()`, `head_state() -> {Branch(name), Detached}`, `staged_paths()`, `file_at(rev, path)`. Each thread opens its own `Repository` (R-GIT-076).
- The allocation lock is `fairlead-alloc.lock` in the common git directory. `seal`, `move` and `upgrade` take it in addition to `.fairlead/lock`. Without git only the per-worktree lock applies (R-GIT-052, R-GIT-073, D-52). It reuses the 0.1 lock format (A-39: three LF lines).
- `restructure::plan(loaded, op) -> MovePlan { renames, text_rewrites, membership, obligations, code_sites, tombstones, refusals }` is pure. `restructure::apply` uses the 0.1 transaction plus a journal (R-PORT-005).
- The obligations diff is the effective-view diff of 0.1's `SCOPE-REPARENTED` machinery (`registry/view.rs`), rendered as `+ rule.x`, `- rule.y` and `~ policy.z`, plus `code-site <path>:<line>` lines (R-CLI-044).

## Tasks

**T1. Locks across worktrees.** R-GIT-022, R-GIT-052, R-GIT-073, D-52. Add the allocation lock and wire it into `seal` (replacing the P1-T16 stub), `move` and `upgrade`. Tests: `r_git_052__second_worktree_seal_refused`, `r_git_052__no_git_uses_worktree_lock_only`.

**T2. Walk exclusions and paths.** R-GIT-020, R-GIT-021, R-GIT-078:
- Linked worktrees and submodules are excluded from the walk: a `.git` file or a gitlink (mode 160000), including `.claude/worktrees/`.
- NFC names and forward slashes.
- A case-only rename is a move.
- Junctions are not followed.

Tests:
- `r_git_021__nested_worktree_excluded`
- `r_git_021__submodule_excluded`
- `r_git_078__case_only_rename_is_move`

**T3. Dirty-tree and merge-state rules.** R-GIT-060 to R-GIT-065, R-GIT-011:
- Readers work in any state.
- `sync` and `new` may overwrite generated files, but only the authored files they create.
- `move` and `upgrade --yes` refuse a dirty write-set unless `--allow-dirty` is given.
- `seal` refuses unless the only diff is its write-set.
- During a merge, `sync` regenerates conflict-marked generated files and refuses unmerged authored files, listing them.
- On a detached HEAD, `seal` refuses.

Tests: one per table row.

**T4. Git-aware `check` flags.** R-CLI-022, R-GIT-041, R-GIT-042, R-AGT-014, R-AGT-020:
- `--staged` limits reporting to the staged paths.
- `--base <ref>` sets the coverage base and the changed set.
- `--since <rev>` reports `must-review` items: a lowered pin, a deleted `ovr`, an added `ovr`, and edits to instruction files or authored `fairlead/` docs, with rule-bearing edits listed first.
- `--against <ref>` warns when local sealed version files differ from the upstream copy.

Tests: one per flag, in temp repositories.

**T5. `check --ci`.** R-CHK-058, R-VER-057, R-GIT-054:
- CI mode computes `HN-D001` on the merged tree and runs the P1-T14 version checks.
- It runs `verify-commit` only with signed records (P3-T13).
- A skip variable such as `FAIRLEAD_SKIP` is ignored when `CI` is set. Each use is listed in the summary.

Tests: `r_git_054__skip_variable_ignored_under_ci`, `r_chk_058__ci_runs_no_subprocess_by_default`.

**T6. Shallow clones and no git.** R-GIT-006, R-GIT-040, R-GIT-074:
- Shallow detection uses the repository flag.
- Scope-level state works.
- Coverage and history report `H-GIT-SHALLOW` (HN-V010) and the affected check does not exit 0.
- Without git, `check` and `context` run, and coverage and history say "unavailable".

Tests: `r_git_040__shallow_never_silently_passes`, `r_git_006__no_git_reports_unavailable`.

**T7. `init` and `.gitattributes`.** R-GIT-012, R-GIT-013. `init --yes` appends three `.gitattributes` lines, and the plan shows them first:
- `**/fairlead/gen/** linguist-generated=true -diff`
- `**/fairlead/ledger/** merge=union`
- `eol=lf` for the managerial folder

Existing lines are not duplicated. Golden: update the 0.1 init trycmd cases, and record the change in the errata.

**T8. `move` for files, folders and scopes.** R-CLI-040, R-CLI-044, R-PORT-004, R-PORT-005, R-PORT-006:
- `move SRC DST [--dry-run]`. The plan rewrites directives, `covers`, globs in docs and control files, and membership shards.
- Code sites are listed, never rewritten.
- A loosening reparent is refused unless an `approve` record exists.
- Grants from the old ancestors lapse, and the inert `ovr`s are listed.
- The operation is transactional with a journal. Half-applied state is `MOVE-INCOMPLETE` (HN-M008).

Tests:
- `r_cli_044__move_rewrites_own_text_lists_code_sites`
- `r_port_004__loosening_reparent_refused`
- `r_port_005__interrupted_move_reported`
- `r_port_006__lapsed_grant_listed_inert`

**T9. Restructure doc.** R-VER-032, R-CHK-044, R-GIT-075. `move` writes a generated restructure change doc whose `covers` lists both the old and the new paths. It covers the move under docs-first. Test: `r_chk_044__move_writes_restructure_doc`.

**T10. `move --merge`.** R-PORT-011, R-RET-070 (0.2 part):
- Colliding IDs are renamed through a `replaced-by` tombstone alias (P3-T9), never by silently picking a winner.
- Reserved IDs of both sides are kept. A reserved ID colliding with a live one is an error.
- Doc ordinals are renumbered with aliases. Ledgers are concatenated into the receiving scope's open year with `scope=<origin>`.

Tests: `r_port_011__collision_renamed_through_alias`, `r_port_011__reserved_vs_live_is_error`.

**T11. `--demote` and `--delete`.** R-PORT-010, R-PORT-012, R-LC-023:
- Demote turns defs and ovrs into ancestor entries with `applies`, leaves a scope tombstone and an alias.
- Delete requires zero live references. Otherwise it lists them and refuses, unless `--force` was recorded by `approve`. Live references to deleted items are `DANGLING` (HN-M007).
- Group and ungroup are no-ops for scope semantics.

Tests: one per operation.

**T12. `move --detach`.** R-CLI-047, R-PORT-013:
- Refuse (exit 4) unless the scope is `portable = true` or inherits nothing.
- Write the inherited view into `gen/scope.lock` as a `detached` snapshot layer and mark the scope detached in the parent shard.
- Old grants lapse. The scope is reported as `DETACHED` (HN-M011, info).

Tests: `r_port_013__detach_refused_unless_portable_or_empty`, `r_port_013__detached_snapshot_written`.

**T13. CLI `move` and codes.** Replace the reserved verb with the full flag set. `--cut`, `--copy`, `--attach` and `--adopt` parse and exit 2 "arrives in 0.3". trycmd cases:
- `move-plan`
- `move-folder`
- `move-merge`
- `move-detach-refused`
- `move-dirty-refused`

Remove `HN-M007`, `M008`, `M011` and `HN-V010` from `PENDING_0_2`.
