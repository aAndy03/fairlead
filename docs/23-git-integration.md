# Git Integration
> Status: draft · Milestone: mixed · Decisions: D-03, D-10, D-13, D-14, D-19, D-25, D-26, D-27, D-31, D-38, D-41, D-46, D-48 · Related: 11-files-and-classes.md, 15-cli.md, 20-versions-and-seal.md, 22-editions-and-upgrade.md, 32-portability-and-restructure.md, 40-agent-integration-and-trust.md, 50-architecture.md

## Purpose
Fairlead's state is files in a git repository, and its design must survive every normal and abnormal git operation without a daemon, a merge driver, or stored commit hashes. This chapter fixes the principle (what is stored and what is derived), lists the behaviour for each git case, defines how generated-file conflicts are resolved by regeneration, gives implementation notes for the `git2` crate, and sets the dirty-tree and interruption rules.

## Concepts

| Term | Definition |
|---|---|
| **Write-set** | The exact files a writing command declares it may change. A command MUST NOT write outside it. |
| **Regeneration** | Producing a generated file again from authored inputs. Because generated files are deterministic (D-19), regeneration replaces conflict resolution. |
| **Transaction directory** | `.fairlead/tx/<id>/`, gitignored staging area for multi-file operations. |
| **Commit point** | The single file renamed last in a multi-file operation; its presence defines success (for seal: `versions/<version>.md`). |
| **Integration line** | The protected branch (default `main`) on which seals are made and CI gates run. |

## Rules

### Principle
- **R-GIT-001** [0.1] `check` MUST be read-only on tracked files and MUST operate on the working tree. Writers (`sync`, `new`, `move`, `seal`, `upgrade`, `approve`, `init`) MUST write only their declared write-set; `compact` is not a verb (D-25: `seal` absorbs it).
- **R-GIT-002** [0.2] Fairlead MUST NOT store commit SHAs for unsealed state. Identity is by draft ID, sealed version and content hash, because SHAs die in amend, rebase and squash.
- **R-GIT-003** [0.2] Fairlead MUST NOT run any git write (commit, tag, add) unless the user passes `--commit` (and `--tag`). `--commit` makes exactly one commit with trailer `Fairlead-Version: <version>`.
- **R-GIT-004** [0.2] No state may live in a mutable append-only or pointer file shared across branches (version pointer, running log). State is either a new file per event (version file, ledger shard) or a regenerable generated file.
- **R-GIT-005** [0.2] Nothing in fairlead MAY depend on commit trailers, authorship or message conventions for correctness (agents omit them). The `Fairlead-Version` trailer is optional output of `--commit`; `check` and CI MUST NOT require it.
- **R-GIT-006** [0.2] Fairlead MUST work without git: `check` and `context` run; coverage and file history report "unavailable" explicitly, never as a pass. Other version control systems (Jujutsu without a colocated git repository, Mercurial) and plain directories are unsupported beyond this "git unavailable" behaviour; the shipped docs MUST say so.
- **R-GIT-007** [0.1] In a sparse checkout or partial clone (detected by `core.sparseCheckout` and skip-worktree index entries), a scope whose managerial folder or files are absent from the working tree MUST be reported as unknown (`H-GIT-SPARSE`, warn; `context` marks the scope unknown), never as passing. A scope absent from the working tree or a sparse checkout makes `check` exit 5 (cannot determine, D-44, D-56; [CLI R-CLI-003](15-cli.md)); an unresolved scope (an unparseable control file) stays exit 2.

### Regenerate instead of merge
- **R-GIT-010** [0.2] Generated files MUST be pure functions of authored inputs and the edition, so that `fairlead sync` resolves any conflict in them by overwriting them from scratch.
- **R-GIT-011** [0.2] `sync` MUST regenerate conflict-marked generated files, and MUST refuse to touch unmerged authored files, listing them.
- **R-GIT-012** [0.2] Fairlead MUST NOT require a custom merge driver. `init` shows these `.gitattributes` lines in its plan and appends them only with `--yes` ([CLI R-CLI-015](15-cli.md)): `**/fairlead/gen/** linguist-generated=true -diff` and `**/fairlead/ledger/** merge=union`, and `eol=lf` for the managerial folder. A `merge=fairlead-gen` driver (keep-ours) is an optional optimisation only, because drivers are not versioned.
- **R-GIT-013** [0.2] Generated files MUST always be emitted with LF. Hashes of text MUST normalise line endings and trailing whitespace; BOM and CR on read MUST be stripped and reported.
- **R-GIT-014** [0.2] Hand edits to a generated file MUST be flagged (`H-GEN-STALE`); there is no "keep mine" mode.
- **R-GIT-015** [0.2] Ledger files MUST be append-only line files so that `merge=union` (R-GIT-012) is the only merge strategy. A shard type that is not line-append-only MUST be stored as one file per event (R-GIT-004). After a union merge `check` MUST verify the checkpoint continuity of the ledger (the hash chain of [retention](33-retention-and-compaction.md) once it ships, 0.3); a break is an error. From 0.3 `sync` regenerates the open-year checkpoint lines, which are derived data ([retention](33-retention-and-compaction.md) R-RET-035), so a union merge or re-sort never leaves a broken chain; in 0.2 a break is repaired by resealing the later seal (R-GIT-031).

