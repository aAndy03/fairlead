# P4: editions, pins and `upgrade`

> Chapter [22-editions-and-upgrade.md](../../22-editions-and-upgrade.md) (all [0.2] rules); [15-cli.md](../../15-cli.md) R-CLI-042; [50-architecture.md](../../50-architecture.md) R-ARCH-124; [60-roadmap.md](../../60-roadmap.md) R-REL-002, R-REL-003, R-REL-010, R-REL-011, R-REL-026; [40-agent-integration-and-trust.md](../../40-agent-integration-and-trust.md) R-AGT-014. Decisions D-17, D-30, D-33, D-45.

## Goal

The edition is a byte-stable contract. Binaries refuse a mismatched edition and print the exact bridging install line. Lowering a pin is a reported relaxation that needs approval. `upgrade` moves a codebase exactly one edition through a two-step, approved, plan-hashed transaction. Sealed sub-roots may stay on another edition.

## Design

- `edition::Registry` is a static table of the editions this binary supports. Each entry is an `EditionSpec { number, grammar, header_format, migrations_from_previous }`. 0.2 supports edition 1 only. A `test-edition` Cargo feature adds a synthetic edition 2 for tests (owner question Q-1).
- `edition::Migration` is a list of pure, idempotent `Step`s: `RewriteDirectiveKey`, `RenameMarker`, `RenameFolder`, `RegenerateReference`, `ReshardGenerated`, and `NeedsHuman { path, field, reason }`.
- `edition::plan(tree, from, to) -> UpgradePlan { edits, needs_human, new_rules_dry_run, plan_hash }`. The hash is blake3 over the sorted edit list, so the same tree and binary give the same hash (R-EDN-037).
- Apply reuses `write::Transaction`. The control file, carrying the new `edition`, is renamed last.

## Tasks

**T1. Edition registry and pins.** Rules R-EDN-001, R-EDN-005, R-EDN-010, R-EDN-012 to R-EDN-014 and R-REL-011:
- Replace the 0.1 constant `SUPPORTED_EDITION` with the registry.
- The pin refusal (HN-C006, exit 4) names the supported editions and the bridging `cargo install ... --locked` line.

Tests:
- `r_edn_012__refusal_has_exact_install_line`
- `r_edn_013__newer_edition_refused_exit_4`
- `r_edn_005__one_edition_per_root`

**T2. Writers refuse an older edition.** R-EDN-011. `sync`, `new`, `move` and `seal` refuse an older-edition codebase until `upgrade --yes` has been applied. `check` follows R-EDN-010: no read-only window. Test: `r_edn_011__writers_refuse_older_edition`.

**T3. Monotonic pins.** R-EDN-006, R-AGT-014:
- Lowering `edition`, `fairlead-version` or `fairlead-range` without an approved change doc is a relaxation. It appears in the summary line `relaxations:` and warns.
- `check --since <rev>` (P5-T4) lists a lowered pin as `must-review`.

Tests: `r_edn_006__lowered_pin_is_relaxation_without_approval`, `r_agt_014__since_flags_pin_decrease`.

**T4. Stability promises.** R-EDN-002 to R-EDN-004, R-REL-003:
- Extend the P1-T1 byte-stability fixture to every supported edition.
- Add a registry test that every rule added after the freeze has default stage `off` or `observe`. A rule row carries the binary minor it arrived in.
- `context` prints "N new rules available".

Tests: `r_edn_004__new_rules_ship_off_or_observe`, `r_edn_004__context_lists_new_rules`.

**T5. Unknown classes, keys and must-understand.** R-EDN-020 to R-EDN-024. Most of this exists from 0.1 (`HN-S001`, `HN-S011`, `!`). Add the round-trip guarantee: every rewriter (`move`, `upgrade`, `sync --migrate-vocab`) preserves unknown keys, `x-` keys and unknown frontmatter keys. Proptest: `r_edn_021__rewrites_preserve_unknown_keys`.

**T6. Upgrade plan.** R-EDN-030, R-EDN-034, R-EDN-037 to R-EDN-039:
- `upgrade` without `--yes` prints the plan and exits as not applied.
- The plan lists the edits, the `needs-human` items, and the new rules with dry-run violation counts.
- N to N+2 is refused with the list of steps.
- No expiry is ever filled in.

Tests:
- `r_edn_037__same_tree_same_plan_hash`
- `r_edn_039__two_edition_jump_refused_with_steps`
- `r_edn_034__needs_human_blocks_apply`

**T7. Two-step apply.** R-EDN-031 to R-EDN-033, R-EDN-035, R-EDN-036, R-CLI-042:
- With no matching approved doc, `upgrade --yes` writes only the draft doc `d-<draft>-upgrade-edition-N-to-M.md` (`kind = "upgrade"`, `impact = "break"`) and exits 4.
- With a `ready` doc carrying an `[[approval]]` and a matching hash, it applies through the transaction with the control file last, then runs `check`.
- Otherwise it reports "plan stale, re-plan" or "approval missing" (exit 4).
- The write-set must equal HEAD; `--allow-dirty` overrides.
- An interrupted apply is resumable. A half-migrated tree is `H-UPG-PARTIAL` (HN-V011).
- There is no undo log.

Tests: one per branch of R-EDN-031, plus `r_edn_032__interrupted_apply_resumes`, using the `test-edition` feature.

**T8. Marker and folder rename.** R-EDN-007, D-30, R-REL-043:
- `upgrade --rename-marker X --rename-folder Y` is a migration step. It rewrites every directive prefix and the managerial folder name, and updates `[fairlead] marker` and `[settings] folder`.
- Rename only; edition unchanged.
- This is also the tool for the 0.1 pre-publish rename (R-REL-043), if one is ever needed.

Test: `r_edn_007__rename_rewrites_every_directive_and_folder` (golden repo `ts-react`, rename, then `check` passes).

**T9. Sealed sub-root editions and `--fetch`.** R-EDN-040 to R-EDN-044, R-EDN-025, R-ARCH-124:
- A registered sealed sub-root is read under its own edition's parser. Inheritance across the boundary is off.
- `upgrade --scope <sealed-root>` upgrades the sub-root alone.
- An unreadable sub-root is reported and the rest continues; the parent fails under the pin refusal.
- Mount registration itself is 0.3 (portability). Here only the edition dispatch and the refusal path are built, tested with a fixture that registers a sub-root by hand.
- `--fetch` follows owner question Q-2: the `net` module and Cargo feature `fetch` exist, are off by default, and contain no client. The flag exits 2 with the documented message. A test asserts that no other verb references the module.

Tests:
- `r_edn_040__sub_root_read_under_own_edition`
- `r_edn_044__unreadable_sub_root_fails_parent_only`
- `r_arch_124__net_module_only_reachable_from_upgrade`

Fixtures: `tests/editions/1/` (byte-stability) and `tests/editions/1-to-2/` (plan, apply, check, a second apply that is a no-op: R-EDN-052, 60-roadmap 4.2 criterion 2).
