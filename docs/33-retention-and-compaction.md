# Retention and Compaction
> Status: draft · Milestone: mixed (tombstones and graveyard 0.2; compaction, ledger and TTL 0.3) · Decisions: D-03, D-10, D-14, D-16, D-19, D-23, D-24, D-25, D-26, D-39 · Related: 11-files-and-classes.md, 13-checks-and-diagnostics.md, 14-context.md, 15-cli.md, 20-versions-and-seal.md, 21-environments-maturity-lifecycle.md, 23-git-integration.md, 32-portability-and-restructure.md

## Purpose

A codebase governed for decades accumulates change docs, requests, waivers and handoffs faster than anyone reads them. This chapter bounds that growth without losing what matters: the reasons behind decisions, the memory of rejected ideas, and the fact that an ID once existed. Compaction is mechanical, deterministic, runs only inside `seal`, and never needs a language model, because every record carries an author-written summary from the day it is created.

## Concepts

| Term | Definition |
|---|---|
| Record | A document-like item with history value: change doc, request, ADR, graveyard entry, handoff, brainstorm, waiver history, sealed changelog. Live state (registry, rules, item shapes) is not a record and is never compacted. |
| Decision-grade | A record whose outcome must stay visible forever: adopted ADR, graveyard entry, removal reason, exception (waiver). |
| Temperature | Hot, warm or cold, by version distance. Controls whether the record appears in `context`. |
| Age | Number of sealed versions after the version in which the record closed. A record of the open version has age 0. |
| Summary | The author-written `sum:` and `out:` fields, copied verbatim into the ledger at compaction. |
| Tombstone | A permanent registry line saying an ID existed and was removed or deleted. |
| Ledger | The append-only, hash-chained file class (D-19) that stores compacted summaries: `ledger/hist/<year>.hl`. |
| Rollup | A quarterly count line that replaces non-decision-grade summaries. |

### Temperature (D-24)

| Temp | Default | Appears in `context`? |
|---|---|---|
| Hot | open version plus the last 3 sealed versions (age 0 to 3) | yes |
| Warm | age 4 to 8 | only on a query hit or in `explain` |
| Cold | age above 8, compacted | one-line summary in L3, only by query |

Decisions, graveyard entries and tombstones are not subject to the cold step beyond their own body: their one-line form is kept forever ([14-context.md](14-context.md) tier L3).

## Rules

### Record classes and fields
- **R-RET-001** [0.3] Retention MUST apply only to records. The live registry, rules and item shapes MUST NOT be compacted; they are bounded by the size of the codebase.
- **R-RET-002** [0.3] Every record kind subject to retention MUST carry `sum` (non-empty, at most 240 characters) from creation, and `out` (empty while the record is open; one of `adopted`, `rejected`, `removed`, `superseded`, `abandoned` when it closes, D-39). `fairlead new` MUST create both slots; fairlead MUST NOT generate their content (D-24: fairlead has no LLM, no network).
- **R-RET-003** [0.3] A record with `out` of `rejected` or `removed` MUST also carry `why` (at most 240 characters).
- **R-RET-004** [0.3] `check` SHOULD warn, following the observe, warn, enforce ramp (D-28), when an accepted or closed record has an empty `sum` or `out`, so that the seal gate (R-RET-022) is never the first signal.
- **R-RET-005** [0.3] The summary MUST preserve: ID, kind, scope, title, opened and closed versions, outcome, reason, alternatives tried, successor IDs, tags (`kw`), a `dnr` (do-not-retry) flag, the commit hash that last held the full record, and a digest of the full record. Typical cost is 60 to 80 tokens per record.
- **R-RET-006** [0.2] The fields `sum`, `out`, `why` and `alt` MUST be capped at 240 characters each and MUST NOT contain secret-like values: a key or token pattern in them is `HN-F001` (floor rule, never baselined or waived), and `seal` refuses while one exists. Redaction of a value already committed is a human action; `sync --purge` (R-RET-043) handles records already in history.

