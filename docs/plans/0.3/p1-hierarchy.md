# P1: hierarchy model, structure and the `use` graph

> Chapter [30-hierarchy-and-engines.md](../../30-hierarchy-and-engines.md) R-HIER-001 to R-HIER-035, R-HIER-050 to R-HIER-052, R-HIER-070 to R-HIER-081, R-HIER-100 to R-HIER-103; [10-scopes-and-inheritance.md](../../10-scopes-and-inheritance.md) R-SCOPE-066; [50-architecture.md](../../50-architecture.md) R-ARCH-034. Decisions D-05, D-20, D-27, D-40.

## Goal

The adoption ramp works from L0 to L2:
- **L0:** containers with paths.
- **L1:** units with a layer and a container.
- **L2:** `use` edges checked for rank, DAG, cycles and visibility, plus evidence and refactor suggestions.

L3 (entries and bridges) is P2.

## Design

- **Control tables (root only, sealed):**
  - `[layers.<ladder>] <layer> = <rank>`
  - `[free.<name>] of may-use usable-by`
  - `[[flow]] from to via`
  - `[kinds.<k>] layers`
  - `[layer-policy."<ladder>.<layer>"] used-by`
  - `[hier] level same-rank unbound-ok governed promote`

  Parsed into `hier::Model`. A child may alias layer wording and narrow with `accepts=`. A reordering attempt becomes an escalation (R-HIER-004).
- **Declarations** are `def` directives with keys `layer container parent path sym vis steps on-error etype unit reaches implements`. A unit binds by a `path=` glob or by a code `def` extent, never both (R-HIER-014).
- **Graph.** `hier::Graph { units, containers, edges: Vec<Edge { from, to, kind: Use | Implements | Dyn, source: Declared | Cargo | Evidence }> }`. The container graph aggregates unit edges.
- **Ordering.** Every collection is a `BTreeMap`/`BTreeSet` keyed by ID, so reports are deterministic.
- **Generated shards.** `gen/hier.graph` holds the per-scope `use` facts (terse lines, D-32). `gen/hier.suggest` holds the suggestions (R-HIER-072).

## Tasks

**T1. Activate 0.3.** Set `ACTIVE_MILESTONE = M03`, add `PENDING_0_3` to `tests/codes.rs`, bump to `0.3.0-dev`, and pin the 0.2 golden outputs as a regression fixture.

**T2. Model parsing.** Rules R-HIER-001 to R-HIER-006 and R-HIER-003:
- Parse the ladders, free layers, flows, kinds and layer policy.
- Units name layers, never ranks.
- `fairlead new ladder ui|core` scaffolds the presets of 30-hierarchy into the control file through the transaction (D-20).

Tests:
- `r_hier_001__ladder_tables_root_only`
- `r_hier_003__unit_names_declared_layer`
- `r_hier_004__child_reorder_becomes_escalation`
- `d_20__presets_are_content`

**T3. Containers and binding.** R-HIER-010 to R-HIER-014:
- One container per unit (`hier.single-container`, HN-H001).
- An acyclic `parent` chain (`hier.parent-cycle`, H002).
- `path` claims may overlap only when nested, and never enter a sibling scope.
- No second container: a second membership is expressed with `see=`.
- A unit with both `path=` and a code `def` is `hier.double-binding` (H023).

Tests: one per rule.

**T4. Structural audits (L0, L1).** R-HIER-050 to R-HIER-052, R-HIER-080:
- `hier.orphan-unit` (H004) covers no container, a missing container, or a marker naming an undeclared ID.
- `hier.orphan-file` (H003) is a governed file bound to nothing. `unbound-ok` globs are exempt.
- `hier.hollow` (H005) is a unit with no files, except while `proposed`.
- `[hier] level` switches checks on per the ramp table.

Tests: one per audit, plus `r_hier_080__checks_above_level_are_off`.

**T5. Coverage ratchet.** R-HIER-081. Report bound files over governed files. The ratio goes into the baseline (D-28) and must not drop without a waiver. Strict totality is per-scope opt-in. Test: `r_hier_081__coverage_ratchet_blocks_decrease`.

