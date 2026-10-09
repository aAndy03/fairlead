# P2: entries, reachability, dormancy, bridges

> Chapter [30-hierarchy-and-engines.md](../../30-hierarchy-and-engines.md) R-HIER-040 to R-HIER-046, R-HIER-060 to R-HIER-068, R-HIER-090 to R-HIER-092; [12-directive-syntax.md](../../12-directive-syntax.md) R-SYN-043, R-SYN-049; [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) R-CHK-043, R-CHK-045 (ID-level part). Decision D-29.

## Goal

This phase is adoption level L3:
- Entries start reachability. `stale`, `dormant` and `forgotten` audit units.
- TS-to-Rust bridges are checked by text for presence, wire literal and version pin.
- Hierarchy changes are covered by change docs.

## Tasks

**T1. Entries.** Rules R-HIER-040 and R-HIER-044:
- `def ent.* etype=<word> unit=<id>`, with optional `reaches=` (`none` means local-only).
- `etype` words come from the control file. The shipped words are `user-action`, `cli`, `export`, `test` and `registered`.
- Tests, stories and benchmarks are auto-entries of type `test`, excluded from fan-in counts.

Tests:
- `r_hier_040__etype_must_be_declared`
- `r_hier_044__reaches_checked_against_chain`
- `r_hier_040__tests_are_auto_entries`

**T2. Reachability per env.** R-HIER-041, R-HIER-046, R-ENV-023:
- A declared, bound unit that no entry reaches in the envs where the unit exists is `hier.stale` (H018).
- Units named in `adds` or `touches` of an unsealed change doc are exempt.

Tests: `r_hier_041__dev_only_unit_not_stale_in_prod`, `r_hier_046__draft_adds_exempts_until_sealed`.

**T3. No entries declared.** R-HIER-042, 4.3 criterion 1: `stale` is skipped with the info "no entries declared". Test: `r_hier_042__zero_entries_skips_stale`.

**T4. Dormant and forgotten.** R-HIER-043, R-HIER-045:
- Each seal records a per-unit digest in the ledger. This extends 0.2's checkpoint records with `unit=<id> ud=<digest>` lines.
- `hier.dormant` (H019, info) fires when the digest is unchanged for N seals. Counting starts at the seal where the unit first appears. Git history is never read.
- `hier.forgotten` (H020) is stale and dormant. It proposes a graveyard entry; `explain` shows the last-touch seal and the last known entry.

Tests: `r_hier_045__dormancy_from_ledger_not_git`, `r_hier_043__forgotten_proposes_graveyard`.

**T5. Bridge declarations.** R-HIER-060, R-HIER-061, R-SYN-043:
- `def brg.*` with `proto dir wire ver params ts= rs=`, plus optional `contract= reaches= implements= spell= exposure=`.
- `proto` words are user-defined. Params are declared by name, and the report says "fields unverified".
- Events and error shapes are ordinary kinds with a `proto` word.

Test: `r_hier_060__bridge_requires_both_ends_and_wire`.

**T6. Bridge end checks.** R-HIER-062, R-HIER-063, R-SYN-049, R-CHK-043:
- `hier.bridge-end-missing` (H021) when an end lacks its `def brg.* side=ts|rs` marker or its `#symbol` text.
- `hier.bridge-wire` (H025) when the wire literal is missing on one side. Use `spell=` for the expected spelling.
- `hier.bridge-pin` (H022) when `ver` on an end marker or a consumer `use ... pin=<N>` differs from the declaration.
- A dynamic name is `hier.unverifiable` (H024, warn). It is never guessed.

Tests: one per code.

**T7. Contract hash.** R-HIER-064, R-HIER-066, R-HIER-067, R-HIER-068:
- `seal` records the contract file hash. A changed contract needs a change doc touching both ends' scopes.
- Files generated from the contract are vendor-owned, external-generated or `def foreign` regions with `gen=brg.<id>`. They are flagged stale when the hash changes and they are not regenerated.
- `exposure=` is reported as unverified.
- External build inputs are `def ext.<name> role=input`.

Tests: `r_hier_064__contract_change_needs_doc_on_both_sides`, `r_hier_066__generated_binding_stale_on_contract_change`.

**T8. Bridge flow rules and traces.** R-HIER-065, R-HIER-023:
- A `use` of a bridge with `reaches=` is checked against `flow` using the reached unit's layer.
- An `implements=` bridge with no `reaches` is exempt.
- `explain <entry>` prints the expected chain (`steps=` and `on-error=`) as a trace.

Tests: `r_hier_065__reaches_checked_against_flow`, and trycmd `explain-entry-trace`.

**T9. Hierarchy docs-first.** R-HIER-090 to R-HIER-092, R-CHK-045 (ID level):
- A new unit, container or bridge must appear in a change doc's `adds` or `touches` (`doc.undocumented-unit`, HN-D004).
- A listed ID that does not exist is `doc.dangling` (HN-D005).
- From 0.3, IDs in `covers` count toward coverage.
- `new change --from-diff` scaffolds a rolling chore doc.

Tests: one per rule. Remove `HN-H018` to `H025`, `HN-D004` and `D005` from `PENDING_0_3`.
