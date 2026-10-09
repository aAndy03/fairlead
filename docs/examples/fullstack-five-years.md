# Fullstack five years (notes-app, Tauri v2)

> Status: draft · Milestone: mixed · Decisions: D-02, D-05, D-11, D-12, D-13, D-14, D-15, D-16, D-17, D-18, D-19, D-20, D-22, D-23, D-24, D-25, D-27, D-29, D-31, D-32, D-33, D-37, D-38, D-40, D-42, D-45, D-46, D-49 · Related: 10-scopes-and-inheritance.md, 11-files-and-classes.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 14-context.md, 15-cli.md, 20-versions-and-seal.md, 21-environments-maturity-lifecycle.md, 22-editions-and-upgrade.md, 23-git-integration.md, 30-hierarchy-and-engines.md, 32-portability-and-restructure.md, 33-retention-and-compaction.md

## Purpose

This scenario follows one fullstack codebase (a Tauri v2 notes app: React frontend, Rust backend) through five years of change. Each step is tagged with the milestone it needs, so the reader sees what 0.1 covers alone and where 0.2 and 0.3 are required. Diagnostics are cited by their HN code from [checks](13-checks-and-diagnostics.md); a mnemonic in brackets is the display alias (R-CHK-042).

Milestone tags: **[0.1]** alpha; **[0.2]** needs seal, lifecycle, editions or git integration; **[0.3]** needs ladders, portability, retention or bridges. `[git]` marks a step done with git alone.

## Concepts

- **Root**: the scope whose control file has `[fairlead] root = true` (D-11, D-31, D-33). One per repo.
- **Scope ID**: a stored slug in the scope's control file, never a path (D-11). Here `root`, `web` (`src/`) and `tauri` (`src-tauri/`).
- **ID**: `[scope:]kind.name` (D-18). `brg.note-search` has kind `brg` and name `note-search`.
- **Kind**: a 2 to 8 lowercase letter prefix declared in control ([syntax](12-directive-syntax.md)). Shapes are `shp`, because `shape` is a key of `[kinds.K]`.
- **Version**: `MAJOR.MINOR.PATCH` (D-42). The sealed version is the highest `versions/<version>.md`; the open version is the sealed one bumped by the largest declared impact since that seal (`break` -> major, `add` -> minor, `fix` -> patch). Both are derived, never stored (D-14). This codebase sets `version.start = "1.0.0"` (R-VER-003).
- **Item version**: `ver=N` on a bridge end (R-HIER-060), independent of the global version (D-13).

## Layout (from Year 0)

```text
notes-app/
  .gitignore                    # .fairlead/ line appended only with --yes (R-FILE-026)
  AGENTS.md                     # fairlead marker block only (R-FILE-027)
  package.json index.html vite.config.js src/        # frontend
  src-tauri/ Cargo.toml build.rs tauri.conf.json capabilities/ src/lib.rs src/main.rs
  fairlead/                        # root managerial folder, committed
    control.toml                # [fairlead] root = true, edition = 1; [settings], [kinds.K], [policies] [0.1]
    approvals.toml              # [[approval]] records written by fairlead approve (D-34) [0.1]
    docs/                       # change docs, requests, ADRs; random draft IDs [0.2]
    decl/bridge.md              # bridge declarations as def directives in frontmatter [0.1 syntax; checks 0.3]
    versions/                   # one file per seal [0.2]
    tombstones/ graveyard/      # lifecycle records [0.2]
    gen/                        # generated terse-line shards, committed [0.1]
  src/fairlead/                    # scope "web", created when needed [0.1]
  src-tauri/fairlead/              # scope "tauri", created when needed [0.1]
  .fairlead/                       # gitignored cache, disposable [0.1]
```

A new doc gets a once-only `created` stamp (the as-of date in 0.1, the open version from 0.2; R-VER-012, D-57); no per-file version header exists (D-13).

## Five years

### Year 0: init, first feature, docs-first

**Steps**

1. `fairlead init --dry-run`, then `fairlead init --yes`. Without `--yes` the plan is printed and the verb exits 4 (R-CLI-002). `init` creates `fairlead/control.toml` with a `[fairlead]` header (`root = true`, `edition = 1`, D-33) and a `[settings]` table. [0.1]
2. Child scopes come later; only `fairlead new scope` creates one (R-SCOPE-010). Until then, folders stay implicit. [0.1]
3. `fairlead new change note-search --scope root`. The draft ID is random, eight base32 characters (`d-7f3a91c2`), so it does not change with the title (R-VER-040, D-14). [0.2]
4. Directive lines in code, six verbs only (D-18); a bridge end is a `def` of a `brg` ID declared in a doc (D-40). Rust, then React:

