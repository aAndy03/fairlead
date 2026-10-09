# Hierarchy and Engines
> Status: draft · Milestone: 0.3 · Decisions: D-20, D-29, D-11, D-12, D-18, D-02, D-15, D-16, D-28, D-35, D-37, D-40 · Related: 10-scopes-and-inheritance.md, 11-files-and-classes.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 14-context.md, 21-environments-maturity-lifecycle.md, 31-variation-protocol.md, 32-portability-and-restructure.md, 33-retention-and-compaction.md

## Purpose

Scopes say who owns a folder. They say nothing about how the pieces inside fit together: which unit may call which, what a feature is built from, which code nobody reaches any more. This chapter specifies a layered model (ladders of ranked layers), a containment tree, a `use` graph, entry points and cross-language bridges, plus the checks that keep them honest. Everything is user data in the control file; the checker knows only "ranked layers inside a named ladder".

The model generalises **Atomic Design** (Brad Frost): atoms, molecules, organisms, templates and pages for UI. Frost presents it as a mental model, not a strict process. `fairlead` keeps that spirit. The UI ladder is a shipped preset, and the same machinery expresses a backend ladder (primitive, adapter, engine, pipeline, service, shell). Nothing in the binary knows the word "atom".

## Concepts

| Term | Definition |
|---|---|
| Ladder | A named, ordered list of layers with ranks (`ui`, `core`). Ranks are compared only inside one ladder. |
| Layer | A named level in a ladder (`ui.atom`). Ranked, or *free* (unranked cross-cutting: `chrome`, `platform`). Unit declarations name layers, never numbers. |
| Unit | A reusable, ID'd thing with a layer, a container, and a home scope (`kind.name`; kinds are user-defined, e.g. `cmp`, `eng`, `pip`). |
| Container | A logical grouping of units, declared, nestable, may span folders and languages. Not a scope. |
| `use` edge | A fact written in code (`// fairlead: use <id>`), aggregated upward into the unit graph and the container graph. |
| Entry | A declared start of reachability: user action, CLI command, export, test, registered hook. |
| Bridge | A unit with two typed ends in different languages and a wire contract (TS to Rust). |
| Orphan / stale / dormant / forgotten | Three separate audits. Orphan: no container (structural). Stale: not reachable from any entry. Dormant: unchanged for N seals. Forgotten: stale AND dormant. |
| Adoption ramp level (L0-L3) | How much of the model a scope has adopted, set by `[hier] level`. Not the context tiers of [14-context.md](14-context.md) (glossary: "adoption ramp" vs "tiers"). |

### Scopes, containers and units

A **scope** is folder-derived and managerial (D-11); it decides inheritance and ownership. A **container** is logical and can combine a TS feature folder and a Rust crate. They share only a default: the home scope of a unit is the scope whose registry declares it. Do not unify them.

### Three structures plus traces

| Structure | Shape | Authored or fact | Where it lives |
|---|---|---|---|
| Scope tree | folder-derived | authored | managerial folders |
| Containment tree | every unit has exactly one container; containers nest | authored (intent) | `def` lines with `container=` / `parent=` |
| `use` DAG | edges between units, rank-checked | fact, written where the code is | `// fairlead: use` lines, aggregated into generated shards |
| Entry traces | entry to expected chain | authored (intent) | `def ent.*`, `steps=` on pipelines |

Intent flows down (what should be). Facts flow up (what is). Reviewers read the generated graph and its diff against intent.

## Presets (D-20)

`fairlead new ladder ui` and `fairlead new ladder core` scaffold these into the control file. They are content, not semantics.

| UI ladder | rank | Backend ladder `core` | rank | Analogy |
|---|---|---|---|---|
| atom | 10 | primitive (pure fn, value type, port trait) | 10 | indivisible, no I/O |
| molecule | 20 | adapter (I/O implementing a port) | 20 | small thing acting as a whole |
| organism | 30 | engine (one capability: verify, purify, validate) | 30 | complex, reusable |
| template | 40 | pipeline (ordered or branching composition of engines) | 40 | structure without content |
| page | 50 | service (pipeline wired to real adapters, exposed at a boundary) | 50 | instance with real content |
| (free) shell, chrome | | shell (host process: commands, CLI, window; entry-only, R-HIER-006) | 60 | app-level frames |

Ranks use gaps of 10 so a scope can insert a layer (`ui.widget = 25`).

