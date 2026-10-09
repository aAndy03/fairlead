# Portability and Restructure
> Status: draft · Milestone: mixed · Decisions: D-03, D-10, D-11, D-17, D-22, D-25, D-26, D-31, D-33, D-34, D-38, D-40 · Related: 10-scopes-and-inheritance.md, 11-files-and-classes.md, 12-directive-syntax.md, 15-cli.md, 20-versions-and-seal.md, 22-editions-and-upgrade.md, 23-git-integration.md, 30-hierarchy-and-engines.md, 33-retention-and-compaction.md

## Purpose
Codebases are reorganised constantly, often with plain `git mv`, and sometimes a folder travels to another repository. This chapter defines which tool handles each restructure operation (rename, move, group, ungroup, split, merge, delete, reparent), how fairlead detects work done outside its tools, and, from 0.3, how a scope declares what it needs and offers (`requires`, `provides`, the lock), how a root is mounted inside a bigger codebase, how a scope is copied or cut between repos, and how submodules and vendored trees behave.

## Concepts
- **Restructure.** Any change to the scope tree or to path membership. Everything fairlead stores is keyed by scope id and item ID, so most restructures need no edits (D-11).
- **Effective-view diff.** The report of how a restructure changes the resolved view of any path: gained, lost or changed items, grants and policies.
- **`move`.** The one structural verb (D-25; 0.2 per D-38). Its flags are `--merge`, `--detach`, `--demote` and `--delete` (0.2), and `--cut`, `--copy`, `--attach` and `--adopt` (0.3); [CLI](15-cli.md) lists them all as `move` flags, not as new verbs (D-25). `--detach SRC`, `--demote SRC`, `--delete SRC` and `--adopt SCOPE` take no destination ([CLI](15-cli.md) R-CLI-040). Restructure without `move` is `sync` plus `new`.
- **Mounted root.** A root with its own control file (`root = true`) found inside a bigger codebase. It becomes a **sealed sub-root** of the outer root (D-22).
- **`requires` / `provides`.** A scope's declared needs and exports. `requires` is generated from `use` and `extends` plus authored additions; `provides` is authored.
- **Lock.** A generated file recording the digest (and for `portable` scopes the snapshot) of everything a scope requires.
- **Adopt.** The explicit act that opens a sealed sub-root to the outer inheritance chain.

## Rules