| Record kind | Default TTL | Decision-grade | After TTL |
|---|---|---|---|
| Change doc (`chg`) | 8 versions | no (its ADR, if any, is) | Summary line, later folded into a rollup |
| Request (`rq`) | 8 versions | no; `rejected` yes | Summary line (`rejected`: kept) |
| Brainstorm, handoff | 3 versions | no | Summary line, then rollup |
| ADR (`adr`), adopted | never | yes | Body may fold into summary; line kept forever |
| Graveyard (`grv`) | never | yes | Body folds to summary; `match` terms and fingerprint kept forever |
| Exception / waiver history | never | yes | One line kept; active waivers stay live state |
| Sealed changelog (`versions/<version>.md`) | never deleted | no | Older bodies MAY reduce to header plus summary; the file stays |
| Check snapshot | 3 versions | no | Deleted; counts survive in the rollup |

### TTL forms and policy
- **R-RET-010** [0.3] TTL MUST default to version distance (`ttl = "8v"`). A version TTL applies only once the record is also at least `min-age` old, by the recorded seal timestamps (default `"90d"`, D-39). `ttl = "life"` with `bind = <ID>` makes the record eligible one grace version after the bound item reaches `removed`; `max` caps it. `ttl = "never"` means only the user removes it.
- **R-RET-011** [0.3] Calendar TTLs (`"2y"`, `"90d"`) MAY be configured. They MUST be evaluated against the seal's recorded timestamp, never the wall clock (R-RET-020).
- **R-RET-012** [0.3] Retention policy MUST live in the control file under `[retain.<kind>]`; the root owns the defaults. Hot and warm windows are `hot` and `ttl`.
- **R-RET-013** [0.3] A child scope MAY override `hot`, `ttl` and `min-age` only to keep records longer (a tightening, D-34); an override that shortens retention is inert and becomes an escalation proposal to the ancestor (D-02). `ttl = "never"` can never be shortened.

```toml
# fairlead/control.toml
[retain]
min-age = "90d"              # calendar floor for every version TTL (D-39)

[retain.chg]
hot = 3
ttl = "8v"

[retain.adr]
ttl = "never"

[retain.rq]
ttl = "8v"
keep-rejected = true

[retain.handoff]
ttl = "3v"
```

### Compaction inside seal
- **R-RET-020** [0.3] Compaction MUST run inside `seal`, never as a separate background or scheduled action (D-26). Eligibility MUST be evaluated against `--as-of`, which defaults to the timestamp recorded in the version file of the seal being written. The wall clock MUST NOT be read for eligibility or for rollup bucketing.
- **R-RET-021** [0.3] Compaction MUST be deterministic and idempotent: bytewise order on (year, ID), LF-only, no locale dependence. Two machines compacting the same tree MUST produce identical bytes.
- **R-RET-022** [0.3] `seal` MUST refuse to close a version that contains a due record with empty `sum` or invalid `out`, listing every offender (`HN-K001`).
- **R-RET-023** [0.3] Compaction output MUST be staged with the rest of the seal write-set and applied before the version file, which stays last and is the commit point (D-14). An interrupted seal leaves nothing half-compacted.
- **R-RET-024** [0.3] `seal --dry-run` MUST print the compaction plan (records to summarise, records to fold, lines to append). `seal --compact-only` MAY re-run compaction without a new version, using the latest seal's timestamp; it MUST be a no-op when nothing is due.
- **R-RET-025** [0.3] Steps for each due record: (1) copy the summary line into the current year's ledger file; (2) delete the record body from the working tree (git keeps it); (3) leave the ID resolvable (R-RET-050).
- **R-RET-026** [0.3] A non-decision-grade summary that has been cold for `roll-after` versions (default 8, under `[retain]`) MUST be folded into a quarterly rollup line carrying the count, the ordinal range, the decision count and a digest of the folded batch. The quarter is that of the closing seal's recorded timestamp (UTC).
- **R-RET-027** [0.3] Ordinal allocation for a scope and kind MUST use `max(live ordinals, rollup ranges) + 1`, so an ordinal is never reused after folding.
- **R-RET-028** [0.3] Abandoned change docs and requests are eligible like any other; `out = "abandoned"` prompts a graveyard candidate in `context`.

