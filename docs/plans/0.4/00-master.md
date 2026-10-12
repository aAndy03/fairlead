# fairlead 0.4 implementation plan (master)

> Release 0.4, "the content model". Scope: [00-outline.md](00-outline.md) (D-63, owner answers D-64). The v1 API freezes at the end of 0.4 (D-64): the last phase finalises [../../v1-api.md](../../v1-api.md) and turns the 0.3 snapshot test into a freeze. Rules for the new surface are written into the chapters as each phase lands (a new chapter, `34-content-model.md`, holds the object model, constants, `map`, `query`/`require` and agent mode), tagged `[0.4]`.
>
> **Level of detail.** As for 0.3: tasks, interfaces, file formats and test names; the executor writes the code against the cited chapter. The chapters and [decisions.md](../../decisions.md) win over this plan; differences go into `plans/0.4/errata.md` (E4-n).

## Starting point

- 0.3 is built (hierarchy, variation, portability, retention, task queries, MCP server). Its unbuilt remainder is [../1.0-gaps.md](../1.0-gaps.md); the rows targeted at 0.4 are scheduled in P9 below.
- Everything fairlead knows about a file today comes from line-based readers: directives, CSS tokens, regex imports, literals, identifier hashes. 0.4 adds **objects** parsed from the file's content, without removing any line-based reader: a file whose language has no parser keeps today's behaviour exactly.
- The 0.3 code table is complete and `PENDING_0_3` is empty. 0.4 registers its codes at milestone `M04`, sets `ACTIVE_MILESTONE = M04` in P1-T1 and keeps a `PENDING_0_4` list in `tests/codes.rs`, as 0.2 and 0.3 did.
- All 0.1, 0.2 and 0.3 suites must stay green, including the edition byte-stability fixtures and the v1 API snapshot (a planned change updates the snapshot with an errata entry until the freeze in P9).

## Global constraints (added to those of 0.1 to 0.3)

1. **Fast first.** Nothing is parsed unless a rule, a map, a directive or a constant needs it. Objects are cached by content hash and parser version; a warm run re-parses only changed files. Budgets on the bench harness's 100k-file corpus: warm `check` with object rules within 1.2x of the 0.3 time, cold within 2x. A phase that breaks a budget does not merge.
2. **Pinned parsers.** Parsers are tree-sitter grammars pinned to exact versions, or the pure Rust parsers already in the tree (TOML, Markdown, JSON). Adopting tree-sitter raises the MSRV to 1.90 (R-REL-006), announced in the 0.4 changelog. The grammars' crates are new downloads: the owner allows them before P1 starts (question Q4-4).
3. **Objects are data.** A parse never changes the registry by itself: an object becomes an item only through a directive, a `const` declaration or a rule that names it. Rules over objects are declarative; there is no scripting and no network (D-26).
4. **Writers show their plan.** Rewriting references (P6), governed config editing and agent-mode approvals (P8) are plans first and transactions second, like every writer since 0.1. `--dry-run` writes nothing.
5. **Agent mode never weakens a floor.** Floor rules never yield, sealed keys outside the named scopes still need an owner, and every agent-mode approval is recorded and listed (outline, "Agent mode").

## Document map

| File | Phase | Covers | Tasks |
|---|---|---|---|
| [p1-objects.md](p1-objects.md) | P1 | object model, content cache, the Rust parser, benchmarks, tree-sitter adoption | 9 |
| [p2-constants.md](p2-constants.md) | P2 | kind `const`, drift, redundancy, repeated literals, unused constants, ownership | 9 |
| [p3-anchors.md](p3-anchors.md) | P3 | directives at any line bound to the object below, identity across edits and moves | 6 |
| [p4-map.md](p4-map.md) | P4 | `fairlead map` (text, JSON, MCP), split suggestions, duplicates, stale doc references | 8 |
| [p5-rules.md](p5-rules.md) | P5 | the `query` and `require` rule types and their language | 8 |
| [p6-restructure.md](p6-restructure.md) | P6 | reference rewriting on rename and move (lifting D-50), automatic follow-up | 7 |
| [p7-languages.md](p7-languages.md) | P7 | TypeScript and JavaScript, CSS, Markdown, TOML and JSON parsers | 6 |
| [p8-config-agent-mode.md](p8-config-agent-mode.md) | P8 | `fairlead config` (governed editing), agent mode | 8 |
| [p9-gaps-freeze.md](p9-gaps-freeze.md) | P9 | the 0.3 gaps for 0.4, exit criteria, field test, the v1 API freeze | 10 |

71 tasks.

## Phase graph

