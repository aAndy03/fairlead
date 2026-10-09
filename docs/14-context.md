# Context
> Status: draft · Milestone: mixed · Decisions: D-06, D-10, D-11, D-18, D-23, D-25, D-26, D-27, D-32, D-41, D-46 · Related: 10-scopes-and-inheritance.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 15-cli.md, 20-versions-and-seal.md, 30-hierarchy-and-engines.md, 33-retention-and-compaction.md

## Purpose

`fairlead context` is what an agent runs at session start and before touching a path. It returns the smallest accurate brief of the rules, items, drift and history that apply there, in a terse line format, packed to a token budget. Its cost depends on tree depth and the query, never on the age of the project. It is also the main measurable claim of 0.1: agents given this output change behaviour (see [60-roadmap.md](60-roadmap.md)).

## Concepts

| Term | Definition |
|---|---|
| Brief | The output of one `context` call. Text first; `--format json` for tools. |
| Tier | A size class of information: L0 index, L1 scope card, L2 detail, L3 archive. Not the hierarchy adoption levels of [30-hierarchy-and-engines.md](30-hierarchy-and-engines.md). |
| Effective view | A scope's rules, items and policies after resolving the chain from the root (D-10). Never copied to disk. |
| Digest | `d`: the first 40 bits of blake3 over canonical bytes, shown as 8 base32 characters. |
| Delta | A brief limited to what changed since a version, a git revision or a digest. |
| Budget | `--budget N` estimated tokens; `ceil(bytes/4)` unless the control file sets `tokenizer=`. |
| Terse line | `ID k=v ...`; kind implied by the ID prefix, defaults omitted, fields never aliased. |

### Tiers

| Tier | Content | Size bound | Source | Milestone |
|---|---|---|---|---|
| L0 index | One line per scope: `id parent d=<8> items drift purpose<=80ch` | about 30 tokens per scope | computed on demand; committed rollup `index.l0` written only by `seal` | on demand 0.1; rollup 0.2 |
| L1 scope card | Purpose (<=160 chars), item counts per kind, error-severity rules applying here (ID plus a title of about 12 tokens), exception count, drift count, child L0 lines; from 0.2 also the last 5 sealed one-liners | hard cap 400 tokens | authored sources | 0.1 (sealed lines 0.2) |
| L2 detail | Full rule text, item shapes, ADRs, change docs, use sites | unbounded, only sliced | `fairlead explain` | 0.1 |
| L3 archive | Tombstones, graveyard, compacted history | only by query | ledger | 0.2 (rollups 0.3) |

### Digest chain

Scope effective digest = `H(parent effective digest, own authored digest, generator version)`, a Merkle chain. The digest of an ancestor chain is therefore one token-sized fact. A change in a leaf never changes an ancestor's digest; intent flows down, so only descendants see an ancestor's change. A generator-version or edition change changes every digest, so stale digests fail safe.

## Rules

### Command and queries
- **R-CTX-001** [0.1] `fairlead context` with no argument MUST print the session-start brief: root L1, child L0 lines, the newest handoff as a `handoff:` line (R-CTX-008) and, from 0.2, the open version.
- **R-CTX-002** [0.1] `fairlead context <path>` MUST resolve the nearest scope upward and return that scope's brief for the path. A path is any existing file or directory.
- **R-CTX-003** [0.1] A missing path (an argument that contains `/` and is not an ID) MUST resolve to its nearest existing ancestor, with the note on stderr and (from 0.2) the move-ledger hint; with no existing ancestor it resolves to the root brief. A path outside the codebase is exit 2. A path in a gitignored area still resolves its scope and is flagged `ignored`.
- **R-CTX-004** [0.1] An argument that is neither an existing path nor an ID MUST, in 0.1 and 0.2, exit 2 with `no match: task queries need 0.3; pass a path or an ID`. It never guesses and never ranks. An argument matching the ID grammar is an entity lookup and MUST be forwarded to `fairlead explain`.
- **R-CTX-005** [0.3] Any other free text is a task query (below).
- **R-CTX-006** [0.1] `context` MUST NOT write tracked files and MUST NOT start a background process (D-26). It MAY refresh the disposable cache.
- **R-CTX-007** [0.1] If the control file of a scope in the chain cannot be parsed, `context` MUST print the parse diagnostic and exit 2 instead of a partial brief (fail closed, D-11).