### Scope of the git walk
- **R-GIT-020** [0.2] The file walk MUST use the project ignore rules without requiring a repository and without global git ignores (D-27: `ignore` crate, `require_git(false)`, `git_global(false)`).
- **R-GIT-021** [0.2] Linked worktrees and submodules (a directory whose `.git` is a file, or a gitlink) MUST be excluded from the walk, including worktrees placed inside the repository (for example `.claude/worktrees/`).
- **R-GIT-022** [0.2] `.fairlead/` and its lock MUST be per worktree. Hooks MUST be installed in the common git directory and MUST honour `core.hooksPath`.
- **R-GIT-023** [0.2] A submodule MUST be a separate codebase; the parent MAY pin it with `requires` plus its `provides` digest.

### Version collisions and repair
- **R-GIT-030** [0.2] Two branches MAY claim the same open version before seal. A collision (two version files with the same version, or an add/add conflict on `versions/<version>.md`) MUST be reported as `H-VER-COLLISION` at merge or CI. Seals are made on the integration line by default (`version.seal-where = "integration"`, [versions](20-versions-and-seal.md) R-VER-055); `any` is for solo work.
- **R-GIT-031** [0.2] `fairlead seal --reseal` MUST renumber the user's own unintegrated seal (new number, new ordinals, content otherwise identical) in one transaction; it MUST refuse to renumber a version already present on the integration line.
- **R-GIT-032** [0.2] A seal commit with no predecessor version (cherry-picked alone) MUST fail the `prev` continuity check, directing the user to cherry-pick the change docs instead.
- **R-GIT-033** [0.2] The same doc ID in two lines is accepted only if the target doc declares `backport-of` and the body hash is identical.
- **R-GIT-034** [0.2] Reverting a seal commit un-seals (the version file vanishes and `O` returns to that number); this MUST be consistent because `S` is derived.
- **R-GIT-035** [0.2] The integration line is the only line that owns the version sequence; there are no per-line version counters. A maintenance or hotfix line that must ship before merge-back seals locally (with `seal-where = "any"` set on that line) and its seal is unintegrated: on merge-back the collision rule applies and `seal --reseal` renumbers it. Backported docs declare `backport-of` (R-GIT-033).
- **R-GIT-036** [0.2] Reverting the commit that introduced an ID: if the ID was never sealed, it simply no longer exists and nothing dangles; if it was sealed, the revert MUST be covered by a `kind = "revert"` doc with `of = "<id>"` ([versions](20-versions-and-seal.md) R-VER-038), and the tombstone state follows the content (a removed ID gets its tombstone at the next seal, [lifecycle](21-environments-maturity-lifecycle.md) R-LC-020). References left on other branches then report as removed-ID references.

### Shallow clones, rewrites, remote comparison
- **R-GIT-040** [0.2] In a shallow clone, scope-level state (ledger files) MUST work. Where the coverage base or file history is needed and unavailable, `check` MUST report `H-GIT-SHALLOW` (warn) and MUST NOT exit 0 for the affected check ([CLI R-CLI-003](15-cli.md)); it MUST NEVER silently pass.
- **R-GIT-041** [0.2] `check --ci` SHOULD accept `--base <ref>` and CI SHOULD fetch enough depth (`fetch-depth: 0`).
- **R-GIT-042** [0.2] `check --against <ref>` and `context` SHOULD warn when local sealed version files differ in content from the upstream copy (detects force-push divergence).