```toml
# fairlead/control.toml (root; vocabulary keys are root-only sealed, D-02)
[layers.ui]
atom = 10
molecule = 20
organism = 30
template = 40
page = 50

[layers.core]
primitive = 10
adapter = 20
engine = 30
pipeline = 40
service = 50
shell = 60

[free.chrome]
of = "ui"
may-use = ["ui.atom", "ui.molecule", "ui.organism"]

[free.platform]
usable-by = ["core.*"]            # logging, config, errors, clock

[[flow]]                          # the only sanctioned cross-ladder edges
from = "ui.*"
to = ["core.service", "core.pipeline"]
via = "bridge"

[kinds.cmp]  layers = ["ui.*"]
[kinds.eng]  layers = ["core.engine"]
[kinds.pip]  layers = ["core.pipeline"]
[kinds.svc]  layers = ["core.service"]
[kinds.prt]  layers = ["core.primitive"]
[kinds.ctr]  # container
[kinds.ent]  # entry
[kinds.brg]  # bridge

[layer-policy."core.shell"]       # R-HIER-006
used-by = "entry"                 # reached only from entries, never by a `use` edge

[hier]
level = 2                         # adoption level, section "Adoption ramp"
same-rank = "allow"               # allow | container | forbid
unbound-ok = ["**/tests/**", "**/fixtures/**", "**/generated/**"]
promote = { fan-in-containers = 3, single-consumer-seals = 4 }
```

## Declarations

Units, containers, entries and bridges are declared with `def` directives (D-40), either in code comments next to the symbol or in `<!-- fairlead: ... -->` blocks of a Markdown registry doc in the owning scope's managerial folder. The examples use the `//` carrier; in a Markdown registry doc the same body sits in `<!-- fairlead: ... -->`. Code carries only IDs and edges. Keys: `layer`, `container`, `parent`, `path`, `sym`, `vis`, `steps`, `on-error`, `etype`, `unit`, `reaches`, `implements`, `spell`, `exposure`, `role`.

```fairlead
// fairlead: def ctr.sync-integrity parent=ctr.sync path="crates/sync-core/src/integrity/**"
// fairlead: def eng.sync-verify layer=core.engine container=ctr.sync-integrity sym=verify_manifest since=0.2.0
// fairlead: def pip.sync layer=core.pipeline container=ctr.sync sym=run_sync steps=eng.sync-verify,net:eng.conn-validate,eng.sync-transfer,eng.sync-purify on-error=eng.sync-edge-partial,eng.sync-edge-offline
```

Membership binds in exactly one of two ways (R-HIER-014): a `path=` glob on the container or unit, or a `def` in code next to the symbol. A code `def` binds by its extent in [syntax](12-directive-syntax.md) (R-SYN-016 to R-SYN-019: the standalone block, `+N`, or `+end` ... `end`); several units in one file are separate `def`s with their own extents. Using both for one unit is `hier.double-binding` (error). Containment is named only by the `container=` key on a unit's `def` and the `parent=` key on a container's `def` (R-HIER-013).

In code:

```rust
// fairlead: def eng.sync-verify
// fairlead: use prm.hash-blake
```

## Rules

### Ladders and layers
- **R-HIER-001** [0.3] Ladders, layers, kinds and `flow` rules MUST be declared only in the root control file. Layer names and rank order are `sealed`; adding a layer or container below an ancestor is `open`.
- **R-HIER-002** [0.3] Ranks MUST be compared only between layers of the same ladder. A cross-ladder `use` MUST match a `flow` entry or it is `hier.rank`.
- **R-HIER-003** [0.3] A unit's `layer=` MUST name a declared layer; unit declarations MUST NOT contain rank numbers.
- **R-HIER-004** [0.3] A child scope MAY alias a layer's wording and MAY narrow with `accepts=`. It MUST NOT reorder ranks; an attempt is ignored and becomes an escalation proposal to the ancestor (D-02).
- **R-HIER-005** [0.3] A free layer MUST be reachable only through its `may-use` / `usable-by` lists. Free layers MUST NOT be assigned ranks.
- **R-HIER-006** [0.3] A ranked layer MAY be marked `used-by = "entry"` under `[layer-policy."<ladder>.<layer>"]` in the root control file (sealed, R-HIER-001). A unit of such a layer MUST be reached only through entries (`def ent.* unit=<id>` or `reaches=<id>`, R-HIER-040); a `use` edge from a unit outside the layer that targets it is `hier.rank`. The `core` preset marks `core.shell` this way (D-20); the UI `shell` stays a free layer.

### Containment
- **R-HIER-010** [0.3] Every unit MUST have exactly one container (`hier.single-container`). Two container `path` globs MAY overlap only when one is nested in the other.
- **R-HIER-011** [0.3] The container `parent` chain MUST be acyclic (`hier.parent-cycle`). `parent=` MAY point into an ancestor scope's container; container `path` claims MUST NOT enter a sibling scope's subtree.
- **R-HIER-012** [0.3] A unit that conceptually belongs to two containers MUST be split or related with `see=`; a secondary membership MUST NOT be expressed as a second container.
- **R-HIER-013** [0.3] Containment MUST be declared with `def` directives (D-40): a unit names its single container with `container=<ctr id>` on its own `def`, and a container names its parent with `parent=<ctr id>` on its `def`. There is no TOML containment table and no `in=` alias. Ladders, layers and kinds stay in the control file (D-05).
- **R-HIER-014** [0.3] A container binds files by `path=` globs; a unit binds either by a `path=` glob or by a code `def` extent, never both. A unit with both is `hier.double-binding` (HN-H023, error, ambiguous binding). A code `def` binds only inside its own file.

