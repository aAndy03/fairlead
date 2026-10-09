# P1: version model, change docs, ledger and `seal`

> Chapters: [20-versions-and-seal.md](../../20-versions-and-seal.md) (all), [33-retention-and-compaction.md](../../33-retention-and-compaction.md) R-RET-006 and R-RET-036, [23-git-integration.md](../../23-git-integration.md) R-GIT-002 to R-GIT-005 and R-GIT-030 to R-GIT-036, [11-files-and-classes.md](../../11-files-and-classes.md) R-FILE-006 and the layout. Decisions D-13, D-14, D-22, D-23, D-37, D-42, D-49.

## Goal

A codebase gets a derived version line (`S` sealed, `O` open), change docs with draft IDs and ordinals, per-scope ledger shards with checkpoint lines, `gen/scope.lock`, Merkle digests, and an all-or-nothing `fairlead seal` whose commit point is `versions/<O>.md`.

## Design

### Data and files

| File | Class | Written by | Format |
|---|---|---|---|
| `fairlead/versions/<MAJOR.MINOR.PATCH>.md` (root only) | authored after seal, immutable | `seal` (last) | TOML frontmatter `version prev bump sealed-at scopes docs carried dirty` plus a generated body (R-VER-053) |
| `<scope>/fairlead/docs/d-<draft>-<slug>.md` | authored | `new change` | frontmatter `id kind status impact scope covers created` (+ `sealed body-hash ordinal` at seal) |
| `<scope>/fairlead/ledger/hist/<year>.hl` | ledger | `seal` | header `# scope=<id> year=<y> prev=<digest> edition=<e>`, terse record lines ending `d=<digest>`, `# ckpt v= n= d=` per sealed version |
| `<scope>/fairlead/gen/scope.lock` | generated | `sync`, `seal` | `reconciled=<version>`, `ancestor-digest=<hash>` (R-VER-015) |
| `<scope>/fairlead/gen/alias` | generated | `sync`, `seal` | `d-<draft> ord=<n>` per sealed doc (R-VER-046) |

The year of a ledger file is the UTC year of `sealed-at` of the seal that writes it. `sealed-at` is the only new wall-clock read. It goes through `time.rs` (a second function next to `AsOf::today_utc`), so the dogfood `rule.no-clock` waiver covers it. Extend that `ovr`'s `why` in the same commit; the owner re-approves (A-68).

### Types (in `fairlead-core`)

```text
version::Version { major, minor, patch }      Ord, Display "1.2.3", parse rejects two-part values (R-VER-070)
version::Impact { Fix, Add, Break }           max over pending docs
version::bump(s: Option<Version>, impact, start) -> Version       major-0 shift (R-VER-006)
version::VersionLine { sealed: Option<Version>, open: Version }   derive_line(root_folder, docs) (R-VER-001..003)
docs::ChangeDoc { id: DraftId, kind, status, impact, scope, covers, created, sealed, body_hash, ordinal, rolling, hotfix, of, supersedes, backport_of }
docs::DraftId(String)                          "d-" + 8 base32, random at `new` (R-VER-040)
docs::Status { Draft, Ready, Sealed, Abandoned }
ledger::RecordLine, ledger::Checkpoint { v, n, d }, ledger::YearFile { header, lines, checkpoints, end }
seal::SealPlan { open, bump, docs, carried, ordinals, write_set, due_blockers }   built purely from Loaded + VersionLine
```

`seal` runs as a pure planner (`seal::plan(&Loaded, &VersionLine, as_of) -> Result<SealPlan, Refusal>`) plus the existing `write::Transaction` for staging and apply. The version file is the last rename (R-VER-050 step 6). The planner reads no clock: `sealed-at` is passed in.

## Tasks

**T1. Activate 0.2 and pin edition-1 output.** Set `ACTIVE_MILESTONE = M02`. Add `PENDING_0_2` (every M02 code) to `tests/codes.rs`. Bump the workspace version to `0.2.0-dev`. Copy the expected outputs of the 11 goldens to `tests/editions/1/` and add `r_edn_051__edition_1_generated_output_is_byte_stable`, which fails on any byte difference. Done when the 0.1 suites pass unchanged.

**T2. `Version` and the bump table.** Module `version.rs`, covering R-VER-006 and D-42. Tests:
- `r_ver_006__major_zero_shifts_one_level_down`
- `d_42__two_part_value_is_not_a_version`
- proptest `r_ver_002__bump_is_monotonic`