### Interruption and concurrency
- **R-GIT-050** [0.2] Single-file writes MUST use temp file, fsync and atomic rename. Multi-file operations MUST stage in `.fairlead/tx/<id>/` and apply in a fixed order, commit-point file last.
- **R-GIT-051** [0.2] Recovery MUST be detection, not log replay: a stale generated hash or a shard without its version file MUST give `H-TX-INCOMPLETE`; re-running the same command MUST be idempotent; `context` MUST show `--resume` or `--abort`.
- **R-GIT-052** [0.2] `.fairlead/lock` MUST hold pid, host and start time. Readers (`check`, `context`, `explain`) never take the lock; writers take it exclusively ([CLI R-CLI-008](15-cli.md)). A lock whose pid is dead MUST be broken with a note; a held lock MUST refuse at once (refusal, [CLI R-CLI-003](15-cli.md)) and print `lock held by pid N`. The lock lives only for the command (D-26). Writers that allocate versions or move declarations (`seal`, `move`, `upgrade`) MUST additionally take one lock in the common git directory (`fairlead-alloc.lock`), so two worktrees cannot run them at once; without git only `.fairlead/lock` applies.
- **R-GIT-053** [0.2] Hooks are advisory and opt-in: `init --hooks` offers them and nothing installs them by default (D-38). CI MUST be the gate. The optional pre-commit hook MUST check staged files only within a small time budget (default 2 s); heavy checks run in CI.
- **R-GIT-054** [0.2] A skip variable (such as `FAIRLEAD_SKIP`) MUST be ignored when `CI` is set, and each use MUST appear in the CI summary.

### Dirty-tree rules

| Operation | Dirty tree behaviour | Rule |
|---|---|---|
| `check`, `context`, `explain` | Any state; read-only | R-GIT-060 |
| `sync`, `new` | Generated files always overwritable; authored files only those the command creates | R-GIT-061 |
| `move`, `upgrade --yes` | Refuse if any file in the write-set differs from HEAD (`--allow-dirty` overrides) | R-GIT-062 |
| `seal` | Refuse unless the only diff is the seal write-set (`--allow-dirty` records `dirty = true`) | R-GIT-063 |
| During merge/rebase (unmerged paths) | `sync` regenerates generated conflicts; every authored-file writer and `seal` refuse | R-GIT-064 |
| Detached HEAD | `check` works; `seal` refuses | R-GIT-065 |

- **R-GIT-060 to R-GIT-065** [0.2] The behaviours in the table above are normative.

### Implementation notes (`git2`, D-27)
- **R-GIT-070** [0.2] (D-48) Reading repository state (status, diff, history, rename detection) MUST go through the `git2` crate (libgit2 bindings); the only other git access is the `git` executable of R-GIT-076; `gix` is pre-1.0 and excluded. The call surface MUST be confined to one module (see [architecture](50-architecture.md)) so it can be replaced.
- **R-GIT-071** [0.2] The codebase root is resolved by D-31 (`--root`, then `FAIRLEAD_ROOT`, then the nearest upward managerial folder whose control file has `root = true`). The git repository MUST be opened from that root directory and never discovered from the cwd, so a codebase root nested inside a bigger repository (sealed sub-root) still resolves its own work tree.
- **R-GIT-072** [0.2] Blobs read through libgit2 are unfiltered (no autocrlf smudge). Fairlead MUST normalise line endings itself (R-GIT-013) before hashing.
- **R-GIT-073** [0.2] Linked worktrees: use the common directory for hooks and the allocation lock of R-GIT-052 (`seal`, `move`, `upgrade`), the per-worktree directory for `.fairlead/` and its lock.
- **R-GIT-074** [0.2] Shallow detection MUST use the repository's shallow flag. Submodules are recognised by gitlink (mode 160000) entries.
- **R-GIT-075** [0.2] Changed-set computation MUST diff the base tree against the working directory including the index, with untracked files included and gitignored files excluded. Rename detection is used only to de-duplicate noise, never as the source of file history (similarity detection loses history on large splits). A history-based result (`explain` file version, audits) MUST report "unavailable" when the rename similarity is below the detection threshold, and the restructure doc written by `move` lists both the old and the new path.
- **R-GIT-076** [0.2] (D-48) `--commit` and `--tag` MUST create the commit and tag by running the `git` binary as an explicit foreground subprocess (D-26), so commit hooks and the user's signing configuration run as with a hand-made commit; if `git` is not on the path `--commit` MUST fail with a message. Everything read-only, and every other git interaction, goes through `git2` (R-GIT-070, D-48). The subprocess call sits in the same module as the git2 surface. The `Repository` handle MUST NOT be shared across threads; each thread opens its own.
- **R-GIT-077** [0.2] Distribution MUST avoid requiring a system libgit2 (vendored build); Windows binaries MUST be prebuilt (see `cargo binstall`).
- **R-GIT-078** [0.2] Non-UTF-8, case-insensitive and long paths: scope paths MUST be normalised to forward slashes with NFC names; a case-only rename is treated as a move; junctions are not followed.

