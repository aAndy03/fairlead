# P4: `fairlead map`, duplicates and stale doc references

> Outline goals 4 (no drift, no redundancy) and 5 (modularity), and the new surface `fairlead map`; [14-context.md](../../14-context.md) budgets; [40-agent-integration-and-trust.md](../../40-agent-integration-and-trust.md) data fencing. Decisions D-40, D-62, D-63, D-64 (Q4-1).

## Goal

People and agents see the object graph: who declares, uses, imports and owns what. Fairlead reports duplicated code (never merges it, Q4-1), doc text that names objects that no longer exist, and scopes whose objects form separate clusters.

## Design

- **Graph.** Nodes: objects (P1), registry items, scopes and files. Edges: `declares`, `uses` (identifier evidence, P2), `imports` (0.3 evidence, now resolved to objects), `owns` (home scope), `binds` (anchored directives, P3). Built on demand from the cache; never stored except as the JSON output.
- **Output.** `fairlead map [path | ID | scope]`: terse text by default (one edge per line, a budget like `context`), `--format json` with a documented shape (added to the v1 snapshot), `--depth N`. The MCP server gains a `map` tool.
- **Duplicates.** Two function or block bodies whose normalised token stream (identifiers renamed to placeholders, comments and whitespace removed) hashes equal, above `[content] duplicate-min-tokens` (default 60). Reported only.
- **Clusters.** Within a scope, connected components of the `uses` graph over its objects; a scope whose objects split into two or more components of at least `[content] split-min-objects` (default 10) each gets a split suggestion.

## Tasks

**T1. The graph.** `map::Graph::build(reg, corpus, objects)` with the node and edge kinds above; deterministic order. Test: `r_map_001__graph_edges_from_a_fixture`.

**T2. `fairlead map` text.** The verb, its flags (`--depth`, `--budget`, `--format`), and a terse renderer. trycmd: `map-path`, `map-id`.

**T3. `map --format json`.** The JSON shape, a schema snapshot in `tests/v1_api/map.shape.txt`. Test: the v1 snapshot.

**T4. `map` in the MCP server.** A `map` tool with `target`, `depth`, `budget`. Test: `r_arch_004__the_map_tool`.

**T5. Duplicates (`HN-O007`).** Normalised token hashing over P1 objects; one finding per duplicate group at the first occurrence, listing the others. Tests: `r_dup_001__renamed_copies_are_duplicates`, `r_dup_001__small_bodies_are_never_reported`.

**T6. Stale doc references (`HN-O008`).** Markdown headings and fenced code samples of registry docs and change docs that name an object path (`module::fn`, `file.rs#name`) or an ID that no longer resolves. Test: `r_doc_001__a_doc_naming_a_removed_function_is_stale`.

**T7. Split suggestions (`HN-O010`).** Components as above; `sync` writes them to `gen/map.suggest` beside `hier.suggest` (only when there are any). Test: `r_map_002__two_clusters_suggest_a_split`.

**T8. Budgets.** `map` on the 100k corpus within the `context` budget rules; the graph for one path is built without loading the whole tree's objects (by the cache index). Done when the bench numbers are recorded.