### Edges
- **R-HIER-020** [0.3] A `use` edge within one ladder MUST satisfy rank(target) <= rank(source). Equal ranks follow `same-rank`: `allow`, `container` (same container only) or `forbid`.
- **R-HIER-021** [0.3] The unit graph MUST be a DAG (`hier.dag`). Declared self-recursion MAY be marked `self`. Same-rank edges never exempt a cycle.
- **R-HIER-022** [0.3] The container graph is the aggregation of unit edges; a strongly connected component is `hier.cycle`, and the report MUST list the unit edges that cause it.
- **R-HIER-023** [0.3] `implements` edges (a bridge or adapter implementing a port) are exempt from the rank rule by definition.
- **R-HIER-024** [0.3] An upward runtime need MUST be inverted: a port unit in a lower layer, implemented by an adapter or bridge. Otherwise an `ovr rule.hier-rank` with `why=`, `owner=` and an expiry (`until=<version>`, D-37) is required and lands in the exception ledger; an `ovr` without expiry is rejected. There is no separate `allow-up` mechanism.
- **R-HIER-025** [0.3] A unit MAY declare a dynamic-dispatch edge as `use <id> dyn=1` ([syntax](12-directive-syntax.md) R-SYN-015). This is a declared edge with no evidence requirement.
- **R-HIER-026** [0.3] A cross-scope `use` MUST name a unit whose `vis` allows the consumer (see [31-variation-protocol.md](31-variation-protocol.md), R-VAR-010), else `hier.visibility`.
- **R-HIER-027** [0.3] Cargo dependency edges between workspace members are read from the manifests (the `Cargo.toml` files are parsed directly, read-only; fairlead does not run `cargo metadata`, D-50; `dev-dependencies` excluded). An edge from crate A to crate B is a container edge between the containers whose `path` claims cover the two crates. It feeds `hier.cycle` (R-HIER-022) and, within one ladder, a container-level `hier.rank` check using the highest layer among each container's units. Such findings are labelled `evidence=cargo`. The default `[hier] same-rank` is `allow`.
- **R-HIER-028** [0.3] Visibility and direction are two independent rules. `provides` / `vis` gates who may reference a unit (R-HIER-026, `hier.visibility`); ranks gate which direction an edge may point (R-HIER-020, `hier.rank`). An edge MUST pass both; passing one never exempts the other.

### Evidence (AST-free)
- **R-HIER-030** [0.3] A unit MAY declare `sym=`, a distinctive identifier. `sym` MUST be at least 6 characters and unique across declared syms, else `hier.sym-ambiguous`, and evidence checks for that unit are suppressed until fixed.
- **R-HIER-031** [0.3] `hier.uses-noevidence`: a file declares `use X` but never mentions X's `sym` (comments excluded via per-extension comment syntax; a match is a whole identifier, not a substring). `hier.uses-undeclared`: it mentions the `sym` without `use X`.
- **R-HIER-032** [0.3] An import extractor (tree-sitter, syn) MAY later replace the *source* of the `use` facts. The rules in this chapter MUST NOT change when it does.
- **R-HIER-033** [0.3] Import evidence MAY be gathered by a regex pass over TS `import` / `from` specifiers and Rust `use crate::` paths. It is evidence only: it can confirm or contradict declared `use` edges (`hier.uses-undeclared`) and feeds `requires` candidates ([32-portability-and-restructure.md](32-portability-and-restructure.md), R-PORT-047), but it never creates rank or stale verdicts on its own and is never required. Nothing in 0.1 or 0.2 extracts imports (D-27, D-40).
- **R-HIER-034** [0.3] `use` edges live in code comments (`// fairlead: use <id>`) and in doc-level `use` of Markdown registry docs (D-32); there is no other store. Every report that depends on them (rank, cycle, stale, orphan) MUST state that it reflects declared edges and is a lower bound until an import extractor exists (D-40).
- **R-HIER-035** [0.3] TypeScript parsing via tree-sitter MUST NOT be adopted until a CI compatibility test shows the chosen `tree-sitter-typescript` grammar loads and parses a fixture corpus under the pinned `tree-sitter` core version. Until then the TS evidence source is the regex pass of R-HIER-033.