## Examples

Hook installed in the common git directory (intent, no code):
```text
.git/hooks/pre-commit         fairlead check --staged --budget 2s
.git/hooks/pre-push           fairlead check --ci --base origin/main
```

CI step:
```text
fairlead check --ci --base origin/main --format sarif
  pinned: fairlead 0.3.2 (cargo install fairlead --version 0.3.2 --locked)
  verifies: no stale generated files, no gaps/duplicates, no tamper, coverage
```

Collision on rebase:
```text
H-VER-COLLISION  versions/1.0.3.md exists on origin/main (sealed by d-2n8c4wre)
                 yours (d-7k3m9qxa) is unintegrated.
  fix: fairlead seal --reseal    (renumber to 1.0.4; new ordinals; one transaction)
```

`.gitattributes` offer:
```text
# fairlead init proposes (not applied silently)
fairlead/** text eol=lf
**/fairlead/gen/** linguist-generated=true -diff
**/fairlead/ledger/** merge=union
```

## Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | commit | Nothing; optional hook runs `check --staged` | R-GIT-053 |
| 2 | amend | No effect; no SHAs stored | R-GIT-002 |
| 3 | rebase onto a base that sealed the same number | `H-VER-COLLISION`; `seal --reseal` renumbers yours | R-GIT-030, R-GIT-031 |
| 4 | cherry-pick of a doc | ID is kept; same ID plus same body hash accepted with `backport-of` | R-GIT-033 |
| 5 | cherry-pick of a version file alone | `prev` continuity fails; cherry-pick the docs instead | R-GIT-032 |
| 6 | merge with a rare textual conflict | New per-version files do not conflict; semantic collisions are `H-VER-COLLISION` | R-GIT-004, R-GIT-030 |
| 7 | squash-merge | Docs and ledger are files and survive; file-level `git log` is coarse; nothing depends on trailers | R-GIT-005 |
| 8 | revert of a code commit | Needs a `kind = "revert"` doc; coverage accepts reverts of sealed docs | R-VER-038 |
| 9 | revert of the seal commit | Un-seals; sealed docs return to `ready` | R-GIT-034 |
| 10 | stash | Invisible; cache is keyed by content hash | R-GIT-001 |
| 11 | branches claiming the same open version | Allowed pre-seal (symbolic); collision only at seal/integration; recommend trunk-mode seals | R-GIT-030 |
| 12 | force-push rewrites main | Version files in the tree stay the truth; `--against` flags divergence; nothing dangles | R-GIT-042 |
| 13 | shallow CI clone | `H-GIT-SHALLOW`; the affected check does not exit 0 ([CLI R-CLI-003](15-cli.md)); never a silent pass | R-GIT-040 |
| 14 | worktrees (including one inside the repo) | Per-worktree `.fairlead/`; excluded from the walk; two worktrees sealing means two branches | R-GIT-021, R-GIT-022 |
| 15 | submodule | Walk stops at the gitlink; separate codebase | R-GIT-023 |
| 16 | subtree or vendored copy with its own `fairlead/` | Sealed sub-root with its own edition; parent seals never write inside | R-EDN-040 |
| 17 | bisect over old commits | `check` reads that commit's edition and writes only cache; unsupported edition is refused ([CLI R-CLI-005](15-cli.md)) | R-EDN-013 |
| 18 | conflict markers in a generated file | `sync` regenerates; if authored inputs also conflict, it refuses and lists them | R-GIT-011 |
| 19 | merge conflict inside the control file | Left to git (authored); `check` fails closed on unparseable control | R-GIT-011 |
| 20 | `--no-verify` | Hooks advisory; CI repeats the check | R-GIT-053 |
| 21 | Hook skip variable left in CI config | Ignored when `CI` is set; each use listed | R-GIT-054 |
| 22 | Interrupted `sync` or `seal` (disk full, killed process) | Staged writes and commit-point last; `H-TX-INCOMPLETE`; re-run idempotent | R-GIT-050, R-GIT-051 |
| 23 | Two fairlead processes at once | Shared vs exclusive lock; bounded wait; stale lock stolen | R-GIT-052 |
| 24 | `core.autocrlf=true` on Windows | Hashes normalise line endings; generated files LF | R-GIT-013 |
| 25 | Case-only rename on Windows or macOS | Treated as a move; scope names normalised | R-GIT-078 |
| 26 | Rename plus content edit | Coverage requires both paths; rename detection only de-duplicates noise | R-GIT-075 |
| 27 | Squash merge erases PR order | Order cannot be audited after the fact; only coverage | R-VER-039 |
| 28 | Human merges generated shards "ours" | Next `sync` or CI detects stale hash and regenerates | R-GIT-010 |
| 29 | Parallel agents add commands in one shared registry file | Authored per-doc files do not conflict; generated registry regenerates; collisions on ID are `H-ID-COLLISION` | R-GIT-010, R-VER-042 |
| 30 | Human renames a wire name in code only, no doc | Consistency check fails the merged tree; coverage reports the uncovered file | R-VER-032 |
| 31 | Octopus merge or merge queue reorders seals | Same collision rule; run `seal` as the queue's last step | R-GIT-030 |
| 32 | Hotfix on maintenance branch while main is a later line | Maintenance seal is unintegrated; resealed on merge-back; backport docs carry `backport-of` | R-GIT-035 |
| 33 | Submodule bumped, `provides` digest unchanged | No parent action; changed digest gives `H-REQ-CHANGED` info | R-GIT-023 |
| 34 | `.fairlead/` accidentally tracked | `context` warns; `init` writes the ignore entry | R-GIT-001 |
| 35 | Windows reserved or over-long paths | Rejected by `new`/`check` for names; long-path names beyond the OS limit are rejected with a clear message | R-GIT-078 |
| 36 | Sparse checkout hides a scope | Scope reported unknown (`H-GIT-SPARSE`), never green | R-GIT-007 |
| 37 | Revert of the commit that added a sealed ID | Needs a `revert` doc; tombstone follows content | R-GIT-036 |
| 38 | Jujutsu or Mercurial workspace | Unsupported; `check`/`context` run with "git unavailable" | R-GIT-006 |
| 39 | `move` in worktree A while `seal` runs in worktree B | Second one refuses on the common-dir allocation lock | R-GIT-052 |
| 40 | Union merge of ledger files corrupts the chain | `check` reports the break; `sync` regenerates open-year checkpoints (0.3), else reseal | R-GIT-015 |
| 41 | `seal --commit` with a signing key configured | The `git` subprocess signs and runs hooks | R-GIT-076 |
| 42 | Large split loses rename history | History reported "unavailable"; restructure doc lists both paths | R-GIT-075 |