- **R-CTX-008** [0.1] Reasoning and memory outside fairlead are lost between sessions; fairlead does not capture transcripts. The mitigation is the authored handoff: `fairlead new handoff` scaffolds a handoff doc (built-in doc kind `handoff`, mandatory `sum`) and the session-start brief prints the newest handoff's `sum` as a quoted `handoff:` line, data per R-CTX-032. The residual risk (work and reasoning that nobody wrote down) is accepted.

### Path query content
- **R-CTX-010** [0.1] A path brief MUST contain, in this order: header (scope ID, effective digest, edition, ancestor chain), purpose, rules applying to the path (error severity first), items the path declares or uses, drift under the path, active waivers.
- **R-CTX-011** [0.1] Rules MUST be filtered by their `glob` against the path; a rule with no glob applies scope-wide.
- **R-CTX-012** [0.1] Declared and used IDs MUST come from the cache of `def`/`use` directives under the path. For a directory the top-K by use count are listed, with `+N more -> fairlead explain <x>`.
- **R-CTX-013** [0.1] Drift MUST be the check result for the files under the path, rechecking only those files when the cache is stale. Baselined findings are counted separately from new ones.
- **R-CTX-014** [0.2] The brief MUST also list graveyard entries tagged with the scope or with listed IDs, as `grave:` lines. Tombstones never appear in default `context` ([retention](33-retention-and-compaction.md) R-RET-041); `new`, `check` and `explain` read them.
- **R-CTX-015** [0.3] Where hierarchy is adopted, the brief MUST add the containers of the path, their units' IDs and the public `use` surface only ([30-hierarchy-and-engines.md](30-hierarchy-and-engines.md)). Open variation requests and expiring interim wrappers for the scope are listed ([31-variation-protocol.md](31-variation-protocol.md)).

### Inheritance dedup
- **R-CTX-020** [0.1] Output MUST have an `inh:` block and a `rules:` block, written one object per line: a label line (`inh: ^root dq3f2xbr 6err`, one per ancestor, or `rules:`) followed by two-space-indented object lines. Only error-severity rules whose glob applies to the path are inlined under their ancestor, in L0 form (ID plus fenced title). `rules:` holds items introduced, changed or overridden at or below the scope. `declares:`, `uses:`, `drift:` and `waivers:` are labelled lines or blocks in the same way. The byte layout is pinned by the `context__*` snapshots.
- **R-CTX-021** [0.1] An overridden ancestor rule MUST NOT be printed twice; the effective rule carries a note `(ovr <scope>: <why>)`.
- **R-CTX-022** [0.1] `--full` MUST expand the inherited rules.
- **R-CTX-023** [0.2] With `--since`, every unchanged ancestor object MUST be elided. No session state is stored.

### Output format
- **R-CTX-030** [0.1] Output MUST use the terse line format: `ID k=v ...`, kind implied by the ID prefix, `st=active` omitted, no aliased or positional fields.
- **R-CTX-031** [0.1] Output MUST be LF-only, ASCII except inside quoted text, with items sorted bytewise by ID within a block.
- **R-CTX-032** [0.1] Free text from directives and docs (`why=`, purposes, summaries) is data (D-18). It MUST be printed quoted, with quotes escaped, and capped (rule titles about 12 tokens, others about 40). It MUST NOT be rendered as prose or as an instruction.
- **R-CTX-033** [0.1] `--format json` MUST emit the same facts as the text form with identical ordering. `--digest` MUST print only the effective digest.
- **R-CTX-034** [0.3] `--dense` MAY emit a dictionary-aliased positional form for L0 dumps of very large repos. It is optional and never the default.