### Entries and reachability
- **R-HIER-040** [0.3] An entry MUST have an `etype` from the control file; shipped words: `user-action`, `cli`, `export`, `test`, `registered`. Tests, stories and benchmarks are auto-entries of type `test`, excluded from fan-in and single-consumer counts.
- **R-HIER-041** [0.3] `hier.stale`: a declared, bound unit not reachable from any entry of the environment(s) in which the unit exists. A dev-only unit unreachable in prod is by design, not stale.
- **R-HIER-042** [0.3] With zero entries declared, `stale` MUST be skipped with an info "no entries declared".
- **R-HIER-043** [0.3] `hier.dormant` (info): a unit digest unchanged for N seals, from the seal ledger (R-HIER-045). `hier.forgotten`: stale AND dormant; it proposes a graveyard entry and `explain` shows last-touch seal and last known entry.
- **R-HIER-044** [0.3] `reaches=none` on an entry declares a local-only entry; `reaches=<id>` is checked against the computed chain.
- **R-HIER-045** [0.3] The data source for dormancy is the per-unit digest recorded at each seal in the seal ledger ([versions](20-versions-and-seal.md)). Git history MUST NOT be read for it, so squashed or rewritten history cannot change the verdict. A unit first seen at a seal starts counting at that seal.
- **R-HIER-046** [0.3] A unit named in `adds` or `touches` of a change doc that is not yet sealed (status `draft` or `ready`) is exempt from `hier.stale` until that doc is sealed or abandoned. Entries are declared with kinds (R-HIER-040); there is no other suppression.

### Orphans
- **R-HIER-050** [0.3] `hier.orphan-unit`: a unit with no container, a container that does not exist, or a `def`/`use` marker naming an undeclared ID.
- **R-HIER-051** [0.3] `hier.orphan-file`: a file under a `governed` glob bound to no container or unit. `unbound-ok` globs (tests, fixtures, generated, vendor-owned) are exempt globally, not per file.
- **R-HIER-052** [0.3] `hier.hollow`: a unit with no bound files; exempt while `lifecycle` is `proposed`.

### Bridges (D-29)
- **R-HIER-060** [0.3] A bridge MUST declare `proto`, `dir`, `wire`, `ver`, `params`, one `ts=` and one `rs=` end as `path#symbol`, and optionally `contract=`, `reaches=`, `implements=` or `spell=` (the wire spelling convention of this bridge, e.g. `snake`, [syntax](12-directive-syntax.md) R-SYN-043). `proto` words are user-defined (`tauri-command`, `tauri-event`, `http`, `grpc`, `ffi`, `wasm`). Events, `Channel` arguments and error shapes are ordinary user-defined kinds whose `proto` word names the transport.
- **R-HIER-061** [0.3] Params are declared by name (top-level invoke keys), not as struct definitions. Payload field equality is out of scope until a parser or generator exists, and the report MUST say `fields unverified` rather than pass.
- **R-HIER-062** [0.3] `hier.bridge-end-missing` MUST fire when an end lacks its `def brg.* side=ts|rs` marker, or its `#symbol` does not appear textually in its file.
- **R-HIER-063** [0.3] The wire literal MUST appear on both sides (TS call literal, Rust fn name equal to `wire`), and `ver` on both end markers MUST equal the declaration (`hier.bridge-pin`). A consumer `use <bridge id> pin=<N>` records the version the consumer was written against and MUST equal the declared `ver` (`hier.bridge-pin`, [syntax](12-directive-syntax.md) R-SYN-049). A dynamic name is `hier.unverifiable` (HN-H024, warn on the stage ramp, [checks](13-checks-and-diagnostics.md) R-CHK-043) and is never guessed by regex.
- **R-HIER-064** [0.3] The contract file hash is recorded at `seal`. A contract change MUST be covered by a change doc touching both ends' scopes.
- **R-HIER-065** [0.3] A `use` of a bridge with `reaches=` is checked against `flow` using the reached unit's layer. A bridge with `implements=` and no `reaches` (inverted, event style) is exempt from `flow` and rank checks.
- **R-HIER-066** [0.3] Files generated from a contract (ts-rs, specta, protobuf) MUST be vendor-owned or `external-generated` ([files](11-files-and-classes.md) R-FILE-009), or their regions MUST sit in `def foreign.<tool>` ... `end` (D-35); each declared item they contain carries `gen=brg.<id>` on its `def` (D-35). There is no file-level `generated-from` marker. They are excluded from rank, fork and token scans and flagged stale when the contract hash changes and they are not regenerated.
- **R-HIER-067** [0.3] A bridge MAY declare `exposure=<word>` (words are user-defined, e.g. `all-windows`). It is intent only: fairlead does not parse Tauri `capabilities/*.json`, `AppManifest` or `build.rs`, and the report MUST say `exposure unverified`.
- **R-HIER-068** [0.3] An artifact that a build configuration consumes from outside the declaring scope or repository (Tauri `frontendDist`, the output of `beforeBuildCommand`) MUST be declared as an external input with `def ext.<name> role=input path=...` at the boundary scope. Such an input is vendor-owned for scanning purposes ([31-variation-protocol.md](31-variation-protocol.md), R-VAR-071). Output of binding generators such as `tauri-specta` follows R-HIER-066.

