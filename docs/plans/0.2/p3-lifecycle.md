# P3: environments, maturity, lifecycle, tombstones, graveyard

> Chapter [21-environments-maturity-lifecycle.md](../../21-environments-maturity-lifecycle.md) (all). Also [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) R-CHK-059; [12-directive-syntax.md](../../12-directive-syntax.md) R-SYN-026, R-SYN-042, R-SYN-048; [33-retention-and-compaction.md](../../33-retention-and-compaction.md) R-RET-040 to R-RET-042; [14-context.md](../../14-context.md) R-CTX-014; [15-cli.md](../../15-cli.md) R-CLI-046. Decisions D-16, D-49, D-50, D-51.

## Goal

Every ID'd item has three orthogonal axes:
- **env:** where it exists.
- **maturity:** how settled it is, per env.
- **lifecycle:** the plan, and its actual state.

Plan flags become due dates against `O`. Removed IDs leave permanent tombstones and stay reserved. Rejected proposals leave graveyard entries whose fingerprints catch re-introduction.

## Design

- `lifecycle::Vocab` comes from the root control tables `[env] order`, `[env.alias]`, `[maturity]`, `[lifecycle.alias]` and `[bind.<env>]`. These are root-only sealed keys (R-ENV-001). A child's attempt becomes an escalation proposal, never applied.
- `lifecycle::ItemAxes { envs: BTreeSet<EnvSlot>, maturity: BTreeMap<EnvSlot, Rank>, state: State, plan: Option<PlanFlag> }` is resolved from `def` keys `in=`, `mat=` and `bind=`, plus `flag` directives. Defaults follow the nearest scope policy (R-ENV-005).
- `lifecycle::due(plan, open, as_of, warn_window) -> DueState { NotDue, Approaching, Due }` (R-LC-004, R-LC-005).
- Tombstones: `fairlead/tombstones/<year>.toml`, TOML `[[tomb]]` with `id removed by why paths` (+ `deleted-by at d` for user deletions). The file is append-only. It is parsed in the load stage and joins the registry as `Retired` entries.
- Graveyard: TOML-frontmatter docs in `docs/graveyard/` with `id state at what why match revisit-after fingerprint`. The `grv` kind is reserved.
- A fingerprint is computed once, at the seal that records the removal: `ids` are the normalised identifiers, and `minhash` is 32 values over k=5 token shingles. The hash family is blake3 keyed by the constant index 0..31, so fingerprints are deterministic on every platform.
- A candidate matches when the identifier overlap is at least 0.6 or the shingle similarity is at least 0.5. Both thresholds are configurable.

## Tasks

**T1. Vocabulary.** R-ENV-001 to R-ENV-004, R-ENV-037:
- Parse the tables.
- Ordered envs, and ranked maturity with gaps allowed.
- Aliases stay valid forever.
- Unknown words are `H-VOC-UNKNOWN` (HN-E004), and so are theme words used as envs.
- `sync --migrate-vocab` rewrites to canonical words through the transaction.

Tests:
- `r_env_002__envs_ordered_by_exposure`
- `r_env_003__alias_valid_unknown_word_reported`
- `r_env_037__theme_word_is_not_an_env`
- `r_env_001__child_vocab_becomes_escalation`

**T2. Item axes.** R-ENV-005, R-ENV-010, R-ENV-011, R-ENV-012, R-ENV-040, R-ENV-041:
- `in=`, `mat=` (also the per-env form) and `bind=` on `def`. The `env` verb gates a region.
- Defaults come from the scope policy. Otherwise a missing maturity is `H-MAT-MISSING` (HN-E005).
- `H-MAT-ORDER` (HN-E006) when a later env is more mature.
- `H-MAT-REGRESS` (HN-E007) needs a change doc and writes a ledger entry.
- A child may only narrow. Loosening needs an `ovr` with `until=`.

Tests: one per rule.

**T3. Edge rules.** R-ENV-020, R-ENV-022, R-ENV-023, R-ENV-024:
- `rule.maturity-gap` is an error for a gap of 2 ranks or more and a warning for 1. A root `sev` replaces this default. The rule is waivable.
- `rule.env-leakage` uses declared `use` edges and Cargo manifest edges only. `watch` terms give a textual warning.
- Reachability is evaluated per env.

Tests:
- `r_env_020__gap_two_is_error_gap_one_warn`
- `r_env_020__root_sev_replaces_gap_default`
- `r_env_022__leak_over_declared_edge`
- `r_env_023__dev_only_unreachable_in_prod_is_by_design`

**T4. Bindings.** R-ENV-030 to R-ENV-036:
- Check only that each binding exists.
- Rust: the feature is in the nearest `Cargo.toml` `[features]` and not in `default`. A dev-only crate in a prod crate's `[dependencies]` is an error (`H-ENV-GATE`, HN-E001).
- TS: `import.meta.env.*` and `process.env.NODE_ENV` tokens are matched against the table. A token outside the table is `H-ENV-UNBOUND` (HN-E003).
- `bind=flag:` gives info only.
- A file mentioning a binding without an env declaration is `H-ENV-UNDECLARED` (HN-E002).
- Test-path defaults follow R-ENV-036.
- Output wording is "declared + bound + syntactically present".

Tests: one per rule. Fixtures go in `tests/fixtures/env/`.