### The hash-chained ledger class (D-19)
- **R-RET-030** [0.3] Compacted summaries MUST be stored in the append-only ledger class: `ledger/hist/<year>.hl`, one file per scope and year, sorted, LF-only, sharded so sealed years are immutable.
- **R-RET-031** [0.3] The chain runs through checkpoints (D-39). Seal writes a checkpoint line `# ckpt v=<version> n=<records> d=<digest>` into each year file it touches; `d` covers the per-record digests (the `d=` field of each record line) written since the previous checkpoint and that checkpoint's digest. A finalised year file ends with `# end d=<digest>` (its last checkpoint), and the next year's header MUST carry `prev=<that digest>`. `check` MUST verify the chain and report a break as `HN-G003` (ledger hash chain, [checks](13-checks-and-diagnostics.md)). A year becomes final when a seal first records a timestamp in a later year.
- **R-RET-032** [0.3] `sync` MUST NOT rewrite a finalised year file. It MAY re-sort the open year file (for example after a union merge). A hand edit to a finalised file MUST fail the chain and MUST NOT be auto-fixed.
- **R-RET-033** [0.3] Rewriting a record inside a finalised year file MUST be possible only through the explicit purge path (R-RET-043). Purge MUST NOT re-chain: the record keeps its `d=` digest, checkpoint digests cover record digests, so every later checkpoint and year stays valid (D-39).
- **R-RET-036** [0.2] Checkpoint continuity is checkable from the first version that writes a checkpoint line (0.2 ledgers of [git](23-git-integration.md) R-GIT-015, before compaction ships in 0.3): `check` MUST verify (break reported as `HN-G003`), per year file, that `v=` values ascend, that each `n=` equals the number of record lines since the previous checkpoint, and that each `d=` equals the digest recomputed from those record digests and the previous checkpoint digest. After a `merge=union` merge a break in the open year is repaired by `sync` (R-RET-035); a break in a finalised year is not auto-fixed (R-RET-032) and is repaired by resealing the later seal (R-GIT-031).
- **R-RET-034** [0.3] The ledger is regenerable only from git history. `init` SHOULD offer `merge=union` for `**/ledger/**` in `.gitattributes`; for generated files a conflict is resolved by regeneration (D-14).
- **R-RET-035** [0.3] Checkpoint lines of the open year file are derived data. They sit after the last record line of their `v=` version and are ordered by version. `sync` MUST regenerate them deterministically from the record lines grouped by closing version, so a `merge=union` merge or a re-sort of the open year never leaves a broken chain; only finalised years are immutable (R-RET-032).

### Tombstones and graveyard permanence
- **R-RET-040** [0.2] A tombstone MUST be written when an ID is removed, replaced or deleted by the user. It is a TOML `[[tomb]]` table in `fairlead/tombstones/<year>.toml` (D-16), live state and not only ledger text. Removed IDs are reserved forever; `new` MUST refuse reuse, `check` MUST report a `def` of a removed ID as `HN-D003`, and a reference to it as `HN-R006`.
- **R-RET-041** [0.2] Tombstones MUST NOT expire, MUST NOT be folded into rollups, MUST NOT be grouped by removed container, and MUST be sharded by year to bound merge conflicts. They never appear in default `context`; `new` and `check` read them.
- **R-RET-042** [0.2] A graveyard entry MUST keep its `what`, `why`, `match` terms and fingerprint forever. Only its long body may compact into the summary.
- **R-RET-043** [0.3] User deletion (`git rm` of a record): the next `sync` MUST detect that the ledger or registry knows the ID but the source is gone, and write a `[[tomb]]` table with `deleted-by = "user"` and `at = "<version>"`. `sync --purge <id>` MUST reduce the record to `id + date + digest` for erasure requests; it is the only operation permitted to rewrite finalised ledger years.
- **R-RET-044** [0.3] A draft deleted before it was ever sealed or referenced MUST leave no tombstone.
- **R-RET-045** [0.3] A deleted decision-grade record MUST appear in `check` output as a notice with its digest, because its re-introduction protection weakens. Rejection records rely on D-03 review of the managerial folder, since owner authority is unauthenticated without signed commits (D-16).

### Resolution of compacted IDs
- **R-RET-050** [0.3] A reference to a compacted record (`use d-3f9a8b11`) MUST resolve to its summary line; a reference to a rolled-up record MUST resolve to the rollup line with a note "compacted in 2031Q2". A reference to a removed feature remains drift (`HN-R006`).
- **R-RET-051** [0.3] `fairlead explain <id>` MUST print the summary and the recorded commit hash. If the commit is not found (squashed or rewritten history) it MUST say "full text unavailable (commit not found)" and MUST NOT error.

