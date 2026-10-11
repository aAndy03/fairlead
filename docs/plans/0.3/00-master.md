# fairlead 0.3 implementation plan (master)

> Release 0.3, "hierarchy, variation, portability, retention". Scope: [60-roadmap.md](../../60-roadmap.md) section 3.3. Exit criteria: section 4.3. Rules: every rule tagged `[0.3]` (175 by the roadmap count, 190 tagged lines), mainly chapters 30 to 33. The v1 API freeze moved to the end of 0.4 (D-64); 0.3 writes the surface down (P7-T7) but does not freeze it.
>
> **Level of detail.** As for 0.2: tasks, interfaces, file formats and test names; the executor writes the code against the cited chapter. Chapters and [decisions.md](../../decisions.md) win over this plan, and differences go into `plans/0.3/errata.md`.

## Starting point

- 0.2 is released: seals, the ledger with checkpoints, change docs, tombstones, the graveyard, `move` (0.2 flags), editions and `upgrade`, and git integration. 0.3 builds on all of them:
  - dormancy reads the seal ledger;
  - requests are change docs;
  - compaction runs inside `seal`;
  - `move` gains `--cut`, `--copy`, `--attach` and `--adopt`.
- The code table already registers every 0.3 code at milestone `M03`: `HN-H001` to `H025`, `HN-W001` to `W004`, `HN-T006`, `HN-T007`, `HN-D004` to `D006`, `HN-K001`, `HN-M004` to `M006`, `M009`, `M010`, `M012`, `HN-V012` and `HN-V014`. The first task sets `ACTIVE_MILESTONE = M03` and adds a `PENDING_0_3` list to `tests/codes.rs`, as 0.2 did.
- All 0.1 and 0.2 suites must stay green (exit criterion 6).

## Global constraints (added to those of 0.1 and 0.2)

1. **Still AST-free by default.** Import evidence is a regex pass (R-HIER-033). It confirms or contradicts declared edges and never creates a rank or stale verdict on its own. Every report that depends on `use` edges says it is a lower bound (R-HIER-034, D-40).
2. **Tree-sitter is gated.** TypeScript parsing through tree-sitter is adopted only after the CI compatibility test of R-ARCH-154 and R-HIER-035 passes. Adopting it raises the MSRV to 1.90 (R-REL-006). It is optional for 0.3 (4.3 criterion 7).
3. **The wall clock is read only at seal** (`sealed-at`). Retention eligibility and rollup quarters use recorded seal timestamps (R-RET-011, R-RET-020).
4. **No cross-repository transaction** (R-PORT-043). Nothing is pushed between repositories (D-26).
5. **Configuration is user data.** The binary knows only "ranked layers inside a named ladder" (30-hierarchy, preamble). Presets are scaffolded content (D-20).

## Document map

| File | Phase | Covers | Tasks |
|---|---|---|---|
| [p1-hierarchy.md](p1-hierarchy.md) | P1 | ladders, layers, kinds, containers, units, bindings; L0 to L2 checks; `use` graph; evidence; suggestions | 14 |
| [p2-entries-bridges.md](p2-entries-bridges.md) | P2 | entries, reachability, dormancy, bridges, traces, hierarchy docs-first | 9 |
| [p3-variation-tokens.md](p3-variation-tokens.md) | P3 | home and visibility, requests, answers, variants, axes, wrappers, adapters, forks, richer token checks | 13 |
| [p4-portability.md](p4-portability.md) | P4 | `requires`/`provides`, lock, mounts, adopt, cut/copy/attach, submodules, manifests | 12 |
| [p5-retention.md](p5-retention.md) | P5 | record fields, TTL policy, compaction in seal, hash chain, rollups, purge, references to compacted records | 11 |
| [p6-context-tooling.md](p6-context-tooling.md) | P6 | BM25 task queries, `--dense`, hierarchy in briefs, general signed-commit verification, import-extractor seam, MCP crate | 8 |
| [p7-quality-freeze.md](p7-quality-freeze.md) | P7 | exit criteria 4.3, v1 API freeze, 1.0 path | 8 |

75 tasks.

## Phase graph

```text
P1 hierarchy --> P2 entries + bridges --\
      \--------> P3 variation ----------+--> P7 quality + freeze
P4 portability (needs 0.2 move) --------+
P5 retention (needs 0.2 seal/ledger) ---+
P6 context + tooling (needs P1, P5) ----/
```

P1 is first because P2 and P3 extend its graph. P4 and P5 depend only on 0.2 and can run in parallel with P1. P6 needs P1 (hierarchy in briefs) and P5 (temperature).