**T5. Plan flags and due computation.** R-LC-001 to R-LC-007, R-LC-009, R-SYN-048:
- `flag deprecate|replace|edit|remove` with `target=`.
- `by-date=` is opt-in; whichever fires first wins.
- Symbolic targets resolve at the seal that first contains the flag, and the concrete version is recorded.
- Replacement chains resolve transitively. Cycles are an error, printed.
- Repeated retargeting gives `H-LC-STALE-PLAN` (HN-L001).
- Early removal needs `kind = "early-removal"`.

Tests:
- `r_lc_004__due_when_open_reaches_target_never_equal_only`
- `r_lc_005__first_of_date_and_version_wins`
- `r_lc_007__chain_resolves_cycle_errors`
- `r_lc_003__symbolic_target_recorded_at_seal`

**T6. Due gate in seal.** R-LC-006, R-VER-070. A due item or expiring exception blocks the seal unless its references are gone with the removal doc in this seal, an `until` exception exists, or a `retarget` doc moves it. This replaces the P1-T8 stub. Tests: `r_lc_006__due_item_blocks_seal`, `r_lc_006__retarget_doc_unblocks`.

**T7. Deprecated and removed references.** R-LC-008, R-LC-011, R-LC-022, R-ENV-024, R-CHK-059, R-SYN-042:
- `HN-R006` warns until due and is an error from due.
- `HN-R004` is a warning with a `by` alias and an error without one.
- `HN-D003` for a `def` of a tombstoned ID.
- Existing references are baselined. New ones are reported as new, and counts per version go into the ledger.

Tests:
- `r_chk_059__three_codes_are_distinct`
- `r_lc_022__alias_reference_warns_names_successor`
- `r_lc_008__new_reference_beyond_baseline_is_new`

**T8. Native deprecation attributes.** R-LC-010. A public item with `flag deprecate` but no `#[deprecated]` or `@deprecated` is flagged, and so is the reverse. This is a textual search within the item's extent. Test: `r_lc_010__flag_and_attribute_must_agree`.

**T9. Tombstones.** R-LC-020, R-LC-021, R-LC-023, R-RET-040, R-RET-041, R-CLI-046, R-CLI-027:
- `seal` writes `[[tomb]]` entries for lifecycle removals.
- `sync` appends one for an ID deleted outside the flow. It is listed in the plan and flagged for the author.
- `new` refuses a reserved ID.
- A scope tombstone is written on demote or delete.
- Tombstones never expire and never appear in default `context`.

Tests:
- `r_lc_020__seal_writes_tombstone_with_by_alias`
- `r_cli_046__sync_tombstones_user_deletion`
- `r_lc_021__new_refuses_reserved_id`
- `r_ret_041__tombstones_not_in_default_context`

**T10. Graveyard entries and fingerprints.** R-LC-030, R-LC-031, R-RET-042. Parse entries. Compute the fingerprint at seal from the removed or rejected files only, and never rescan later. Tests:
- `r_lc_031__fingerprint_is_deterministic` (proptest)
- `r_lc_031__thresholds_match_and_miss`
- `r_ret_042__match_terms_and_fingerprint_kept`

**T11. Graveyard at proposal, check and context time.** Covers R-LC-032 to R-LC-037, R-CTX-014 and R-LC-034:
- `new change "<title>"` matches the effective graveyard. On a hit it requires `revisits=` plus a reason.
- `H-GRAVE-MATCH` (HN-L002) fires in three cases: a draft or ready doc matching an entry without `revisits=`; a new or changed file matching a glob, regex or fingerprint; a tombstone's `paths` or ID.
- Silencing raises `tripwire`.
- `context` prints `graveyard: N rejected under here` and `grave:` lines.
- An abandoned doc prompts "graveyard candidate?".

Tests: one per rule.

**T12. Revive.** R-LC-024, R-LC-035:
- A `kind = "revive"` doc with `revives=` and an `[[approval]]` creates a new ID with a version suffix.
- The tombstone is not rewritten.
- The graveyard entry becomes `revisited`, and the link is ledgered at seal.

Test: `r_lc_024__revive_needs_approval_and_new_id`.

**T13. Signed authority records.** R-LC-038, R-LC-040, R-AGT-027 (the 0.2 opt-in part):
- With `authority.signed-records = true`, `check --ci` runs `git verify-commit` (subprocess, git module) on the commits introducing `rejected`, `revisited` and `do-not-reintroduce` records and `[[approval]]` tables.
- An unsigned one is `H-AUTH-UNSIGNED` (HN-X015).
- `explain authority` states the limit (R-LC-038, R-LC-039).

Tests:
- `r_lc_040__unsigned_approval_reported_in_ci_mode`, signing per Q-6
- `r_lc_040__no_subprocess_without_setting`

**T14. Remove P3 codes from `PENDING_0_2`.** These are `HN-E001` to `E007`, `HN-L001`, `HN-L002`, `HN-R004`, `HN-R006`, `HN-D003` and `HN-X015`. Run the codes test.

## Spec issues to log

- R-RET-040 says a reference to a removed ID is `HN-R006`, and R-CLI-040 says a reference through an alias is `HN-R006`. R-LC-022, R-CHK-059 and D-51 say `HN-R004`. Follow D-51 (`HN-R004` for removed or aliased IDs, `HN-R006` for deprecated ones still declared), and ask the owner to correct the two chapters.