## Resolved questions
- Q23-1 (resolved 2026-10-09): `--commit` and `--tag` shell out to the `git` binary so hooks and signing run; git2 for all reads (D-48); see R-GIT-076
- Q23-2 (resolved 2026-10-09): trunk-mode seal by default, `any` for solo work; see R-GIT-030, R-VER-055
- Q23-3 (resolved 2026-10-09): non-git VCS unsupported beyond "git unavailable"; see R-GIT-006
- Q23-4 (resolved 2026-10-09): hooks are opt-in, `init` offers and never installs by default (D-38); see R-GIT-053
- Q23-5 (resolved 2026-10-09): `merge=union` is the only ledger strategy; non-append shards are one file per event; chain checked after merge; see R-GIT-015
- Q-GIT-1 (resolved 2026-10-09): trailers are optional output and never relied on; see R-GIT-005
- Q-GIT-2 (resolved 2026-10-09): history-based audits report "unavailable" on low similarity; the restructure doc lists both paths; see R-GIT-075
- Q-GIT-3 (resolved 2026-10-09): scopes outside a sparse checkout or partial clone are unknown, never green; see R-GIT-007
- Q-GIT-4 (resolved 2026-10-09): one allocation lock in the common git directory covers seal, move and upgrade; see R-GIT-052, R-GIT-073
- Q-GIT-5 (resolved 2026-10-09): a revert of a sealed ID needs a `revert` doc and the tombstone follows content; see R-GIT-036
- Q-GIT-6 (resolved 2026-10-09): non-git workspaces and Jujutsu are unsupported beyond `check` and `context`; see R-GIT-006
