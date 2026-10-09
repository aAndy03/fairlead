# Versions and Seal
> Status: draft · Milestone: 0.2 · Decisions: D-02, D-10, D-13, D-14, D-15, D-19, D-24, D-34, D-22, D-23, D-37, D-39, D-41, D-42, D-46 · Related: 10-scopes-and-inheritance.md, 11-files-and-classes.md, 13-checks-and-diagnostics.md, 15-cli.md, 21-environments-maturity-lifecycle.md, 22-editions-and-upgrade.md, 23-git-integration.md, 33-retention-and-compaction.md

## Purpose
A codebase under fairlead needs a version line for its declared intent (rules, contracts, docs, IDs) so that deprecations, waivers and plans can be expressed as "by version X.Y.Z" and so that agents can ask "what changed since version X.Y.Z". This chapter defines that version model, what is and is not stamped into files, the `fairlead seal` flow that closes a version, the docs-first change-doc lifecycle with its coverage check, and version-based exception expiry. Git interaction (collisions, rebases, shallow clones) is in [git integration](23-git-integration.md).

## Concepts

| Term | Definition |
|---|---|
| **Global version** | Version of the codebase's declared intent. Independent of Cargo/npm release numbers. Format `MAJOR.MINOR.PATCH`. |
| **Sealed version `S`** | The highest `<version>` (`MAJOR.MINOR.PATCH`, D-42) for which `fairlead/versions/<version>.md` exists under the root managerial folder. Derived, never stored in a pointer file. |
| **Open version `O`** | `S` bumped by the largest `impact` among pending change docs, per the table below (D-42). Derived, never stored. Until seal, docs refer to it only symbolically (`@open`, `next-major`). |
| **Scope version** | The global version of the last seal that touched the scope. Derived from the scope's ledger shards; not an independent counter. |
| **File version** | Virtual. `fairlead explain <file>` prints it, computed from the ledger plus git history (D-13). Never written into the file. |
| **Ledger shard** | The seal-written facts of one scope for one version: terse `ID k=v` lines (D-32) appended to `<scope>/fairlead/ledger/hist/<year>.hl` (the append-only ledger class of [files](11-files-and-classes.md), hash-chained per [retention](33-retention-and-compaction.md)), with a checkpoint line per touched version. Written only for scopes touched by that seal. |
| **Version file** | `fairlead/versions/<version>.md` at the root: TOML frontmatter plus a generated body (summary, doc list, exception delta, due-soon list, graveyard additions). Written last by seal; it is the commit point. |
| **Change doc** | Authored doc with `kind = "change"` that declares and covers a set of changes. Status `draft -> ready -> sealed` or `abandoned`. |
| **Draft ID** | `d-` plus 8 random base32 characters, drawn at `fairlead new`. Never derived from content; survives edits. Namespaced by scope as `<scope>:d-<draft>` (D-22); the `d-` form is not a `kind.name` ID (R-VER-049). |
| **Ordinal** | Per-scope, per-kind display number assigned at seal (`0042`). Display only; references always use IDs. |
| **Impact** | Change-doc field `break | add | fix`; maps to major, minor, patch. Largest wins. |

Version bump from impact:

| Max impact in the seal | Bump |
|---|---|
| `break` | major |
| `add` | minor |
| `fix` | patch |

While `MAJOR` is `0`, each bump shifts one level down: `break` bumps minor, `add` and `fix` bump patch (D-42, R-VER-006). D-42 names the same three levels breaking, feature and fix/chore.

## Rules

### Version model
- **R-VER-001** [0.2] `S` MUST be computed as the maximum of the version files present; a mutable pointer file for the current version MUST NOT exist.
- **R-VER-002** [0.2] `O` MUST be computed, never stored. All branches observe the same `O` until one of them seals.
- **R-VER-003** [0.2] A fresh codebase MUST start with `S = none` and `O = <version.start>` (default `0.1.0`); `check` MUST treat an absent `S` as `none`.
- **R-VER-004** [0.2] The global version MUST be independent of package release versions. If `version.mirror` is set (for example `cargo:workspace.package.version`), `check` SHOULD warn on divergence and MUST NOT fail on it.
- **R-VER-005** [0.2] A scope version MUST be derived from ledger shards. Publishable packages keep their own semver; fairlead MAY bind it read-only (`binds`), never write it.
- **R-VER-006** [0.2] While `MAJOR` is `0`, the mapping shifts one level down: `break` bumps minor; `add` and `fix` bump patch (D-42). No control-file key selects another mapping.

