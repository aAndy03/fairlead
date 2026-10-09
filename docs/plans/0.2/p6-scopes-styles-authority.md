# P6: scopes, styles, authority, expiry and context deltas

> Chapters: [10-scopes-and-inheritance.md](../../10-scopes-and-inheritance.md) R-SCOPE-019, R-SCOPE-029, R-SCOPE-057; [12-directive-syntax.md](../../12-directive-syntax.md) R-SYN-047; [11-files-and-classes.md](../../11-files-and-classes.md) R-FILE-028, R-FILE-035, R-FILE-043, R-FILE-055, R-FILE-056; [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) R-CHK-047, R-CHK-048, R-CHK-054; [40-agent-integration-and-trust.md](../../40-agent-integration-and-trust.md) R-AGT-021, R-AGT-029, R-AGT-030; [20-versions-and-seal.md](../../20-versions-and-seal.md) R-VER-070 to R-VER-075; [14-context.md](../../14-context.md) R-CTX-023, R-CTX-051 to R-CTX-054. Decisions D-02, D-34, D-37, D-49.

## Goal

This phase holds everything in 0.2 that does not depend on the seal machinery, plus the two items that do: version-based expiry and `context --since`. Tasks 1 to 9 can start right after 0.1. In the roadmap's cut order (60-roadmap 8.1), tasks 5 to 7 (styles) can slip to 0.2.x without breaking the edition.

## Tasks

**T1. `[delegate]`.** Rules R-SCOPE-051 (G2 part), R-SCOPE-057 and R-ENV-040:
- A root `[delegate]` table maps `"<key pattern>" = "tighten"`.
- A child may then set a matching G2 key, monotonically only.
- An unlisted key stays non-delegable (HN-C003).
- G1 keys cannot be listed.

Tests:
- `r_scope_057__delegated_key_tightens_only`
- `r_scope_057__vocab_key_cannot_be_delegated`
- `r_scope_057__unlisted_key_stays_c003`

**T2. `own` convention and `SCOPE-AMBIG`.** R-SCOPE-019, R-SCOPE-003:
- `[[convention]] own = "<scope id>"` assigns matching files to a named descendant scope. One file still has one scope.
- A scope directory `D/` beside a sibling module file `D.<ext>` with no `own` entry is `SCOPE-AMBIG` (HN-C014).
- `sync --scaffold-own` writes entries from manifest members, only when asked.

Tests: `r_scope_019__own_assigns_descendant`, `r_scope_019__edge_rs_beside_edge_dir_is_ambiguous`.

**T3. `provides` and `NOT-PROVIDED`.** R-SCOPE-029. A qualified sideways reference must name an item in the target scope's `provides`, or one declared `vis=public`. Otherwise it is `NOT-PROVIDED` (HN-R008). Test: `r_scope_029__sideways_ref_needs_provides`.

**T4. `[[attach]]` for comment-free files.** R-SYN-047:
- A control-file table `path` (a scope-relative glob) plus `directives` (an array of bodies).
- Each directive binds in file form to every matching file.
- Only `def use ovr flag env` are allowed: no `end`, no regions.

Tests: `r_syn_047__attach_binds_file_form`, `r_syn_047__end_and_regions_rejected`.

**T5. Tailwind v4 cascade checks.** R-FILE-035, R-CHK-047. Extends the 0.1 styles extractor (`extract/styles.rs`):
- `HN-T008` checks the declared `@layer` order statement and the stylesheet import order.
- `HN-T009` flags a bare override: an unlayered or out-of-order rule overriding a token or utility without an `ovr`.
- (a) `@theme inline` is required for `var()` tokens bridged by `@theme`.
- (d) A `:root`-only override without its `.dark` value is reported.
- (e) The `--color-*` namespace must be declared foreign or external.
- (f) `@source` globs are existence-checked.

Tests: one per letter; fixtures go in `tests/fixtures/tailwind4/`.

**T6. Derived-token graph.** R-FILE-035(b), R-CHK-046 (graph part). A token whose CSS value is `var(--other)` must either be declared derived in the contract or be re-declared in every overriding theme. Otherwise it is `TOK-DERIVED-GRAPH` (HN-T010). Test: `r_chk_046__derivation_found_by_graph`.

**T7. Class safelists.** R-CHK-048. A token `def` may carry `classes=<pattern,...>`. A matching class counts as a use of the token and is leak-checked in its property context. Without a safelist, computed names stay unreported. Test: `r_chk_048__safelisted_class_is_a_use`.

**T8. Vendor and generated drift.** R-FILE-043, R-FILE-055, R-FILE-056, R-FILE-028:
- `FOREIGN-LOST` (HN-G006) when a foreign region's anchors vanish. The region is rebuilt from the ledger where possible.
- A named `external-generated` file hashes over normalised content, so a change is reported as generator-input drift.
- `[file-kinds.K]` custom extractors are a documented seam (an interface plus one built-in example). No safelist.
- Merge-semantic checks on control files run under `check --ci`.

Tests: one per rule.

**T9. Authority.** R-CHK-054, R-AGT-021, R-AGT-029, R-AGT-030:
- An `owners` entry may be `kind = "agent"`. An agent-handle approval, grant or baseline acceptance on a floor-adjacent key is `APPROVER-AGENT` (HN-X013) and is ineffective.
- `HN-A003` (info) flags instruction-like phrases in authored fields, using a fixed phrase list. It is not a security boundary.
- An escalation (`[[req]]`) is resolved by the owners of the nearest governing ancestor, through `approve`. Root owners resolve vocabulary keys.

Tests:
- `r_chk_054__agent_approver_on_floor_adjacent_key_is_ineffective`
- `r_agt_021__instruction_text_flagged_info`
- `r_agt_030__nearest_governing_owner_resolves`

**T10. Version-based expiry.** R-VER-070 to R-VER-075, R-SYN-012 (0.2 part), D-37, D-49:
- `ovr until=<MAJOR.MINOR.PATCH>` is effective while `O < until`. A two-part value, `never`, `none` or an empty value is invalid.
- `exp=` dates stay valid as an opt-in.
- The cap is `O` with its minor raised by `exceptions.max-minors`, patch 0. A larger `until` is `EXC-CAP` (HN-X016).
- Expired waivers fail `check`. Waiver and renewal counts appear in `context` and in the seal rollups.
- A second consecutive renewal needs an approval.
- No rule or upgrade fills in an expiry.

Tests: one per rule. Dogfood: convert this repository's `rule.no-clock` waiver to `until=` after the first seal of the fairlead repository itself; the owner re-approves.

**T11. `context --since`.** R-CTX-023, R-CTX-051 to R-CTX-054:
- `--since <version|git-rev>` computes the delta from the sealed changelog and the generated-file diff. Unchanged ancestor objects are elided.
- `--since <digest>` uses a manifest in the gitignored cache. Unchanged objects become `~` lines.
- An unknown manifest gives the full slice with `since=unknown`, never an error.

Tests: one per rule.

**T12. Remaining `explain` and `context` additions.** R-CTX-014 lines come from P3. Here add `context` "N new rules available" (from P4-T4) and the scope's open escalations. Remove `HN-C014`, `HN-R008`, `HN-T008` to `T010`, `HN-G006`, `HN-X013` and `HN-A003` from `PENDING_0_2`.