```fairlead
// fairlead: def brg.note-search side=rs ver=1
#[tauri::command]
pub fn note_search(query: String) -> Result<Vec<NoteHit>, CmdError> { /* ... */ }
```

```fairlead
// fairlead: def brg.note-search side=ts ver=1
// fairlead: use brg.note-search pin=1
const hits = await invoke<NoteHit[]>("note_search", { args: { query } });
```

[0.1 for `def` and `use` syntax; `pin=` and the bridge checks 0.3]

5. The bridge is declared as `def` directives in the TOML frontmatter of `fairlead/decl/bridge.md`. Bridges are not `[cmd.*]` tables (R-SYN-050). [0.1 syntax; bridge keys and checks 0.3]

```fairlead
+++
title = "Bridge declarations"
directives = [
  'def brg.note-search proto=tauri-command dir="ts->rs" wire=note_search ver=1 params=args:shp.note-search-args home=root ts="src/api/notes.ts#noteSearch" rs="src-tauri/src/lib.rs#note_search"',
]
+++
```

The struct behind `args` is not declared field by field, because D-29 declares params, not structs. `home=root`, not `home=tauri`: the two ends sit in sibling scopes, so their nearest common ancestor is root (R-SCOPE-017). A home that is not an ancestor-or-self of every end is HOME-MISMATCH (HN-C013).

6. Until the seal, the draft's `covers` list names 3 of the 5 changed files, so `check` reports HN-D006 (`doc.uncovered`) on the other two. [0.2]
7. `check` never writes (R-CLI-024). `fairlead seal` writes `versions/1.0.0.md`. [0.2]

**Drift:** HN-D006 on two files until the seal (D-15 default severity is warn).

### Month 3: new agent session, no memory

1. `fairlead context` prints the session-start brief in the `ID k=v` form ([context](14-context.md) R-CTX-001). It replaces `fairlead status` (D-25). Nothing is pending, so the open version equals the sealed one. The default brief budget is 500 tokens (R-CTX-044). [0.1; the open and sealed fields 0.2]

```text
$ fairlead context
@root d=8f2a91c0 ed=1 open=1.0.0 sealed=1.0.0
rule.no-unwrap sev=error
brg.note-search ver=1 home=root
drift: 0 errors, 2 warnings (HN-D006) | next: fairlead context <path>
```

2. `fairlead context src/api/notes.ts` returns the path query: the IDs used under that path, the contract, and the rules that apply (700-token default, R-CTX-044). [0.1]
3. A human edit to `src/components/SearchBar.tsx` between sessions has no change doc, so `check` reports HN-D006. [0.2]

**Drift inherited:** the uncovered edit appears on the next `check` at warn severity. A free-text query such as `fairlead context "fix search"` exits 2 with `no match`, because task queries need 0.3 (R-CTX-004).

### Year 1: built, removed, tombstoned, re-add attempted

**Steps**

1. Feature `feat.lan-share` with command `brg.share.start` (a bridge), event `evt.peer.found` (a user kind with `proto=tauri-event`, R-SYN-043), two Rust files under `src-tauri/src/p2p/`, and one component. The directive lines are [0.1]; the checks below need [0.2] and [0.3].
2. The owner rejects the feature. The removal doc declares `impact = "break"` and one lifecycle line per removed ID (R-SYN-005). `since` is recorded by seal, not written by the author (R-SYN-048, D-49):

```fairlead
<!-- fairlead: flag remove feat.lan-share target=2.0.0 -- rejected by the owner -->
<!-- fairlead: flag remove brg.share.start target=2.0.0 -- command removed with the feature -->
<!-- fairlead: flag remove evt.peer.found target=2.0.0 -- event removed with the feature -->
```

[0.1 syntax; checks 0.2]

3. `fairlead seal` writes a `[[tomb]]` record for each removed ID, because removed IDs are reserved forever (R-LC-020, D-16). The removal doc's break impact makes this seal write `versions/2.0.0.md` (from 1.0.0). The owner rejected the feature, so it also gets a graveyard entry whose fingerprint is computed once at this seal and stored (R-LC-031). [0.2]
4. The removal's `covers` list misses `src-tauri/src/p2p/discovery.rs`, so that file is uncovered: HN-D006 (warn) [0.2]. It becomes HN-H003 (`hier.orphan-file`, no container) only once containers exist [0.3].