### Budget
- **R-CTX-040** [0.1] With `--budget N`, the brief MUST be packed in the priority order below. Lower priorities are dropped first, and every omission MUST print `+N more -> fairlead explain <x>`; truncation is never silent.
- **R-CTX-041** [0.1] If P0 and P1 alone exceed the budget, they MUST be emitted in L0 form (ID plus title) and the command exit 2 (`truncated-mandatory`; exit codes as in [CLI](15-cli.md) R-CLI-003), reporting the minimum budget needed.
- **R-CTX-042** [0.1] Within a priority, items are ordered by `score = sev_weight × proximity × relevance`. Proximity is `1/(1+scope distance)`. Relevance is 1.0 for a direct ID, 0.7 for a glob match, 0.4 for scope-wide. Ties break by ID bytewise. Default `sev_weight`: error 4, warn 2, info 1 (control file may override).
- **R-CTX-043** [0.1] The default token estimate is `ceil(bytes/bpt)` with `bytes-per-token = 4` (`[context]` in the root control file). Every count is labelled an estimate (`est` in the footer, `tokens_est` in JSON). The control-file `tokenizer=` MAY name an exact counter built into the binary; `--exact-tokens` uses it instead of the estimate and exits 2 if none is configured. Per-object estimates are cached so packing never re-tokenises.
- **R-CTX-044** [0.1] Default budgets, used when `--budget` is absent: 500 tokens for the no-argument brief and 700 for a path query (`[context] budget-start`, `budget-path`). `--budget 0` removes the cap. These values are fixed for the 0.1 controlled test.
- **R-CTX-045** [0.1] The `sev_weight` values (4/2/1) and the relevance constants (1.0/0.7/0.4) of R-CTX-042 ship as defaults and are configurable (`[context] sev-weight`). They are tuned from the 20-task test; a changed default takes effect only with a new edition, because output is a byte-stable contract (D-17).
- **R-CTX-046** [0.1] Budget overflow is answered by a `+N more -> fairlead explain <x>` line for what was dropped. There is no continuation token and no session state.

| Priority | Content | Data available |
|---|---|---|
| P0 | Header: scope ID, digest, edition, ancestor chain | 0.1 |
| P1 | Error-severity rules that apply to the path | 0.1 |
| P2 | Open drift and waivers on the path | 0.1 |
| P3 | Items the path declares or uses | 0.1 |
| P4 | Warn-severity rules | 0.1 |
| P5 | Recent changes; matching graveyard entries | 0.2 |
| P6 | Sibling and child L0 | 0.1 |
| P7 | Ancestor L1 | 0.1 |

### Digests and deltas
- **R-CTX-050** [0.1] The brief MUST end with the footer `=d <digest> est=<n> next: fairlead explain <x>`, where `est` is the estimated token count of the whole text (R-CTX-043) and `next:` is a built-in fixed-form hint. The digest generator string is `ctx1/ed<edition>`, bumped only when the context byte format changes.
- **R-CTX-051** [0.2] `--since <version|git-rev>` MUST be the primary delta form, computed deterministically from the sealed changelog and the generated-file diff. It works on any machine.
- **R-CTX-052** [0.2] `--since <digest>` is a best-effort accelerator: it returns the new digest plus only objects whose digests differ; unchanged objects appear as `~` lines. It relies on a manifest in the gitignored cache.
- **R-CTX-054** [0.2] A delta is always recomputed on the current tree. On a merged or switched branch the version or git-rev form is correct by construction, and a digest the manifest does not know degrades to the full slice (R-CTX-053), never to a wrong delta.
- **R-CTX-053** [0.2] If the manifest is unknown (other machine, other generator version or edition), the output MUST be the full slice with `since=unknown`. It is never an error.