## Module map (new)

| Module (`crates/fairlead-core/src/`) | Content | Phase |
|---|---|---|
| `hier/` | `Ladder`, `Layer`, `Unit`, `Container`, `Entry`, `Bridge`; binding; graph; checks; suggestions shard | P1, P2 |
| `evidence/` | regex import pass (TS `import`/`from`, Rust `use crate::`), `sym` whole-identifier search | P1 |
| `variation/` | requests, answers, axes, variants, wrappers, adapters, fork heuristics | P3 |
| `tokens/` (extends `extract/styles.rs`, `extract/literals.rs`) | colour canonicaliser, unit-aware lengths, Tailwind arbitrary values, one-level `var()` resolution | P3 |
| `portable/` | `requires`/`provides`, lock entries and snapshots, mounts, attach diff | P4 |
| `retention/` | record schema, TTL evaluation, compaction plan, rollups, chain, purge | P5 |
| `query/` | BM25 inverted index (in the gitignored cache), scorer, task-query brief | P6 |
| `crates/fairlead-mcp/` | MCP server, tools and resources only, depends on `fairlead-core` | P6 |

## Owner questions

| # | Question | Needed by | Recommendation |
|---|---|---|---|
| Q-1 | Do the 0.3 keys (`pin=`, `spell=`, `container=`, `layer=`, `vis=`...) stay in edition 1, or does 0.3 ship edition 2? | P1 | Stay in edition 1. They are new keys on known verbs, and older binaries warn and preserve them (R-EDN-021). Ship edition 2 only if a grammar change is unavoidable. Decide before the v1 freeze. |
| Q-2 | Adopt tree-sitter for TS imports in 0.3? | P6 | Not in 0.3. Keep the regex evidence; build only the extractor seam (R-ARCH-034) and the compatibility test. |
| Q-3 | Ship the MCP server (`rmcp`) in 0.3? | P6 | Yes, firm (owner, 2026-10-11, D-62): a separate crate, built in P6 whatever the timing of P1 to P5 (R-ARCH-004). |
| Q-4 | The BM25 query set for R-CTX-063 needs real short task texts with expected scopes. | P6 | Agent-run (D-59): protocol B-2, one writer agent and a separate labeller agent. |
| Q-5 | Hierarchy precision review (4.3 criterion 1): blind review of findings on three repositories. | P7 | Agent-run (D-59): protocol B-1, a blind reviewer agent. |

## Definition of done for 0.3

| # | Criterion (60-roadmap 4.3, short) | Verified by |
|---|---|---|
| 1 | Hierarchy findings: at most 2% false on blind review of three repositories; `stale` skipped with zero entries | P2-T3 (skip), agent protocol B-1 |
| 2 | Five-year simulated history compacts idempotently and byte-identically on two machines; leaf `context` size independent of history | P7-T2 |
| 3 | Cut, copy, mount, submodule and vendored-subtree scenarios pass goldens; no silent rank guessing at attach | P7-T3 |
| 4 | Scripted request-to-variant cycle completes; every consumer reference migrated; no undeclared fork flagged | P7-T4 |
| 5 | Bridge: removing either end, or changing the contract, is detected | P2-T7, P7-T5 |
| 6 | All 0.1 and 0.2 criteria still hold | P7-T6 |
| 7 | Tree-sitter used only if its compatibility test passes; otherwise regex evidence only | P6-T6 |

Also: `PENDING_0_3` is empty, the v1 API surface is written down and frozen (P7-T7), and the 1.0 gap list is opened (D-53).

## Risks

| Risk | Mitigation |
|---|---|
| Hierarchy is a large rule surface (58 rules) and can produce noisy findings. | The adoption ramp L0 to L3 (`[hier] level`) gates every check. Findings are capped by the stage ramp. Suggestions are info and land in a generated shard, not in the findings list. |
| Retention rewrites history-bearing files. | Compaction runs only inside `seal`'s transaction, before the commit point. Finalised years are immutable; purge is the only rewrite and does not re-chain (R-RET-033). Determinism is tested on two platforms. |
| Portability crosses repository boundaries. | No transaction spans two repositories (R-PORT-043). Everything imported is pinned and inert until approved (R-PORT-023, R-PORT-044). |
| Fork detection false positives annoy users. | It ships off (R-VAR-084). Similarity measures are reserved and unspecified (R-VAR-081). |
| The v1 freeze locks mistakes in. | The freeze is at the end of 0.4 (D-64), after the content model; P7 writes the 0.3 surface down as its draft. |