**T3. Version line derivation.** Rules R-VER-001 to R-VER-004:
- `S` is the maximum over `versions/*.md`.
- `O` is `bump(S, max impact of non-abandoned pending docs)`.
- A fresh codebase gives `S = none` and `O = version.start`.
- `version.mirror` warns on divergence and never fails.

Tests: `r_ver_001__sealed_is_max_of_version_files`, `r_ver_003__fresh_codebase_opens_at_start`, `r_ver_004__mirror_divergence_warns_only`.

**T4. Change-doc model and `new change`.** Covers R-VER-012, R-VER-030, R-VER-031, R-VER-040, R-VER-049, R-SYN-052:
- A frontmatter schema for `kind = "change"` (also `adr`, `revert`, `upgrade`, `early-removal`, `revive`, `bulk`, `mechanical`, `chore`).
- `new change "<title>"` draws a random draft ID, refuses one already present in the tree, and writes `created = "<O>"`.
- The ID grammar admits `d-<8 base32>` and `<scope>:d-<draft>` in the doc-reference keys only.

Tests:
- `r_ver_040__draft_id_is_random_and_unique_in_tree`
- `r_ver_012__created_is_the_open_version_and_immutable`
- `r_ver_049__draft_word_only_in_doc_reference_keys`
- `r_ver_031__ready_requires_covers_ids_and_impact`

**T5. Ordinals and the alias map.** R-VER-041, R-VER-042, R-VER-046:
- Ordinals are assigned per scope and kind as max + 1, ordered by (`created`, draft ID).
- The file is renamed to `<ordinal>-<slug>.md`, with the draft ID kept in `id`.
- `gen/alias` is a pure function of the docs.
- `H-ID-COLLISION` (HN-V006) for the same draft ID with a different body; `H-ORD-DUP` (HN-V007).
- `new --reid` renames the later doc and rewrites references inside docs.

Tests: one per rule, plus `r_ver_046__alias_map_is_a_pure_function_of_docs`.

**T6. Ledger year files.** R-FILE-006, R-RET-036, R-GIT-015:
- Parse and render `.hl` files.
- The record digest is the first 8 base32 characters of blake3 over the normalised record line without its `d=`.
- The checkpoint digest is blake3 over the previous checkpoint digest plus the record digests since then.
- `check` verifies ascending `v=`, `n=` counts and `d=` digests. A break is `HN-G003`.

Tests: `r_ret_036__checkpoint_continuity_detects_each_break_kind` (one case per field), `r_file_006__sync_never_rewrites_a_ledger`.

**T7. Merkle digests and `scope.lock`.** R-VER-015, R-VER-016, R-VER-020, R-VER-021:
- A scope digest hashes the sorted entries of its own files plus its children's digests. It is computed from the 0.1 content-hash cache, and unchanged files are not re-read.
- The ancestor digest covers only the effective inherited view.
- `sync` writes `gen/scope.lock` for its own scope only.
- `H-SYNC-ANCESTOR` (HN-V004) is info for `ancestor-grace` seals, warn afterwards, and error at seal.

Tests:
- `r_ver_016__changing_one_file_rehashes_only_its_path_to_root`
- `r_ver_016__unchanged_files_are_not_read`
- `r_ver_020__ancestor_drift_ramps_info_warn_error`
- `r_ver_021__sync_never_touches_ancestor_files`

**T8. Seal planner.** R-VER-050 steps 1 to 3, R-VER-043, R-VER-045, R-VER-058, R-RET-006:
- Preflight: not detached; no unmerged paths; clean except the write-set; check gate; no `HN-F001`; coverage. Coverage is a stub until P2-T3 and returns pass.
- Bump from impacts, ordinals, and symbolic targets resolved.
- The due and expiry gate is a stub until P3-T6.
- Carried drafts, the sweep report and escalation listing.

Each refusal is a `Refusal` with exit 4. Tests: one per preflight refusal (`r_ver_050__refuses_when_<cause>`), `r_ver_043__unready_drafts_are_carried`, `r_ver_045__sweep_report_is_advisory`, `r_ver_058__escalation_older_than_grace_blocks`.