### Suggestions (info, never block)
- **R-HIER-070** [0.3] Implied layer = the layer above the highest layer a unit uses in its ladder. Declared below implied is `hier.rank` (error). Declared more than one layer above implied suggests demotion.
- **R-HIER-071** [0.3] `hier.promote`: a unit used from at least K distinct containers while in a leaf container, suggest the lowest common ancestor container. `hier.demote`: a high-layer, non-public unit with one consumer for N seals. `hier.split`: fan-out above F with mixed container targets. `hier.merge`: container with at most one unit for N seals.
- **R-HIER-072** [0.3] `vis=public` or `etype=export` MUST suppress demotion. Thresholds come from `[hier]`; suggestions are written to a generated suggestions shard.
- **R-HIER-073** [0.3] `hier.maturity`: a `stable` unit depending on `alpha`, or a prod-reaching unit depending on a dev-only unit ([21-environments-maturity-lifecycle.md](21-environments-maturity-lifecycle.md)).

### Adoption ramp
- **R-HIER-080** [0.3] Coverage MUST NOT be a totality rule. `[hier] level` in the control file selects the checks that run, and checks above it are off.
- **R-HIER-081** [0.3] Coverage is reported as bound files / governed files with a ratchet baseline (D-28): it MUST NOT decrease without a waiver. Strict totality is opt-in per scope.

| Level | You add | Checks switched on |
|---|---|---|
| L0 | containers with `path=` only | `hier.orphan-file`, `hier.single-container`, `hier.parent-cycle` |
| L1 | `def` units (layer, container) | `hier.orphan-unit`, `hier.hollow`, `hier.sym-ambiguous`, `hier.double-binding` |
| L2 | `use` lines, optionally `sym=` | `hier.rank`, `hier.dag`, `hier.cycle`, `hier.visibility`, evidence checks, suggestions |
| L3 | entries, `steps=`, bridges | `hier.stale`, `hier.dormant`, `hier.forgotten`, bridge checks, trace `explain` |

### Docs-first per scope
- **R-HIER-090** [0.3] A new unit, container or bridge MUST appear in a change doc's `adds` or `touches` (`doc.undocumented-unit`); a doc listing an ID that does not exist is `doc.dangling`. Both follow the observe, warn, enforce ramp (D-15).
- **R-HIER-091** [0.3] Coverage is path-based: each changed file MUST be covered by a change doc in its scope or an ancestor via `touches` (unit ID, container ID, scope or glob). A change spanning scopes has one doc in the lowest common ancestor scope with `covers`; child scopes need no copy.
- **R-HIER-092** [0.3] Trivial work MUST use a rolling `chore` doc (`fairlead new change --from-diff`). Ordering (doc before code) is not provable and MUST NOT be claimed; only an optional hook can enforce it.

### Refactors
- **R-HIER-100** [0.3] Container merge, split, removal and unit rewrite MUST be recorded in a change doc of `type = "refactor"` and applied with `fairlead move` (D-25), which rewrites authored `container=`/`parent=` fields deterministically. Unit IDs and code markers MUST NOT change.
- **R-HIER-101** [0.3] A retired container leaves a TOML `[[tomb]]` table with `moved-to` (written by seal, [lifecycle](21-environments-maturity-lifecycle.md)); a retired unit is `flag replace <ID> to=<successor> target=<version>` (D-49). Both tombstones are permanent (D-16, [retention](33-retention-and-compaction.md)).
- **R-HIER-102** [0.3] After a refactor, a unit still naming a retired container is `hier.orphan-unit` with fix `moved-to`. Refs to a deprecated unit warn with counts until the removal version; at removal, any ref is an error.
- **R-HIER-103** [0.3] A transitional container cycle MAY be waived with `ovr rule.hier-cycle why=... owner=... until=<version>`. An unbounded cycle waiver MUST be rejected.

## Checks summary

| Check | Verdict | Default ceiling | Needs | Level |
|---|---|---|---|---|
| hier.single-container, hier.parent-cycle | structural | error | declarations | L0 |
| hier.orphan-file | structural | error | file tree | L0 |
| hier.orphan-unit, hier.hollow, hier.double-binding | structural | error / warn / error | declarations | L1 |
| hier.rank, hier.dag, hier.cycle | layering | error | `use` lines | L2 |
| hier.visibility | home scope | error | `vis`, scopes | L2 |
| hier.uses-noevidence / -undeclared | evidence-grep | warn | `sym` | L2 |
| hier.promote / demote / split / merge | suggestion | info | `use` graph | L2 |
| hier.stale | reachability | warn | entries | L3 |
| hier.dormant | audit | info | seal ledger | L3 |
| hier.forgotten | stale and dormant | warn | entries, ledger | L3 |
| hier.bridge-* | contract | error | bridge decl | L3 |