### Task query
- **R-CTX-060** [0.3] Matching MUST be deterministic BM25 over a precomputed inverted index (no embeddings, no network) over ID slugs, item titles, doc headings, authored `kw=` tags and tombstone summaries.
- **R-CTX-061** [0.3] Output MUST begin with `matched: <scope> <score>, ...; pin with --scope` listing the top 3 candidate scopes, then the top-K items and docs, with matching graveyard entries first.
- **R-CTX-063** [0.3] BM25 recall on very short task text is measured on the 0.3 query set. Where recall fails, the kind MAY be made to require authored `kw=` tags (a `check` finding on an item that lacks them), scope-name boosts apply, and `--scope` pins the scope. BM25 is not dropped.
- **R-CTX-062** [0.3] Zero matches MUST return the root L1 plus top-level L0 and state "no match". It never guesses. Ties across scopes break by scope ID.

### Performance
- **R-CTX-070** [0.1] `context` SHOULD stat only the managerial folders in the chain and the queried path, not the whole tree. Targets (goals, not requirements until R-CTX-072 has run): warm `context` under 30 ms.
- **R-CTX-072** [0.1] The performance figures in this chapter, including the cost of Merkle digests and the ledger at 100k files, are goals until the 0.1 release benchmark has run. The same benchmark counts tokens of the sample slices with the target model's tokenizer and records the measured bytes-per-token ratio in the [roadmap](60-roadmap.md); the default ratio changes only with an edition (R-CTX-045).
- **R-CTX-071** [0.1] In a codebase with hundreds of scopes, `context` MUST show only the chain to the path plus sibling and child counts under the budget. A whole-tree L0 requires `--depth N`.

## What the 0.1 minimal version does

| Included in 0.1 | Deferred |
|---|---|
| No-arg brief and path query | Task queries by BM25 (0.3) |
| Scope chain, effective rules, items, drift, waivers | Graveyard, recent changes (0.2); tombstones only via `explain` |
| Terse format, quoting of free text, `--format json`, `--digest` | `--since` deltas of any kind (0.2) |
| Inheritance dedup (`inh:` / `rules:` / `--full`) | Committed L0 rollup `index.l0`; last 5 sealed lines in L1; open version (0.2) |
| `--budget` packing for P0-P4, P6, P7, exit 2; default budgets 500 and 700 | L3 rollups (0.3), hierarchy and variation blocks (0.3) |
| Effective digest printed in header and footer | `--dense` (0.3, optional) |
| `fairlead explain` as the L2 drill-down; newest handoff line (`new handoff`) | Digest manifest (0.2) |

The 0.1 controlled test (about 20 tasks, D-06) measures whether agents that read this output change behaviour. The format and budget rules are therefore fixed in 0.1 so the test results carry forward.

## Why the terse line (token comparison)

Sample slice: scope `web`, 6 items: 2 tokens, 1 rule, 1 component, 1 variant, 1 deprecated engine, each with a usage count.

Assumptions: about 4 characters per token; counts are hand-computed and approximate (plus or minus 10%); no real tokenizer was run. Punctuation-heavy formats (JSON, YAML, tables) usually tokenise worse than 4 characters per token, so the true gap probably favours terse formats more. Hex values and IDs cost the same in every format.

Verbose YAML for two of the six items:

```yaml
- id: cmp.button
  kind: component
  scope: web
  status: active
  layer: atom
  home: ui
  usage_count: 14
- id: eng.sync-verify
  kind: engine
  scope: web
  status: deprecated
  layer: engine
  since: "1.3"
  replaced_by: eng.sync-check
  usage_count: 5
```

The same two items, terse:

```text
cmp.button layer=atom home=ui uses=14
eng.sync-verify layer=engine st=dep since=1.3.0 repl=eng.sync-check uses=5
```

The full slice in terse form (about 260 characters):

```text
# web
tok.primary value=#2563eb uses=31
tok.surface value=#ffffff uses=22
rule.no-raw-hex sev=error
cmp.button layer=atom home=ui uses=14
cmp.button-danger variant-of=cmp.button uses=3
eng.sync-verify layer=engine st=dep since=1.3.0 repl=eng.sync-check uses=5
```

