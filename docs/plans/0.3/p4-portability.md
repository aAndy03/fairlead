# P4: portability (requires, provides, lock, mounts, cut/copy/attach)

> Chapter [32-portability-and-restructure.md](../../32-portability-and-restructure.md) R-PORT-020 to R-PORT-055; [22-editions-and-upgrade.md](../../22-editions-and-upgrade.md) R-EDN-045; [10-scopes-and-inheritance.md](../../10-scopes-and-inheritance.md) R-SCOPE-066; [33-retention-and-compaction.md](../../33-retention-and-compaction.md) R-RET-071. Decisions D-03, D-22, D-26, D-50.

## Goal

A scope declares what it needs and offers. A digest-only lock (snapshots only for `portable` scopes) pins the inherited items. Roots mounted inside a codebase are sealed sub-roots, opened only by an approved `adopt`. Scopes can be cut, copied and attached across repositories in two steps, with no shared transaction.

## Design

- `provides` (authored) and `requires` (generated from `use` and `extends` plus authored additions) live in the scope's control file and in `gen/requires`.
- **Lock entries** in `gen/scope.lock`, extending the 0.2 file:
  - `req=<id> from=<scope> dg=<digest> ver=<version>` per required item;
  - `chain dg=<digest>`;
  - the edition;
  - for `portable = true` scopes only, a snapshot block with the full text of the required items and the L0 lines of the required docs.
- **Mounts:** `[[mount]] scope path as inherit` in the outer root's control file. A registered mount is a sealed sub-root, read under its own edition (0.2 dispatch, P4-T9 of 0.2) and its own version line.
- **`move --cut|--copy|--attach|--adopt`** extend 0.2's `restructure` planner. Attach classifies each `requires` entry as satisfied, differing or missing.

## Tasks

**T1. `requires` and `provides`.** Rules R-PORT-020, R-PORT-021 and R-SCOPE-066:
- `requires` is generated, with authored additions merged in.
- A `use` of a qualified ID that is missing from `requires`, or from the target's `provides`, is reported.
- Cycles: a `requires` cycle is a dependency warning; an `extends` cycle is an error.

Tests:
- `r_port_020__requires_generated_from_use_and_extends`
- `r_port_021__requires_cycle_warns_extends_cycle_errors`

**T2. Digest-only lock.** R-PORT-022, R-PORT-023:
- Per required item, the lock stores the resolved-value digest, source scope, chain digest and edition.
- Snapshots are stored only for `portable` scopes.
- Imported `ovr`, `allow` and exceptions stay inert until approved at attach.

Tests: `r_port_022__lock_is_digest_only_unless_portable`, `r_port_023__imported_ovr_inert_until_approved`.

**T3. Scoped doc IDs and claims.** R-PORT-024, R-PORT-025:
- Outside its own scope, a doc ID is written as `<scope>:d-<draft>`. `context` always qualifies doc IDs.
- An unqualified `d-<draft>` resolves along the scope chain.
- A restated claim without `contradicts` is `DUP-CLAIM` (HN-M004).

Tests: one per rule.

**T4. Mount registration.** R-PORT-030, R-PORT-031, R-PORT-035, R-EDN-045:
- An unregistered inner `root = true` is `ROOT-DUP` (HN-C004, from 0.1).
- A registered mount is a sealed sub-root. Outer inheritance reaches it only through `inherit` or satisfied `requires`. The outer root reads only its rollup.
- `as=<prefix>` namespaces the inner scope ids in the outer view.
- An edition mismatch is reported once, at the boundary, as an upgrade diagnostic (`H-UPG-BOUNDARY`, HN-V014, info).

Tests:
- `r_port_030__unregistered_inner_root_is_root_dup`
- `r_port_031__outer_reads_rollup_only`
- `r_port_035__as_prefix_namespaces_ids`
- `r_edn_045__edition_mismatch_reported_once_at_boundary`

**T5. Write isolation and projections.** R-PORT-032, R-PORT-036:
- Outer tools never write into a sealed sub-root unless invoked inside it.
- Generated `:root` projections are written only inside their root's own subtree. Two roots generating the same path is `PROJECTION-OUTSIDE` (HN-M010).

Tests: one per rule.

**T6. `move --adopt`.** R-PORT-033, R-PORT-034, R-CLI-002:
- Without `--yes`, print the diff of the sub-root's defs against the outer effective view and exit 4.
- Conflicts become `ovr`s or alias renames.
- Applying needs an `[[approval]]` in the outer root.
- Adopting removes the inner `root = true`.
- Both managerial folders must be covered by CODEOWNERS (HN-A002).

Tests: `r_port_033__adopt_needs_approval`, `r_port_033__adopt_removes_inner_root_flag`.

**T7. `move --cut` and `--copy`.** R-PORT-040, R-PORT-045, R-RET-071:
- A cut keeps the scope ID and logs `moved-from` in both places.
- A copy mints a new ID with `derived-from=`.
- Ledger history and tombstones travel with the scope.
- Extraction to a repository with no outer root makes the scope that repository's root.
- Sealed docs keep their `covers` verbatim. Unsealed docs must be retargeted, or they are `DANGLING` (HN-M007).

Tests: one per rule.

**T8. `move --attach`.** R-PORT-041, R-PORT-042, R-PORT-044, R-PORT-047:
- Attach diffs `requires` against the new ancestors.
- A missing item is served from the `portable` snapshot as a `detached` layer, or else is `UNSATISFIED` (HN-M005) and fails `check`.
- Grants from the old ancestors are ungranted, and `req` scaffolds are offered.
- Provenance (source repository and commit) is recorded.
- Imported items are never auto-resolved.
- Outward imports found by the evidence pass are listed as `requires` candidates. An unsatisfied one is `UNSATISFIED`, and the report says the list is a lower bound.

Tests: one per rule.

**T9. Lock staleness.** R-PORT-043, R-PORT-046:
- `sync` re-resolves the lock from the recorded source: a local path, a submodule, or a git ref at a pinned version.
- `check` in CI compares the lock digests with the source when it is reachable. A mismatch is `LOCK-STALE` (HN-M012); an unreachable source is reported as "offline pin".
- Nothing is pushed anywhere.

Tests: `r_port_046__lock_stale_when_source_changed`, `r_port_046__offline_pin_when_unreachable`.

**T10. Submodules and vendored subtrees.** R-PORT-050 to R-PORT-052, R-GIT-023:
- A vendored subtree with a managerial folder is a foreign boundary: excluded from enforcement, and its `provides` are readable.
- A submodule root is a mounted root, pinned by commit and manifest digest. A bumped pin marks the parent lock stale.
- A submodule that is not checked out runs in offline mode from the snapshot.
- Out-of-set scopes in sparse checkouts are `NOT-CHECKED` (HN-M006), never a silent pass.

Tests: one per rule.

**T11. Manifests.** R-PORT-053 to R-PORT-055:
- Cargo members are suggested scope boundaries, never created automatically.
- A publishable crate whose manifest does not exclude the managerial folder, or whose `path` and `version` dependencies disagree, gets `PKG-HINT` (HN-M009, info).
- Manifests are read, never edited.
- No registry integration.

Tests: one per rule.

**T12. Codes.** Remove `HN-M004` to `M006`, `M009`, `M010`, `M012`, `HN-V012` and `V014` from `PENDING_0_3`. `H-REQ-CHANGED` (V012) is reported when a required item's digest changes at `sync`.