Every ceiling is further capped by the scope's rollout ramp (D-28).

## Worked example: "Sync with server"

Scopes: `app` (root; TS UI in `app/src`, Tauri shell in `app/src-tauri`), `ui` (`packages/ui`), `sync` (`crates/sync-core`), `net` (`crates/net`).

```fairlead
// ---- scope ui: home of the atoms ----
// fairlead: def ctr.kit
// fairlead: def cmp.button layer=ui.atom container=ctr.kit vis=public
// fairlead: def cmp.icon   layer=ui.atom container=ctr.kit vis=public

// ---- scope app ----
// fairlead: def ctr.features
// fairlead: def ctr.sync-ui parent=ctr.features path="app/src/features/sync/**"
// fairlead: def cmp.sync-button layer=ui.molecule container=ctr.sync-ui sym=SyncButton
// fairlead: def cmp.sync-panel  layer=ui.organism container=ctr.sync-ui sym=SyncPanel
// fairlead: def brg.tauri-sync-run proto=tauri-command dir="ts->rs" wire=sync_run ver=1 params=req reaches=sync:svc.sync contract="docs/contracts/sync-run.md" ts="app/src/api/sync.ts#syncRun" rs="app/src-tauri/src/cmds.rs#sync_run"
// fairlead: def brg.tauri-evt-sync-progress proto=tauri-event dir="rs->ts" wire=sync_progress ver=1 implements=sync:prt.progress-sink ts="app/src/api/events.ts#onSyncProgress" rs="app/src-tauri/src/events.rs#emit_progress"
// fairlead: def ent.sync-click etype=user-action unit=cmp.sync-button reaches=sync:svc.sync

// ---- scope sync (containers: sync, sync-integrity, sync-edge, events, hygiene, prims) ----
// fairlead: def svc.sync layer=core.service container=ctr.sync sym=SyncService vis=public
// fairlead: def pip.sync layer=core.pipeline container=ctr.sync steps=eng.sync-verify,net:eng.conn-validate,eng.sync-transfer,eng.sync-purify on-error=eng.sync-edge-partial,eng.sync-edge-offline
// fairlead: def eng.sync-verify layer=core.engine container=ctr.sync-integrity sym=verify_manifest
// fairlead: def eng.sync-transfer layer=core.engine container=ctr.sync sym=transfer_batch
// fairlead: def eng.sync-purify layer=core.engine container=ctr.hygiene sym=purify_records
// fairlead: def eng.progress-propagate layer=core.engine container=ctr.events sym=propagate_progress
// fairlead: def prt.progress-sink layer=core.primitive container=ctr.events sym=ProgressSink vis=public
// fairlead: def prm.hash-blake layer=core.primitive container=ctr.prims vis=public

// ---- scope net ----
// fairlead: def eng.conn-validate layer=core.engine container=ctr.conn sym=validate_connection vis=public
```

Code edges: `SyncButton.tsx` has `// fairlead: use ui:cmp.button ui:cmp.icon brg.tauri-sync-run`; `SyncPanel.tsx` has `// fairlead: use brg.tauri-evt-sync-progress`; `pipeline.rs` has `// fairlead: use eng.sync-verify net:eng.conn-validate eng.sync-transfer eng.sync-purify eng.progress-propagate`; `progress.rs` has `// fairlead: use prt.progress-sink`. Each bridge end carries `// fairlead: def brg.tauri-sync-run side=ts ver=1` or `side=rs ver=1`.

What the checks do:

| Concern | Outcome |
|---|---|
| Rank | service(50) to pipeline(40) to engine(30) to primitive(10): ok. `cmp.sync-button`(20) to `ui:cmp.button`(10): ok. |
| UI to core | Only through `brg.tauri-sync-run`; its `reaches` unit is `core.service`, which `flow` permits (R-HIER-065). |
| Upward need | Progress is engine to UI. Inverted via `prt.progress-sink` (rank 10); the event bridge `implements` it; exempt by R-HIER-023. No `ovr` needed. |
| Trace | `fairlead explain ent.sync-click` prints `cmp.sync-button -> brg.tauri-sync-run => svc.sync -> pip.sync -> {eng.sync-verify, net:eng.conn-validate, eng.sync-transfer, eng.sync-purify}; on-error {eng.sync-edge-*}; side {eng.progress-propagate -> prt.progress-sink <= brg.tauri-evt-sync-progress -> cmp.sync-panel}`. |
| Pipeline `steps` | Intent. The `use` lines MUST contain every step (set check). Step order is documentation only; it is not provable without an AST. |
| Cross-container | `ctr.sync` depends on integrity, conn, hygiene, events, edge. None points back: no cycle. If `net` used `eng.sync-transfer`: `hier.cycle ctr.sync <-> ctr.conn`; fix hint: extract the shared piece to a lower container. |
| Visibility | `net:eng.conn-validate` is used by a sibling scope, hence `vis=public` (R-HIER-026). |
| Bridge | Removing `syncRun` from `sync.ts` gives `hier.bridge-end-missing`. Editing the contract file requires a change doc touching both ends' scopes. |