### What gets stamped
- **R-VER-010** [0.2] Fairlead MUST NOT write a version counter, hash or date header into authored source files, generated files other than the ledger and lock, JSON, lockfiles or vendored files (D-13).
- **R-VER-011** [0.2] Tracked per scope: a ledger shard per touched version (Concepts) and `gen/scope.lock` (R-VER-015). Tracked per ID'd contract item: `since` (recorded by seal, [R-LC-002](21-environments-maturity-lifecycle.md)), `until` and `deprecated` in its declaration and the ledger (see [lifecycle](21-environments-maturity-lifecycle.md)).
- **R-VER-012** [0.2] A new doc (change doc, ADR, request, upgrade doc) MUST carry an immutable `created` frontmatter key, written once by `fairlead new`: in 0.1 the as-of date (`YYYY-MM-DD`, D-57), from 0.2 the open version (`created = "<version>"`) at creation. It is an identity fact (D-13), never incremented and never read as a per-file version counter. Sealing a change doc or ADR writes `sealed = "<version>"` and `body-hash` once, at the seal that closes it, and they are never rewritten afterwards. `created` is the 0.1 stamp, with the date form above ([files](11-files-and-classes.md) R-FILE-010); `sealed` and `body-hash` are the only seal-time stamps and arrive in 0.2. No other stamp exists and no version counter in any form is allowed.
- **R-VER-013** [0.2] Fairlead MUST NOT offer per-file version headers in any file class (D-13). The only stamp is the `created` key of new docs (R-VER-012). Mutable per-file counters MUST NOT be offered.
- **R-VER-014** [0.2] `fairlead explain <file>` MUST report the virtual file version (`last changed @1.0.2, change doc d-7k3m9qxa`) from the ledger plus `git log -- path`; without git history it MUST say "unavailable".
- **R-VER-015** [0.2] `<scope>/fairlead/gen/scope.lock` (generated, terse `k=v`, D-32) MUST hold `reconciled=<global version>` and `ancestor-digest=<hash>`. The digest covers only the effective inherited view (rules, tokens, policies the scope actually resolves from ancestors). The 0.3 requires-digests of [portability](32-portability-and-restructure.md) are written to the same file.
- **R-VER-016** [0.2] Digests (ledger shard, scope, ancestor view) MUST be Merkle digests (D-23): a scope digest hashes the sorted entries of its own files (blake3 content hash, line endings normalised per [git](23-git-integration.md) R-GIT-013) and the digests of its child scopes, so a change rehashes only the path from the file to the root. Digests MUST be computed from the content-hash cache and unchanged files MUST NOT be re-read. The digest design MUST be validated on a 100k-file benchmark fixture before 0.2 ships ([roadmap](60-roadmap.md)).

### Scope sync guard
- **R-VER-020** [0.2] `check` in a scope MUST read ancestors read-only. If the ancestor digest differs from `gen/scope.lock`, it MUST report `H-SYNC-ANCESTOR` as info for `version.ancestor-grace` seals (default 3), warn afterwards, and error at `seal`.
- **R-VER-021** [0.2] `sync` in a scope MUST rewrite only that scope's lock and generated files and MUST report which inherited items changed. It MUST NOT touch ancestor files.