**Re-add attempts**

1. `git revert` of the removal brings the files back with no declaration. A revert of a sealed ID needs a `kind = "revert"` doc with `of` (R-GIT-036). Without one, the restored `def brg.share.start` is HN-D003 (tombstoned-ID reuse, error), the restored files that match the tombstone's `paths` are HN-L002 (H-GRAVE-MATCH, warn), and the uncovered files are HN-D006. This attempt is caught. [0.2]
2. Re-declaring `feat.lan-share` is HN-D003, because IDs are never reused (D-16; R-LC-021). The only path is a `kind = "revive"` change doc with `revives = "feat.lan-share"` and an `[[approval]]` recorded by `fairlead approve`. It creates a new version-suffixed ID, `feat.lan-share-v2`, and the old ID stays reserved (R-LC-024). [0.2]
3. A new ID `feat.device-sync` with new paths and new code has no mechanical match, and no free-text matching exists. This attempt is not caught; it is an accepted residual risk.

### Year 2: restructure (layers renamed, containers merged, folder split)

**Steps**

1. Child scopes `src` and `src-tauri` get managerial folders with stored IDs `web` and `tauri` (D-11). [0.1]
2. Layers renamed: `widget` to `molecule`, `screen` to `page`, `service` to `engine`, `db` to `storage`. Ladders and ranks live in TOML, and unit declarations hold `<ladder>.<layer>` names, not numbers (R-HIER-003). [0.3]
3. Containers `cont.notes.commands` and `cont.notes.handlers` merge into `cont.notes.api`. Each old ID gets one `flag replace` with the same `to` (R-LC-011); on removal, the successor becomes a permanent `replaced-by` alias in the old ID's tombstone (D-50). The lines sit in the registry doc `fairlead/decl/containers.md`:

```fairlead
<!-- fairlead: flag replace cont.notes.commands to=cont.notes.api target=2.1.0 -->
<!-- fairlead: flag replace cont.notes.handlers to=cont.notes.api target=2.1.0 -->
```

[0.1 syntax; checks 0.2 for the flags; the containers 0.3]

4. `src-tauri/src/commands.rs` (900 lines) is split by hand into `commands/mod.rs`, `notes.rs`, `tags.rs` and `sync.rs`. IDs do not depend on paths, and Rust module paths are aliases only (D-27, D-18). [0.1]
5. The wire name `note_search` does not change this year, so the bridge stays at `ver=1` and the TS literal still matches (D-29). [0.1]
6. One change doc, homed at the lowest common ancestor (`root`), covers the restructure. `fairlead move` writes the restructure doc itself, and its `covers` list names the old and the new paths (R-CHK-044, R-GIT-075). [0.2]
7. The split is done by hand. `fairlead move OLD NEW` reconciles membership and rewrites fairlead's own text; it lists `generate_handler!` entries and `mod` paths as code sites and edits no code (R-CLI-044). [0.2]
8. A waiver for the rank violation `storage/index.rs` to `engine.purify`. The owner writes an `ovr` with a reason, an owner and a version expiry (D-02, D-37). [0.1 for `ovr`; `until=` needs seals, 0.2; rank check 0.3]

```fairlead
// fairlead: ovr rule.hier-rank why="migration in progress" owner=@andy until=2.3.0
```

`@andy` must be listed in the root `owners`; a free-text owner is HN-X012 and the `ovr` is ineffective (R-SYN-011, R-AGT-028).

9. `fairlead seal` writes `versions/2.1.0.md` (impact `add`). [0.2]

**Drift:** HN-R006 (`DEPRECATED-REF`) on annotations that still name `cont.notes.handlers`: warn until the due version, error from it (R-LC-008). HN-D006 on `commands/mod.rs` until a doc covers it. HN-H003 on that file only once containers exist [0.3]. HN-G001 on the old shards until `fairlead sync` rewrites them; `check` then exits 3 at warn or enforce (R-CHK-040).

### Year 3: frontend extracted to a second repo

**Steps**