### Later refactor, expressed and checked

```toml
+++
kind = "change"
id = "d-2b40c1d8"
scope = "sync"
type = "refactor"
status = "ready"
created = "0.3.0"
touches = ["ctr.sync-edge", "ctr.sync-integrity", "eng.sync-purify", "ctr.events"]
sum = "Edge cases folded into integrity; purify rewritten; events moved to generic jobs"
out = "adopted"

[[op]]
action = "merge"
from = "ctr.sync-edge"
into = "ctr.sync-integrity"

[[op]]
action = "supersede"
unit = "eng.sync-purify"
by = "eng.sync-purify2"
removal = "0.4.0"

[[op]]
action = "remove"
container = "ctr.events"
moved-to = "jobs:ctr.queue"
+++
```

```fairlead
// fairlead: flag deprecate eng.sync-purify to=eng.sync-purify2 target=0.4.0
```

| Step | Check result |
|---|---|
| `fairlead move --merge ctr.sync-edge ctr.sync-integrity` | Authored `container=` fields rewritten; unit IDs untouched; tombstone for `ctr.sync-edge` with `moved-to`. |
| `ctr.events` removed | Needs `moved-to` or `reason`, else `doc.uncovered`; units still naming it are `hier.orphan-unit`. |
| `eng.sync-purify` deprecated | Warnings with ref counts until `0.4.0`; at removal any ref is an error. |
| `eng.sync-purify2` | No change doc covers it: `doc.undocumented-unit`. |

## Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Upward runtime need (engine reports to UI) | Invert via port plus `implements` bridge; else `ovr` with expiry in the ledger | R-HIER-024 |
| 2 | Cross-cutting concern (log, config) | Free layer with `usable-by`; never given a high rank | R-HIER-005 |
| 3 | Dynamic dispatch, DI, string-keyed registry | `registered` entry or `use <id> dyn=1`; otherwise a stale false positive | R-HIER-025 |
| 4 | Public library API unused internally | `etype=export` entry; demotion suppressed | R-HIER-072 |
| 5 | Unit belongs to two containers | Forbidden; split or `see=` | R-HIER-012 |
| 6 | Overlapping container globs, not nested | `hier.single-container` naming the overlap | R-HIER-010 |
| 7 | Several components in one file | Each `def` binds to its own extent (`+end`/`end` for regions); splitting later moves only the `def` lines | R-SYN-018, R-SYN-019 |
| 8 | Marker plus `path=` on one unit | `hier.double-binding` (error, ambiguous binding) | R-HIER-014 |
| 9 | Generic `sym` gives false evidence | `hier.sym-ambiguous`; evidence suppressed for that unit | R-HIER-030 |
| 10 | Symbol renamed, `sym=` not updated | `hier.uses-noevidence`; hint names the decl line | R-HIER-031 |
| 11 | Zero entries declared | `stale` skipped with info | R-HIER-042 |
| 12 | Env-gated unit (`env dev`) | Stale evaluated per env; prod entry reaching it is `hier.maturity` | R-HIER-041, R-HIER-073 |
| 13 | Bridge half removed | `hier.bridge-end-missing`; deprecating a bridge deprecates both ends in one change doc | R-HIER-062 |
| 14 | Dynamic `invoke(name)` | `hier.unverifiable` (HN-H024) warn (stage ramp); no regex guess | R-HIER-063 |
| 15 | Generated bindings (ts-rs, specta) | Excluded from scans; stale when the contract hash changes | R-HIER-066 |
| 16 | Layer removed at root | Descendant units dangle: `hier.orphan-unit` per scope until `fairlead move` remaps | R-HIER-100 |
| 17 | Scope mounted elsewhere with different ladders | Units bind by layer name; unknown names give a mismatch report at attach, not a rank guess | R-HIER-003, [32-portability-and-restructure.md](32-portability-and-restructure.md) |
| 18 | Folder with units moved outside its declaring scope | `hier.visibility` / home-outside-scope warn; `fairlead move` relocates the registry lines | R-HIER-026 |
| 19 | Transitional container cycle | `ovr rule.hier-cycle` with `until=`; unbounded is rejected | R-HIER-103 |
| 20 | Typo-level change | Rolling `chore` doc | R-HIER-092 |
| 21 | Two registries declare the same unit ID after a branch merge | `HN-D001` (duplicate ID, [checks](13-checks-and-diagnostics.md)) on the merged tree; CI checks the merge result | D-12 |
| 22 | Backend `shell` unit used by a `service` | `hier.rank` (entry-only layer); reach it through an entry | R-HIER-006 |
| 23 | Crate A depends on crate B in `Cargo.toml` and B's container is higher-ranked | Container-level `hier.rank` with `evidence=cargo`; equal ranks follow `same-rank` | R-HIER-027 |
| 24 | Edge passes the rank rule but the target is `vis=subtree` in another scope | `hier.visibility`; rank never exempts visibility | R-HIER-028 |
| 25 | Unit named by a draft change doc is unreachable | `hier.stale` suppressed until the doc is sealed or abandoned | R-HIER-046 |
| 26 | History squashed after many seals | Dormancy unchanged: it reads seal-ledger digests, not git | R-HIER-045 |
| 27 | Tauri `frontendDist` points at a frontend built in another repo | Declared as `def ext.<name> role=input`; bridge `exposure` stays unverified | R-HIER-068, R-HIER-067 |
| 28 | Regex import pass sees an import with no `use` line | `hier.uses-undeclared`; never a rank or stale verdict by itself | R-HIER-033 |