### Change docs and coverage (docs-first)
- **R-VER-030** [0.2] A change doc MUST carry `covers`: globs, IDs or scope names, restricted to paths under the doc's home scope. A change spanning scopes MUST be ONE doc homed at the lowest common ancestor; affected child scopes get a generated back-reference in their index. IDs in `covers` are recorded and validated (R-VER-031) but in 0.2 only file globs count toward coverage; ID-level coverage arrives in 0.3 ([checks](13-checks-and-diagnostics.md) R-CHK-045).
- **R-VER-031** [0.2] Doc status `ready` MUST require non-empty `covers`, valid IDs, an `impact`, and affected scopes passing `check`.
- **R-VER-032** [0.2] The coverage check MUST verify path coverage only: every changed, non-exempt file matches the `covers` of at least one non-abandoned doc (draft, ready, or sealed in `O`). It MUST NOT assert temporal order (D-15). The unit of coverage is the file, so one doc covering a shared file is enough. A move-only or rename-only change is covered by the generated restructure doc that `move` writes, whose `covers` lists the moved paths ([checks](13-checks-and-diagnostics.md) R-CHK-044); no exempt glob is needed for it.
- **R-VER-033** [0.2] The changed set is the files differing between the working tree and the base. The default base is the commit that introduced `versions/<S>.md`; `--base <ref>` overrides it. Exempt: generated files, `.fairlead/`, gitignored output, globs in `docs-first.exempt`.
- **R-VER-034** [0.2] Severity of the coverage check MUST follow the ramp `observe -> warn -> enforce`, default `warn`; a scope MAY opt in to `enforce` (error) via `docs-first.level`. A scope MAY raise but a child MUST NOT lower an ancestor's level without an `ovr`.
- **R-VER-035** [0.2] A rolling `chore` doc (`rolling = true`, fixed `covers`) MAY cover recurring low-intent changes (dependency bumps, formatting). At seal it is recorded in the version and a successor draft with identical `covers` is created.
- **R-VER-036** [0.2] A `kind = "mechanical"` doc with `tool=` MUST cover only files whose whitespace- and comment-normalised hash is unchanged; other files need a real doc.
- **R-VER-037** [0.2] `covers` broader than the doc's home subtree MUST yield `H-DOC-BROAD` (warn). `**` at the root requires `kind = "bulk"` with a reason; bulk docs are counted per version in the drift report.
- **R-VER-038** [0.2] A revert of a sealed change MUST be covered by a new doc `kind = "revert"` with `of = "<id>"`; the sealed doc is not edited. Reverts of sealed docs SHOULD be auto-accepted by coverage.
- **R-VER-039** [0.2] The tool MUST state in its shipped docs that coverage proves docs-present, not docs-first. Order is enforced only by optional hooks (pre-commit/agent hooks) that reject code-only commits.

### IDs and ordinals
- **R-VER-040** [0.2] A draft ID MUST be random (40 bits), rejected at creation if present in the tree. The filename `d-<id>-<slug>.md` is fixed at creation; the frontmatter ID is authoritative.
- **R-VER-041** [0.2] At seal, ordinals MUST be assigned per scope and per doc kind as `max(existing in scope and kind) + 1`, ordered by (`created`, draft ID). Seal renames the file to `<ordinal>-<slug>.md` (a pure git rename); the draft ID stays inside as the alias.
- **R-VER-042** [0.2] Same draft ID with a different body on merge is `H-ID-COLLISION`; `fairlead new --reid` or `seal` renames the later doc and rewrites references inside docs. Duplicate ordinals in one scope are `H-ORD-DUP`.
- **R-VER-043** [0.2] Draft docs not `ready` at seal MUST be carried to the next open version and listed in the version file as carried.
- **R-VER-044** [0.2] There MUST be no ID claim, lease or reservation file for declarations on parallel branches. Two branches declaring the same ID are found by `check` on the merged tree (`HN-D001`, [checks](13-checks-and-diagnostics.md)); `check --ci` on the integration line is the gate. Draft doc IDs are random (R-VER-040), so docs never need claims.
- **R-VER-045** [0.2] `seal` MUST print a sweep report of drafts that are orphaned: a draft whose `covers` match no existing path or ID, or that has been carried for more than `version.sweep-after` seals (default 3). The report is advisory; fairlead MUST NOT abandon a draft automatically. A human sets `status = "abandoned"` (R-LC-037 then applies).
- **R-VER-046** [0.2] Ordinal renaming at seal MUST keep the draft ID as the alias: the draft ID stays in the doc frontmatter `id`, and every reference to `d-<id>` MUST keep resolving. The alias map (`d-<id>` to ordinal) is a generated, derived file in the scope's `gen/` (terse `k=v`, D-32), a pure function of the docs; it holds no counter.
- **R-VER-047** [0.2] A sanctioned hotfix path MUST exist as an opt-in: with `docs-first.hotfix = true` at the root, `fairlead new change --hotfix` opens a backfill debt, a draft change doc with `hotfix = true` that must be `ready` within `docs-first.hotfix-window` days (default 3, which is the 72 hours of [trust](40-agent-integration-and-trust.md)). The window is whole days in UTC, evaluated against `--as-of` (D-37). `context` MUST list open debts with their deadline; `check` MUST warn while open and fail (error) once the deadline passes. There is no bypass variable (R-GIT-054).
- **R-VER-048** [0.2] Commit order is never part of the coverage check (R-VER-032). An optional `docs-first.strict-order = true` MAY be offered by hooks only (pre-commit and agent hooks reject a code-only commit); `check` and CI MUST NOT enforce it, because squash-merge erases order.
- **R-VER-049** [0.2] The draft ID form `d-<8 base32>` is a doc word admitted by the ID grammar of [syntax](12-directive-syntax.md) in the doc-reference keys (`to=`, `answers=`, `revisits=`, `covers`, `supersedes`, `of`); the scope-qualified form is `<scope>:d-<draft>` (D-22). It is not a `kind.name` ID and never appears as a `def` ID. Per-kind aliases such as `chg-` or `rq-` MUST NOT exist.
- **R-VER-060** [0.2] A `def` that exists only in a change doc whose status is `draft`, `ready` or `abandoned` (the status ladder `draft -> ready -> sealed`, or `abandoned`) MUST NOT satisfy a `use`, `ovr` or `flag` reference to that ID: the reference is dangling (`HN-R001`, [checks](13-checks-and-diagnostics.md)) with a note naming the draft doc. The ID counts as declared only once the doc is `sealed`, or once a code or registry `def` exists.