1. `git subtree split --prefix=src` keeps history in the new repo. Git does this step. [git]
2. `fairlead init` runs in the new repo, which is its own root, because a second repo is a separate codebase (D-11, D-17). [0.1 for init; 0.3 for the cross-repo contract]
3. The consumed contract is pinned in a lock. The lock is digest-only unless the scope is declared `portable` (R-PORT-022, D-22). [0.3]
4. The new repo has no `brg.note-search` declaration. It consumes the pinned version with `// fairlead: use brg.note-search pin=3`; the pin is the `pin=` key of the use (R-SYN-049). [0.3]
5. Inherited rules are not copied. The extracted scope becomes the new root, and inherited rules arrive through pinned lock entries that the owner adopts with `approve` (R-PORT-045). [0.3]

**Drift:** with `src/` gone from the old repo's working tree, its `web` scope is absent: `check` reports HN-V013 and exits 5 for verdicts that depend on it, including the bridge's TS end (D-56, R-CHK-057). HN-M007 (DANGLING) marks unsealed docs whose `covers` still name `src/**` (R-PORT-045). The backend bumps `brg.note-search` to `ver=4` in the old repo. The new repo still passes against the lock at version 3 until someone pulls: `fairlead sync` re-resolves the lock, and CI's `fairlead check` reports HN-M012 (LOCK-STALE) when the source is reachable (R-PORT-046).

**Resolved here:** a root-level `shared/format.ts` imported by `src/api/notes.ts` is a `requires` candidate and is HN-M005 (UNSATISFIED) at attach (R-PORT-047). `frontendDist` and `beforeBuildCommand` in `tauri.conf.json` are external inputs declared with `def ext.<name> role=input path=...` (R-HIER-068) [0.3].

### Year 4: edition upgrade, 1 to 2

**Assumed edition-2 changes (illustrative, not decided)**

- A new rule ships at severity `observe` only (D-17, D-28).
- Generated shards are split per scope and kind (D-19).
- Reference docs are regenerated from the new binary.

**Steps**

1. `fairlead upgrade` without `--yes` prints the plan and exits 4 as not applied (R-CLI-002). `fairlead upgrade --yes` writes one draft upgrade doc in the root scope and nothing else: `d-<draft>-upgrade-edition-1-to-2.md` with `kind = "upgrade"`, `impact = "break"` and a plan hash (R-EDN-030). The dry-run counts for the new rules are listed in that doc; the baseline is written only by `fairlead sync --baseline` (D-38). [0.2]
2. After `fairlead approve` records an `[[approval]]` on the ready doc (R-CLI-037), `fairlead upgrade --yes` applies the edits in one pass to `fairlead/control.toml`, `fairlead/ref/**`, `fairlead/gen/**` and any annotation the new grammar requires. It exits 4 ("plan stale" or "approval missing") on a mismatch, and every file in the write-set must equal HEAD (R-EDN-031). [0.2]
3. `fairlead check`, then `fairlead seal` writes `versions/3.0.0.md`. The upgrade doc's break impact makes the bump major, from 2.1.0 (D-42). [0.2]
4. `notes-web` upgrades separately, later. An edition is one per root (D-17), so the two repos can sit on different editions with no shared state. [0.2]

**Drift:** HN-G001 on every shard until the first `sync`, with exit 3 while stale. In the second repo, a binary pinned to edition 1 that reads a lock written by edition 2 refuses with exit 4 (R-CLI-005, R-EDN-014). HN-R006 on old reference names until their due version; a reference through a `replaced-by` alias is HN-R004 (R-LC-022).

**Resolved here:** the plan lists mechanical edits only, and rollback is git (R-EDN-035, R-EDN-038). Implementation sites use `def`, not `impl`: `impl` is not a verb (D-18), and `implements=` is a 0.3 bridge key (D-20).

### Year 5: two agents and one human edit, overlapping scopes, three branches

**Base:** `main` is sealed at version 3.0.0. Both agent branches derive open version 3.1.0, because each carries a feature (`add`) and the minor bumps from 3.0.0 (D-42). Neither branch seals, because seal runs on the integration line by default (R-VER-055).

**Branches**

- `agent/tags` (A): `brg.tags-list` added to `fairlead/decl/bridge.md`; `src-tauri/src/commands/tags.rs`; a `generate_handler!` entry; `src/api/tags.ts`; draft `d-a1b2c3d4`; `fairlead sync` regenerates `gen/`.
- `agent/search-limit` (B): a field added to the args struct, and `brg.note-search` bumped to `ver=4` in `decl/bridge.md`. Draft `d-c3d4e5f6`; `gen/` regenerated.
- `human/search-rename`: wire renamed to `search_notes` in `decl/bridge.md`, `notes.rs`, `lib.rs` and the TS literal. No change doc.