| Encoding | Chars | Tokens (est.) | vs YAML | Notes |
|---|---|---|---|---|
| Verbose YAML (`- id:`, `kind:`, `scope:`, `status:` per item) | 707 | 177 | 100% | kind, scope and status repeated per item |
| Compact JSON array | 687 | 172 | 97% | quotes and braces; real cost about 10% higher |
| TOML tables | 660 | 165 | 93% | |
| Markdown table | 504 | 126 | 71% | repeated columns, padding pipes |
| Terse line | 263 | 66 | 37% | self-describing, grep- and diff-friendly |
| Dictionary-aliased positional | about 250 | 62 | 35% | header amortises; at 1,000 items about 33% smaller than terse. Positional misreads, unreadable in review. Optional `--dense` only |

Terse is about 63% smaller than verbose YAML: about 11 tokens per item against about 30. Linear extrapolation to 1,000 items gives about 11k against about 30k tokens (the scope header and the dictionary amortise away, so the real ratio varies). The positional form's extra 4 to 6% is not worth the misread risk, so aliasing is not the default. Committed generated shards omit usage counts, which churn on every edit; counts live in the cache and in `context` output.

## Examples

Path query, 0.1 shape plus the 0.2 `grave:` line:

```text
$ fairlead context apps/web/src/settings --budget 700
@settings d=ahq7k2mz ed=1 ^root:dq3f2xbr ^web:7bq4x2ec
purpose: data apps/web/fairlead/control.toml@a1b2c3 "user preferences UI (organisms and templates)"
rules:
  rule.form-schema sev=error data apps/web/src/settings/fairlead/control.toml@e5f6a7 "forms via schema"
inh: ^root dq3f2xbr 6err
  rule.no-eval data fairlead/control.toml@9c8d7e "no eval"
inh: ^web 7bq4x2ec 2err
  rule.no-raw-hex data apps/web/fairlead/control.toml@d4e5f6 "tokens not hex"
declares: cmp.form-row
uses: cmp.button tok.primary tok.surface
drift: new=2 baselined=0
  HN-T001 rule.no-raw-hex apps/web/src/Panel.tsx:44,81 sev=error n=2
grave: grv.0004 data fairlead/docs/graveyard/0004.md@b7c8d9 "autosave on blur" rejected 1.2 (drafts lost) do-not-retry
=d 7fq3a2kc est=214 next: fairlead explain HN-T001
```