```text
P1 objects + cache + Rust --> P2 constants --> P3 anchors --\
          \                       \------------------------+--> P5 rules --> P6 restructure --\
           \--> P7 languages (after P2 proves the model) ---/                                  +--> P9 gaps + freeze
P4 map (needs P1, P2) ---------------------------------------------------------------------- /
P8 config + agent mode (needs only 0.3) ------------------------------------------------------/
```

P1 is first: every other phase reads objects, and the benchmarks gate the design. P7 waits until P2 has proved the model on Rust, then adds languages in the owner's order (Q4-3). P8 depends only on 0.3 and can run in parallel.

## Module map (new)

| Module (`crates/fairlead-core/src/`) | Content | Phase |
|---|---|---|
| `content/` | `Object`, `ObjectKind`, `ObjectId`, the `Parser` trait, per-language parsers, the object cache | P1, P7 |
| `content/consts.rs` | constant extraction and value canonicalisation (reuses `tokens::canon`) | P2 |
| `anchor.rs` | binding a directive to the object below it; object identity across edits | P3 |
| `map/` | the object graph, renderers, cluster analysis for split suggestions, duplicate detection | P4 |
| `rules/query/` | the rule language: parser, checker, evaluator | P5 |
| `restructure/rewrite.rs` (extends 0.2 `restructure.rs`) | reference rewriting plans | P6 |
| `config_edit.rs` | governed control-file edits | P8 |
| `agent_mode.rs` | the agent-mode window, its approvals and their listing | P8 |

## New codes (milestone `M04`)

| Code | Name | Default | Phase |
|---|---|---|---|
| HN-O001 | `obj.parse-fallback` | info / observe | P1 |
| HN-O002 | `const.drift` | error / warn | P2 |
| HN-O003 | `const.redundant` | warn / warn | P2 |
| HN-O004 | `const.literal` | error / warn | P2 |
| HN-O005 | `const.unused` | info / observe | P2 |
| HN-O006 | `obj.ownership` | error / enforce | P2 |
| HN-O007 | `obj.duplicate` | warn / observe | P4 |
| HN-O008 | `doc.stale-object` | warn / warn | P4 |
| HN-O009 | `anchor.unbound` | warn / warn | P3 |
| HN-O010 | `map.split-suggest` | info / observe | P4 |
| HN-Q001 | `rule.query` | per rule | P5 |
| HN-Q002 | `rule.require` | per rule | P5 |
| HN-Q003 | `rule.query-invalid` | error / enforce | P5 |
| HN-Y001 | `agent-mode.approval` | info / warn | P8 |
| HN-Y002 | `agent-mode.outside` | error / enforce | P8 |

## Exit criteria (pre-registered, R-ROAD-014)

| # | Criterion | Measured by |
|---|---|---|
| 1 | Warm `check` with the bundled object rules within 1.2x of 0.3 on the 100k corpus; cold within 2x; peak memory within 1.5x | bench harness, P1-T9 and P9-T5 |
| 2 | Constant drift and redundancy: on the seeded constants fixture, precision at least 95% and recall at least 90% | `xtask const-fixture`, P2-T9 |
| 3 | An anchored directive follows its object through 50 scripted edits (insertions above, renames of neighbours, moves to another file with `fairlead move`) with no lost binding | P3-T6 |
| 4 | A rename or move rewrites every reference fairlead can see, and a second run of the same plan is a no-op; on three golden repositories `check` is unchanged before and after | P6-T7 |
| 5 | Agent mode: every bound (scopes, versions, floor rules, sealed keys) refuses outside its window; every approval is listed by `seal`, `context` and `check --ci` | P8-T8 |
| 6 | Every 0.1 to 0.3 suite passes unchanged; `PENDING_0_4` is empty; the v1 API snapshot is frozen | P9-T10 |
| 7 | An agent field test on a real repository, as for 0.2, with its findings fixed or recorded | P9-T8 |

## Owner questions

| # | Question | Recommended default |
|---|---|---|
| Q4-4 | Allow the tree-sitter core and grammar crates (new downloads) and the MSRV rise to 1.90 at the start of P1? | Yes, after the weekly gate of 0.3 (E3-36) has passed once. |
| Q4-5 | Allow `rmcp` for the MCP transport (G-1)? | Yes, in P9, pinned to an exact minor; the tool surface does not change. |
| Q4-6 | The `query` rule language: a small path-and-predicate syntax in TOML strings, or structured TOML tables? | Structured TOML tables, with a one-line string form for the simple cases (P5-T1 writes both and the owner picks). |
| Q4-7 | Publish `fairlead-mcp` on crates.io from 0.4? | Yes, beside the three crates. |

Q4-1 to Q4-3 were answered in D-64.