**Merge order: human, A, B.**

1. A conflicts in `gen/` and on one adjacent `lib.rs` line. `gen/` is regenerated with `fairlead sync`, not merged by hand, because there is no merge driver (D-14, R-FILE-025). The `lib.rs` line is resolved by hand. [0.1]
2. B conflicts in `decl/bridge.md` and in the TS file. Taking one side of the TS conflict leaves a literal that does not match the wire name. `check` reports HN-H025 (`hier.bridge-wire`: the wire literal differs between the two ends), and at runtime the `invoke` rejects. [0.3]
3. Both drafts are sealed after the merge, on the integration line. Ordinals are assigned per scope and kind at seal (R-VER-041), and one seal writes `versions/3.1.0.md`. [0.2]
4. No version collision arises: nothing is stored on a branch, so both branches derive the same open version (R-VER-002, R-GIT-030). B's `context --since` digest predates the rename, so its delta is recomputed on the merged tree and never trusted (R-CTX-054). [0.2]
5. The human's `notes.rs` edit has no change doc: HN-D006 at the default warn severity (D-15). The owner may opt the `tauri` scope into error; that is per scope, not the default. [0.2]
6. B's added field is not checked. Field equality is out of scope (D-29), so the only signal is the version bump in `decl/bridge.md`, which a person or agent must write.

**After merge:** `brg.tags-list` has no call sites until A's frontend lands. HN-H018 (`hier.stale`) appears only once entries are declared; with none, the check is skipped with an info line (R-HIER-042). [0.3]

**Resolved here:** nothing claims a new ID, so two agents can declare the same one on separate branches; HN-D001 finds it on the merged tree (R-VER-044). `decl/bridge.md` is shared, and its home is root, the nearest common ancestor of its sites (R-SCOPE-017).

## Bridge contract, as declared (Year 2+ form)

```fairlead
+++
title = "Bridge declarations, Year 2+"
directives = [
  'def brg.note-search proto=tauri-command dir="ts->rs" wire=note_search ver=3 params=args:shp.note-search-args home=root exposure=all-windows ts="src/api/notes.ts#noteSearch" rs="src-tauri/src/commands/notes.rs#note_search"',
]
+++
```

`exposure` is intent only: fairlead does not parse capability files, so the report says "exposure unverified" (R-HIER-067).

### Checks by milestone

| check | verifies | milestone |
|---|---|---|
| impl end | `def brg.note-search side=rs` present; the Rust fn name equals `wire` | [0.1] text; [0.3] check (R-HIER-062) |
| call end | `def ... side=ts` present and the `wire` literal appears in the annotated block; a literal that differs from the other end is HN-H025 | [0.3] (R-HIER-063) |
| pin | `use ... pin=N` equals the declared `ver` (HN-H022, `hier.bridge-pin`) | [0.3] |
| params | presence only; field equality not checked (D-29) | [0.3] |
| dynamic invoke | a variable name cannot be verified; HN-H024 at warn on the ramp (R-CHK-043) | [0.3] |

Events (`evt.*`) and error shapes are user kinds whose `proto` word names the transport; `spell=` declares each bridge's spelling (R-SYN-043).

## Reachability audit (0.3)

The verdicts are orphan (HN-H003 for a file, HN-H004 for a unit), stale (HN-H018: not reachable from a declared entry) and forgotten (HN-H020: stale and dormant). Entries are declared with `def ent.* ...` and an `etype` (D-40, R-HIER-040). Dormancy reads the per-unit digest recorded at each seal, never git history (R-HIER-045). Every report says it reflects declared edges; orphan, stale and rank verdicts are lower bounds until an import extractor exists (R-HIER-034).

## What this scenario shows

- **0.1 is useful alone.** Directive lines, the bridge as text, `context` and path checks cover Year 0 and Month 3 without any seal.
- **Seal and lifecycle carry the long-term value.** Tombstones and the graveyard handle removal and re-add; a re-add under a new ID with new code is not caught.
- **Derived versions stop branch collisions.** Nothing is stored on a branch (D-14, D-42).
- **Declared-only checks have limits.** Bridges check wire names and pins, not field equality (D-29); reachability sees only declared edges (D-20).
- **Edition upgrades are the riskiest step.** Year 4 changes control, annotations and generated files together; git is the only rollback.

## Resolved questions