## Examples

See [examples/rust-workspace.md](examples/rust-workspace.md) (backend ladder over a Cargo workspace, Cargo edges versus `use`) and [examples/fullstack-five-years.md](examples/fullstack-five-years.md) (bridge contract across a five-year history).

## Resolved questions

All chapter-local and register questions were accepted by their stated leaning on 2026-10-09 (D-46).

- Q30-1 (resolved 2026-10-09): units, containers, entries and bridges are `def` directives in code comments or `<!-- fairlead: -->` blocks; generated items carry `gen=`; ladders and kinds stay TOML (D-40, D-35, D-05); see R-HIER-013.
- Q30-2 (resolved 2026-10-09): `use` edges live in code comments with evidence-grep; an extractor feeds the same graph later; results are lower bounds; see R-HIER-034, R-HIER-032.
- Q30-3 (resolved 2026-10-09): the entry kind key is `etype`; see R-HIER-040.
- Q30-4 (resolved 2026-10-09): context uses "tiers", the adoption ramp uses "levels" (`[hier] level`); see the glossary.
- Q30-5 (resolved 2026-10-09): a ranked backend `shell` is entry-only via `used-by = "entry"`; see R-HIER-006.
- Q30-6 (resolved 2026-10-09): dynamic-dispatch edges are `use <id> dyn=1`; see R-HIER-025.
- Q-HIER-1 (resolved 2026-10-09): a regex import pass over TS `import` and Rust `use crate::` is optional lower-bound evidence from 0.3; nothing is extracted in 0.1 or 0.2 (partly superseded by D-27, D-40); see R-HIER-033.
- Q-HIER-2 (resolved 2026-10-09): Cargo workspace edges are read as container edges in addition to `use` edges; same-rank default `allow`; see R-HIER-027, R-HIER-020.
- Q-HIER-3 (resolved 2026-10-09): two rules, `provides` / `vis` gates visibility and ranks gate direction; see R-HIER-028.
- Q-HIER-4 (resolved 2026-10-09): "dormant" and "forgotten" read seal-ledger digests, not git history; see R-HIER-045.
- Q-HIER-5 (resolved 2026-10-09): entries are declared with kinds and unsealed change docs suppress `hier.stale`; see R-HIER-040, R-HIER-046.
- Q-HIER-6 (resolved 2026-10-09): upward flows use a port unit plus `implements`, otherwise `ovr rule.hier-rank` with an expiry; no `allow-up`; see R-HIER-023, R-HIER-024.
- Q-HIER-7 (resolved 2026-10-09): superseded by D-40; containment is named by `container=` and `parent=` keys on `def` directives, no TOML table and no `in=`; see R-HIER-013.
- Q-HIER-8 (resolved 2026-10-09): containers bind by `path=` globs, units by `path=` or a code `def`, never both; see R-HIER-010, R-HIER-014.
- Q-HIER-9 (resolved 2026-10-09): superseded by D-35; generated bindings are vendor-owned, in `def foreign` regions, or carry `gen=` on their `def`, with no `generated-from` marker; see R-HIER-066.
- Q-HIER-10 (resolved 2026-10-09): `sym=` guardrails of length, uniqueness and comment stripping stay, plus whole-identifier matching; see R-HIER-030, R-HIER-031.
- Q-TOOL-3 (resolved 2026-10-09): tree-sitter TypeScript parsing waits for a CI compatibility test; the regex pass is the evidence source until then; see R-HIER-035.
- Q-TOOL-4 (resolved 2026-10-09): external build inputs (Tauri `frontendDist`) are declared as `ext` inputs and generated bindings follow R-HIER-066 (`gen=`, superseding `generated-from` per D-35); bridge `exposure` is intent only (Q-PORT-4); see R-HIER-068, R-HIER-067.