### Seal flow
- **R-VER-050** [0.2] `fairlead seal` MUST run these steps in order; any failure before step 6 leaves the tree unchanged.
  1. **Preflight**: HEAD not detached; no unmerged paths; process lock held; authored files clean against HEAD except the seal write-set (`--allow-dirty` overrides and records `dirty = true` in the version file); `check` has no errors at or above the gate severity (a secret-like value in a record field `sum`, `out`, `why` or `alt`, `HN-F001`, always refuses the seal, [retention](33-retention-and-compaction.md) R-RET-006); no `ready` doc has uncovered claims; coverage passes; `versions/<O>.md` does not yet exist.
  2. **Compute**: bump from impacts; assign ordinals; resolve symbolic targets (`next-major`, `@open`) to concrete versions recorded in the ledger.
  3. **Expiry and due gate**: items due at `O` (see [lifecycle](21-environments-maturity-lifecycle.md)) and exceptions expiring at `O` block the seal unless satisfied (R-VER-070).
  4. **Stage**: write every output of the write-set (R-VER-059) into `.fairlead/tx/<id>/`: ledger shards, regenerated generated files, rollups, tombstones, graveyard additions, `versions/<O>.md`, optional changelog.
  5. **Apply**: atomic rename per file in a fixed order; compaction ([retention](33-retention-and-compaction.md), 0.3; none in 0.2) runs here using the recorded seal timestamp, never the wall clock.
  6. **Commit point**: rename `versions/<O>.md` LAST.
  7. **Optional**: `--commit` makes one git commit with trailer `Fairlead-Version: <O>`; `--tag` adds tag `fairlead/<O>`. Without `--commit` fairlead MUST NOT run any git write.