### Restructure basics [0.1]
- **R-PORT-001 [0.1]** Fairlead MUST support restructuring done with plain filesystem tools. `sync` reconciles after the fact by scope id, item ID and content hash, and MUST produce an effective-view diff for any scope whose chain or membership changed.
- **R-PORT-002 [0.1]** Scope ids and item IDs MUST be unchanged by a folder rename or move. Only scope-relative globs, native import paths and prose may contain paths, and `sync` MUST report ancestor-owned absolute globs that no longer match as `GLOB-DEAD` (HN-M001) ([checks](13-checks-and-diagnostics.md)).
- **R-PORT-003 [0.1]** `check` MUST fail when a scope's chain changed since the last committed membership shards and the new effective view loosens any rule, policy, severity or required slot for that scope (`SCOPE-REPARENTED`, HN-M002). A reparent that only tightens or leaves the view unchanged MUST warn, not fail.
- **R-PORT-004 [0.2]** `move` MUST refuse a reparent that loosens the effective restrictions of the moved subtree (strict subtree into a lenient parent) unless the user passes an explicit override that `fairlead approve` has recorded (D-03).
- **R-PORT-005 [0.2]** Every `move` operation MUST be transactional: validated first, written per file by temp-write then rename with a journal in `.fairlead/`, and all-or-nothing from the user's view. `check` MUST report half-applied states (mixed membership or stale shards) as `MOVE-INCOMPLETE` (HN-M008).
- **R-PORT-006 [0.2]** After `move` rebinds a subtree, grants (`[[allow]]` tables, D-34) given by the old ancestors MUST no longer apply. Exceptions relying on them become inert and are listed in the report; fairlead MAY offer `req` scaffolds.
- **R-PORT-007 [0.1]** Case-only renames MUST not change identity; membership is path-derived and ids are stored. Scope paths and ids are compared lowercase-normalised (after NFC, R-SYN-040), so a case-only rename is a move of the path with an unchanged chain, reported by `sync` on case-insensitive filesystems (NTFS, APFS) as well. Collisions that differ only by case are errors (R-SCOPE-007, `HN-P001`).
- **R-PORT-008 [0.1]** A hand-edited scope id MUST be detected (`SCOPE-ID-CHANGED` (HN-M003), by comparison with the parent's membership shard). There is no `move --id` in 0.1 to 0.3 (D-50): a deliberate id rename is declared with `replaced-by` and leaves a tombstone alias, stored in the permanent tombstone ([lifecycle](21-environments-maturity-lifecycle.md)); a reference through the alias is reported until it is updated.
- **R-PORT-009 [0.1]** Rust-aware rewrites (`mod`, `use crate::`, `pub use`) and TS import/`@import` rewrites are language-specific. The AST-free core MUST NOT perform them; it MAY run a configured per-language rewrite hook and MUST report the imports it could not verify.

### Operation table
| Operation | Effect | Tool | Milestone |
|---|---|---|---|
| Rename or move a folder containing a scope | Id stable; membership shard updated; chain change gives effective-view diff | `sync` detects, `move` applies globs/imports | 0.1 (detect); 0.2 (apply) |
| Move a non-scope folder | Files may fall under another scope; same diff | `sync` / `move` | 0.1 (detect); 0.2 (move) |
| Group (wrap in new folder G) | No semantic change; ancestor absolute globs rewritten | `move` | 0.2 |
| Ungroup (remove G) | If G is a scope: demote into parent first, `applies` = union of members | `move --demote` then move | 0.2 |
| Promote folder to scope | Lossless (equivalence invariant) | `new scope` (`--hoist`) | 0.1 |
| Demote scope to folder | Defs/ovrs become ancestor entries with `applies`; tombstone + alias | `move --demote` | 0.2 |
| Delete scope | Items tombstoned in the nearest ancestor graveyard; refs dangle until fixed; docs archived with summary | `move --delete` | 0.2 (graveyard entries 0.2) |
| Split A into A-core, A-ui | `new scope` plus `--hoist` by glob; ids unchanged, homes change; ledger entries re-homed | `new` | 0.1 |
| Merge A1 + A2 | ID collisions renamed through tombstone aliases (`replaced-by`); doc ordinals renumbered with alias; ledgers concatenated | `move --merge` | 0.2 |
| Reparent | Lost grants make ovrs inert; unsatisfied `requires` listed | `move` | 0.2 |
| Detach (leave the ancestor chain, stay in place) | Inherited view snapshotted into the lock as a `detached` layer; old grants lapse; own id, grants and items kept | `move --detach SRC` | 0.2 |
- **R-PORT-010 [0.2]** Group and ungroup MUST be no-ops for scope semantics when G is not a scope. Delete MUST require zero live references in the subtree, or list them and refuse unless `--force` is recorded by `approve` (live references to a deleted item are `DANGLING`, HN-M007).
- **R-PORT-011 [0.2]** Merge MUST rename colliding IDs through `replaced-by` plus a tombstone alias (D-50), never silently pick a winner, and MUST preserve reserved (tombstoned) IDs of both sides; a reserved ID from one side colliding with a live ID on the other is an error.
- **R-PORT-012 [0.2]** Tombstones left by demotion or deletion are kept forever (D-16); retention never removes them (see [retention](33-retention-and-compaction.md)).
- **R-PORT-013 [0.2]** `move --detach SRC` (no DST, [CLI](15-cli.md) R-CLI-047) detaches the scope SRC from its ancestor chain without moving or deleting it. It MUST (1) require `portable = true` on SRC, or an empty inherited view, else refuse with exit 4 (15-cli.md R-CLI-003); (2) write the inherited effective view of SRC into `gen/scope.lock` as a synthetic `origin=snapshot` layer flagged `detached` (R-PORT-041); (3) mark SRC `detached` in its parent's membership shard and log `moved-from`; (4) let grants (`[[allow]]` tables, D-34) from the old ancestors lapse (R-PORT-006), listing the exceptions that became inert. SRC keeps its scope id, own grants and own items. A detached scope is reported as `DETACHED` (HN-M011, info) until it is attached (`move --attach`) or adopted. `--cut` is `--detach` plus relocation (R-PORT-040).
- **R-PORT-014 [0.1]** Path handling MUST be tested on Windows from the first commit: paths longer than 260 characters (extended-length form, or a clear error, never a silent skip), network drives, locked files and antivirus latency. Findings from the measurement feed the scan-time budget of [architecture](50-architecture.md).
- **R-PORT-015 [0.1]** Writers take the advisory lock `.fairlead/lock` per worktree, held only while a command runs (D-26, D-52; [CLI](15-cli.md) R-CLI-008 defines the lock and its exit behaviour). A second writer in the same worktree is refused and the holder reported. `move` (0.2) and `seal` additionally take the allocation lock in the git common directory ([git](23-git-integration.md) R-GIT-052, R-GIT-073), so two worktrees cannot `move` or `seal` at the same time; the second one is refused. Collisions across branches are still repaired as in [versions](20-versions-and-seal.md) (D-14).
- **R-PORT-016 [0.1]** A file that is not valid UTF-8 MUST be hashed over its raw bytes, skipped for directive scanning with a notice (`ENCODING-SKIP`, HN-I003), and moved byte-for-byte by `move`.

### Cross-scope contracts: requires, provides, lock [0.3]
- **R-PORT-020 [0.3]** (`requires` and `extends` are separate fields, [scopes](10-scopes-and-inheritance.md) R-SCOPE-066.) A scope MAY declare `provides` (ID'd items other scopes may reference sideways, and the visibility gate for qualified references) and `requires` (IDs, kinds and optional version ranges). `requires` MUST be generated from the scope's `use` and `extends` lines plus authored additions; `check` reports a `use` of a qualified ID missing from `requires` or the target's `provides`.
- **R-PORT-021 [0.3]** `requires` is a DAG. Cycles between `requires` are a dependency warning; `extends` cycles are errors (R-FILE-051). Parent/child structure is not expressible as `requires`.
- **R-PORT-022 [0.3]** The lock (`gen/scope.lock`) MUST be digest-only by default: per required item its resolved-value digest, source scope id, ancestor-chain digest and edition. Only scopes declared `portable = true` additionally store the full text of required items and L0 lines of required docs (D-22).
- **R-PORT-023 [0.3]** Lock entries and everything imported through them are `external` with a pinned hash. An imported `ovr`, `allow` or exception MUST NOT take effect until approved at attach time.
- **R-PORT-024 [0.3]** Doc IDs are namespaced by scope (D-22): a doc is `d-<draft>` (glossary; [versions](20-versions-and-seal.md)), written `<scope>:d-<draft>` outside its own scope and always qualified by `context`. An unqualified `d-<draft>` resolves along the chain (R-SYN-025). Ordinals are display only.
- **R-PORT-025 [0.3]** A doc contradiction is a declared `contradicts` on an ID'd claim (D-22). The same claim ID restated without it is `DUP-CLAIM` (HN-M004). Undeclared prose contradictions are out of scope.

### Mounted roots and adopt [0.3]
- **R-PORT-030 [0.3]** A second `root = true` control file inside a codebase MUST be treated as a mounted root only if the outer root registers it (in its control file, by scope id and path). An unregistered one is `ROOT-DUP` (HN-C004, R-SCOPE-004).
- **R-PORT-031 [0.3]** A registered mounted root is a sealed sub-root: it is resolved and checked under its own edition (D-17) and its own version line; outer inheritance reaches it only through an explicit `inherit` list or satisfied `requires`; the outer root reads only its rollup, whose schema is versioned independently of rule syntax. Exception expiries evaluate in the owning root's version line. Working inside a registered sub-root, the nearest `root = true` control file is the sub-root's (D-31).
- **R-PORT-032 [0.3]** Outer tools MUST NOT write into a sealed sub-root unless invoked inside it.
- **R-PORT-033 [0.3]** `move --adopt` opens the boundary explicitly. It MUST show a diff of the sub-root's defs against the outer effective view; conflicts become `ovr`s or alias renames; adopting MUST require an `approve` record (an `[[approval]]` in `fairlead/approvals.toml`, D-34). Adopting a root also removes its `root = true`. The adopt changes the outer root's control file and the inner root's managerial folder, so each managerial folder MUST be covered by CODEOWNERS (`HN-A002`); the approval is recorded in the outer root.
- **R-PORT-034 [0.3]** When the outer and inner editions differ, each boundary MUST be checked by its own edition. A rollup schema mismatch MUST produce an `upgrade` diagnostic, not a failure.
- **R-PORT-035 [0.3]** Scope ids MUST be unique across the whole tree including mounts. A `[[mount]]` entry MAY carry `as=<prefix>`; the outer root then addresses the inner scope ids as `<prefix>-<id>` and the inner ids stay unchanged inside the sub-root. At attach, an incoming id that collides with a destination id is namespaced the same way (`as=`) with a tombstone alias (`replaced-by`) for the old id; a bare duplicate is `DUP-SCOPE-ID` (HN-C007).
- **R-PORT-036 [0.3]** Generated `:root` projections (CSS custom properties and similar) of a root MUST be written only inside that root's own scope subtree. The outer root MUST NOT declare a projection target inside a sealed sub-root; two roots generating the same path is `PROJECTION-OUTSIDE` (HN-M010).

### Copy, cut and attach [0.3]
- **R-PORT-040 [0.3]** `move --cut` keeps the scope id and logs `moved-from`. `move --copy` MUST mint a new scope id with `derived-from=<old id>`; two scopes with the same id in one tree are `DUP-SCOPE-ID`.
- **R-PORT-041 [0.3]** At the destination, `attach` MUST diff the scope's `requires` against the new ancestors' effective items and classify each as satisfied, differing or missing. Missing items MAY be served from the lock snapshot as a synthetic `origin=snapshot` ancestor layer, flagged `detached`, until adopted or re-pointed; only `portable` scopes carry a snapshot (R-PORT-022). A digest-only scope has no snapshot, so its missing requires are `UNSATISFIED` (HN-M005) and fail `check`; attach never copies everything.
- **R-PORT-042 [0.3]** Overrides that relied on grants from the old ancestors are ungranted at the destination; `attach` offers `req` scaffolds. Provenance (`moved-from`, `derived-from`, source repo and commit) MUST be recorded.
- **R-PORT-043 [0.3]** No transaction spans two repositories and there is no two-phase commit. A cross-repo cut is two steps (export at the source, attach at the destination), each repository seals alone, and the source copy remains until the user deletes it. A signed bundle carrying scope plus lock is not part of 0.3.
- **R-PORT-044 [0.3]** Fairlead MUST NOT auto-resolve imported items: approval at attach is required (D-03).
- **R-PORT-045 [0.3]** Extraction as a new root: `move --cut` (or `--copy`) to a repository with no outer root and `move --attach` there make the scope that repository's root (`[fairlead] root = true`). It keeps its scope id after a cut and mints a new id with `derived-from` after a copy (R-PORT-040). Inherited rules are not copied: they arrive through lock entries that are `external` and pinned (R-PORT-023), as a `detached` snapshot layer for `portable` scopes (R-PORT-041), and the owner adopts each into authored control by `approve`. Sealed change docs keep their `covers` text verbatim as archived history and are not checked against the new tree; unsealed docs MUST be retargeted or `check` reports `DANGLING` (HN-M007).
- **R-PORT-046 [0.3]** Lock staleness is detected by explicit pull only: `sync` in the consuming repository re-resolves the lock from the source recorded at attach (a local path, submodule or git ref at a pinned version), and `check` in CI compares the lock digests with that source when it is reachable, reporting `LOCK-STALE` (HN-M012) on a mismatch and "offline pin" when it is not. Nothing is pushed between repositories (D-26); cross-repo consumers stay invisible to local `affects` ([variation](31-variation-protocol.md), R-VAR-041).
- **R-PORT-047 [0.3]** On export, imports that leave the scope subtree (found by the import evidence pass of [hierarchy](30-hierarchy-and-engines.md) R-HIER-033, e.g. a root-level `shared/` module) MUST be listed as `requires` candidates. An outward import that is neither declared in `requires` nor satisfied at the destination is `UNSATISFIED` (HN-M005) at attach. The list is a lower bound (D-40) and the report says so.

### Submodules and vendored subtrees [0.3]
- **R-PORT-050 [0.3]** A vendored subtree with its own managerial folder is a foreign boundary: excluded from enforcement by default, its `provides` consumable read-only. A subtree without a managerial folder is plain files, excluded by negative `applies` or an `external` declaration. Annotations and scopes inside it are ignored unless declared.
- **R-PORT-051 [0.3]** A git submodule root is a mounted root. The parent lock records the submodule commit and manifest digest as its `pin`. The parent compares pins only and never reaches inside; a bumped pin marks the parent lock stale.
- **R-PORT-052 [0.3]** A submodule that is not checked out MUST be handled in offline mode from the lock snapshot, and `check` MUST say "offline pin". Sparse or partial checkouts that exclude scopes MUST NOT make `check` pass silently: out-of-set scopes are reported as `NOT-CHECKED` (HN-M006).
- **R-PORT-053 [0.3]** Cargo workspace members and npm packages are suggested scope boundaries (`check` hints) and never created automatically; membership is read directly from the Cargo `Cargo.toml` manifests (TOML only; fairlead reads no YAML and does not run `cargo metadata`, D-50), never edited.
- **R-PORT-054 [0.3]** A crate manifest that may be published MUST exclude the managerial folder (`exclude = ["fairlead/"]` in `[package]`, using the configured folder name); `check` hints (`PKG-HINT`, HN-M009, info) when it does not. Dependencies that give both `path` and `version` MUST agree with the member's own version; a disagreement is the same hint. Fairlead reads manifests and never edits them (R-PORT-053).
- **R-PORT-055 [0.3]** Published packages consumed by another repository (crates.io, npm) are covered only by `requires` / `provides` digests; there is no registry integration in 0.3.

## Examples
Manifest of a portable scope, and the generated lock:
```toml
# apps/web/sidebar/fairlead/control.toml
[fairlead]
edition  = 1
scope    = "ui-sidebar"            # the scope id lives only here (D-33)

[scope]                            # portability keys; no id
portable = true
provides = ["cmp.sidebar", "tok.sidebar-width"]
requires = ["rule.no-inline-style@^1", "tok.color-primary", "kind.stylesheet"]
```
```text
# scope ui-sidebar edition=1
req=rule.no-inline-style from=root dg=9fd2 ver=1.4
req=tok.color-primary from=root dg=1ab7
chain dg=77c0
```
Hand `git mv` followed by `sync` (the report is the contract):
```text
$ git mv apps/web/sidebar apps/admin/sidebar
$ fairlead sync
scope ui-sidebar: path apps/web/sidebar -> apps/admin/sidebar ; parent ui-web -> ui-admin
effective-view diff:
  - rule.max-lines limit 300 -> 200 (tighten)
  - exc.x4 (tok.radius-base) now INERT: grant from ui-web lost
check: SCOPE-REPARENTED warn (tightening only)
```
Mounted root registration and adopt:
```toml
# root fairlead/control.toml
[[mount]]
scope   = "billing-core"
path    = "vendor/billing"
as      = "billing"         # R-PORT-035: outer view addresses inner scope ids as billing-<id>
inherit = ["tok.color-*"]   # explicit; default is none
```
```text
$ fairlead move --adopt billing-core        # without --yes: prints the diff only; applying needs an approve record ([CLI R-CLI-002](15-cli.md))
```

## Edge cases
| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Plain `mv` of a scope under another parent | Id stable; `sync` shows view diff; loosening fails `check` | R-PORT-001, 003 |
| 2 | Two sibling scopes swapped through a temp dir in one commit | Ids stored, so identity follows the control file; exceptions stay bound to the right scope | R-PORT-002 |
| 3 | Strict subtree moved under a lenient ancestor to shed restrictions | `move` refuses; plain `mv` fails `check` | R-PORT-003, 004 |
| 4 | Scope folder copied inside the repo (`cp -r`) | `DUP-SCOPE-ID` (HN-C007); `move --copy` mints a new id with lineage | R-PORT-040 |
| 5 | Ungroup a folder that is a scope | Demote into parent first; no lost items; `applies` is the union of member paths | R-PORT-010 |
| 6 | Delete a scope whose `provides` a sibling uses | Tombstone + dangling refs flagged on the sibling; delete refused without `approve` while live refs exist | R-PORT-010 |
| 7 | Merge two scopes with the same bare ID or same reserved ID | Alias rename for live IDs; reserved vs live collision is an error | R-PORT-011 |
| 8 | Reparent drops a grant an exception relied on | Exception inert; `req` scaffolded; reported in the diff | R-PORT-006 |
| 9 | Ancestor-owned absolute glob after a group move | `GLOB-DEAD` (HN-M001); `move` rewrites, plain `mv` leaves the diagnostic | R-PORT-002 |
| 10 | Scope cut to another repo with grant-dependent ovrs | Ungranted at destination; attach scaffolds `req`s; imports need approval | R-PORT-042, 044 |
| 11 | Root X mounted under Y, X keeps its own `root = true` | Sealed sub-root only if registered; else `ROOT-DUP` (HN-C004); adopt opens it with an approved diff | R-PORT-030, 033 |
| 12 | X on edition 1, Y on edition 2 | Each boundary checked by its own edition; rollup mismatch gives an `upgrade` diagnostic | R-PORT-034 |
| 13 | Vendored subtree ships a `fairlead/` and `// fairlead:` comments | Foreign, ignored by default; `provides` read-only | R-PORT-050 |
| 14 | Submodule not checked out, or pin bumped | Offline mode from lock; `check` notes "offline pin"; parent lock stale on bump | R-PORT-051, 052 |
| 15 | Sparse checkout hides sibling scopes | `NOT-CHECKED` (HN-M006) for out-of-set scopes; never a silent pass | R-PORT-052 |
| 16 | Lock from another repo carries `ovr` lines | Imported items are `external`, pinned; nothing effective before approval | R-PORT-023 |
| 17 | Rename that leaves Rust `mod`/`use` or TS imports stale | Core does not rewrite; hook or report of unverified imports | R-PORT-009 |
| 18 | `move` interrupted (crash, locked file on Windows) | Journal replayed or rolled back; `check` reports mixed state (`MOVE-INCOMPLETE`, HN-M008) | R-PORT-005 |
| 19 | `move --detach` on a non-`portable` scope that inherits rules | Refused, exit 4; mark it `portable` first. On success grants from old ancestors lapse and exceptions relying on them are inert and listed | R-PORT-013, R-PORT-006 |
| 20 | Two worktrees run `move` or `seal` at once | The second is refused on the common-directory lock; each worktree keeps its own `.fairlead/lock` for other writers | R-PORT-015 |
| 21 | Legacy-encoded (non-UTF-8) file inside a moved scope | Hashed as bytes, skipped for directives with `ENCODING-SKIP` (HN-I003), moved unchanged | R-PORT-016 |
| 22 | `Docs/` renamed to `docs/` on NTFS | Case-only rename: path changes, chain unchanged, no collision | R-PORT-007 |
| 23 | Scope extracted into a new repository as its root | Id kept after cut; inherited rules arrive as pinned lock entries or a detached snapshot; sealed `covers` kept as history | R-PORT-045 |
| 24 | Consuming repo lock is behind the source repo | Detected only when `sync` or CI `check` can reach the source; else "offline pin" | R-PORT-046 |
| 25 | Root-level `shared/format.ts` imported by the extracted scope | Listed as a `requires` candidate; `UNSATISFIED` (HN-M005) at attach if not satisfied | R-PORT-047 |
| 26 | Two mounted roots both declare id `core` | Outer view uses `as=` prefixes; a bare duplicate is `DUP-SCOPE-ID` | R-PORT-035 |
| 27 | Outer root and a sub-root both generate the same `:root` projection file | `PROJECTION-OUTSIDE` (HN-M010); each root writes only inside its own subtree | R-PORT-036 |
| 28 | Published crate would ship `fairlead/` | `PKG-HINT` (HN-M009) until `exclude` lists it | R-PORT-054 |

## Resolved questions

All chapter-local and register questions were accepted by their stated leaning on 2026-10-09 (D-46).

- Q32-1 (resolved 2026-10-09): cross-repo cut is two steps, export then attach, with provenance; a signed bundle is not in 0.3; see R-PORT-043.
- Q32-2 (resolved 2026-10-09): missing requires are served from the lock snapshot as a synthetic layer for `portable` scopes only; digest-only scopes are `UNSATISFIED` (HN-M005); see R-PORT-041, R-PORT-022.
- Q32-3 (resolved 2026-10-09; no stated leaning, chosen for consistency with D-03): an adopt needs an `approve` record in the outer root and CODEOWNERS coverage of both managerial folders (`HN-A002`); no further mechanism; see R-PORT-033.
- Q32-4 (resolved 2026-10-09; no stated leaning, chosen for consistency with D-26 and D-52): `.fairlead/lock` per worktree, and `move` and `seal` additionally take the lock in the git common directory so two worktrees cannot run them concurrently (Q-GIT-4); see R-PORT-015.
- Q32-5 (resolved 2026-10-09): non-UTF-8 files are hashed as bytes and skipped for directives with a notice; see R-PORT-016.
- Q32-6 (resolved 2026-10-09): the extra flags are `move` flags in [CLI](15-cli.md), not new verbs (D-25); `--detach` is defined here (R-PORT-013) and cited by R-CLI-047; see Concepts.
- Q-PORT-1 (resolved 2026-10-09): an extracted scope becomes a new root whose inherited rules arrive through the lock (pinned, external, not copied); sealed `covers` are kept as history; see R-PORT-045.
- Q-PORT-2 (resolved 2026-10-09): lock staleness is found by explicit pull plus a CI check; no daemon (D-26); see R-PORT-046.
- Q-PORT-3 (resolved 2026-10-09): superseded in part by D-27 and D-40; outward imports found by the regex evidence pass are listed as `requires` candidates and unresolved ones are `UNSATISFIED` (HN-M005) at attach, as a lower bound; see R-PORT-047, R-HIER-033.
- Q-PORT-4 (resolved 2026-10-09): `exposure` is intent only and reported `exposure unverified`; manifests are not parsed in 0.3; see R-HIER-067.
- Q-PORT-5 (resolved 2026-10-09): cross-repo transactions are out of scope; each repository seals alone; see R-PORT-043.
- Q-PORT-6 (resolved 2026-10-09): a detached scope materialises a synthetic `detached`, `origin=snapshot` layer until adopted or re-pointed; see R-PORT-013, R-PORT-041.
- Q-PORT-7 (resolved 2026-10-09): scope paths and ids are compared lowercase-normalised; a case-only rename is a move; see R-PORT-007.
- Q-PORT-8 (resolved 2026-10-09): Windows is tested from the first commit (long paths, network drives, antivirus); see R-PORT-014.
- Q-PORT-9 (resolved 2026-10-09): ids are namespaced on import and mounts take `as=`; bare duplicates are `DUP-SCOPE-ID` (HN-C007); see R-PORT-035, R-PORT-040.
- Q-PORT-10 (resolved 2026-10-09): `:root` projections are written only inside each root's own scope; see R-PORT-036.
- Q-PORT-11 (resolved 2026-10-09): published packages are out of scope in 0.3, covered only by `requires` / `provides` digests; see R-PORT-055.
- Q-TOOL-10 (resolved 2026-10-09): published crates exclude the managerial folder and `path`-plus-`version` edges must agree, as read-only hints; see R-PORT-054.
- Pending-fix record (2026-10-09): the portability example no longer duplicates the scope id in `[scope]`; the id lives in `[fairlead] scope` (D-33); `move --detach` is defined (R-PORT-013).