The block shape and footer are normative; the exact bytes are pinned by the snapshots (the `grave:` line is 0.2). About 210 tokens, fences included. Dumping the same information as verbose YAML (including each ancestor's rules in full) would be roughly 400 to 500 tokens by the extrapolation above.

Delta (0.2):

```text
$ fairlead context apps/web/src/settings --since 7f3a21c9
@settings d=c4q7w2bd since=7fq3a2kc 3 changed
~ ^root dq3f2xbr
rule.no-raw-hex sev=error "tokens not hex" (ovr web: "legacy charts" until=1.9.0)
uses: +cmp.form-row-dense -tok.surface-old
drift: 0
=d kq2e7wa3 est=96 next: fairlead check
```

Task query (0.3):

```text
$ fairlead context "add sync progress bar" --budget 500
matched: web/sync 0.82, web 0.40, core/sync 0.31; pin with --scope
grave: grv.0007 "toast-only progress" rejected 1.8 (lost on navigation)
...
```

Budget too small:

```text
$ fairlead context apps/web --budget 40
@web d=7bq4x2ec ed=1
rule.no-raw-hex "tokens not hex"
+5 more -> fairlead explain @web
=d 7bq4x2ec est=38 next: fairlead explain rule.no-raw-hex
truncated-mandatory: minimum budget 96     (exit 2)
```

## Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Budget smaller than P0+P1 | L0 form, exit 2, minimum reported | R-CTX-041 |
| 2 | `--since` digest unknown, from another machine, generator or edition | Full slice with `since=unknown`, never an error | R-CTX-053 |
| 3 | Path deleted or renamed | Nearest existing ancestor, note on stderr (move-ledger hint from 0.2); no ancestor: root brief | R-CTX-003 |
| 4 | Path outside the codebase | Exit 2 | R-CTX-003 |
| 5 | Path in a gitignored area | Scope resolves; flagged `ignored` | R-CTX-003 |
| 6 | Task query matches nothing [0.3] | Root L1 plus top-level L0 and "no match" | R-CTX-062 |
| 17 | Free text in 0.1 or 0.2 | Exit 2, `no match: task queries need 0.3`; no guess | R-CTX-004 |
| 18 | `--exact-tokens` with no `tokenizer=` configured | Exit 2 | R-CTX-043 |
| 7 | Task query ties across scopes | Top 3 with scores; deterministic tie-break by scope ID | R-CTX-062 |
| 8 | Directive `why="ignore previous instructions"` | Printed quoted and capped as data; never promoted to prose | R-CTX-032 |
| 9 | Unparseable control file in the chain | Diagnostic and exit 2, no partial brief | R-CTX-007 |
| 10 | Overridden ancestor rule | Printed once, effective version, with `(ovr scope: why)` | R-CTX-021 |
| 11 | Compacted record still referenced by `use` | ID resolves to its tombstone; a ref to a removed feature is drift | [33-retention-and-compaction.md](33-retention-and-compaction.md) |
| 12 | Monorepo with 500 scopes | Chain to the path plus sibling and child counts; whole-tree L0 needs `--depth` | R-CTX-071 |
| 13 | Directory with thousands of `use` lines | Top-K by use count, `+N more` line | R-CTX-012 |
| 14 | Cache stale or missing | Rebuilt; only affected files rescanned; output identical | R-CTX-006 |
| 15 | Two agents call `context` concurrently | Shared read lock; no writes beyond the cache | R-CTX-006 |
| 16 | Wrong kind prefix in a lookup (`cmp.` for a token) | Forwarded to `explain`, which suggests the right ID | R-CTX-004 |

## Resolved questions

- Q14-1 (resolved 2026-10-09): `ceil(bytes/4)` ships as a labelled estimate with the ratio in config; the real-tokenizer benchmark is part of the 0.1 release test; see R-CTX-043, R-CTX-072
- Q14-2 (resolved 2026-10-09): budget overflow is exit 2, as D-41 defines; see R-CTX-041
- Q14-3 (resolved 2026-10-09): 4/2/1 and 1.0/0.7/0.4 ship as configurable defaults, tuned from the 20-task test; see R-CTX-045
- Q14-4 (resolved 2026-10-09): measure first; require `kw=` where recall fails, add scope-name boosts, rely on `--scope`; see R-CTX-063
- Q14-5 (resolved 2026-10-09): free text is a hard error in 0.1 and 0.2 (exit 2, "no match"), no lexical fallback; see R-CTX-004
- Q-CTX-1 (resolved 2026-10-09): estimate with a configurable ratio, exact counts opt-in, default budgets 500 and 700; see R-CTX-043, R-CTX-044
- Q-CTX-2 (resolved 2026-10-09): only path and ID queries work in 0.1; a task query answers "no match" rather than guessing; see R-CTX-004
- Q-CTX-3 (resolved 2026-10-09): handoff docs via `new handoff`, shown in the session brief; residual risk accepted; see R-CTX-008
- Q-CTX-4 (resolved 2026-10-09): `--since` by version or git rev is primary, digest is an accelerator, deltas are recomputed on the merged tree; see R-CTX-051, R-CTX-052, R-CTX-054
- Q-CTX-5 (resolved 2026-10-09): a `+N more` line plus `explain`, no continuation token; see R-CTX-040, R-CTX-046
- Q-CTX-6 (resolved 2026-10-09): benchmark before the targets become requirements; see R-CTX-072
- Q-CTX-7 (resolved 2026-10-09): measure BM25 recall, then require `kw=` where it fails; see R-CTX-063