- **R-VER-051** [0.2] Seal MUST be all-or-nothing across every scope in the seal. A crash before the commit point MUST leave no sealed version; stray shards MUST be reported as `H-TX-INCOMPLETE`; `seal --resume` or `seal --abort` MUST finish or delete them. Re-running `seal` is idempotent because output is deterministic.
- **R-VER-052** [0.2] Rollups MUST be computed only at seal (D-10): each scope's shard carries counts (waivers, deprecated-reference burn-down, exceptions added/expired) and a content digest; each ancestor's shard carries the subtree sums and child digests so a reader of the root does not descend.
- **R-VER-053** [0.2] The version file body is the changelog source: one line per sealed doc (`impact`, title, home scope, ID), the exception delta, items due in the next seal, graveyard additions. If `changelog.file` is set, a generated aggregate (for example `CHANGELOG.md`) MAY be written from the version files; it is a generated file.
- **R-VER-054** [0.2] Sealed change docs and version files are immutable. A body-hash mismatch MUST be `H-SEAL-TAMPER`; the fix is a superseding doc (`supersedes = "<id>"`). A doc class with `mutable = true` (for example ADRs) MAY allow typo fixes, each recorded as a changelog line.
- **R-VER-055** [0.2] Seal SHOULD run on the integration branch or CI by default (`version.seal-where = "integration"`). `any` is allowed for solo work and relies on collision repair (see [git integration](23-git-integration.md)). The integration line is the only line that owns the version sequence; a maintenance or hotfix line does not keep its own counter and reseals on merge-back (R-GIT-035).
- **R-VER-056** (withdrawn, D-14, Q-VER-3) Partial seal. There is no partial seal in 0.2: seal is all-or-nothing across every scope (R-VER-051), and `seal` accepts no `--scope` flag.
- **R-VER-057** [0.2] `check --ci` on the integration branch MUST fail if a version file exists whose scopes have inconsistent digests or that lists unready docs.
- **R-VER-058** [0.2] Unresolved escalations (a child's `[[req]]` for a vocabulary change, D-02) MUST NOT block seal by default. `seal` MUST list them and `check` MUST warn at seal. An escalation older than `version.escalation-grace` seals (default 3) MUST be an error at seal; one flagged `blocking = true` by its author or the ancestor owner MUST block the very next seal.
- **R-VER-059** [0.2] The seal write-set is exactly: ledger shards and their checkpoint line (the per-scope snapshot of counts and digest, R-VER-052) for touched scopes; each touched scope's `gen/scope.lock` and other regenerated generated files including the alias map (R-VER-046); ordinal renames of sealed docs (R-VER-041); the tombstones and graveyard entries written by lifecycle ([lifecycle](21-environments-maturity-lifecycle.md)); `versions/<O>.md`; and the optional changelog aggregate. Seal MUST NOT write any other file. Checkpoint lines of the open-year ledger are derived data: from 0.3 `sync` regenerates them ([retention](33-retention-and-compaction.md) R-RET-035), so a union merge cannot leave a broken chain.

### Exception expiry
- **R-VER-070** [0.2] An exception (`ovr`) MUST carry `until=<version>`. It is effective while `O < until` and expired when `O >= until`. `until` takes the full `MAJOR.MINOR.PATCH` form (D-42); a two-part value is not a valid expiry. A seal that would produce an `O >= until` MUST fail while the exception is still referenced.
- **R-VER-071** [0.2] Expiry options at seal: remove the exception, renew it by a change doc that sets a new `until`, or promote it by an ancestor `[[allow]]` table in its control file (D-34). A second consecutive renewal SHOULD require an approval, recorded as an `[[approval]]` table in `fairlead/approvals.toml` by `fairlead approve` (D-03).
- **R-VER-072** [0.2] Date-based expiry (`exp=YYYY-MM-DD`, the date key of [syntax](12-directive-syntax.md); `until=` is never a date) is opt-in. It MUST be evaluated against `--as-of` (default: system UTC date), at day granularity; check output and cache keys MUST include the `--as-of` value when any date-based item exists.
- **R-VER-073** [0.2] Expired waivers MUST fail `check` (not warn). The count of waivers and renewals per scope is reported in `context` and rolled up at seal (D-28).
- **R-VER-074** [0.2] `never`, `none`, an empty value or any non-version string MUST NOT be a valid expiry. A rule or edition change that newly requires an expiry (for example [upgrade](22-editions-and-upgrade.md) R-EDN-034) MUST NOT fill one in automatically; a human sets it.
- **R-VER-075** [0.2] Exceptions MUST have a bounded lifetime. From 0.2 the cap is version-based: `until` MUST NOT exceed the cap version, which is the open version `O` with its minor raised by `exceptions.max-minors` (default 3) and patch `0`; a larger `until` is an error at declaration. [0.1] Before versions exist the cap is date-based (`exceptions.max-days`, default 90, relative to `--as-of`, D-37). A cap MAY be lowered by a child scope, never raised.

## Examples

Control file excerpt (root):
```toml
[version]
start = "0.1.0"
mirror = "cargo:workspace.package.version"   # optional; warn-only
seal-where = "integration"                    # integration | any
integration-branch = "main"
ancestor-grace = 3
escalation-grace = 3                          # seals before an open escalation errors
sweep-after = 3                               # seals before a carried draft is listed

[docs-first]
level = "warn"                                # observe | warn | enforce
exempt = ["**/*.lock", "dist/**"]
hotfix = true                                 # opt-in: new change --hotfix
hotfix-window = 3                             # days to backfill
strict-order = false                          # hook-only

[exceptions]
max-minors = 3                                # version cap from 0.2
max-days = 90                                 # date cap in 0.1

[changelog]
file = "CHANGELOG.md"                         # optional generated aggregate
```

Change doc before and after seal:
```toml
+++
id = "d-7k3m9qxa"
kind = "change"
status = "ready"
impact = "add"
scope = "engines"
covers = ["src/diff/**", "engine.diff-v2"]
created = "1.0.2"
+++
Adds diff v2; deprecates engine.legacy-diff.
```
```toml
+++
id = "d-7k3m9qxa"
kind = "change"
status = "sealed"
sealed = "1.1.0"
body-hash = "b3:5f2e..."
ordinal = 42
impact = "add"
scope = "engines"
covers = ["src/diff/**", "engine.diff-v2"]
created = "1.0.2"
+++
Adds diff v2; deprecates engine.legacy-diff.
```

Version file (frontmatter):
```toml
+++
version = "1.1.0"
prev = "1.0.2"
bump = "minor"
sealed-at = "2026-10-08T09:14:00Z"
scopes = ["root", "engines"]
docs = ["d-7k3m9qxa", "d-2n8c4wre"]
carried = ["d-9p1x6tbh"]
dirty = false
+++
```

Exception with version expiry (see [syntax](12-directive-syntax.md)):
```fairlead
// fairlead: ovr rule.layer-down why="spike for streaming" owner=@engines until=1.4.0
```

Seal output (`text`):
```text
fairlead seal --yes     (without --yes: plan printed, not applied; [CLI R-CLI-002](15-cli.md))
  preflight ok (HEAD main, clean, check: 0 errors, coverage 41/41)
  bump: add -> 1.0.2 => 1.1.0 (2 docs, 1 carried)
  stage .fairlead/tx/q8f2 (7 files)
  apply 6/7 ... versions/1.1.0.md (commit point)
sealed 1.1.0; run: git add -A && git commit   (or re-run with --commit)
```

## Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Crash mid-seal | No version file, so not sealed; stray shards give `H-TX-INCOMPLETE`; `seal --resume` or `--abort` | R-VER-051 |
| 2 | Seal on a dirty tree | Refused unless the only diff is the seal write-set; `--allow-dirty` records `dirty = true` and `check` warns | R-VER-050 |
| 3 | Seal on a feature branch, branch abandoned | Nothing leaks; `S` derives from the merged integration line | R-VER-001 |
| 4 | Two branches seal the same number | `H-VER-COLLISION` at merge/CI; repair in [git integration](23-git-integration.md) | R-VER-055 |
| 5 | Edit to a sealed doc | `H-SEAL-TAMPER`; supersede instead | R-VER-054 |
| 6 | Version gap (1.0.2 to 1.0.4) | `H-VER-GAP` warn via `prev` continuity; a waiver note is accepted (revert and renumber is legitimate) | R-VER-001 |
| 7 | Revert of the seal commit | Version file vanishes, `O` returns to that number; sealed docs return to `ready` | R-VER-001 |
| 8 | Revert of a sealed change | New `kind = "revert"` doc in the open version; sealed doc stays | R-VER-038 |
| 9 | Agent commits code with no doc | Allowed; coverage fails at seal/CI per ramp level; `context` lists uncovered files | R-VER-032, R-VER-034 |
| 10 | Formatter rewrites 400 files | `kind = "mechanical"` doc; normalised hash makes whitespace-only diffs exempt | R-VER-036 |
| 11 | Rename plus content edit in one commit | Both paths must be covered; `fairlead move` emits the restructure doc | R-VER-032 |
| 12 | Doc for work that never lands | `H-DOC-EMPTY` info; `abandoned` prompts a graveyard candidate | R-VER-031 |
| 13 | Draft not ready at seal | Carried to the next version; listed in the version file | R-VER-043 |
| 14 | Draft ID collision across branches | `H-ID-COLLISION`; `--reid` on the later doc; tool rewrites inner references | R-VER-042 |
| 15 | Child scope stale vs ancestor | `H-SYNC-ANCESTOR` info, then warn, then error at seal; fixed by `sync` in the child | R-VER-020 |
| 16 | Hotfix commit with no doc, hook bypassed | CI repeats coverage; severity per ramp; backfill with a chore or change doc | R-VER-034 |
| 17 | Hand-edited generated file | `H-GEN-STALE`; `sync` quarantines the edit and refuses without `--force`; intent belongs in an authored input | [CLI R-CLI-026](15-cli.md) |
| 18 | Clock skew on date expiry | Day granularity, UTC, `--as-of`; version targets preferred | R-VER-072 |
| 19 | Exception renewed repeatedly by an agent | Second consecutive renewal needs an approval | R-VER-071 |
| 20 | Two teams, independent cadence in one monorepo | One version line sealed all-or-nothing; if cadences truly diverge, split into sealed sub-roots with their own lines | R-VER-051 |
| 21 | Many PRs, one open version | Docs reference draft IDs, so PRs never conflict on a number; seal frequency is policy (daily/weekly in CI) | R-VER-002 |
| 22 | Brand-new repo | `S = none`, `O = start` | R-VER-003 |
| 23 | `seal --scope A` requested | Not available; seal covers every scope | R-VER-056 (withdrawn) |
| 24 | Urgent fix with no time for a doc | `new change --hotfix` opens a 3-day backfill debt; error after the deadline | R-VER-047 |
| 25 | Same ID declared on two branches | Found on the merged tree by `check --ci`; no claim files | R-VER-044 |
| 26 | Draft left behind after a squash | Listed in the seal sweep report; never auto-abandoned | R-VER-045 |
| 27 | Open escalation at seal | Warn; error after 3 seals or at once if `blocking` | R-VER-058 |
| 28 | Exception with `until` far in the future | Error above the cap (`exceptions.max-minors`) | R-VER-075 |
| 29 | Edition change needs an expiry nobody set | Plan item `needs-human`; `never` is invalid | R-VER-074 |
| 30 | Pasted key in a record `sum` | `HN-F001` (floor); seal refuses until replaced | R-VER-050 |
| 31 | `covers` lists only an ID | Validated and recorded, but covers no file in 0.2 | R-VER-030 |
| 32 | `use engine.diff-v2` while only a draft change doc defines it | Dangling until the doc is sealed or a code `def` exists | R-VER-060 |

## Resolved questions
- Q20-1 (resolved 2026-10-09): hotfix path is an opt-in time-boxed backfill debt (`new change --hotfix`, 3 days); see R-VER-047
- Q20-2 (resolved 2026-10-09): order is never checked by `check`; an optional `strict-order` exists as hook-only; see R-VER-048
- Q20-3 (resolved 2026-10-09): while major is 0, `break` bumps minor and `add`/`fix` bump patch, not configurable (D-42); see R-VER-006
- Q20-4 (resolved 2026-10-09): digests are Merkle over content hashes, computed from the hash cache and validated on a 100k-file fixture before 0.2 ships; see R-VER-016
- Q20-5 (resolved 2026-10-09): unresolved escalations warn at seal, error after K seals, block at once only if flagged `blocking`; see R-VER-058
- Q20-6 (resolved 2026-10-09): `d-<draft>` is a doc word admitted by the ID grammar, `<scope>:d-<draft>` when qualified; no per-kind aliases; see R-VER-049
- Q20-7 (resolved 2026-10-09): the open version is the sealed version bumped by the largest impact, never "sealed + 1" (D-42); see R-VER-002
- Q-VER-1 (resolved 2026-10-09): `created` at creation plus once-only `sealed` and `body-hash` at seal, no version counter; see R-VER-012, R-VER-013
- Q-VER-3 (resolved 2026-10-09): no partial seal in 0.2; see R-VER-056 (withdrawn), R-VER-051
- Q-VER-4 (resolved 2026-10-09): no ID claims or leases; duplicates found by CI on the merged tree; see R-VER-044
- Q-VER-5 (resolved 2026-10-09): an older binary refuses a newer edition or pin; a lowered pin is reported as a relaxation; see R-EDN-006, R-EDN-013
- Q-VER-6 (resolved 2026-10-09): mechanical upgrade plan with git as rollback; no semantic diff in 0.2; see R-EDN-035, R-EDN-038
- Q-VER-7 (resolved 2026-10-09): an expiry is never filled in automatically and `never` is invalid; see R-VER-074, R-EDN-034
- Q-VER-8 (resolved 2026-10-09): escalations warn at seal and error after K seals; see R-VER-058
- Q-VER-9 (resolved 2026-10-09): sweep report at seal, no auto-abandon; see R-VER-045
- Q-VER-10 (resolved 2026-10-09): seal on the integration line only; maintenance lines reseal on merge-back; see R-VER-055, R-GIT-035
- Q-VER-11 (resolved 2026-10-09): version-based lifetime cap from 0.2, date cap in 0.1; see R-VER-075
- Q-VER-12 (resolved 2026-10-09): per-boundary check, a mounted sub-root mismatch is an upgrade diagnostic; see R-EDN-045
- Q-VER-13 (resolved 2026-10-09): alias, snapshot and ordinal write set are defined inside seal; see R-VER-046, R-VER-059