### Temperature in context
- **R-RET-060** [0.3] Hot records MAY appear in `context`. Warm records MUST appear only on a query hit or in `explain`. Cold records MUST appear only as their summary line, by query. Default context cost MUST NOT depend on history size (bounded by the budget rules in [14-context.md](14-context.md), R-CTX-040).

### Restructure and ledgers
- **R-RET-070** [0.3] When a scope is merged into another or demoted (`fairlead move --merge`, `--demote`, [portability](32-portability-and-restructure.md)), its ledger lines MUST be appended to the receiving scope's open year file with `scope=<origin>` and ordinals prefixed by the origin scope (`web:0007`); finalised year files of the receiving scope are never rewritten. Record digests do not change, so the origin chain stays verifiable from git.
- **R-RET-071** [0.3] A scope that is detached or cut (`move --detach`, `--cut`) takes its `ledger/hist/` files and tombstones with it; the chain continues unchanged in the new location, and a `moved-from` line is logged in both places.

## Layout

```text
fairlead/
  ledger/hist/2030.hl          finalised: header, sorted lines, "# end d=..."
  ledger/hist/2031.hl          open year: appended by seal, re-sortable by sync
  tombstones/2031.toml         live state, never compacted (TOML [[tomb]], D-16)
  docs/graveyard/              entries; long bodies compact, match + fingerprint stay
```

Ledger sample:

```text
# scope=web year=2031 prev=5c0e91d2 edition=1
d-3f9a8b11 kind=adr ord=0007 title="no tailwind" opened=1.2.0 closed=1.2.0 out=adopted sum="CSS vars plus shadcn tokens; tailwind rejected (build cost)" kw=css,tailwind git=ab12cd3 d=3f9a8b11
d-71c2b0ee kind=change ord=0412 title="dark mode via tokens" opened=1.8.0 closed=1.9.0 out=rejected why="token drift across 40 components" alt="per-component toggles" succ=d-5a9e0c3d dnr=1 kw=theme,dark git=9d41e0a d=71c2b0ee
roll.2031q2 kind=change n=214 ord=0201..0414 dec=12 d=a8e1c405
# ckpt v=1.9.0 n=3 d=c17e09a4
# end d=c17e09a4
```

Tombstone sample:

```toml
[[tomb]]
id = "cmp.old-btn"
removed = "1.5.0"
repl = "cmp.button2"
why = "v1 button, replaced (ADR-014)"

[[tomb]]                     # user deletion (R-RET-043)
id = "d-7f3a21c9"
deleted-by = "user"
at = "1.6.0"
d = "7f3a21c9"
```

A record scaffold with the mandatory slots:

```toml
+++
kind = "change"
id = "d-91cc04ab"
scope = "web"
type = "variant"
status = "draft"
created = "1.9.0"
sum = "Add compact variant on the density axis"
out = ""                     # filled when the record closes (D-39)
why = ""
+++
```

## Growth over 100 years

Default context cost is O(depth × K) and independent of history: L0 and L1 have schema-capped sizes and L2 is selected by query under a budget. What does grow, per scope, under stated assumptions:

| Component | Growth | After 100 years (assumption) | In default context? |
|---|---|---|---|
| Live registry | O(live items) | bounded by the codebase | by path |
| Hot plus warm records | constant (about 12 versions of records) | constant | hot only |
| Decision-grade lines | about 20 per year | 2,000 lines: about 40k tokens at 20 tokens per terse line, up to 120k to 160k with full 240-character summaries (60 to 80 tokens) | never; query only |
| Quarterly rollups | 4 per year | 400 lines, about 10k tokens | never |
| Tombstones | one per removed ID | 50 removals a year gives 5,000 lines, about 150k tokens | never; checked by `new`/`check` |
| Ledger files | one per year | 100 files per scope | no |

Assumptions: 4 characters per token, 20 decisions and 50 removals per year per scope. The 40k figure uses the short line; the 120k to 160k figure uses the maximum summary. Neither is ever loaded whole: L3 is sliced by query ([14-context.md](14-context.md)). Raw text stays in git; the tree holds summaries and pointers. Sealed years are immutable, so merges are trivial.

## Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Summary missing when a record falls due (8 versions after authoring, by someone gone) | `seal` refuses and lists offenders; `check` warns earlier so it is not the first signal | R-RET-022, R-RET-004 |
| 2 | Compaction on two machines | Byte-identical: seal timestamp, bytewise order, LF | R-RET-020, R-RET-021 |
| 3 | Two branches append to the same year file | Sorted append-only; union merge; `sync` re-sorts the open year | R-RET-032, R-RET-034 |
| 4 | Squashed or rewritten git history | Summary is self-sufficient; `explain` says full text unavailable | R-RET-051 |
| 5 | Compacted record still referenced (`use adr.0007`) | Resolves to the summary; removed feature remains drift | R-RET-050 |
| 6 | Hand-edited finalised ledger | `HN-G003` chain break, never auto-fixed | R-RET-031, R-RET-032 |
| 7 | `ttl = "life"` whose feature is never removed | Never expires; `max` caps it | R-RET-010 |
| 8 | User `git rm`s a warm record | Tombstone `deleted-by=user`; ID stays reserved | R-RET-043 |
| 9 | User deletes a graveyard entry or ADR | Tombstone plus check notice with digest; summary line remains | R-RET-045 |
| 10 | Erasure request (`sync --purge`) | Only `id + date + digest` remain; later years not re-chained (checkpoints cover record digests); git history untouched | R-RET-043 |
| 11 | Draft deleted before first seal | No tombstone | R-RET-044 |
| 12 | Ordinal after folding into a rollup | Allocation uses `max(live, rollup range) + 1`; no reuse | R-RET-027 |
| 13 | Seal interrupted during compaction | Version file absent; rerun is idempotent | R-RET-023 |
| 14 | First seal of a new year | Previous year finalised and chained | R-RET-031 |
| 15 | No git or shallow clone | Commit hash recorded as `unknown`; digest still recorded; never an error | R-RET-051 |
| 16 | Re-introduction of a removed ID | Refused by `new`; `check` reports `HN-D003` on a `def` | R-RET-040 |
| 17 | `seal --compact-only` with nothing due | No-op, success ([CLI R-CLI-003](15-cli.md)) | R-RET-024 |
| 18 | `sum` contains a pasted API key | `HN-F001`, floor; `seal` refuses until the value is replaced | R-RET-006 |
| 19 | Child scope sets `ttl = "4v"` where the root says `"8v"` | Inert; escalation proposal to the ancestor | R-RET-013 |
| 20 | Child scope sets `ttl = "16v"` | Takes effect (keeps longer) | R-RET-013 |
| 21 | Union merge reorders checkpoint lines in the open year | `sync` regenerates the open-year checkpoints; chain valid | R-RET-035 |
| 22 | Scope merged into its parent | Ledger lines appended to the parent's open year with `scope=` and prefixed ordinals | R-RET-070 |
| 23 | Scope cut to another repository | Ledger files and tombstones travel with it | R-RET-071 |

## Resolved questions

All chapter-local and register questions were accepted by their stated leaning on 2026-10-09 (D-46).

- Q33-1 (resolved 2026-10-09): version TTL with a calendar `min-age` floor (default 90 days) on recorded seal timestamps (D-39); see R-RET-010.
- Q33-2 (resolved 2026-10-09): the chain runs over checkpoints covering record digests; purge keeps digests and never re-chains (D-39); see R-RET-031, R-RET-033.
- Q33-3 (resolved 2026-10-09): one checkpoint per seal that touches a year file; open-year checkpoints are derived and regenerated by `sync`, so union merges and re-sorts cannot break the chain; see R-RET-031, R-RET-035.
- Q33-4 (resolved 2026-10-09): `sum` is mandatory at creation, `out` is filled at close (D-39); see R-RET-002, R-RET-004.
- Q33-5 (resolved 2026-10-09): tombstones are kept, sharded by year, never grouped and never in default context; see R-RET-041.
- Q33-6 (resolved 2026-10-09): a child scope may lengthen a TTL and may not shorten it; see R-RET-013.
- Q33-7 (resolved 2026-10-09): merged or demoted ledgers join the receiving scope's open year with prefixed ordinals, and a detached scope takes its chain with it; see R-RET-070, R-RET-071.
- Q-CTX-8 (resolved 2026-10-09): `sync --purge` keeps only id, date and digest; see R-RET-043.
- Q-CTX-9 (resolved 2026-10-09): summary fields are capped at 240 characters and refused when they hold secret patterns (`HN-F001`); see R-RET-006, R-RET-002.
