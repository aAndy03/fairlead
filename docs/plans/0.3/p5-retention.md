# P5: retention and compaction

> Chapter [33-retention-and-compaction.md](../../33-retention-and-compaction.md) R-RET-001 to R-RET-071 (0.3 rules); [20-versions-and-seal.md](../../20-versions-and-seal.md) R-VER-050 step 5, R-VER-059; [15-cli.md](../../15-cli.md) R-CLI-041 (`--compact-only`), `sync --purge`. Decisions D-14, D-16, D-19, D-24, D-26, D-39.

## Goal

Records (change docs, requests, ADRs, graveyard entries, handoffs, waiver history) keep `sum` and `out` from creation. Inside `seal`, due records compact to one summary line in the hash-chained ledger and later fold into quarterly rollups. IDs stay resolvable. Default `context` cost does not grow with history.

## Design

- **Record slots.** `fairlead new` creates `sum` and `out` (empty while open), and `why` for rejected or removed records. Fields are capped at 240 characters and scanned for secrets (`HN-F001`, from 0.2).
- **Policy.** `[retain]` and `[retain.<kind>]` carry `hot ttl min-age keep-rejected bind max roll-after`. A child may only lengthen (R-RET-013).
- **Eligibility.** `retention::eligible(record, line, seal_ts) -> bool` is pure. It uses the recorded seal timestamps, never the wall clock (R-RET-011, R-RET-020).
- **Plan.** `retention::plan(loaded, version_line, seal_ts) -> CompactionPlan { summarise, fold, append_lines, delete_bodies }`. It is staged in the 0.2 seal transaction before the version file (R-RET-023).
- **Chain.** Finalised year files end with `# end d=`. The next year's header carries `prev=`. `sync` regenerates the open year's checkpoint lines (R-RET-035).

## Tasks

**T1. Record slots.** Rules R-RET-002 to R-RET-005:
- `new` writes the `sum`, `out` and `why` slots.
- `check` warns, on the ramp, when an accepted or closed record has an empty `sum` or `out`.
- The summary line preserves the fields listed in R-RET-005.

Tests:
- `r_ret_002__new_creates_sum_and_out_slots`
- `r_ret_004__closed_record_missing_sum_warns`
- `r_ret_005__summary_line_has_required_fields`

**T2. Policy parsing.** R-RET-010, R-RET-012, R-RET-013:
- TTL forms: `8v`, `life` with `bind`, `never`, and calendar values.
- `min-age` is the calendar floor.
- A child override that shortens retention is inert and becomes an escalation. `never` can never be shortened.

Tests: one per rule.

**T3. Eligibility.** R-RET-010, R-RET-011, R-RET-020, R-RET-001. A record is eligible by version distance and `min-age`, using recorded seal timestamps. Live state (registry, rules, item shapes) is never eligible. Tests:
- `r_ret_020__wall_clock_never_read` (static guard and behaviour)
- `r_ret_010__version_ttl_waits_for_min_age`
- `r_ret_001__live_state_never_compacted`

**T4. Seal gate.** R-RET-022. A due record with an empty `sum` or an invalid `out` blocks the seal, and every offender is listed as `SUMMARY-MISSING` (HN-K001). Test: `r_ret_022__due_record_without_summary_blocks_seal`.

**T5. Compaction steps.** R-RET-021, R-RET-023, R-RET-025, R-RET-030:
- Each due record's summary line is appended to the current year file.
- The record body is deleted from the working tree.
- The ID stays resolvable.
- The steps are staged and applied before the version file, and the output is deterministic: bytewise order on (year, ID), LF only.

Tests:
- `r_ret_021__two_runs_byte_identical`
- `r_ret_023__interrupted_seal_leaves_nothing_half_compacted`
- `r_ret_025__body_deleted_id_resolvable`

**T6. Rollups and ordinals.** R-RET-026 to R-RET-028:
- A non-decision-grade summary cold for `roll-after` versions folds into a quarterly rollup line: `roll.<yyyy>q<n> kind= n= ord=<a>..<b> dec= d=`. The quarter comes from the closing seal's timestamp.
- Ordinals continue from max(live ordinals, rollup ranges) + 1.
- Abandoned docs are eligible and prompt a graveyard candidate.

Tests:
- `r_ret_026__fold_into_quarter_of_closing_seal`
- `r_ret_027__ordinal_never_reused_after_fold`

**T7. Hash chain.** R-RET-031 to R-RET-035, R-GIT-015 (0.3 part):
- Finalised years end with `# end`, and the next header carries `prev=`.
- A year becomes final when a seal records a timestamp in a later year.
- `check` verifies the chain (`HN-G003`).
- `sync` never rewrites a finalised year. It may re-sort the open year and regenerate its checkpoint lines, so a union merge cannot break the chain.

Tests:
- `r_ret_031__chain_across_years_verified`
- `r_ret_032__hand_edit_of_final_year_fails_chain`
- `r_ret_035__union_merge_then_sync_keeps_chain`

**T8. Purge and user deletion.** R-RET-033, R-RET-043 to R-RET-045:
- `sync --purge <id>` reduces a record to `id + date + digest`. It is the only rewrite of a finalised year, and it does not re-chain.
- A `git rm` of a record leads `sync` to write a `[[tomb]]` with `deleted-by = "user"`.
- A draft that was never sealed leaves no tombstone.
- A deleted decision-grade record appears as a notice with its digest.

Tests: one per rule.

**T9. References to compacted records.** R-RET-050, R-RET-051:
- `use d-<draft>` of a compacted record resolves to its summary line. A rolled-up record resolves to its rollup line with the note "compacted in <yyyy>Qn".
- `explain <id>` prints the summary and the recorded commit. A missing commit gives "full text unavailable (commit not found)", not an error.

Tests: one per rule.

**T10. Temperature in context.** R-RET-060. Hot records may appear in `context`. Warm records appear only on a query hit (P6) or in `explain`. Cold records appear only as a summary line, by query. Test: `r_ret_060__leaf_context_size_independent_of_history`, on the 5-year simulation from P7-T2.

**T11. Seal flags and scope moves.** R-RET-024, R-RET-070, R-CLI-041:
- `seal --dry-run` prints the compaction plan.
- `seal --compact-only` re-runs compaction with the latest seal's timestamp. It is a no-op when nothing is due.
- `move --merge` and `--demote` append the origin's ledger lines to the receiver's open year with `scope=<origin>` and prefixed ordinals. Finalised years are never rewritten.

Tests: one per rule. Remove `HN-K001` from `PENDING_0_3`.