- Q-CHK-1 (resolved 2026-10-09): HN codes are stable and frozen; see R-CHK-042.
- Q-SYN-1 (resolved 2026-10-09): `--dry-run` on every writer; plan-first `init`, `upgrade`, `seal`; see R-CLI-043.
- Q-FILE-1 (resolved 2026-10-09): `.fairlead/` is appended to `.gitignore` only with `--yes`; see R-FILE-026.
- Q-FILE-2 (resolved 2026-10-09): AGENTS.md carries a fairlead-owned pointer block; see R-FILE-027.
- Q-FILE-3 (resolved 2026-10-09): the schema is open and unknown keys warn; see R-SYN-039.
- Q-CTX-1 (resolved 2026-10-09): budgets are 500 and 700 tokens, as `ceil(bytes/4)` estimates; see R-CTX-044.
- Q-CTX-2 (resolved 2026-10-09): 0.1 and 0.2 answer path and ID queries only; see R-CTX-004.
- Q-CTX-3 (resolved 2026-10-09): handoff docs carry the written reasoning; see R-CTX-008.
- Q-CTX-4 (resolved 2026-10-09): `--since` takes a version; deltas are recomputed on the merged tree; see R-CTX-054.
- Q-VER-1 (resolved 2026-10-09): `created` is the as-of date in 0.1 and the version from 0.2; see R-VER-012, D-57.
- Q-VER-4 (resolved 2026-10-09): no ID claims; HN-D001 on the merged tree; see R-VER-044.
- Q-VER-5 (resolved 2026-10-09): an older binary refuses a newer edition, exit 4; see R-EDN-014.
- Q-VER-6 (resolved 2026-10-09): mechanical plan, git as rollback; see R-EDN-035, R-EDN-038.
- Q-SCOPE-3 (resolved 2026-10-09): `home=` names an ancestor-or-self of every site; see R-SCOPE-017.
- Q-AGT-2 (resolved 2026-10-09): `owner=` must be listed in `owners`, which is root-only in 0.1 (D-51); see R-SYN-011, R-AGT-028.
- Q-GIT-2 (resolved 2026-10-09): low-similarity history reads "unavailable"; see R-GIT-075.
- Q-LIFE-1 (resolved 2026-10-09): revive only through an approved `revive` doc; see R-LC-024.
- Q-LIFE-2 (resolved 2026-10-09): renames use `flag replace` and a permanent `replaced-by` alias; no `move --id` (D-50); see R-LC-011.
- Q-LIFE-3 (resolved 2026-10-09): fingerprint computed once at seal and stored; see R-LC-031, R-LC-033.
- Q-LIFE-4 (resolved 2026-10-09): replaced IDs warn until due, removed IDs always error; see R-LC-008.
- Q-SYN-3 (resolved 2026-10-09): `flag` takes `target=`, never `until=`; see R-SYN-048.
- Q-SYN-4 (resolved 2026-10-09): the alias is the tombstone's `replaced-by` link, not a source line (D-50); see R-SYN-042.
- Q-SYN-5 (resolved 2026-10-09): the consumer pin is `pin=` on `use`; see R-SYN-049.
- Q-SYN-6 (resolved 2026-10-09): events are user kinds with a `proto` word; see R-SYN-043.
- Q-SYN-7 (resolved 2026-10-09): entries and bridges are declared with `def`; see R-HIER-040.
- Q-SYN-8 (resolved 2026-10-09): `move` rewrites fairlead text and lists code sites; see R-CLI-044.
- Q-CHK-3 (resolved 2026-10-09): unverifiable names are HN-H024 at warn; see R-CHK-043.
- Q-CHK-4 (resolved 2026-10-09): a move-only change is covered by the restructure doc; see R-CHK-044.
- Q-PORT-1 (resolved 2026-10-09): the extracted scope is a new root and inherited rules are pinned; see R-PORT-045.
- Q-PORT-2 (resolved 2026-10-09): explicit pull plus a CI check, no daemon; see R-PORT-046.
- Q-PORT-3 (resolved 2026-10-09): outward imports are `requires` candidates; see R-PORT-047.
- Q-PORT-4 (resolved 2026-10-09): `exposure` is intent only; see R-HIER-067.
- Q-HIER-4 (resolved 2026-10-09): dormancy reads seal-ledger digests; see R-HIER-045.
- Q-HIER-5 (resolved 2026-10-09): unsealed change docs suppress `hier.stale`; see R-HIER-046.
- Q-TOOL-4 (resolved 2026-10-09): external build inputs are `def ext.<name> role=input`; see R-HIER-068.
