# P6: task queries, hierarchy in briefs, authority, extractor seam, MCP

> [14-context.md](../../14-context.md) R-CTX-005, R-CTX-015, R-CTX-034, R-CTX-060 to R-CTX-063; [40-agent-integration-and-trust.md](../../40-agent-integration-and-trust.md) R-AGT-027; [50-architecture.md](../../50-architecture.md) R-ARCH-004, R-ARCH-034, R-ARCH-154; [30-hierarchy-and-engines.md](../../30-hierarchy-and-engines.md) R-HIER-035. Decisions D-23, D-26, D-27, D-40.

## Tasks

**T1. Task queries.** Rules R-CTX-005, R-CTX-060 and R-CTX-062:
- Free text that is not a path or an ID becomes a task query. In 0.1 and 0.2 it exited 2.
- Matching is deterministic BM25 over an inverted index built at load and kept in the gitignored cache. The cache key adds the index schema version.
- Indexed fields: ID slugs, item titles, doc headings, `kw=` tags and tombstone summaries.
- Ties break by scope ID.
- Zero matches return the root L1 plus the top-level L0 and "no match"; the query never guesses.

Tests:
- `r_ctx_060__bm25_is_deterministic_across_runs`
- `r_ctx_062__no_match_returns_root_l1`
- `r_ctx_062__ties_break_by_scope_id`

**T2. Query output.** R-CTX-061, R-LC-034:
- Output starts with `matched: <scope> <score>, ...; pin with --scope`, listing the top 3 scopes.
- Then come the top-K items and docs, with matching graveyard entries first, all within budget.
- `--scope` pins the scope.

trycmd: `context-task-query`, `context-task-query-pinned`.

**T3. Recall measurement.** R-CTX-063, owner question Q-4:
- `cargo xtask query-recall --set <file>` reports recall at 3 against the labelled query set.
- Where recall fails, a kind may require `kw=`; a `check` finding fires on an item lacking it.
- Scope-name boosts apply.
- BM25 is not dropped.

Test: the xtask unit tests. The query set comes from agent protocol B-2 ([../agent-runbook.md](../agent-runbook.md)).

**T4. Hierarchy in the brief.** R-CTX-015. Where hierarchy is adopted, the brief adds:
- the path's containers;
- their unit IDs;
- the public `use` surface only;
- open variation requests and expiring interim wrappers.

All within the L1 budget. Test: `r_ctx_015__brief_lists_containers_and_public_surface`.

**T5. `--dense`.** R-CTX-034. An optional dictionary-aliased positional form for L0 dumps of very large repositories. It is never the default, and the dictionary is printed first. Test: `r_ctx_034__dense_round_trips_to_terse`.

**T6. Import-extractor seam and the tree-sitter gate.** R-ARCH-034, R-ARCH-154, R-HIER-035, owner question Q-2:
- `trait ImportExtractor` exists from P1-T11.
- Add a CI job, weekly only, that loads the pinned `tree-sitter` core and the `tree-sitter-typescript` grammar in a throwaway crate (not in the workspace), then parses the TS fixture corpus. The job reports pass or fail.
- Adoption is a separate owner decision and raises the MSRV to 1.90 (R-REL-006).

Done when the job exists and its result is recorded.

**T7. General signed-commit verification.** R-AGT-027 (0.3 mechanism):
- `check --ci` verifies signatures on commits introducing any authority-bearing record (`ovr`, rejection and approval records), using `git verify-commit` as in 0.2's opt-in.
- It is enabled by `authority.verify-signatures = true`.
- The limits table in `explain authority` is updated.

Test: as in 0.2 P3-T13, extended to `ovr` records.

**T8. MCP server (firm, D-62).** R-ARCH-004, owner question Q-3:
- A separate crate, `crates/fairlead-mcp`, with tools and resources only: `check`, `context`, `explain` and read-only resources.
- `rmcp` is pinned to an exact minor.
- No network stack enters the `fairlead` binary.
- A test asserts that `fairlead`'s dependency tree contains no `rmcp`.

It ships in 0.3: an agent field test of 0.1 and 0.2 (D-62) found that agents meet fairlead through their tool loop, so the MCP tools are the main entry point for them, not an extra.