**T6. `use` graph and rank.** R-HIER-020, R-HIER-023 to R-HIER-025, R-HIER-002, R-HIER-006, R-HIER-028:
- Within one ladder, `rank(target) <= rank(source)`. Equal ranks follow `same-rank`.
- A cross-ladder edge needs a `flow` entry.
- `implements` edges are exempt.
- `dyn=1` is a declared edge with no evidence requirement.
- An entry-only layer is reached only from entries.
- Violations are `hier.rank` (H007). An upward need needs an `ovr rule.hier-rank` with `until=`; an `ovr` without expiry is rejected.

Tests: one per rule.

**T7. DAG and cycles.** R-HIER-021, R-HIER-022, R-HIER-103:
- The unit graph must be a DAG (`hier.dag`, H008). Declared `self` recursion is allowed.
- Strongly connected components in the container graph are `hier.cycle` (H009), listing the unit edges that cause them.
- A transitional cycle waiver needs `until=`.

Use Tarjan's algorithm with deterministic order. Tests:
- `r_hier_021__unit_cycle_reported`
- `r_hier_022__container_scc_lists_causing_edges`
- `r_hier_103__unbounded_cycle_waiver_rejected`

**T8. Visibility.** R-HIER-026, R-HIER-028, R-VAR-010, R-VAR-011. A cross-scope `use` must name a unit whose `vis` allows the consumer (default `subtree`). Otherwise it is `hier.visibility` (H010): a warning before the stage reaches enforce. Visibility and rank are independent checks. Tests: `r_hier_026__sibling_use_of_subtree_unit_flagged`, `r_hier_028__passing_one_never_exempts_the_other`.

**T9. Cargo edges.** R-HIER-027:
- Workspace member dependencies come from `Cargo.toml` (the 0.1 manifest reader; `dev-dependencies` excluded).
- They become container edges between the containers whose `path` claims cover the crates.
- They feed `hier.cycle` and a container-level rank check, labelled `evidence=cargo`.

Test: `r_hier_027__cargo_edge_feeds_cycle_and_rank`.

**T10. `sym` evidence.** R-HIER-030, R-HIER-031:
- `sym` must be at least 6 characters and unique. Otherwise it is `hier.sym-ambiguous` (H006), and evidence checks for the unit are suppressed.
- Whole-identifier search with comments excluded, using 0.1's `lexer::comment_spans`.
- `hier.uses-noevidence` (H011): `use X` without the `sym`.
- `hier.uses-undeclared` (H012): the `sym` without `use X`.

Tests: one per check, plus `r_hier_031__match_in_comment_ignored`.

**T11. Regex import evidence.** R-HIER-032 to R-HIER-034, R-ARCH-034:
- `evidence::imports(text, ext) -> Vec<ImportSpec>` reads TS `import ... from '<spec>'`, `export ... from`, `require('<spec>')` and Rust `use crate::<path>`.
- It produces only `source: Evidence` edges, which confirm or contradict declared edges.
- Reports carry the "declared edges, lower bound" note.
- The seam (`trait ImportExtractor`) lets a later tree-sitter or `syn` extractor replace the source without rule changes.

Tests: `r_hier_033__evidence_never_creates_rank_verdict`, `r_hier_034__reports_state_lower_bound`.

**T12. Implied layer and maturity.** R-HIER-070, R-HIER-073:
- The implied layer is the one above the highest layer used. Declared below implied is `hier.rank`; declared more than one above suggests demotion.
- `hier.maturity` (H017) uses the 0.2 maturity ranks and env sets.

Tests: one per rule.

**T13. Suggestions shard.** R-HIER-071, R-HIER-072:
- `hier.promote` (H013), `demote` (H014), `split` (H015) and `merge` (H016), with thresholds from `[hier] promote`. Seal counts come from the ledger.
- `vis=public` or `etype=export` suppresses demotion.
- `sync` writes the suggestions to `gen/hier.suggest`. They are info, never findings at a higher severity.

Tests: one per suggestion.

**T14. Refactors through `move`.** R-HIER-100 to R-HIER-102:
- `move` rewrites authored `container=` and `parent=` deterministically. A refactor change doc of `type = "refactor"` covers it.
- Unit IDs and code markers never change.
- A retired container leaves `[[tomb]]` with `moved-to`. A unit still naming it is `hier.orphan-unit` with the `moved-to` fix.

Tests: `r_hier_100__move_rewrites_container_keys_only`, `r_hier_102__retired_container_hint`.