**T9. Seal transaction.** R-VER-050 steps 4 to 6, R-VER-051, R-VER-059:
- Stage the write-set in `.fairlead/tx/<id>/`. Apply in a fixed order with `versions/<O>.md` last.
- `H-TX-INCOMPLETE` (HN-V005) for stray shards without a version file.
- `seal --resume` and `--abort`.

Fault-inject a failure at every rename (pattern of the 0.1 tx tests). Tests:
- `r_ver_051__crash_before_commit_point_leaves_no_version`
- `r_ver_051__resume_finishes_and_abort_cleans`
- `r_ver_059__write_set_is_exact`
- `r_ver_051__rerun_is_idempotent`

**T10. Rollups and the version file body.** R-VER-052, R-VER-053:
- Per-scope checkpoint counts (waivers, deprecated-reference burn-down, exceptions added and expired) plus a digest. Ancestors carry subtree sums.
- Version-file body lines are rendered from sealed docs.
- The optional `changelog.file` aggregate is a generated file.

Tests: `r_ver_052__ancestor_rollup_sums_children`, `r_ver_053__version_body_lists_each_sealed_doc`.

**T11. Immutability.** R-VER-054, R-VER-012:
- `body-hash` is written once at seal.
- A mismatch is `H-SEAL-TAMPER` (HN-V003).
- `mutable = true` doc classes allow typo fixes, each recorded as a changelog line.

Tests: `r_ver_054__edited_sealed_doc_is_tamper`, `r_ver_054__mutable_class_allows_recorded_fix`.

**T12. `seal --commit` and `--tag`.** R-GIT-003, R-GIT-005, R-GIT-076, R-CLI-010:
- `git commit` runs as a foreground subprocess with trailer `Fairlead-Version: <O>`. `--tag` creates `fairlead/<O>`.
- A missing `git` fails with a message.
- Nothing else spawns a process: extend the 0.1 static guard test to allow exactly this call site.

Tests: `r_git_003__commit_only_with_flag`, `r_git_076__missing_git_fails_commit_with_message`.

**T13. Collisions and continuity across branches.** Covers R-GIT-030 to R-GIT-036 and R-VER-055:
- `H-VER-COLLISION` (HN-V001) covers two version files with one version, and an add/add conflict.
- `H-VER-GAP` (HN-V002) is a `prev` continuity break. That includes a cherry-picked seal with no predecessor (R-GIT-032).
- `seal --reseal` renumbers an unintegrated seal and refuses one already present on the integration line.
- `seal-where = "integration"` checks the current branch against `integration-branch` (Q-3).
- Reverting a seal commit un-seals (R-GIT-034).
- `backport-of` is accepted only with an identical body hash.

Tests: one per rule, using real `git` in temp repositories (the `tests/testkit` helpers).

**T14. `check --ci` version checks.** R-VER-057, R-VER-044:
- Fail when a version file lists unready docs or its scopes have inconsistent digests.
- `HN-D001` is computed on the merged tree. There are no claim files.

Tests: `r_ver_057__ci_fails_on_inconsistent_seal`.

**T15. `explain <file>` virtual file version.** R-VER-014, R-CLI-036. The last change is read from the ledger plus `git log -- path` through git2. Without history the output is "unavailable". Tests: `r_ver_014__file_version_from_ledger_and_git`, `r_ver_014__unavailable_without_history`.

**T16. CLI `seal`.** R-CLI-041, R-CLI-002:
- Replace the reserved verb with `seal [--dry-run|--yes] [--commit] [--tag] [--resume|--abort|--reseal] [--allow-dirty] [--as-of D]`.
- Plan-only without `--yes`.
- The text output follows the 20-versions sample.
- Writers take the allocation lock in the common git dir (R-GIT-052; the lock module from P5-T1, stubbed here as `.fairlead/lock` only).

trycmd cases:
- `seal-plan`
- `seal-yes`
- `seal-dirty-refused`
- `seal-detached-refused`
- `seal-resume`

Remove `HN-V001` to `V007`, `HN-G003` and `HN-X016` from `PENDING_0_2` as their checks land.

## Spec issues to log

- 33-retention's tombstone sample uses `repl = ...`, while R-LC-020 names the field `by`. Use `by`, and record the discrepancy in `plans/0.2/errata.md`.
- `sealed-at` is a timestamp in the version file. The plan treats it as the single 0.2 clock read, beside `AsOf::today_utc`.
