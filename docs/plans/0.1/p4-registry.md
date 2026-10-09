# P4 Registry and Resolve Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `fairlead-core::registry`, the pure, single-threaded, immutable resolver of the scope tree: scopes with stored ids, items, overrides with their states, `[[allow]]` grants, `[[req]]` escalations, effective views with provenance, the exception ledger computed on read, deterministic fold order, fail-closed subtrees, and the pure promotion and demotion model that proves the equivalence invariant (R-SCOPE-012). Output of this phase is `registry::resolve(&ResolveInput) -> Registry` (seam S6) and the diagnostics the contract assigns to P4 (`HN-S002 S009 S011 S016, HN-C007 C008, HN-R005 R007, HN-D001, HN-X012 X014`).

**Architecture:** One pass, scope by scope in depth order, over data that P3 already parsed: `ScopeTable` (ids, parents, depth, membership) -> `KindTable` -> declared `Item`s -> `use` edges and code `def` sites -> `[[allow]]` grants -> raw overrides -> per scope: evaluate each override against the working view (policy, tighten, grants, expiry, approval, overlap, stale) and fold it into the memoised `EffectiveView`. `Registry` owns the result and every accessor takes `&self` (R-ARCH-061). Nothing here reads a file, a clock, an environment variable or the working directory; the only time input is `ResolveInput.as_of`. Files: `registry/{mod,scope,item,ovr,grant,view,ledger,resolve}.rs` exactly as contract Section 3; helper types stay `pub(crate)` unless the contract or a Section "P4 additions" entry below exports them.

**Tech Stack:** Rust edition 2024, MSRV 1.88, resolver 3. Crates from contract Section 2 only: `fairlead-syntax` (path), `globset` 0.4.20 (applies globs), `jiff` 0.2.38 (dates), `blake3` through `fairlead_core::hash` only, `serde` not needed here (the registry is not cached). Test-only: `proptest` (UNVERIFIED version, contract Section 2), nothing else.

**Spec:** [10-scopes-and-inheritance.md](../../10-scopes-and-inheritance.md) (whole chapter), [11-files-and-classes.md](../../11-files-and-classes.md) R-FILE-037/043/052, [12-directive-syntax.md](../../12-directive-syntax.md) R-SYN-011/013/024-026/033 and the schema index, [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) R-CHK-006/025/026/053 and the HN-R/HN-X/HN-C/HN-D/HN-S rows, [40-agent-integration-and-trust.md](../../40-agent-integration-and-trust.md) R-AGT-012, [50-architecture.md](../../50-architecture.md) Section 4.4 and 5, [decisions.md](../../decisions.md) D-02, D-10, D-11, D-12, D-34, D-37, D-49, D-51, D-54, [01-contract.md](01-contract.md) Sections 3, 4.6, 4.8, 5 (S6), 7, 8, 9.

---

## Global Constraints

These apply to every task. A task that violates one is wrong even if its tests pass.

1. **Pure functions, no I/O.** Nothing under `crates/fairlead-core/src/registry/` opens a file, reads an environment variable, calls `SystemTime::now`/`Instant::now`, spawns a thread or touches git. All inputs arrive through `ResolveInput`.
2. **Deterministic order.** Every collection that reaches an accessor or a diagnostic list is a `BTreeMap`/`BTreeSet` or a `Vec` sorted by an explicit total key. `HashMap`/`HashSet` are lint-forbidden. The fold order is the one of R-ARCH-040: per scope in chain order root to nearest, grants, then declarations, then overrides; within each bytewise by ID then source path.
3. **`BTreeMap`/sorted `Vec` only.** No iteration order of any other container reaches output (R-ARCH-060).
4. **MSRV 1.88.** No API newer than 1.88. Do not use `let`-chains (`if let ... && ...`), even though edition 2024 admits them in later compilers; nest instead.
5. **`#![forbid(unsafe_code)]`** is the crate attribute of `fairlead-core`; do not add `unsafe`.
6. **Invalid lines are inert plus a diagnostic.** An invalid `ovr`, `allow`, `req`, `def` or control key never aborts resolution and never aborts another scope (R-ARCH-041, R-ARCH-104, R-SCOPE-038). A malformed line is never silently dropped: it is either represented (`Override` with an `Inert` state) or reported with a `Diagnostic`.
7. **Resolution is independent of the working directory.** `ResolveInput` has no cwd, `Root.source` and `Root.dir` are never read for decisions, and no `std::env` access exists (R-SCOPE-020, R-ARCH-043).
8. **Lints.** `unwrap_used`, `expect_used`, `panic`, `indexing_slicing` are denied in library code outside `#[cfg(test)]`; use `get`, `?`, `match`. The single permitted `expect` is on a synthetic slug built from a fixed prefix (reason comment inline). Integration tests under `tests/` start with `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]`.
9. **Names are the contract's.** `ScopeId`, `ScopeState`, `Scope`, `ItemId`, `Origin`, `Site`, `FieldValue`, `Item`, `Edge`, `OvrId`, `OvrOp`, `InertReason`, `OvrState`, `ApprovalStatus`, `Override`, `Allow`, `Req`, `Provenance`, `EffectiveField`, `EffectiveItem`, `EffectiveView`, `LedgerEntry`, `LedgerView`, `Lookup`, `ResolveInput`, `Registry`, `resolve`, `ovr_fingerprint` are never renamed. Additions are listed once, in "P4 additions", and marked in Contract issues.
10. **Diagnostic codes are constants.** Use `crate::diag::codes::HN_*` (P3, contract Section 8 item 10); no `"HN-..."` string literal appears in `registry/` except inside test assertions through the same constants.
11. **Time.** The only clock input is `ResolveInput.as_of`. Tests never read the clock.
12. **Commits.** One commit per task, message `feat(registry): <task title>`; run `cargo fmt --all` and `cargo clippy -p fairlead-core --all-targets -- -D warnings` before committing a task (never skip hooks).

---

## Design decisions of this plan (the contract leaves them open; each is listed again in "Contract issues")

- **P4-1 Declaration versus site.** A `def` is a *declaration* iff its file is Markdown (`.md`), or its carrier is `Frontmatter`, or its path lies inside a managerial folder; any other `def` is a code *site* of an item declared elsewhere (R-SYN-010). `def foreign.<name>` is neither: it is a region marker, never an item, never a site.
- **P4-2 Membership shard line.** `<managerial>/gen/members` (P5 generates it) is read as: skip blank lines and lines starting `#`; the first whitespace-separated token is the scope id; an optional token `path=<dir>` follows. An absent shard means "nothing registered": every non-root scope then raises `HN-C008`.
- **P4-3 `rules.` alias.** The spec writes policy patterns and `[[allow]]` targets as `rules.max-lines` while rule IDs are `rule.max-lines` (kind `rule`). A pattern or target that starts `rules.` is read as `rule.`.
- **P4-4 Two views.** `Registry::view(scope)` folds only overrides that cover the whole scope (`[[ovr]]` with `applies` absent or `**`). Narrower overrides (file-local directives, `applies` globs) are folded by the additive `Registry::view_at(path)`; the contract has no path-level accessor and R-SCOPE-021 requires file-local lines to be overlaid last.
- **P4-5 Policy always applies.** An `ovr` is judged by its target's policy even when it sits in the item's home scope (file-local exceptions in the home scope are the waivers of chapter 12's Rust example).
- **P4-6 Unapproved and over-horizon.** An override that is structurally valid and unexpired stays `OvrState::Effective`; it changes no value while `approval == Missing` or while `exp` lies beyond the horizon (R-AGT-012). P5 derives `HN-X002`/`HN-X006` from the same data.
- **P4-7 Stale pin.** The stale pin is the user-extension key `x-pin=<12 base32>` (R-SYN-009 keeps `x-` keys); `Override.target_hash` is the target pin computed now; a mismatch with a written `x-pin` is `Stale`.
- **P4-8 Floor targets.** `FLOOR_RULES` is a fixed list of rule names that mirror the floor codes of chapter 13; an `ovr` whose target is `rule.<one of them>` is `Inert(FloorRule)` whatever any `allow` says.
- **P4-9 Remove.** `FieldChange` has no remove variant (contract); for `op = Remove` the `set` values are the items to remove, stored as `FieldChange::Replace(FieldValue::List(..))`, and `Override.op` tells the fold how to read them.
- **P4-10 Synthetic ids.** A scope whose control file is unparseable has no readable id; its id is taken from the parent's membership shard, else `unresolved-<8 base32 of the directory>`. A second scope claiming an existing id gets `dup-<8 base32 of its control path>`.
- **P4-11 Dangling targets.** An `ovr` whose target does not resolve produces no `Override`; `HN-R001` for it is P5's (it re-looks the target up with `Registry::resolve_target`).

## P4 additions to the public API (not in contract Section 4.8; additive, flagged in Contract issues)

`Registry::view_at(&RepoPath) -> EffectiveView`, `Registry::resolve_target(&ScopeId, &str) -> Option<(ItemId, Option<String>)>`, `Registry::glob_matches(&ScopeId, &str, &RepoPath) -> bool`, `Registry::proposals() -> &[Proposal]`, `Registry::nearest_file_item(&ScopeId, &str) -> Option<&ItemId>`, `registry::scope::{parse_members, MemberLine, scope_eligibility, EligibilityHint, promote_model, demote_model, PromotePlan, DemotePlan, Tombstone}`, `registry::ovr::{hoist_candidates, HoistGroup, max_horizon_days, FLOOR_RULES}`, `registry::view::{view_signature}`, `registry::ledger::{LineClass, Propagation, propagation}`, `registry::grant::Proposal`, `registry::item::home_mismatches`.

## Assumed upstream API (P2 and P3 types this plan calls; each use is marked `Verify:` where it first appears)

| Upstream item | Assumed shape | Used for |
|---|---|---|
| `fairlead_syntax::ids::{ScopeSlug, Kind, Name}` | `Clone + Ord + Eq`, `ScopeSlug::parse(&str) -> Result<_, IdError>`, `as_str(&self) -> &str` on all three | ids, sorting |
| `fairlead_syntax::ids::Id` | pub fields `qualifier: IdQualifier`, `kind`, `name`; `Id::parse`; `Display`; `Clone + Eq` | targets, lookups |
| `fairlead_syntax::directive::{Directive, Verb, Carrier, Extent, Arg, ArgOp, ArgValue, known_keys}` | exactly as contract 4.2; `Verb`, `Carrier`, `Extent` derive `Clone, Copy, PartialEq, Eq` | extraction records |
| `fairlead_syntax::extract::render_directive_body` | contract 4.2 | stable text of a directive for `Diagnostic.text` and hoisting |
| `fairlead_core::path::RepoPath` | `new(&str)`, `as_str`, `extension`, `Ord`; `RepoPath::new("")` is the root directory | scope dirs |
| `fairlead_core::hash::Fingerprint` | `Clone + Ord + Eq`, `Fingerprint::from_hash(&ContentHash) -> Fingerprint` (first 12 base32 chars), `as_str(&self) -> &str`, `impl FromStr` | `OvrId`; `x-pin`; `base=` |
| `fairlead_core::hash::{hash_raw, base32}` | contract 4.3 | digests, synthetic slugs, pins |
| `fairlead_core::config::*` | contract 4.6 incl. `Layout.r#gen` (see Contract issues: `gen` is a keyword in edition 2024) | control data |
| `fairlead_core::diag::{Diagnostic, Location, DiagCode, codes::HN_*}` | contract 4.9 and Section 8 item 10 | diagnostics |
| `fairlead_core::facts::{Facts, GitFacts}`, `fairlead_core::manifest::ManifestHints`, `fairlead_core::config::ApprovalsFile` | contract 4.4, 4.6, all fields `pub` | shards, approvals, sparse |
| `fairlead_core::extract::{extract_file, ExtractPlan, StyleKinds, Corpus}`, `classify::ClassifyRules`, `walk::WalkEntry` | contract 4.4, 4.5; `StyleKinds: Default` | test kit only |

---

## File Structure (P4 scope)

```text
crates/fairlead-core/src/registry/
  mod.rs      Registry, accessors, Dg diagnostic builder, re-exports, P4 additions on Registry
  scope.rs    ScopeId, ScopeState, Scope, ScopeTable, build_scope_table, parse_members, eligibility, promote/demote model
  item.rs     ItemId, Origin, Site, FieldValue, Item, Edge, Lookup, KindTable, Index, collect_items, collect_refs, lookup
  ovr.rs      OvrId, OvrOp, InertReason, OvrState, ApprovalStatus, FieldChange, Override, RawOvr, evaluate_scope, hoisting, fingerprint
  grant.rs    Allow, Req, Proposal, collect_grants, collect_requests, proposals
  view.rs     Provenance, EffectiveField, EffectiveItem, EffectiveView, build_views, view_at, view_signature, glob matching
  ledger.rs   LedgerEntry, LedgerView, ledger(), LineClass/propagation table
  resolve.rs  ResolveInput, resolve() sequencing
crates/fairlead-core/tests/
  support/mod.rs            Kit (in-memory repository builder), helpers
  registry_scopes.rs        Tasks 2, 3
  registry_items.rs         Tasks 4 to 7
  registry_overrides.rs     Tasks 8 to 11
  registry_grants.rs        Task 12
  registry_views.rs         Tasks 13, 14, 15
  registry_restructure.rs   Task 16
  prop_resolution_order_independent.rs   Task 17
  prop_promote_demote_roundtrip.rs       Task 16
  registry_scenario.rs      Task 18
```

---

### Task 1: Contract types, `Registry` shell and the in-memory test kit

**Files:**
- Create: `crates/fairlead-core/src/registry/mod.rs`, `scope.rs`, `item.rs`, `ovr.rs`, `grant.rs`, `view.rs`, `ledger.rs`, `resolve.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (add `pub mod registry;` if P3 has not)
- Create: `crates/fairlead-core/tests/support/mod.rs`, `crates/fairlead-core/tests/registry_scopes.rs`

**Interfaces:**

Consumes (from P2/P3, contract 4.1, 4.3, 4.4, 4.6): `ScopeSlug`, `Kind`, `Name`, `Id`, `IdQualifier`, `Policy`, `Owner`, `Location`, `Diagnostic`, `RepoPath`, `ContentHash`, `Fingerprint`, `AsOf`, `Root`, `ControlSet`, `ControlFile`, `Corpus`, `Facts`, `Extent`, `Span`.

Produces (verbatim from contract 4.8): every type listed in Global Constraints item 9, as plain data. This task adds no behaviour; later tasks fill the fields.

- [ ] **Step 1: Write the failing test** `crates/fairlead-core/tests/registry_scopes.rs`

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::registry::{
    EffectiveView, LedgerView, Lookup, Override, Registry, Scope, ScopeId, ScopeState,
};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn r_arch_061__registry_types_are_send_sync_plain_data() {
    assert_send_sync::<Registry>();
    assert_send_sync::<EffectiveView>();
    assert_send_sync::<LedgerView>();
    assert_send_sync::<Override>();
    assert_send_sync::<Scope>();
    assert_send_sync::<Lookup>();
    let _ = |s: &ScopeState, i: &ScopeId| (s.clone(), i.clone());
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core r_arch_061`
Expected: FAIL to compile, `unresolved import fairlead_core::registry` (the module does not exist yet) or, if P3 added an empty module, `no EffectiveView in registry`.

- [ ] **Step 3: Create the type files.** Copy the contract types verbatim; the following is the complete content.

`crates/fairlead-core/src/registry/scope.rs` (types only for now):

```rust
//! Scope tree: stored ids, parents, depth, membership (R-SCOPE-001..005, 054).
use std::sync::Arc;

use fairlead_syntax::ids::ScopeSlug;

use crate::config::{ControlFile, Owner};
use crate::diag::Diagnostic;
use crate::path::RepoPath;

/// Identity is the id, never the path (R-SCOPE-002).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ScopeId(pub ScopeSlug);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScopeState {
    Resolved,
    Unresolved { reason: Vec<Diagnostic> },
    AbsentFromTree,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scope {
    pub id: ScopeId,
    pub dir: RepoPath,
    pub parent: Option<ScopeId>,
    pub depth: u32,
    pub control: Option<Arc<ControlFile>>,
    pub state: ScopeState,
    pub owners: Vec<Owner>,
}
```

`crates/fairlead-core/src/registry/item.rs` (types only):

```rust
//! Items, sites, edges, lookups (R-SCOPE-023..026, R-ARCH-062).
use std::collections::BTreeMap;

use fairlead_syntax::directive::Extent;
use fairlead_syntax::diag::Span;
use fairlead_syntax::ids::{Id, IdQualifier, Kind, Name};

use crate::config::Policy;
use crate::path::RepoPath;
use crate::registry::scope::ScopeId;

/// Fully qualified item identity (home scope + kind + name). `Id` is what was WRITTEN.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ItemId {
    pub scope: ScopeId,
    pub kind: Kind,
    pub name: Name,
}

impl ItemId {
    /// Qualified form `scope:kind.name`.
    pub fn to_id(&self) -> Id {
        Id {
            qualifier: IdQualifier::Scope(self.scope.0.clone()),
            kind: self.kind.clone(),
            name: self.name.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Origin {
    ControlTable { control: RepoPath },
    DocDef { path: RepoPath, span: Span },
}

/// A code `def` site (R-SYN-010).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Site {
    pub path: RepoPath,
    pub span: Span,
    pub extent: Extent,
    pub scope: ScopeId,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FieldValue {
    Scalar(String),
    List(Vec<String>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Item {
    pub id: ItemId,
    pub origin: Origin,
    pub policy: Policy,
    pub sum: Option<String>,
    pub fields: BTreeMap<String, FieldValue>,
}

/// `use` edge (R-SCOPE-060).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Edge {
    pub from: Site,
    pub to: Id,
    pub sym: Option<String>,
    pub resolved: Option<ItemId>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Lookup {
    Found(ItemId),
    Ambiguous(Vec<ItemId>),
    Missing { nearest: Vec<Id> },
}
```

`crates/fairlead-core/src/registry/ovr.rs` (types only; the fingerprint arrives in Task 8):

```rust
//! Overrides, their states and fingerprints (R-SCOPE-031..034, 040, 045..047, 064).
use std::collections::BTreeMap;

use crate::diag::Location;
use crate::hash::{ContentHash, Fingerprint};
use crate::registry::item::{FieldValue, ItemId};
use crate::registry::scope::ScopeId;

/// Fingerprint of (target, scope, why, owner, exp) (R-CHK-026).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct OvrId(pub Fingerprint);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OvrOp {
    Loosen,
    Tighten,
    Set,
    Replace,
    Add,
    Remove,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum InertReason {
    SealedNoAllow,
    Incomplete,
    FloorRule,
    NonDelegable,
    UnknownField,
    DuplicateOvr,
    VersionExpiry,
    OutsideSubtree,
    UnknownOwner,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OvrState {
    Effective,
    Inert(InertReason),
    Expired,
    Stale,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ApprovalStatus {
    NotRequired,
    Approved { renewal: u32 },
    Missing,
}

/// R-SYN-013. For `op = Remove` the `Replace` payload lists the values to remove (P4-9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FieldChange {
    Replace(FieldValue),
    Append(Vec<String>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Override {
    pub id: OvrId,
    pub target: ItemId,
    pub field: Option<String>,
    pub op: OvrOp,
    pub set: BTreeMap<String, FieldChange>,
    pub why: Option<String>,
    pub owner: Option<String>,
    pub exp: Option<jiff::civil::Date>,
    pub base: Option<OvrId>,
    pub declared_in: ScopeId,
    pub applies: Option<String>,
    pub at: Location,
    pub state: OvrState,
    pub approval: ApprovalStatus,
    pub target_hash: Option<ContentHash>,
}
```

`crates/fairlead-core/src/registry/grant.rs`:

```rust
//! `[[allow]]` grants and `[[req]]` escalations (R-SCOPE-036, 053, 058).
use crate::diag::Location;
use crate::registry::item::ItemId;
use crate::registry::scope::ScopeId;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Allow {
    pub id: String,
    pub granting: ScopeId,
    pub target: ItemId,
    pub field: Option<String>,
    pub for_scopes: Vec<ScopeId>,
    pub why: String,
    pub exp: jiff::civil::Date,
    /// authored in the target's home scope (R-SCOPE-036)
    pub valid: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Req {
    pub from: ScopeId,
    pub to: ScopeId,
    pub target: String,
    pub set: Option<String>,
    pub why: String,
    pub blocking: bool,
    pub at: Location,
}
```

`crates/fairlead-core/src/registry/view.rs`:

```rust
//! Effective views with provenance (R-SCOPE-021, R-ARCH-040, R-ARCH-042).
use std::collections::BTreeMap;

use crate::diag::Location;
use crate::hash::ContentHash;
use crate::config::Policy;
use crate::registry::item::{FieldValue, ItemId};
use crate::registry::ovr::OvrId;
use crate::registry::scope::ScopeId;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Provenance {
    pub home: ScopeId,
    pub declared_at: Location,
    pub applied: Vec<OvrId>,
    pub grants: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectiveField {
    pub value: FieldValue,
    pub provenance: Provenance,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectiveItem {
    pub id: ItemId,
    pub policy: Policy,
    pub fields: BTreeMap<String, EffectiveField>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectiveView {
    pub scope: ScopeId,
    pub chain: Vec<ScopeId>,
    pub items: BTreeMap<ItemId, EffectiveItem>,
    pub authored_digest: ContentHash,
}
```

`crates/fairlead-core/src/registry/ledger.rs`:

```rust
//! Exception ledger, computed on read (R-SCOPE-043, 044).
use crate::registry::item::ItemId;
use crate::registry::ovr::{OvrId, OvrState};
use crate::registry::scope::ScopeId;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LedgerEntry {
    pub ovr: OvrId,
    pub target: ItemId,
    pub home: ScopeId,
    pub varied_in: ScopeId,
    pub depth: u32,
    pub why: Option<String>,
    pub owner: Option<String>,
    pub exp: Option<jiff::civil::Date>,
    pub state: OvrState,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LedgerView {
    pub for_scope: ScopeId,
    pub entries: Vec<LedgerEntry>,
}
```

`crates/fairlead-core/src/registry/resolve.rs` (input type only):

```rust
//! Seam S6: `resolve(&ResolveInput) -> Registry` (contract Section 5).
use crate::discover::Root;
use crate::config::ControlSet;
use crate::extract::Corpus;
use crate::facts::Facts;
use crate::time::AsOf;

pub struct ResolveInput<'a> {
    pub root: &'a Root,
    pub controls: &'a ControlSet,
    pub corpus: &'a Corpus,
    /// approvals and gen/members shards on disk (HN-C007, HN-C008)
    pub facts: &'a Facts,
    pub as_of: AsOf,
}
```

`crates/fairlead-core/src/registry/mod.rs`:

```rust
//! Registry: scopes, items, overrides, grants, views, ledger (P4). Pure and immutable (R-ARCH-061).
pub mod grant;
pub mod item;
pub mod ledger;
pub mod ovr;
pub mod resolve;
pub mod scope;
pub mod view;

use std::collections::BTreeMap;

use crate::diag::Diagnostic;
use crate::time::AsOf;

pub use grant::{Allow, Req};
pub use item::{Edge, FieldValue, Item, ItemId, Lookup, Origin, Site};
pub use ledger::{LedgerEntry, LedgerView};
pub use ovr::{
    ApprovalStatus, FieldChange, InertReason, Override, OvrId, OvrOp, OvrState,
};
pub use resolve::ResolveInput;
pub use scope::{Scope, ScopeId, ScopeState};
pub use view::{EffectiveField, EffectiveItem, EffectiveView, Provenance};

/// Immutable result of resolution. Every accessor takes `&self` (R-ARCH-061).
#[derive(Clone, Debug)]
pub struct Registry {
    pub(crate) root: ScopeId,
    pub(crate) scopes: BTreeMap<ScopeId, Scope>,
    pub(crate) items: BTreeMap<ItemId, Item>,
    pub(crate) overrides: Vec<Override>,
    pub(crate) grants: Vec<Allow>,
    pub(crate) requests: Vec<Req>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) sites: BTreeMap<ItemId, Vec<Site>>,
    pub(crate) views: BTreeMap<ScopeId, EffectiveView>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) as_of: AsOf,
    pub(crate) empty_view: EffectiveView,
}
```

Add `pub mod registry;` to `crates/fairlead-core/src/lib.rs`.

- [ ] **Step 4: Create the test kit** `crates/fairlead-core/tests/support/mod.rs`. It builds the whole upstream input in memory with `parse_control` and `extract_file`; it is the only place that knows how `Facts`, `Root` and `ExtractPlan` are assembled, so an upstream rename is a one-file fix.

```rust
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use fairlead_core::classify::ClassifyRules;
use fairlead_core::config::{parse_control, Approval, ApprovalsFile, ControlSet};
use fairlead_core::discover::{Edition, Pin, Root, RootSource};
use fairlead_core::extract::{extract_file, Corpus, ExtractPlan, StyleKinds};
use fairlead_core::facts::{Facts, GitFacts};
use fairlead_core::manifest::ManifestHints;
use fairlead_core::hash::{hash_raw, Fingerprint};
use fairlead_core::path::RepoPath;
use fairlead_core::registry::{resolve, ItemId, Registry, ResolveInput, ScopeId};
use fairlead_core::time::AsOf;
use fairlead_core::walk::WalkEntry;
use fairlead_syntax::delimiters::CommentTable;
use fairlead_syntax::extract::ExtractConfig;
use fairlead_syntax::ids::{Id, ScopeSlug};

pub fn sid(s: &str) -> ScopeId {
    ScopeId(ScopeSlug::parse(s).unwrap())
}

/// `iid("web", "cmp.button")` is the item `web:cmp.button`.
pub fn iid(scope: &str, kind_name: &str) -> ItemId {
    let id = Id::parse(kind_name).unwrap();
    ItemId { scope: sid(scope), kind: id.kind, name: id.name }
}

pub fn date(s: &str) -> jiff::civil::Date {
    s.parse().unwrap()
}

pub fn path(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}

/// Root control file: header, owners, two kinds, the example policies of chapter 10.
pub fn root_toml(extra: &str) -> String {
    format!(
        "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n\
         [settings]\nfolder = \"fairlead\"\nowners = [\"@platform\", \"@andy\"]\n\n\
         [kinds.tok]\ntitle = \"design token\"\nshape = \"doc\"\n\
         [kinds.cmp]\ntitle = \"component\"\nshape = \"either\"\n\n\
         [policies]\n\"rules.*\" = \"sealed\"\n\"tok.*\" = \"needs-exception\"\n\"cmp.*\" = \"open\"\n\n{extra}\n"
    )
}

pub fn child_toml(id: &str, extra: &str) -> String {
    format!("[fairlead]\nscope = \"{id}\"\n\n{extra}\n")
}

/// A `pattern` rule table; `waivable` makes it needs-exception (R-CHK-006).
pub fn rule(name: &str, sev: &str, stage: &str, waivable: bool) -> String {
    format!(
        "[rules.{name}]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"m\"\nsev = \"{sev}\"\n\
         stage = \"{stage}\"\nglob = [\"**/*.rs\"]\nwaivable = {waivable}\nwhy = \"intent\"\n\n"
    )
}

/// `[[ovr]]` table. `extra` holds the field assignments, one `k = v` per line.
/// Verify: if P3 parses field assignments from an `[ovr.set]` sub-table instead of inline keys,
/// change only this helper.
pub fn ovr_table(target: &str, extra: &str) -> String {
    format!("[[ovr]]\ntarget = \"{target}\"\n{extra}\n\n")
}

/// A Markdown doc whose frontmatter carries directive bodies (R-SYN-036).
pub fn doc(directives: &[&str]) -> String {
    let mut s = String::from("+++\ndirectives = [\n");
    for d in directives {
        s.push_str(&format!("  '{d}',\n"));
    }
    s.push_str("]\n+++\n\n# doc\n");
    s
}

pub struct Kit {
    files: BTreeMap<String, String>,
    scopes: Vec<(String, String)>, // (dir, id)
    as_of: String,
    approvals: Vec<Approval>,
    members: bool,
    search_source: bool,
    sparse_absent: Vec<String>,
}

impl Kit {
    pub fn new(root_control: &str) -> Kit {
        let mut files = BTreeMap::new();
        files.insert("fairlead/control.toml".to_string(), root_control.to_string());
        Kit {
            files,
            scopes: vec![],
            as_of: "2026-10-09".to_string(),
            approvals: vec![],
            members: true,
            search_source: false,
            sparse_absent: vec![],
        }
    }
    /// A child scope at `dir` with stored id `id` and control text `control`.
    pub fn scope(mut self, dir: &str, id: &str, control: &str) -> Kit {
        self.files.insert(format!("{dir}/fairlead/control.toml"), control.to_string());
        self.scopes.push((dir.to_string(), id.to_string()));
        self
    }
    pub fn file(mut self, p: &str, text: &str) -> Kit {
        self.files.insert(p.to_string(), text.to_string());
        self
    }
    pub fn as_of(mut self, d: &str) -> Kit {
        self.as_of = d.to_string();
        self
    }
    pub fn no_members(mut self) -> Kit {
        self.members = false;
        self
    }
    pub fn search_source(mut self) -> Kit {
        self.search_source = true;
        self
    }
    pub fn sparse_absent(mut self, dir: &str) -> Kit {
        self.sparse_absent.push(dir.to_string());
        self
    }
    /// Record an approval for an override fingerprint.
    pub fn approve(mut self, fp: Fingerprint, target: &ItemId, renewal: u32) -> Kit {
        self.approvals.push(Approval {
            fp,
            target: target.to_id(),
            scope: target.scope.0.clone(),
            approver: "@andy".to_string(),
            date: date("2026-10-01"),
            renewal,
        });
        self
    }

    fn members_shards(&self) -> BTreeMap<RepoPath, std::sync::Arc<[u8]>> {
        let mut out: BTreeMap<RepoPath, std::sync::Arc<[u8]>> = BTreeMap::new();
        if !self.members {
            return out;
        }
        let dirs: Vec<&str> = self.scopes.iter().map(|(d, _)| d.as_str()).collect();
        let mut by_parent: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (dir, id) in &self.scopes {
            let mut parent = String::new();
            let mut cur = dir.as_str();
            while let Some(i) = cur.rfind('/') {
                cur = &cur[..i];
                if dirs.contains(&cur) {
                    parent = cur.to_string();
                    break;
                }
            }
            by_parent.entry(parent).or_default().push(format!("{id} path={dir}"));
        }
        for (parent, mut lines) in by_parent {
            lines.sort();
            let p = if parent.is_empty() {
                "fairlead/gen/members".to_string()
            } else {
                format!("{parent}/fairlead/gen/members")
            };
            let text = format!("# scope x edition=1 fairlead=0.1.0 body=0\n{}\n", lines.join("\n"));
            out.insert(path(&p), std::sync::Arc::from(text.into_bytes().into_boxed_slice()));
        }
        out
    }

    pub fn resolve(&self) -> Registry {
        let root = Root {
            dir: Utf8PathBuf::from("/repo"),
            folder: "fairlead".to_string(),
            source: if self.search_source { RootSource::Search } else { RootSource::Flag },
            edition: Edition(1),
            pin: Pin { version: None, edition: Edition(1) },
            config_hash: hash_raw(b""),
        };
        let mut controls = ControlSet { files: BTreeMap::new() };
        for (p, text) in &self.files {
            if p.ends_with("fairlead/control.toml") {
                let rp = path(p);
                let is_root_dir = p == "fairlead/control.toml";
                controls.files.insert(rp.clone(), parse_control(&rp, text.as_bytes(), is_root_dir));
            }
        }
        let plan = ExtractPlan {
            syntax: ExtractConfig {
                marker: "fairlead".to_string(),
                comments: CommentTable::builtin(),
                header_skip: vec![],
                max_file: 2_000_000,
                max_directives: 500,
            },
            classify: ClassifyRules::from_controls(&root, &controls),
            styles: StyleKinds::default(), // Verify: P3 derives Default
            leak_properties: vec![],
        };
        let mut corpus = Corpus { records: BTreeMap::new() };
        for (p, text) in &self.files {
            if p.ends_with("fairlead/control.toml") {
                continue;
            }
            let entry = WalkEntry { path: path(p), size: text.len() as u64, mtime_ns: 0 };
            corpus.records.insert(path(p), extract_file(&entry, text.as_bytes(), &plan));
        }
        let facts = Facts {
            generated_on_disk: self.members_shards(),
            agent_files: BTreeMap::new(),
            codeowners: None,
            vendor_hashes: BTreeMap::new(),
            vendor_lock: BTreeMap::new(),
            approvals: ApprovalsFile { approvals: self.approvals.clone() },
            git: GitFacts {
                available: false,
                changed: None,
                sparse_absent: self.sparse_absent.iter().map(|d| path(d)).collect(),
                unreadable: None,
            },
            manifest_hints: ManifestHints {
                workspace_members: BTreeSet::new(),
                default_excludes: vec![],
            },
        };
        resolve(&ResolveInput {
            root: &root,
            controls: &controls,
            corpus: &corpus,
            facts: &facts,
            as_of: AsOf::parse(&self.as_of).unwrap(),
        })
    }
}
```

`resolve` does not exist yet; the kit compiles after Task 2. To keep this task green, add to `registry/resolve.rs` a temporary body and export it, replaced in Task 2:

```rust
use crate::registry::Registry;

pub fn resolve(_input: &ResolveInput<'_>) -> Registry {
    // Replaced in Task 2.
    Registry::__placeholder()
}
```

and in `mod.rs`: `pub use resolve::resolve;` and

```rust
impl Registry {
    pub(crate) fn __placeholder() -> Registry {
        unreachable!("replaced in Task 2")
    }
}
```

Note: `unreachable!` is a panic-family macro; this stub exists for one task only and is deleted in Task 2 Step 3. (`panic` lint: mark `#[allow(clippy::panic)] // removed in Task 2`.)

- [ ] **Step 5: Run, expect PASS**

Run: `cargo test -p fairlead-core r_arch_061`
Expected: PASS (`r_arch_061__registry_types_are_send_sync_plain_data`). Fix any `Send`/`Sync` failure by removing `Rc`/`RefCell` (none are used).

- [ ] **Step 6: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): contract types, Registry shell and test kit"
```

---

### Task 2: Scope table, stored ids, root, membership (R-SCOPE-001, 002, 003, 005 duplicate id, 015)

**Files:**
- Modify: `crates/fairlead-core/src/registry/scope.rs`, `registry/mod.rs`, `registry/view.rs`, `registry/resolve.rs`
- Modify: `crates/fairlead-core/tests/registry_scopes.rs`

**Interfaces:**

Consumes: `ControlSet`, `ControlFile.{fairlead.scope, dir, settings.owners, layout.r#gen}`, `ConfigFailure.{path,diags}`, `Root.folder`, `Facts.generated_on_disk`, `codes::HN_C001`, `codes::HN_C007`.

Produces (contract 4.8, exact):
```rust
impl Registry {
    pub fn root_scope(&self) -> &ScopeId;
    pub fn scopes(&self) -> impl Iterator<Item = &Scope>;      // sorted by id
    pub fn scope(&self, id: &ScopeId) -> Option<&Scope>;
    pub fn scope_of(&self, path: &RepoPath) -> Option<&ScopeId>; // nearest ancestor-or-self scope (R-SCOPE-003)
    pub fn chain(&self, id: &ScopeId) -> Vec<&ScopeId>;          // root ..= id
    pub fn diagnostics(&self) -> &[Diagnostic];
    pub fn as_of(&self) -> AsOf;
    pub fn view(&self, scope: &ScopeId) -> &EffectiveView;
}
```
Internal, shared by every later task:
```rust
pub(crate) struct ScopeTable { root: ScopeId, scopes: BTreeMap<ScopeId, Scope>, by_dir: BTreeMap<String, ScopeId> }
pub(crate) fn build_scope_table(root: &Root, controls: &ControlSet, facts: &Facts, diags: &mut Vec<Diagnostic>) -> ScopeTable;
pub(crate) struct Dg(Diagnostic);   // diagnostic builder: Dg::new(code, sev, &path, line, col).scope(&id).msg(..).hint(..).text(..).done()
```

- [ ] **Step 1: Write the failing tests.** Append to `crates/fairlead-core/tests/registry_scopes.rs`:

```rust
use fairlead_core::diag::codes::HN_C007;
use fairlead_core::registry::ScopeState;
use support::*;

fn two_level() -> Kit {
    Kit::new(&root_toml(""))
        .scope("apps/web", "web-ui", &child_toml("web-ui", ""))
        .scope("apps/web/inner", "inner", &child_toml("inner", ""))
}

#[test]
fn r_scope_001__scope_id_is_the_stored_slug_not_the_path() {
    let reg = two_level().resolve();
    let web = reg.scope(&sid("web-ui")).unwrap();
    assert_eq!(web.dir.as_str(), "apps/web");
    assert_eq!(web.parent, Some(sid("root")));
    assert_eq!(web.depth, 1);
    let inner = reg.scope(&sid("inner")).unwrap();
    assert_eq!(inner.parent, Some(sid("web-ui")));
    assert_eq!(inner.depth, 2);
    let ids: Vec<String> = reg.scopes().map(|s| s.id.0.as_str().to_string()).collect();
    assert_eq!(ids, vec!["inner", "root", "web-ui"]); // sorted by id, not by path
    assert_eq!(reg.root_scope(), &sid("root"));
}

#[test]
fn r_scope_002__identity_survives_a_directory_move() {
    let a = two_level().resolve();
    let b = Kit::new(&root_toml(""))
        .scope("packages/ui", "web-ui", &child_toml("web-ui", ""))
        .scope("packages/ui/inner", "inner", &child_toml("inner", ""))
        .resolve();
    assert_eq!(a.chain(&sid("inner")), b.chain(&sid("inner")));
    assert_eq!(a.scope(&sid("web-ui")).unwrap().dir.as_str(), "apps/web");
    assert_eq!(b.scope(&sid("web-ui")).unwrap().dir.as_str(), "packages/ui");
    assert!(a.diagnostics().is_empty() && b.diagnostics().is_empty());
}

#[test]
fn r_scope_003__membership_is_nearest_ancestor_or_self() {
    let reg = two_level().resolve();
    let of = |p: &str| reg.scope_of(&path(p)).cloned();
    assert_eq!(of("apps/web/src/a.rs"), Some(sid("web-ui")));
    assert_eq!(of("apps/web/inner/x.rs"), Some(sid("inner")));
    assert_eq!(of("apps/web"), Some(sid("web-ui")));
    assert_eq!(of("apps/web/fairlead/docs/x.md"), Some(sid("web-ui")));
    assert_eq!(of("apps/admin/x.rs"), Some(sid("root")));
    assert_eq!(of("README.md"), Some(sid("root")));
}

#[test]
fn r_scope_005__duplicate_scope_id_is_hn_c007_and_the_loser_is_unresolved() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/a", "same", &child_toml("same", ""))
        .scope("apps/b", "same", &child_toml("same", ""))
        .resolve();
    let c007: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_C007).collect();
    assert_eq!(c007.len(), 1);
    assert_eq!(c007[0].location.path.as_str(), "apps/b/fairlead/control.toml");
    assert_eq!(reg.scope(&sid("same")).unwrap().dir.as_str(), "apps/a");
    let loser = reg.scope_of(&path("apps/b/x.rs")).unwrap();
    assert_ne!(loser, &sid("same"));
    assert!(matches!(reg.scope(loser).unwrap().state, ScopeState::Unresolved { .. }));
}

#[test]
fn r_scope_015__leaf_folder_without_managerial_folder_stays_implicit() {
    let reg = two_level()
        .file("apps/web/src/leaf/a.css", "/* fairlead: use tok.x */\n:root{}\n")
        .resolve();
    assert_eq!(reg.scope_of(&path("apps/web/src/leaf/a.css")), Some(&sid("web-ui")));
    assert!(reg.scopes().all(|s| !s.dir.as_str().ends_with("leaf")));
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core r_scope_00`
Expected: FAIL at runtime, `internal error: entered unreachable code: replaced in Task 2` (the Task 1 placeholder).

- [ ] **Step 3: Implement.** Replace `crates/fairlead-core/src/registry/scope.rs` entirely:

```rust
//! Scope tree: stored ids, parents, depth, membership (R-SCOPE-001..005, 054).
use std::collections::BTreeMap;
use std::sync::Arc;

use fairlead_syntax::diag::Severity;
use fairlead_syntax::ids::ScopeSlug;

use crate::config::{ConfigFailure, ControlFile, ControlSet, Owner};
use crate::diag::{codes, Diagnostic};
use crate::discover::Root;
use crate::facts::Facts;
use crate::hash::{base32, hash_raw};
use crate::path::RepoPath;
use crate::registry::Dg;

/// Identity is the id, never the path (R-SCOPE-002).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ScopeId(pub ScopeSlug);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScopeState {
    Resolved,
    Unresolved { reason: Vec<Diagnostic> },
    AbsentFromTree,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scope {
    pub id: ScopeId,
    pub dir: RepoPath,
    pub parent: Option<ScopeId>,
    pub depth: u32,
    pub control: Option<Arc<ControlFile>>,
    pub state: ScopeState,
    pub owners: Vec<Owner>,
}

/// Directory helpers on normalised repo-relative strings; the root directory is "".
pub(crate) fn parent_dir(dir: &str) -> Option<&str> {
    if dir.is_empty() {
        return None;
    }
    Some(match dir.rfind('/') {
        Some(i) => dir.get(..i).unwrap_or(""),
        None => "",
    })
}

pub(crate) fn dir_depth(dir: &str) -> usize {
    if dir.is_empty() { 0 } else { dir.split('/').count() }
}

pub(crate) fn is_within(path: &str, dir: &str) -> bool {
    dir.is_empty()
        || path == dir
        || path.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/'))
}

/// Owner directory of `<dir>/<folder>/control.toml`.
pub(crate) fn dir_of_control(control_path: &str, folder: &str) -> String {
    let tail = format!("{folder}/control.toml");
    if control_path == tail {
        return String::new();
    }
    match control_path.strip_suffix(&format!("/{tail}")) {
        Some(d) => d.to_string(),
        None => {
            let mut it = control_path.rsplitn(3, '/');
            it.next();
            it.next();
            it.next().unwrap_or("").to_string()
        }
    }
}

#[allow(clippy::expect_used)] // a fixed lowercase prefix plus 8 base32 characters always matches the slug grammar
pub(crate) fn synth_slug(prefix: &str, key: &str) -> ScopeSlug {
    let s = format!("{prefix}-{}", base32(&hash_raw(key.as_bytes()).0, 8));
    ScopeSlug::parse(&s).expect("synthetic slug is valid") // Verify: ScopeSlug::parse accepts "dup-xxxxxxxx"
}

/// The scope tree. Internal; `Registry` exposes it through accessors.
#[derive(Clone, Debug)]
pub(crate) struct ScopeTable {
    pub(crate) root: ScopeId,
    pub(crate) scopes: BTreeMap<ScopeId, Scope>,
    pub(crate) by_dir: BTreeMap<String, ScopeId>,
}

impl ScopeTable {
    pub(crate) fn get(&self, id: &ScopeId) -> Option<&Scope> {
        self.scopes.get(id)
    }

    /// `root ..= id`.
    pub(crate) fn chain_ids(&self, id: &ScopeId) -> Vec<&ScopeId> {
        let mut out = Vec::new();
        let mut cur = self.scopes.get(id);
        while let Some(s) = cur {
            out.push(&s.id);
            cur = s.parent.as_ref().and_then(|p| self.scopes.get(p));
        }
        out.reverse();
        out
    }

    /// Nearest ancestor-or-self scope (R-SCOPE-003). Pure function of the path.
    pub(crate) fn scope_of(&self, path: &RepoPath) -> Option<&ScopeId> {
        let mut cur: &str = path.as_str();
        loop {
            if let Some(id) = self.by_dir.get(cur) {
                return Some(id);
            }
            cur = parent_dir(cur)?;
        }
    }

    pub(crate) fn is_ancestor_or_self(&self, anc: &ScopeId, of: &ScopeId) -> bool {
        self.chain_ids(of).into_iter().any(|s| s == anc)
    }

    pub(crate) fn is_resolved(&self, id: &ScopeId) -> bool {
        matches!(self.scopes.get(id).map(|s| &s.state), Some(ScopeState::Resolved))
    }

    /// Scopes ordered by (depth, directory): parents always precede children.
    pub(crate) fn in_depth_order(&self) -> Vec<&Scope> {
        let mut v: Vec<&Scope> = self.scopes.values().collect();
        v.sort_by(|a, b| (a.depth, a.dir.as_str()).cmp(&(b.depth, b.dir.as_str())));
        v
    }

    pub(crate) fn root_control(&self) -> Option<&ControlFile> {
        self.scopes.get(&self.root).and_then(|s| s.control.as_deref())
    }
}

struct Raw {
    dir: String,
    dir_path: RepoPath,
    control_path: RepoPath,
    res: Result<Arc<ControlFile>, ConfigFailure>,
}

fn nearest_scope_dir(dir: &str, by_dir: &BTreeMap<String, ScopeId>) -> Option<ScopeId> {
    let mut cur = dir;
    while let Some(p) = parent_dir(cur) {
        if let Some(id) = by_dir.get(p) {
            return Some(id.clone());
        }
        cur = p;
    }
    None
}

pub(crate) fn build_scope_table(
    root: &Root,
    controls: &ControlSet,
    facts: &Facts,
    diags: &mut Vec<Diagnostic>,
) -> ScopeTable {
    let folder = root.folder.as_str();
    let mut raws: Vec<Raw> = Vec::new();
    for (cpath, res) in &controls.files {
        match res {
            Ok(cf) => raws.push(Raw {
                dir: cf.dir.as_str().trim_matches('/').to_string(),
                dir_path: cf.dir.clone(),
                control_path: cpath.clone(),
                res: Ok(Arc::new(cf.clone())),
            }),
            Err(f) => {
                let dir = dir_of_control(cpath.as_str(), folder);
                let dir_path = RepoPath::new(&dir).unwrap_or_else(|_| cpath.clone());
                raws.push(Raw { dir, dir_path, control_path: cpath.clone(), res: Err(f.clone()) });
            }
        }
    }
    raws.sort_by(|a, b| {
        (dir_depth(&a.dir), a.dir.as_str()).cmp(&(dir_depth(&b.dir), b.dir.as_str()))
    });

    let mut scopes: BTreeMap<ScopeId, Scope> = BTreeMap::new();
    let mut by_dir: BTreeMap<String, ScopeId> = BTreeMap::new();
    let mut claimed: BTreeMap<ScopeId, RepoPath> = BTreeMap::new();
    for raw in raws {
        let parent = nearest_scope_dir(&raw.dir, &by_dir);
        let (mut id, control, mut state, owners) = match &raw.res {
            Ok(cf) => {
                let owners = if raw.dir.is_empty() { cf.settings.owners.clone() } else { Vec::new() };
                (ScopeId(cf.fairlead.scope.clone()), Some(Arc::clone(cf)), ScopeState::Resolved, owners)
            }
            Err(f) => {
                let from_shard = parent
                    .as_ref()
                    .and_then(|p| member_id(&scopes, p, &raw.dir, folder, facts));
                let id = from_shard.unwrap_or_else(|| ScopeId(synth_slug("unresolved", &raw.dir)));
                (id, None, ScopeState::Unresolved { reason: f.diags.clone() }, Vec::new())
            }
        };
        let first_claim = claimed.get(&id).map(|p| p.as_str().to_string());
        if let Some(first) = first_claim {
            let d = Dg::new(codes::HN_C007, Severity::Error, &raw.control_path, 1, 1)
                .scope(&id)
                .msg(format!("scope id \"{}\" is already declared by {first}", id.0.as_str()))
                .hint("scope ids are unique: change [fairlead] scope in one of the two control files")
                .text(id.0.as_str())
                .done();
            diags.push(d.clone());
            id = ScopeId(synth_slug("dup", raw.control_path.as_str()));
            state = ScopeState::Unresolved { reason: vec![d] };
        } else {
            claimed.insert(id.clone(), raw.control_path.clone());
        }
        if state == ScopeState::Resolved {
            let inherited = parent.as_ref().and_then(|p| scopes.get(p)).map(|s| s.state.clone());
            if let Some(ScopeState::Unresolved { reason }) = inherited {
                state = ScopeState::Unresolved { reason };
            }
        }
        let depth = parent
            .as_ref()
            .and_then(|p| scopes.get(p))
            .map(|s| s.depth + 1)
            .unwrap_or(0);
        by_dir.insert(raw.dir.clone(), id.clone());
        scopes.insert(
            id.clone(),
            Scope { id, dir: raw.dir_path.clone(), parent, depth, control, state, owners },
        );
    }

    let root_id = match by_dir.get("") {
        Some(id) => id.clone(),
        None => synth_root(&mut scopes, &mut by_dir, folder, diags),
    };
    ScopeTable { root: root_id, scopes, by_dir }
}

/// Only reachable when `discover` was bypassed: no control file at the root directory.
fn synth_root(
    scopes: &mut BTreeMap<ScopeId, Scope>,
    by_dir: &mut BTreeMap<String, ScopeId>,
    folder: &str,
    diags: &mut Vec<Diagnostic>,
) -> ScopeId {
    let id = ScopeId(synth_slug("unresolved", ""));
    // Verify: P3's RepoPath::new("") denotes the root directory.
    if let (Ok(cpath), Ok(dir)) = (RepoPath::new(&format!("{folder}/control.toml")), RepoPath::new("")) {
        let d = Dg::new(codes::HN_C001, Severity::Error, &cpath, 1, 1)
            .scope(&id)
            .msg("no root control file (root = true) was found")
            .hint("run `fairlead init` at the repository root")
            .done();
        diags.push(d.clone());
        scopes.insert(
            id.clone(),
            Scope {
                id: id.clone(),
                dir,
                parent: None,
                depth: 0,
                control: None,
                state: ScopeState::Unresolved { reason: vec![d] },
                owners: Vec::new(),
            },
        );
        by_dir.insert(String::new(), id.clone());
    }
    id
}

// --- membership shards (the HN-C008 check arrives in Task 3) -------------------------------

/// One line of a `gen/members` shard (P4-2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberLine {
    pub id: String,
    pub path: Option<String>,
}

pub fn parse_members(bytes: &[u8]) -> Vec<MemberLine> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let id = it.next().unwrap_or("").to_string();
        let path = it.find_map(|t| t.strip_prefix("path=")).map(str::to_string);
        out.push(MemberLine { id, path });
    }
    out
}

fn shard_path(parent: &Scope, folder: &str) -> String {
    let gen_dir = parent
        .control
        .as_ref()
        .map(|c| c.layout.r#gen.clone()) // Verify: contract field `gen` must be spelled r#gen in edition 2024
        .unwrap_or_else(|| "gen".to_string());
    let base = parent.dir.as_str().trim_matches('/');
    if base.is_empty() {
        format!("{folder}/{gen_dir}/members")
    } else {
        format!("{base}/{folder}/{gen_dir}/members")
    }
}

/// `None` when the parent has no shard on disk.
pub(crate) fn members_of(parent: &Scope, folder: &str, facts: &Facts) -> Option<Vec<MemberLine>> {
    let key = RepoPath::new(&shard_path(parent, folder)).ok()?;
    facts.generated_on_disk.get(&key).map(|b| parse_members(b))
}

fn member_id(
    scopes: &BTreeMap<ScopeId, Scope>,
    parent: &ScopeId,
    dir: &str,
    folder: &str,
    facts: &Facts,
) -> Option<ScopeId> {
    let lines = members_of(scopes.get(parent)?, folder, facts)?;
    let m = lines.into_iter().find(|m| m.path.as_deref() == Some(dir))?;
    ScopeSlug::parse(&m.id).ok().map(ScopeId)
}
```

In `crates/fairlead-core/src/registry/mod.rs` replace the `Registry` struct, delete `__placeholder`, and add the accessors and the builder (keep the module list and re-exports of Task 1; add `pub use resolve::resolve;` and `pub(crate) use scope::ScopeTable;`):

```rust
use fairlead_syntax::diag::{DiagCode, Severity};

use crate::diag::{Diagnostic, Location};
use crate::path::RepoPath;

/// Immutable result of resolution. Every accessor takes `&self` (R-ARCH-061).
#[derive(Clone, Debug)]
pub struct Registry {
    pub(crate) table: ScopeTable,
    pub(crate) items: BTreeMap<ItemId, Item>,
    pub(crate) overrides: Vec<Override>,
    pub(crate) grants: Vec<Allow>,
    pub(crate) requests: Vec<Req>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) sites: BTreeMap<ItemId, Vec<Site>>,
    pub(crate) views: BTreeMap<ScopeId, EffectiveView>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) as_of: AsOf,
    pub(crate) empty_view: EffectiveView,
}

impl Registry {
    pub fn root_scope(&self) -> &ScopeId {
        &self.table.root
    }
    pub fn scopes(&self) -> impl Iterator<Item = &Scope> {
        self.table.scopes.values()
    }
    pub fn scope(&self, id: &ScopeId) -> Option<&Scope> {
        self.table.get(id)
    }
    pub fn scope_of(&self, path: &RepoPath) -> Option<&ScopeId> {
        self.table.scope_of(path)
    }
    pub fn chain(&self, id: &ScopeId) -> Vec<&ScopeId> {
        self.table.chain_ids(id)
    }
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.items.values()
    }
    pub fn item(&self, id: &ItemId) -> Option<&Item> {
        self.items.get(id)
    }
    pub fn view(&self, scope: &ScopeId) -> &EffectiveView {
        self.views.get(scope).unwrap_or(&self.empty_view)
    }
    pub fn overrides(&self) -> &[Override] {
        &self.overrides
    }
    pub fn grants(&self) -> &[Allow] {
        &self.grants
    }
    pub fn requests(&self) -> &[Req] {
        &self.requests
    }
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
    pub fn sites_of(&self, id: &ItemId) -> &[Site] {
        self.sites.get(id).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn as_of(&self) -> AsOf {
        self.as_of
    }
}

/// Diagnostic builder (not part of the contract); keeps call sites readable.
pub(crate) struct Dg(Diagnostic);

impl Dg {
    pub(crate) fn new(code: DiagCode, severity: Severity, path: &RepoPath, line: u32, col: u32) -> Dg {
        Dg(Diagnostic {
            code,
            severity,
            location: Location { path: path.clone(), line, col },
            scope: None,
            rule: None,
            related: None,
            message: String::new(),
            expected: None,
            did_you_mean: None,
            hint: String::new(),
            text: String::new(),
        })
    }
    pub(crate) fn scope(mut self, s: &ScopeId) -> Dg {
        self.0.scope = Some(s.0.clone());
        self
    }
    pub(crate) fn msg(mut self, m: impl Into<String>) -> Dg {
        self.0.message = m.into();
        self
    }
    pub(crate) fn hint(mut self, h: impl Into<String>) -> Dg {
        self.0.hint = h.into();
        self
    }
    pub(crate) fn text(mut self, t: impl Into<String>) -> Dg {
        self.0.text = t.into();
        self
    }
    pub(crate) fn related(mut self, r: impl Into<String>) -> Dg {
        self.0.related = Some(r.into());
        self
    }
    pub(crate) fn expected(mut self, e: impl Into<String>) -> Dg {
        self.0.expected = Some(e.into());
        self
    }
    pub(crate) fn dym(mut self, d: impl Into<String>) -> Dg {
        self.0.did_you_mean = Some(d.into());
        self
    }
    pub(crate) fn done(self) -> Diagnostic {
        self.0
    }
}
```
`AsOf` must be `Copy` (it wraps `jiff::civil::Date`, which is `Copy`); if P3 omitted the derive, return `self.as_of.clone()` instead (Verify).

Append to `crates/fairlead-core/src/registry/view.rs`:

```rust
use crate::hash::hash_raw;
use crate::registry::scope::ScopeTable;

pub(crate) fn empty_view(scope: &ScopeId, table: &ScopeTable) -> EffectiveView {
    EffectiveView {
        scope: scope.clone(),
        chain: table.chain_ids(scope).into_iter().cloned().collect(),
        items: BTreeMap::new(),
        authored_digest: hash_raw(b""),
    }
}

pub(crate) fn build_views(table: &ScopeTable) -> BTreeMap<ScopeId, EffectiveView> {
    table.scopes.keys().map(|id| (id.clone(), empty_view(id, table))).collect()
}
```

Replace the placeholder `resolve` in `crates/fairlead-core/src/registry/resolve.rs`:

```rust
use crate::diag::Diagnostic;
use crate::registry::scope::build_scope_table;
use crate::registry::{view, Registry};

pub fn resolve(input: &ResolveInput<'_>) -> Registry {
    let mut diags: Vec<Diagnostic> = Vec::new();
    let table = build_scope_table(input.root, input.controls, input.facts, &mut diags);
    let views = view::build_views(&table);
    let empty_view = view::empty_view(&table.root, &table);
    Registry {
        table,
        items: Default::default(),
        overrides: Vec::new(),
        grants: Vec::new(),
        requests: Vec::new(),
        edges: Vec::new(),
        sites: Default::default(),
        views,
        diagnostics: diags,
        as_of: input.as_of,
        empty_view,
    }
}
```

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_scope_00 r_scope_015 r_arch_061`
Expected: PASS (6 tests: r_arch_061, r_scope_001, 002, 003, 005, 015).

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): scope table, stored ids, root and membership"
```

---

### Task 3: Registration (HN-C008), fail-closed subtrees, absent scopes, invocation independence (R-SCOPE-005, 054, 055, R-SYN-033, R-ARCH-043)

**Files:**
- Modify: `crates/fairlead-core/src/registry/scope.rs`, `crates/fairlead-core/tests/support/mod.rs`, `crates/fairlead-core/tests/registry_scopes.rs`

**Interfaces:**

Consumes: `members_of` and `parse_members` (Task 2), `Facts.git.sparse_absent`, `ConfigFailure.diags`, `codes::HN_C008`, `codes::HN_C001`.

Produces: no new public signature. Behaviour: a resolved non-root scope absent from its resolved parent's `gen/members` shard (or whose parent has no shard) raises `HN_C008` (error); an unparseable control file makes its scope and every descendant `ScopeState::Unresolved { reason }` (the reason is the root-cause diagnostics); a shard entry whose `path=` lies inside `Facts.git.sparse_absent` and has no control file becomes `ScopeState::AbsentFromTree`.

- [ ] **Step 1: Extend the kit.** In `tests/support/mod.rs` add a method so a test can list a scope in the parent's shard without a control file (sparse checkout):

```rust
    /// Listed in the parent's `gen/members` shard but with no control file in the tree.
    pub fn phantom(mut self, dir: &str, id: &str) -> Kit {
        self.scopes.push((dir.to_string(), id.to_string()));
        self
    }
```

- [ ] **Step 2: Write the failing tests.** Append to `tests/registry_scopes.rs`:

```rust
use fairlead_core::diag::codes::{HN_C001, HN_C008};

#[test]
fn r_scope_005__scope_missing_from_parent_shard_is_hn_c008() {
    let reg = two_level().no_members().resolve();
    let c008: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_C008).collect();
    assert_eq!(c008.len(), 2); // web-ui and inner; the root needs no registration
    assert_eq!(c008[0].location.path.as_str(), "apps/web/fairlead/control.toml");
    assert!(c008[0].message.contains("web-ui"));
    // registered scopes raise nothing
    assert!(two_level().resolve().diagnostics().iter().all(|d| d.code != HN_C008));
}

#[test]
fn r_scope_054__unparseable_control_leaves_the_subtree_unresolved() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/web", "web-ui", "[fairlead\nscope = ")
        .scope("apps/web/inner", "inner", &child_toml("inner", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .resolve();
    let web = reg.scope(&sid("web-ui")).expect("id is recovered from the parent shard");
    match &web.state {
        ScopeState::Unresolved { reason } => assert!(reason.iter().any(|d| d.code == HN_C001)),
        other => panic!("expected Unresolved, got {other:?}"),
    }
    assert!(web.control.is_none());
    assert!(matches!(reg.scope(&sid("inner")).unwrap().state, ScopeState::Unresolved { .. }));
    assert_eq!(reg.scope(&sid("admin")).unwrap().state, ScopeState::Resolved);
    assert_eq!(reg.scope(&sid("root")).unwrap().state, ScopeState::Resolved);
    assert_eq!(reg.scope_of(&path("apps/web/a.rs")), Some(&sid("web-ui"))); // no silent fallback to root
}

#[test]
fn r_scope_054__scope_in_sparse_absent_dir_is_absent_from_tree() {
    let reg = Kit::new(&root_toml(""))
        .phantom("apps/gone", "gone")
        .sparse_absent("apps/gone")
        .resolve();
    let gone = reg.scope(&sid("gone")).unwrap();
    assert_eq!(gone.state, ScopeState::AbsentFromTree);
    assert_eq!(gone.parent, Some(sid("root")));
    assert!(gone.control.is_none());
}

#[test]
fn r_scope_055__diagnostics_do_not_depend_on_how_the_root_was_selected() {
    let build = |k: Kit| k.no_members().resolve();
    let base = || {
        Kit::new(&root_toml(""))
            .scope("apps/web", "web-ui", "[fairlead\nscope = ")
            .scope("apps/web/inner", "inner", &child_toml("inner", ""))
    };
    let a = build(base());
    let b = build(base().search_source());
    assert_eq!(a.diagnostics(), b.diagnostics());
    let ids = |r: &fairlead_core::registry::Registry| r.scopes().map(|s| s.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&a), ids(&b));
}
```

- [ ] **Step 3: Run, expect FAIL**

Run: `cargo test -p fairlead-core -- r_scope_005 r_scope_054 r_scope_055`
Expected: `r_scope_005__scope_missing_from_parent_shard_is_hn_c008` fails (`assertion left == right: 0 vs 2`), `r_scope_054__scope_in_sparse_absent_dir...` fails with `called Option::unwrap() on a None` (no `gone` scope). The unparseable-control and 055 tests already pass (Task 2 implements propagation); keep them as regression tests.

- [ ] **Step 4: Implement.** In `build_scope_table`, just before `by_dir.insert(raw.dir.clone(), id.clone());` insert the registration check:

```rust
        if state == ScopeState::Resolved {
            if let Some(pid) = parent.as_ref() {
                let registered = scopes
                    .get(pid)
                    .filter(|p| p.state == ScopeState::Resolved)
                    .map(|p| {
                        members_of(p, folder, facts)
                            .unwrap_or_default()
                            .iter()
                            .any(|m| m.id == id.0.as_str())
                    });
                if registered == Some(false) {
                    diags.push(
                        Dg::new(codes::HN_C008, Severity::Error, &raw.control_path, 1, 1)
                            .scope(&id)
                            .msg(format!(
                                "scope \"{}\" is not registered in the membership shard of its parent",
                                id.0.as_str()
                            ))
                            .hint("run `fairlead sync` to register it, or remove the stray managerial folder")
                            .text(id.0.as_str())
                            .done(),
                    );
                }
            }
        }
```

After the `for raw in raws` loop and before the `root_id` match add the sparse pass:

```rust
    let absent: Vec<String> = facts.git.sparse_absent.iter().map(|p| p.as_str().to_string()).collect();
    if !absent.is_empty() {
        let parents: Vec<Scope> =
            scopes.values().filter(|s| s.state == ScopeState::Resolved).cloned().collect();
        for p in parents {
            for m in members_of(&p, folder, facts).unwrap_or_default() {
                let (Some(dir), Ok(slug)) = (m.path.clone(), ScopeSlug::parse(&m.id)) else { continue };
                let id = ScopeId(slug);
                if scopes.contains_key(&id) || !absent.iter().any(|a| is_within(&dir, a)) {
                    continue;
                }
                if let Ok(dp) = RepoPath::new(&dir) {
                    by_dir.insert(dir, id.clone());
                    scopes.insert(
                        id.clone(),
                        Scope {
                            id,
                            dir: dp,
                            parent: Some(p.id.clone()),
                            depth: p.depth + 1,
                            control: None,
                            state: ScopeState::AbsentFromTree,
                            owners: Vec::new(),
                        },
                    );
                }
            }
        }
    }
```

Add `Default` use: `Vec<MemberLine>::unwrap_or_default` is std. No other change.

- [ ] **Step 5: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_scope_005 r_scope_054 r_scope_055 r_scope_00`
Expected: PASS. If `r_scope_054__unparseable...` fails on the `HN_C001` assertion, P3 used another code for the lenient-header failure: use the code P3 returns in `ConfigFailure.diags` (Verify) and adjust the assertion only.

- [ ] **Step 6: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): membership registration, fail-closed subtrees, absent scopes"
```

---

### Task 4: Kind table, control-table items, policy defaults, shadowing, item views (R-SCOPE-023, 026, 030, R-SYN-024 redefinition, 033, R-CHK-006, R-ARCH-104)

**Files:**
- Modify: `crates/fairlead-core/src/registry/item.rs`, `view.rs`, `resolve.rs`, `mod.rs`
- Create: `crates/fairlead-core/tests/registry_items.rs`

**Interfaces:**

Consumes: `ControlFile.{kinds, rules, files, policies.by_pattern, path}`, `RuleDecl`, `FileItemDecl`, `Policy`, `Stage`, `Severity`, `Id::parse`, `ScopeTable` (Task 2), `codes::{HN_C003, HN_C001, HN_D001, HN_R005}`.

Produces:
```rust
pub const BUILTIN_KINDS: [&str; 4] = ["rule", "handoff", "foreign", "file"];
pub(crate) struct KindTable { own: BTreeMap<ScopeId, BTreeMap<String, KindDecl>> }
impl KindTable {
    pub(crate) fn build(table: &ScopeTable, diags: &mut Vec<Diagnostic>) -> KindTable;
    pub(crate) fn declared_names(&self, table: &ScopeTable, scope: &ScopeId) -> Vec<String>; // sorted, builtin included
    pub(crate) fn decl(&self, table: &ScopeTable, scope: &ScopeId, kind: &str) -> Option<&KindDecl>;
    pub(crate) fn is_declared(&self, table: &ScopeTable, from: &ScopeId, id: &Id) -> bool;
}
pub(crate) struct Decls { items: BTreeMap<ItemId, Item>, locs: BTreeMap<ItemId, Location> }
pub(crate) struct Cand { kind: Kind, name: Name, origin: Origin, policy: Policy, sum: Option<String>, fields: BTreeMap<String, FieldValue>, loc: Location }
pub(crate) fn collect_items(table: &ScopeTable, diags: &mut Vec<Diagnostic>) -> Decls;   // Task 6 adds a `docs` parameter
pub(crate) fn policy_for(root: Option<&ControlFile>, kind: &str, name: &str, explicit: Option<Policy>, waivable: bool) -> Policy;
pub(crate) fn levenshtein(a: &str, b: &str) -> usize;
pub(crate) fn wild_match(pat: &str, text: &str) -> bool;
```

- [ ] **Step 1: Write the failing tests** `crates/fairlead-core/tests/registry_items.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::config::Policy;
use fairlead_core::diag::codes::{HN_C003, HN_R005};
use fairlead_core::registry::{FieldValue, ScopeState};
use support::*;

/// `extra` of `root_toml` lands inside the `[policies]` table: policy lines first, then new tables.
fn root_with_rules() -> String {
    root_toml(&format!(
        "\"rules.r3\" = \"needs-exception\"\n\n{}{}{}",
        rule("r1", "error", "enforce", false),
        rule("w1", "warn", "observe", true),
        rule("r3", "warn", "warn", false)
    ))
}

#[test]
fn r_scope_030__rules_default_to_sealed_and_patterns_refine() {
    let reg = Kit::new(&root_with_rules()).resolve();
    let policy = |n: &str| reg.item(&iid("root", n)).unwrap().policy;
    assert_eq!(policy("rule.r1"), Policy::Sealed); // "rules.*" = "sealed"
    assert_eq!(policy("rule.r3"), Policy::NeedsException); // exact pattern beats the wildcard
    let r1 = reg.item(&iid("root", "rule.r1")).unwrap();
    assert_eq!(r1.fields["sev"], FieldValue::Scalar("error".into()));
    assert_eq!(r1.fields["stage"], FieldValue::Scalar("enforce".into()));
    assert_eq!(r1.fields["glob"], FieldValue::List(vec!["**/*.rs".into()]));
}

#[test]
fn r_chk_006__waivable_rule_is_needs_exception() {
    let reg = Kit::new(&root_with_rules()).resolve();
    assert_eq!(reg.item(&iid("root", "rule.w1")).unwrap().policy, Policy::NeedsException);
}

#[test]
fn r_syn_024__child_cannot_redefine_an_inherited_kind() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/web", "web", &child_toml("web", "[kinds.cmp]\ntitle = \"mine\"\n[kinds.wdg]\ntitle = \"widget\"\n"))
        .resolve();
    let c003: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_C003).collect();
    assert_eq!(c003.len(), 1);
    assert!(c003[0].message.contains("cmp"));
}

#[test]
fn r_scope_026__ancestor_declaration_wins_over_a_descendant_redefinition_hn_r005() {
    let reg = Kit::new(&root_with_rules())
        .scope("apps/web", "web", &child_toml("web", &rule("r1", "info", "observe", false)))
        .resolve();
    let r005: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_R005).collect();
    assert_eq!(r005.len(), 1);
    assert_eq!(r005[0].related.as_deref(), Some("root:rule.r1"));
    assert!(reg.item(&iid("web", "rule.r1")).is_none());
    assert!(reg.item(&iid("root", "rule.r1")).is_some()); // R-SCOPE-026: the ancestor's declaration stands
}

#[test]
fn r_syn_033__unresolved_scope_has_no_view_and_never_inherits_the_ancestor_rules() {
    let reg = Kit::new(&root_with_rules())
        .scope("apps/web", "web", "[fairlead\nscope = ")
        .resolve();
    assert!(matches!(reg.scope(&sid("web")).unwrap().state, ScopeState::Unresolved { .. }));
    assert!(reg.view(&sid("web")).items.is_empty());
    assert!(!reg.view(&sid("root")).items.is_empty());
}

#[test]
fn r_arch_104__error_in_one_scope_does_not_hide_an_unrelated_scope() {
    let reg = Kit::new(&root_with_rules())
        .scope("apps/web", "web", "[fairlead\nscope = ")
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .resolve();
    let admin = reg.view(&sid("admin"));
    assert!(admin.items.contains_key(&iid("root", "rule.r1")));
    assert_eq!(admin.chain, vec![sid("root"), sid("admin")]);
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core -- r_scope_030 r_chk_006 r_syn_024 r_scope_026 r_syn_033 r_arch_104`
Expected: FAIL: `r_scope_030...` panics on `Option::unwrap()` of `reg.item(..)` (no items are collected yet).

- [ ] **Step 3: Implement.** Append to `registry/item.rs` (merge the imports into the file header: `std::collections::BTreeSet`, `fairlead_syntax::diag::Severity`, `crate::config::{ControlFile, KindDecl, RuleDecl, Stage}`, `crate::diag::{codes, Diagnostic, Location}`, `crate::registry::scope::{Scope, ScopeState, ScopeTable}`, `crate::registry::Dg`):

```rust
pub const BUILTIN_KINDS: [&str; 4] = ["rule", "handoff", "foreign", "file"];

pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev.get(j).copied().unwrap_or(0) + usize::from(ca != *cb);
            let del = prev.get(j + 1).copied().unwrap_or(0) + 1;
            let ins = cur.get(j).copied().unwrap_or(0) + 1;
            cur.push(sub.min(del).min(ins));
        }
        prev = cur;
    }
    prev.last().copied().unwrap_or(0)
}

/// `*` matches any run of characters (including `.`).
pub(crate) fn wild_match(pat: &str, text: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == text;
    }
    let first = parts.first().copied().unwrap_or("");
    let last = parts.last().copied().unwrap_or("");
    if !text.starts_with(first) {
        return false;
    }
    let mut rest = text.get(first.len()..).unwrap_or("");
    let mid_end = parts.len().saturating_sub(1);
    for p in parts.get(1..mid_end).unwrap_or(&[]) {
        match rest.find(p) {
            Some(i) => rest = rest.get(i + p.len()..).unwrap_or(""),
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// P4-3: `rules.x` in a pattern or target means `rule.x`.
pub(crate) fn normalise_rules_alias(s: &str) -> String {
    match s.strip_prefix("rules.") {
        Some(r) => format!("rule.{r}"),
        None => s.to_string(),
    }
}

// --- kinds ------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub(crate) struct KindTable {
    pub(crate) own: BTreeMap<ScopeId, BTreeMap<String, KindDecl>>,
}

impl KindTable {
    pub(crate) fn build(table: &ScopeTable, diags: &mut Vec<Diagnostic>) -> KindTable {
        let mut kt = KindTable::default();
        for scope in table.in_depth_order() {
            let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else {
                continue;
            };
            let inherited: BTreeSet<String> = table
                .chain_ids(&scope.id)
                .into_iter()
                .filter(|s| **s != scope.id)
                .filter_map(|s| kt.own.get(s))
                .flat_map(|m| m.keys().cloned())
                .collect();
            let mut mine = BTreeMap::new();
            for (name, decl) in &cf.kinds {
                if name == "exc" {
                    continue; // rejected at load by P3 (C-6)
                }
                if inherited.contains(name) {
                    diags.push(
                        Dg::new(codes::HN_C003, Severity::Error, &cf.path, 1, 1)
                            .scope(&scope.id)
                            .msg(format!("kind \"{name}\" is inherited and sealed; a child scope cannot redefine it"))
                            .hint("pick a new kind name, or escalate with `fairlead new escalation`")
                            .text(name.as_str())
                            .done(),
                    );
                    continue;
                }
                mine.insert(name.clone(), decl.clone());
            }
            kt.own.insert(scope.id.clone(), mine);
        }
        kt
    }

    fn visible<'a>(&'a self, table: &'a ScopeTable, scope: &ScopeId) -> impl Iterator<Item = (&'a String, &'a KindDecl)> {
        table.chain_ids(scope).into_iter().filter_map(|s| self.own.get(s)).flat_map(|m| m.iter())
    }

    pub(crate) fn declared_names(&self, table: &ScopeTable, scope: &ScopeId) -> Vec<String> {
        let mut v: BTreeSet<String> = BUILTIN_KINDS.iter().map(|s| s.to_string()).collect();
        v.extend(self.visible(table, scope).map(|(k, _)| k.clone()));
        v.into_iter().collect()
    }

    pub(crate) fn decl<'a>(&'a self, table: &'a ScopeTable, scope: &ScopeId, kind: &str) -> Option<&'a KindDecl> {
        self.visible(table, scope).find(|(k, _)| k.as_str() == kind).map(|(_, d)| d)
    }

    /// Kinds are visible along the chain of the referencing scope, and of the named scope for `q:kind.n`.
    pub(crate) fn is_declared(&self, table: &ScopeTable, from: &ScopeId, id: &Id) -> bool {
        let k = id.kind.as_str();
        if BUILTIN_KINDS.contains(&k) {
            return true;
        }
        let mut scopes = vec![from.clone()];
        if let IdQualifier::Scope(q) = &id.qualifier {
            scopes.push(ScopeId(q.clone()));
        }
        scopes.iter().any(|s| self.decl(table, s, k).is_some())
    }
}

// --- declarations -----------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct Cand {
    pub(crate) kind: Kind,
    pub(crate) name: Name,
    pub(crate) origin: Origin,
    pub(crate) policy: Policy,
    pub(crate) sum: Option<String>,
    pub(crate) fields: BTreeMap<String, FieldValue>,
    pub(crate) loc: Location,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Decls {
    pub(crate) items: BTreeMap<ItemId, Item>,
    pub(crate) locs: BTreeMap<ItemId, Location>,
}

fn sev_text(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

fn stage_text(s: Stage) -> &'static str {
    match s {
        Stage::Observe => "observe",
        Stage::Warn => "warn",
        Stage::Enforce => "enforce",
    }
}

/// Exact root pattern, then `waivable`, then wildcard pattern, then the kind default (R-SCOPE-030, R-CHK-006).
pub(crate) fn policy_for(
    root: Option<&ControlFile>,
    kind: &str,
    name: &str,
    explicit: Option<Policy>,
    waivable: bool,
) -> Policy {
    if let Some(p) = explicit {
        return p;
    }
    let key = format!("{kind}.{name}");
    let mut hits: Vec<(bool, usize, String, Policy)> = Vec::new();
    if let Some(rc) = root {
        for (pat, pol) in &rc.policies.by_pattern {
            let p = normalise_rules_alias(pat);
            if wild_match(&p, &key) {
                let spec = p.chars().filter(|c| *c != '*').count();
                hits.push((!p.contains('*'), spec, p, *pol));
            }
        }
    }
    hits.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)).then_with(|| a.2.cmp(&b.2)));
    match hits.first() {
        Some((true, _, _, p)) => *p,
        top => {
            if waivable {
                Policy::NeedsException
            } else if let Some((_, _, _, p)) = top {
                *p
            } else if kind == "rule" {
                Policy::Sealed
            } else {
                Policy::NeedsException
            }
        }
    }
}

fn control_cands(scope: &Scope, cf: &ControlFile, root: Option<&ControlFile>, diags: &mut Vec<Diagnostic>) -> Vec<Cand> {
    let loc = Location { path: cf.path.clone(), line: 0, col: 0 };
    let mut out = Vec::new();
    for (name, r) in &cf.rules {
        let Ok(id) = Id::parse(&format!("rule.{name}")) else {
            diags.push(bad_decl(scope, cf, &format!("rule name \"{name}\" is not a valid ID name")));
            continue;
        };
        let mut fields = BTreeMap::new();
        fields.insert("sev".to_string(), FieldValue::Scalar(r.sev.map(sev_text).unwrap_or("error").to_string()));
        fields.insert("stage".to_string(), FieldValue::Scalar(r.stage.map(stage_text).unwrap_or("observe").to_string()));
        fields.insert("glob".to_string(), FieldValue::List(r.glob.clone()));
        fields.insert("exclude".to_string(), FieldValue::List(r.exclude.clone()));
        out.push(Cand {
            policy: policy_for(root, "rule", id.name.as_str(), None, r.waivable),
            kind: id.kind,
            name: id.name,
            origin: Origin::ControlTable { control: cf.path.clone() },
            sum: r.why.clone(),
            fields,
            loc: loc.clone(),
        });
    }
    for f in &cf.files {
        let id = match Id::parse(&f.id) {
            Ok(id) if matches!(id.qualifier, IdQualifier::Bare) => id,
            _ => {
                diags.push(bad_decl(scope, cf, &format!("file-item id \"{}\" must be a bare kind.name", f.id)));
                continue;
            }
        };
        let mut fields = BTreeMap::new();
        fields.insert("kind".to_string(), FieldValue::Scalar(f.kind.clone()));
        fields.insert("path".to_string(), FieldValue::Scalar(f.path.clone()));
        if let Some(e) = &f.extends {
            fields.insert("extends".to_string(), FieldValue::Scalar(e.clone()));
        }
        out.push(Cand {
            policy: policy_for(root, id.kind.as_str(), id.name.as_str(), None, false),
            kind: id.kind,
            name: id.name,
            origin: Origin::ControlTable { control: cf.path.clone() },
            sum: None,
            fields,
            loc: loc.clone(),
        });
    }
    out
}

fn bad_decl(scope: &Scope, cf: &ControlFile, msg: &str) -> Diagnostic {
    Dg::new(codes::HN_C001, Severity::Error, &cf.path, 1, 1)
        .scope(&scope.id)
        .msg(msg)
        .hint("fix the declaration in this control file")
        .done()
}

fn insert_cands(decls: &mut Decls, table: &ScopeTable, scope: &Scope, mut cands: Vec<Cand>, diags: &mut Vec<Diagnostic>) {
    cands.sort_by(|a, b| {
        (a.kind.as_str(), a.name.as_str(), a.loc.path.as_str(), a.loc.line)
            .cmp(&(b.kind.as_str(), b.name.as_str(), b.loc.path.as_str(), b.loc.line))
    });
    for c in cands {
        let id = ItemId { scope: scope.id.clone(), kind: c.kind.clone(), name: c.name.clone() };
        let text = format!("{}.{}", c.kind.as_str(), c.name.as_str());
        if decls.items.contains_key(&id) {
            diags.push(
                Dg::new(codes::HN_D001, Severity::Error, &c.loc.path, c.loc.line, c.loc.col)
                    .scope(&scope.id)
                    .msg(format!("`{text}` is declared twice in scope \"{}\"", scope.id.0.as_str()))
                    .hint("keep one declaration or rename the other")
                    .text(text)
                    .done(),
            );
            continue;
        }
        let shadowed = table
            .chain_ids(&scope.id)
            .into_iter()
            .filter(|s| **s != scope.id)
            .map(|s| ItemId { scope: s.clone(), kind: c.kind.clone(), name: c.name.clone() })
            .find(|anc| decls.items.contains_key(anc));
        if let Some(anc) = shadowed {
            diags.push(
                Dg::new(codes::HN_R005, Severity::Error, &c.loc.path, c.loc.line, c.loc.col)
                    .scope(&scope.id)
                    .msg(format!("`{text}` redefines an ID inherited from scope \"{}\"", anc.scope.0.as_str()))
                    .related(anc.to_id().to_string())
                    .hint("convert it to an `ovr` of the inherited item, or rename it")
                    .text(text)
                    .done(),
            );
            continue;
        }
        decls.locs.insert(id.clone(), c.loc.clone());
        decls.items.insert(
            id.clone(),
            Item { id, origin: c.origin, policy: c.policy, sum: c.sum, fields: c.fields },
        );
    }
}

/// Parents are processed before children, so an ancestor always wins (R-SCOPE-026).
pub(crate) fn collect_items(table: &ScopeTable, diags: &mut Vec<Diagnostic>) -> Decls {
    let mut decls = Decls::default();
    let root = table.root_control();
    for scope in table.in_depth_order() {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else {
            continue;
        };
        let cands = control_cands(scope, cf, root, diags);
        insert_cands(&mut decls, table, scope, cands, diags);
    }
    decls
}
```

Replace `build_views` in `registry/view.rs` (the fold of ovrs arrives in Task 13):

```rust
use crate::registry::item::Item;
use crate::registry::scope::ScopeState;

fn effective_item(item: &Item, loc: &Location) -> EffectiveItem {
    EffectiveItem {
        id: item.id.clone(),
        policy: item.policy,
        fields: item
            .fields
            .iter()
            .map(|(k, v)| {
                let provenance = Provenance {
                    home: item.id.scope.clone(),
                    declared_at: loc.clone(),
                    applied: Vec::new(),
                    grants: Vec::new(),
                };
                (k.clone(), EffectiveField { value: v.clone(), provenance })
            })
            .collect(),
    }
}

/// Depth-first, memoised: a child starts from its parent's view (R-SCOPE-027). Unresolved scopes get
/// an empty view and never inherit (R-SCOPE-054).
pub(crate) fn build_views(
    table: &ScopeTable,
    items: &BTreeMap<ItemId, Item>,
    locs: &BTreeMap<ItemId, Location>,
) -> BTreeMap<ScopeId, EffectiveView> {
    let mut views: BTreeMap<ScopeId, EffectiveView> = BTreeMap::new();
    for scope in table.in_depth_order() {
        let mut v = empty_view(&scope.id, table);
        if scope.state == ScopeState::Resolved {
            if let Some(p) = scope.parent.as_ref().and_then(|p| views.get(p)) {
                v.items = p.items.clone();
            }
            for (id, item) in items.iter().filter(|(k, _)| k.scope == scope.id) {
                if let Some(loc) = locs.get(id) {
                    v.items.insert(id.clone(), effective_item(item, loc));
                }
            }
        }
        views.insert(scope.id.clone(), v);
    }
    views
}
```

In `registry/mod.rs` add the field `pub(crate) kinds: KindTable,` and `pub(crate) locs: BTreeMap<ItemId, Location>,` to `Registry`. In `resolve.rs`:

```rust
    let kinds = KindTable::build(&table, &mut diags);
    let decls = collect_items(&table, &mut diags);
    let views = view::build_views(&table, &decls.items, &decls.locs);
    // ...and in the struct literal: kinds, items: decls.items, locs: decls.locs,
```

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_scope_030 r_chk_006 r_syn_024 r_scope_026 r_syn_033 r_arch_104`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): kind table, control-table items, policy defaults, item views"
```

---

### Task 5: Doc declarations, key and kind checks, duplicates, shadowing (R-SYN-024, 026, R-SCOPE-023, 024, 026, 030)

**Files:**
- Modify: `crates/fairlead-core/src/registry/item.rs`, `registry/resolve.rs`, `crates/fairlead-core/tests/registry_items.rs`

**Interfaces:**

Consumes: `Corpus.records`, `FileRecord.extraction`, `Directive`, `Verb`, `Carrier`, `known_keys`, `KindTable`, `Cand`, `insert_cands` (Task 4), `codes::{HN_S002, HN_S011, HN_C013}`.

Produces:
```rust
pub(crate) fn doc_cands(table: &ScopeTable, kinds: &KindTable, folder: &str, corpus: &Corpus,
                        diags: &mut Vec<Diagnostic>) -> BTreeMap<ScopeId, Vec<Cand>>;
pub(crate) fn collect_items(table: &ScopeTable, docs: BTreeMap<ScopeId, Vec<Cand>>, diags: &mut Vec<Diagnostic>) -> Decls;
pub(crate) fn check_keys(d: &Directive, scope: &ScopeId, table: &ScopeTable, kinds: &KindTable,
                         path: &RepoPath, diags: &mut Vec<Diagnostic>) -> bool;   // false = drop the directive
pub(crate) fn undeclared_kind(kinds: &KindTable, table: &ScopeTable, from: &ScopeId, id: &Id,
                              path: &RepoPath, span: Span, text: &str) -> Diagnostic;
```

- [ ] **Step 1: Write the failing tests.** Append to `tests/registry_items.rs`:

```rust
use fairlead_core::diag::codes::{HN_D001, HN_S002, HN_S011};
use fairlead_syntax::diag::Severity;


#[test]
fn r_syn_026__two_doc_defs_of_one_id_in_one_scope_is_hn_d001() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/t.md", &doc(&["def tok.primary applies=color", "def tok.primary applies=background"]))
        .resolve();
    assert_eq!(reg.diagnostics().iter().filter(|d| d.code == HN_D001).count(), 1);
    assert!(reg.item(&iid("root", "tok.primary")).is_some());
}

#[test]
fn r_scope_023__child_doc_def_of_an_inherited_id_is_hn_r005() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.button"]))
        .resolve();
    let d: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_R005).collect();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].location.path.as_str(), "apps/web/fairlead/docs/c.md");
    assert!(reg.item(&iid("web", "cmp.button")).is_none());
}

#[test]
fn r_scope_024__siblings_may_reuse_a_bare_name() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.card"]))
        .file("apps/admin/fairlead/docs/c.md", &doc(&["def cmp.card"]))
        .resolve();
    assert!(reg.item(&iid("web", "cmp.card")).is_some());
    assert!(reg.item(&iid("admin", "cmp.card")).is_some());
    assert!(reg.diagnostics().iter().all(|d| d.code != HN_R005 && d.code != HN_D001));
}

#[test]
fn r_syn_024__undeclared_kind_is_hn_s002_with_declared_kinds_and_a_suggestion() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def comp.button"]))
        .resolve();
    let d = reg.diagnostics().iter().find(|d| d.code == HN_S002).unwrap();
    assert!(d.expected.as_deref().unwrap().contains("cmp"));
    assert_eq!(d.did_you_mean.as_deref(), Some("cmp.button"));
    assert_eq!(reg.items().filter(|i| i.id.scope == sid("root")).count(), 0);
}

#[test]
fn r_syn_013__unknown_def_key_is_hn_s011_warn_and_error_under_bang() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.a bogus=1 x-note=ok", "!def cmp.b bogus=1"]))
        .resolve();
    let d: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_S011).collect();
    assert_eq!(d.len(), 2); // x- keys are never reported
    assert!(d.iter().any(|d| d.severity == Severity::Warn));
    assert!(d.iter().any(|d| d.severity == Severity::Error));
    assert!(reg.item(&iid("root", "cmp.a")).is_some()); // warned, kept
    assert!(reg.item(&iid("root", "cmp.b")).is_none()); // must-understand, dropped
}

#[test]
fn r_scope_030__def_policy_key_refines_the_item_policy() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.locked policy=sealed sum=\"fixed\"", "def cmp.free"]))
        .resolve();
    let locked = reg.item(&iid("root", "cmp.locked")).unwrap();
    assert_eq!(locked.policy, Policy::Sealed);
    assert_eq!(locked.sum.as_deref(), Some("fixed"));
    assert_eq!(reg.item(&iid("root", "cmp.free")).unwrap().policy, Policy::Open); // "cmp.*" = "open"
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core -- r_syn_026 r_scope_023 r_scope_024 r_syn_024 r_syn_013 r_scope_030`
Expected: FAIL: `r_syn_026...` assertion `0 == 1` (doc defs are not read yet).

- [ ] **Step 3: Implement.** Append to `registry/item.rs` (imports to add: `fairlead_syntax::directive::{known_keys, Arg, ArgOp, ArgValue, Carrier, Directive, Verb}`, `fairlead_syntax::diag::Span`, `crate::extract::Corpus`, `crate::registry::scope::is_within`):

```rust
pub(crate) fn verb_name(v: Verb) -> &'static str {
    match v {
        Verb::Def => "def",
        Verb::Use => "use",
        Verb::Ovr => "ovr",
        Verb::Flag => "flag",
        Verb::Env => "env",
        Verb::End => "end",
    }
}

pub(crate) fn arg_scalar(v: &ArgValue) -> String {
    match v {
        ArgValue::Word(s) | ArgValue::Quoted(s) => s.clone(),
        ArgValue::List(l) => l.join(","),
    }
}

pub(crate) fn arg_items(v: &ArgValue) -> Vec<String> {
    match v {
        ArgValue::Word(s) | ArgValue::Quoted(s) => vec![s.clone()],
        ArgValue::List(l) => l.clone(),
    }
}

/// Keys whose value is a list even when one word is written (R-SYN-015: `kw`).
const LIST_KEYS: [&str; 1] = ["kw"];

fn arg_field(a: &Arg) -> FieldValue {
    match (&a.op, &a.value) {
        (ArgOp::Append, v) => FieldValue::List(arg_items(v)),
        (ArgOp::Set, ArgValue::List(l)) => FieldValue::List(l.clone()),
        (ArgOp::Set, v) if LIST_KEYS.contains(&a.key.as_str()) => FieldValue::List(arg_items(v)),
        (ArgOp::Set, v) => FieldValue::Scalar(arg_scalar(v)),
    }
}

pub(crate) fn undeclared_kind(
    kinds: &KindTable,
    table: &ScopeTable,
    from: &ScopeId,
    id: &Id,
    path: &RepoPath,
    span: Span,
    text: &str,
) -> Diagnostic {
    let names = kinds.declared_names(table, from);
    let written = id.kind.as_str();
    let dym = names
        .iter()
        .min_by_key(|n| (levenshtein(n, written), n.as_str()))
        .map(|n| format!("{n}.{}", id.name.as_str()));
    let mut dg = Dg::new(codes::HN_S002, Severity::Error, path, span.line, span.col)
        .scope(from)
        .msg(format!("unknown kind \"{written}\" in \"{id}\""))
        .expected(format!("declared kinds: {}", names.join(" ")))
        .hint("declare the kind under [kinds.<k>] in a control file, or fix the ID; directive dropped")
        .text(text);
    if let Some(d) = dym {
        dg = dg.dym(d);
    }
    dg.done()
}

/// HN-S011 (C-5): keys known to the verb, `x-` extension keys and the kind's own keys are fine.
pub(crate) fn check_keys(
    d: &Directive,
    scope: &ScopeId,
    table: &ScopeTable,
    kinds: &KindTable,
    path: &RepoPath,
    diags: &mut Vec<Diagnostic>,
) -> bool {
    let kind_keys = d
        .ids
        .first()
        .filter(|_| d.verb == Verb::Def)
        .and_then(|id| kinds.decl(table, scope, id.kind.as_str()));
    let mut keep = true;
    for a in &d.args {
        let known = known_keys(d.verb).contains(&a.key.as_str())
            || a.key.starts_with("x-")
            || kind_keys.is_some_and(|k| k.keys.contains_key(&a.key));
        if known {
            continue;
        }
        let sev = if d.must_understand { Severity::Error } else { Severity::Warn };
        keep &= !d.must_understand;
        diags.push(
            Dg::new(codes::HN_S011, sev, path, a.span.line, a.span.col)
                .scope(scope)
                .msg(format!("unknown key \"{}\" on `{}`", a.key, verb_name(d.verb)))
                .hint("remove it, rename it with an x- prefix, or declare it under [kinds.<k>.keys]")
                .text(a.key.as_str())
                .done(),
        );
    }
    keep
}

/// P4-1: Markdown, frontmatter and managerial-folder files declare; everything else is a site.
pub(crate) fn is_decl_site(path: &RepoPath, scope: &Scope, folder: &str, d: &Directive) -> bool {
    if matches!(d.carrier, Carrier::Frontmatter) || path.extension() == "md" {
        return true;
    }
    let base = scope.dir.as_str().trim_matches('/');
    let mdir = if base.is_empty() { folder.to_string() } else { format!("{base}/{folder}") };
    is_within(path.as_str(), &mdir)
}

fn parse_policy(s: &str) -> Option<Policy> {
    match s {
        "open" => Some(Policy::Open),
        "needs-exception" => Some(Policy::NeedsException),
        "sealed" => Some(Policy::Sealed),
        _ => None,
    }
}

pub(crate) fn doc_cands(
    table: &ScopeTable,
    kinds: &KindTable,
    folder: &str,
    corpus: &Corpus,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<ScopeId, Vec<Cand>> {
    let root = table.root_control();
    let mut out: BTreeMap<ScopeId, Vec<Cand>> = BTreeMap::new();
    for (path, rec) in &corpus.records {
        let Some(sid) = table.scope_of(path) else { continue };
        let Some(scope) = table.get(sid).filter(|s| s.state == ScopeState::Resolved) else { continue };
        let Some(ex) = rec.extraction.as_ref() else { continue };
        for d in &ex.directives {
            if d.verb != Verb::Def || !is_decl_site(path, scope, folder, d) {
                continue;
            }
            let Some(id) = d.ids.first() else { continue };
            if id.kind.as_str() == "foreign" {
                continue; // region marker (P4-1)
            }
            let text = fairlead_syntax::extract::render_directive_body(d);
            let qualifier_ok = match &id.qualifier {
                IdQualifier::Bare => true,
                IdQualifier::Scope(q) => *q == scope.id.0,
                IdQualifier::Root => scope.id == table.root,
            };
            if !qualifier_ok {
                diags.push(
                    Dg::new(codes::HN_C013, Severity::Error, path, d.span.line, d.span.col)
                        .scope(&scope.id)
                        .msg(format!("`def {id}` names another scope; an item is declared in the scope that holds the doc"))
                        .hint("drop the qualifier or move the declaration to that scope's docs")
                        .text(text)
                        .done(),
                );
                continue;
            }
            if !kinds.is_declared(table, sid, id) {
                diags.push(undeclared_kind(kinds, table, sid, id, path, d.span, &text));
                continue;
            }
            if !check_keys(d, sid, table, kinds, path, diags) {
                continue;
            }
            let mut explicit = None;
            let mut sum = None;
            let mut fields = BTreeMap::new();
            for a in &d.args {
                match a.key.as_str() {
                    "policy" => match parse_policy(&arg_scalar(&a.value)) {
                        Some(p) => explicit = Some(p),
                        None => diags.push(
                            Dg::new(codes::HN_S011, Severity::Warn, path, a.span.line, a.span.col)
                                .scope(sid)
                                .msg("policy must be open, needs-exception or sealed")
                                .hint("fix the value; the kind default applies meanwhile")
                                .text(arg_scalar(&a.value))
                                .done(),
                        ),
                    },
                    "sum" => sum = Some(arg_scalar(&a.value)),
                    k if k.starts_with("x-") => {}
                    k => {
                        fields.insert(k.to_string(), arg_field(a));
                    }
                }
            }
            out.entry(scope.id.clone()).or_default().push(Cand {
                policy: policy_for(root, id.kind.as_str(), id.name.as_str(), explicit, false),
                kind: id.kind.clone(),
                name: id.name.clone(),
                origin: Origin::DocDef { path: path.clone(), span: d.span },
                sum,
                fields,
                loc: Location { path: path.clone(), line: d.span.line, col: d.span.col },
            });
        }
    }
    out
}
```

Change `collect_items` (Task 4) to take and merge the doc candidates:

```rust
pub(crate) fn collect_items(
    table: &ScopeTable,
    mut docs: BTreeMap<ScopeId, Vec<Cand>>,
    diags: &mut Vec<Diagnostic>,
) -> Decls {
    let mut decls = Decls::default();
    let root = table.root_control();
    for scope in table.in_depth_order() {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else {
            continue;
        };
        let mut cands = control_cands(scope, cf, root, diags);
        cands.extend(docs.remove(&scope.id).unwrap_or_default());
        insert_cands(&mut decls, table, scope, cands, diags);
    }
    decls
}
```

In `resolve.rs` replace the `decls` line and sort the diagnostics at the end:

```rust
    let docs = doc_cands(&table, &kinds, input.root.folder.as_str(), input.corpus, &mut diags);
    let decls = collect_items(&table, docs, &mut diags);
    // ... at the end, before building the Registry (R-ARCH-051 style total order):
    diags.sort_by(|a, b| {
        (a.location.path.as_str(), a.location.line, a.location.col, a.code, a.message.as_str())
            .cmp(&(b.location.path.as_str(), b.location.line, b.location.col, b.code, b.message.as_str()))
    });
```

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_syn_026 r_scope_023 r_scope_024 r_syn_024 r_syn_013 r_scope_030 r_chk_006`
Expected: PASS. If `r_syn_013` finds 0 `HN_S011` the extractor (P2) already dropped unknown keys; then assert on `known_keys` use instead and report the overlap in Contract issues.

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): doc declarations, kind and key checks, duplicates and shadowing"
```

---

### Task 6: Lookup, `use` edges, code `def` sites, `home=` data (R-SCOPE-024, R-SYN-025, R-ARCH-062, R-FILE-037; data for R-SCOPE-017)

**Files:**
- Modify: `crates/fairlead-core/src/registry/item.rs`, `registry/ovr.rs`, `registry/mod.rs`, `registry/resolve.rs`, `crates/fairlead-core/tests/registry_items.rs`

**Interfaces:**

Consumes: `KindTable`, `check_keys`, `undeclared_kind`, `is_decl_site`, `Corpus`, `Directive`, `Extent`, `codes::HN_R007`.

Produces (contract, exact): `Registry::lookup(&self, from: &ScopeId, r: &Id) -> Lookup`, `edges()`, `sites_of()`. Additive: `Registry::resolve_target(&self, from: &ScopeId, text: &str) -> Option<(ItemId, Option<String>)>` (`kind.name` or `kind.name.field`, P4-3 alias applied). Internal:
```rust
pub(crate) struct Index<'a> { table: &'a ScopeTable, kinds: &'a KindTable, items: &'a BTreeMap<ItemId, Item> }
impl Index<'_> { fn lookup(&self, from: &ScopeId, r: &Id) -> Lookup; fn resolve_target(&self, from: &ScopeId, text: &str) -> Option<(ItemId, Option<String>)>; }
pub(crate) struct Refs { edges: Vec<Edge>, sites: BTreeMap<ItemId, Vec<Site>> }
pub(crate) fn collect_refs(idx: &Index<'_>, folder: &str, corpus: &Corpus, diags: &mut Vec<Diagnostic>) -> Refs;
pub const FLOOR_RULES: &[&str];   // ovr.rs, P4-8
```
Rules of lookup: bare IDs walk the chain nearest first; `:kind.n` is the root scope; `scope:kind.n` names that scope; a bare ID found on no chain scope is `Ambiguous` when two or more other scopes declare it, else `Missing` whose `nearest` lists the qualified candidate first.

- [ ] **Step 1: Write the failing tests.** Append to `tests/registry_items.rs`:

```rust
use fairlead_core::diag::codes::HN_R007;
use fairlead_core::registry::Lookup;
use fairlead_syntax::ids::Id;

fn parse_id(s: &str) -> Id {
    Id::parse(s).unwrap()
}

fn nested() -> Kit {
    Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.card", "def tok.sidebar"]))
        .file("apps/admin/fairlead/docs/c.md", &doc(&["def cmp.card"]))
}

#[test]
fn r_syn_025__unqualified_reference_resolves_to_the_nearest_declaring_scope() {
    let reg = nested()
        .file("apps/web/src/a.ts", "// fairlead: use cmp.button cmp.card :cmp.button\nexport const a = 1;\n")
        .resolve();
    let to: Vec<_> = reg.edges().iter().map(|e| e.resolved.clone()).collect();
    assert_eq!(
        to,
        vec![Some(iid("root", "cmp.button")), Some(iid("web", "cmp.card")), Some(iid("root", "cmp.button"))]
    );
    assert_eq!(reg.edges()[0].from.scope, sid("web"));
}

#[test]
fn r_scope_024__ancestor_into_descendant_reference_needs_a_qualifier() {
    let reg = nested().resolve();
    match reg.lookup(&sid("root"), &parse_id("tok.sidebar")) {
        Lookup::Missing { nearest } => assert_eq!(nearest[0].to_string(), "web:tok.sidebar"),
        other => panic!("expected Missing, got {other:?}"),
    }
    assert_eq!(
        reg.lookup(&sid("root"), &parse_id("web:tok.sidebar")),
        Lookup::Found(iid("web", "tok.sidebar"))
    );
}

#[test]
fn r_arch_062__bare_reference_with_two_candidates_is_ambiguous_hn_r007() {
    let reg = nested().file("src/top.ts", "// fairlead: use cmp.card\nexport const t = 1;\n").resolve();
    match reg.lookup(&sid("root"), &parse_id("cmp.card")) {
        Lookup::Ambiguous(c) => assert_eq!(c, vec![iid("admin", "cmp.card"), iid("web", "cmp.card")]),
        other => panic!("expected Ambiguous, got {other:?}"),
    }
    let d = reg.diagnostics().iter().find(|d| d.code == HN_R007).unwrap();
    assert_eq!(d.location.path.as_str(), "src/top.ts");
    assert!(reg.edges()[0].resolved.is_none());
}

#[test]
fn r_arch_062__missing_reference_lists_the_nearest_ids_by_edit_distance() {
    let reg = nested().resolve();
    match reg.lookup(&sid("web"), &parse_id("cmp.buton")) {
        Lookup::Missing { nearest } => {
            assert_eq!(nearest[0].to_string(), "cmp.button");
            assert!(nearest.len() <= 3);
        }
        other => panic!("expected Missing, got {other:?}"),
    }
    // wrong kind prefix: the right ID is offered
    match reg.lookup(&sid("web"), &parse_id("tok.button")) {
        Lookup::Missing { nearest } => assert_eq!(nearest[0].to_string(), "cmp.button"),
        other => panic!("expected Missing, got {other:?}"),
    }
}

#[test]
fn r_file_037__def_in_a_leaf_folder_belongs_to_the_nearest_ancestor_scope() {
    let reg = nested()
        .file("apps/web/src/organisms/sidebar/sidebar.css", "/* fairlead: def tok.sidebar */\n:root{}\n")
        .resolve();
    let sites = reg.sites_of(&iid("web", "tok.sidebar"));
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].scope, sid("web"));
    assert_eq!(sites[0].path.as_str(), "apps/web/src/organisms/sidebar/sidebar.css");
}

#[test]
fn r_syn_024__use_of_an_undeclared_kind_is_hn_s002_and_makes_no_edge() {
    let reg = nested().file("apps/web/src/a.ts", "// fairlead: use comp.x\nexport const a = 1;\n").resolve();
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_S002 && d.location.path.as_str() == "apps/web/src/a.ts"));
    assert!(reg.edges().is_empty());
}

#[test]
fn r_scope_017__home_must_be_an_ancestor_of_every_implementation_site() {
    use fairlead_core::registry::item::home_mismatches;
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.bridge home=web"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/web/src/a.ts", "// fairlead: def cmp.bridge
export const a = 1;
")
        .file("apps/admin/src/a.ts", "// fairlead: def cmp.bridge
export const a = 1;
")
        .resolve();
    let m = home_mismatches(&reg);
    assert_eq!(m.len(), 1); // the admin site; HN-C013 itself is raised by P5
    assert_eq!((m[0].1.scope.clone(), m[0].2.clone()), (sid("admin"), sid("web")));
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core -- r_syn_025 r_scope_024__ancestor r_arch_062 r_file_037 r_syn_024__use`
Expected: FAIL to compile: `no method named lookup found for struct Registry`.

- [ ] **Step 3: Implement.** In `registry/ovr.rs` add (P4-8; the mnemonics of the floor codes of chapter 13):

```rust
/// Rule names that mirror the floor codes (R-SCOPE-039, R-CHK-008). An `ovr` on `rule.<name>` is inert.
pub const FLOOR_RULES: &[&str] = &[
    "baseline-tampered", "control-file", "duplicate-id", "exception-on-floor-rule", "expired-exception",
    "non-delegable-key", "path-hygiene", "root-dup", "secret-like",
];
```

Append to `registry/item.rs`:

```rust
use crate::registry::ovr::FLOOR_RULES;

/// Borrowed read view used by `resolve` (before the `Registry` exists) and by `Registry` accessors.
pub(crate) struct Index<'a> {
    pub(crate) table: &'a ScopeTable,
    pub(crate) kinds: &'a KindTable,
    pub(crate) items: &'a BTreeMap<ItemId, Item>,
}

impl Index<'_> {
    fn item_in(&self, scope: &ScopeId, r: &Id) -> Option<ItemId> {
        let id = ItemId { scope: scope.clone(), kind: r.kind.clone(), name: r.name.clone() };
        self.items.contains_key(&id).then_some(id)
    }

    fn floor_item(&self, r: &Id) -> Option<ItemId> {
        (r.kind.as_str() == "rule" && FLOOR_RULES.contains(&r.name.as_str()))
            .then(|| ItemId { scope: self.table.root.clone(), kind: r.kind.clone(), name: r.name.clone() })
    }

    /// R-SYN-025, R-ARCH-062.
    pub(crate) fn lookup(&self, from: &ScopeId, r: &Id) -> Lookup {
        let direct = |s: &ScopeId| match self.item_in(s, r).or_else(|| self.floor_item(r)) {
            Some(i) => Lookup::Found(i),
            None => Lookup::Missing { nearest: self.nearest(from, r) },
        };
        match &r.qualifier {
            IdQualifier::Root => direct(&self.table.root),
            IdQualifier::Scope(q) => direct(&ScopeId(q.clone())),
            IdQualifier::Bare => {
                for s in self.table.chain_ids(from).into_iter().rev() {
                    if let Some(i) = self.item_in(s, r) {
                        return Lookup::Found(i);
                    }
                }
                if let Some(i) = self.floor_item(r) {
                    return Lookup::Found(i);
                }
                let others: Vec<ItemId> =
                    self.items.keys().filter(|k| k.kind == r.kind && k.name == r.name).cloned().collect();
                if others.len() >= 2 {
                    return Lookup::Ambiguous(others);
                }
                Lookup::Missing { nearest: self.nearest(from, r) }
            }
        }
    }

    /// Up to three IDs: the same ID declared elsewhere (qualified), the same name under another kind,
    /// then the smallest edit distances.
    fn nearest(&self, from: &ScopeId, r: &Id) -> Vec<Id> {
        let want = format!("{}.{}", r.kind.as_str(), r.name.as_str());
        let chain = self.table.chain_ids(from);
        let mut c: Vec<(u8, usize, String, Id)> = Vec::new();
        for item in self.items.keys() {
            let on_chain = chain.contains(&&item.scope);
            let text = format!("{}.{}", item.kind.as_str(), item.name.as_str());
            let id = if on_chain {
                Id { qualifier: IdQualifier::Bare, kind: item.kind.clone(), name: item.name.clone() }
            } else {
                item.to_id()
            };
            if text == want {
                c.push((0, 0, id.to_string(), id));
            } else if on_chain && item.name == r.name {
                c.push((1, 0, id.to_string(), id));
            } else if on_chain {
                let d = levenshtein(&text, &want);
                if d <= (want.len() / 2).max(3) {
                    c.push((2, d, id.to_string(), id));
                }
            }
        }
        c.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
        c.truncate(3);
        c.into_iter().map(|x| x.3).collect()
    }

    /// `kind.name` or `kind.name.field` (R-SCOPE-040); the longest item name wins.
    pub(crate) fn resolve_target(&self, from: &ScopeId, text: &str) -> Option<(ItemId, Option<String>)> {
        let text = normalise_rules_alias(text);
        let id = Id::parse(&text).ok()?;
        if let Lookup::Found(i) = self.lookup(from, &id) {
            return Some((i, None));
        }
        let (head, field) = text.rsplit_once('.')?;
        let id2 = Id::parse(head).ok()?;
        match self.lookup(from, &id2) {
            Lookup::Found(i) => Some((i, Some(field.to_string()))),
            _ => None,
        }
    }
}

// --- edges and sites ----------------------------------------------------------------------

pub(crate) struct Refs {
    pub(crate) edges: Vec<Edge>,
    pub(crate) sites: BTreeMap<ItemId, Vec<Site>>,
}

pub(crate) fn collect_refs(idx: &Index<'_>, folder: &str, corpus: &Corpus, diags: &mut Vec<Diagnostic>) -> Refs {
    let mut refs = Refs { edges: Vec::new(), sites: BTreeMap::new() };
    for (path, rec) in &corpus.records {
        let Some(sid) = idx.table.scope_of(path) else { continue };
        let Some(scope) = idx.table.get(sid).filter(|s| s.state == ScopeState::Resolved) else { continue };
        let Some(ex) = rec.extraction.as_ref() else { continue };
        for d in &ex.directives {
            let is_use = d.verb == Verb::Use;
            let is_site = d.verb == Verb::Def && !is_decl_site(path, scope, folder, d);
            if !(is_use || is_site) || !check_keys(d, sid, idx.table, idx.kinds, path, diags) {
                continue;
            }
            let site = Site { path: path.clone(), span: d.span, extent: d.extent, scope: sid.clone() };
            let sym = d.arg("sym").map(|a| arg_scalar(&a.value));
            for id in &d.ids {
                if id.kind.as_str() == "foreign" {
                    continue;
                }
                let text = fairlead_syntax::extract::render_directive_body(d);
                if !idx.kinds.is_declared(idx.table, sid, id) {
                    diags.push(undeclared_kind(idx.kinds, idx.table, sid, id, path, d.span, &text));
                    continue;
                }
                let found = match idx.lookup(sid, id) {
                    Lookup::Found(i) => Some(i),
                    Lookup::Ambiguous(c) => {
                        let names: Vec<String> = c.iter().map(|i| i.to_id().to_string()).collect();
                        diags.push(
                            Dg::new(codes::HN_R007, Severity::Error, path, d.span.line, d.span.col)
                                .scope(sid)
                                .msg(format!("reference `{id}` is ambiguous: declared in {}", names.join(", ")))
                                .related(names.join(" "))
                                .hint("qualify it as scope:kind.name")
                                .text(text)
                                .done(),
                        );
                        None
                    }
                    Lookup::Missing { .. } => None, // dangling: HN-R001 is raised by P5 from `edges`
                };
                if is_use {
                    refs.edges.push(Edge { from: site.clone(), to: id.clone(), sym: sym.clone(), resolved: found });
                } else if let Some(item) = found {
                    refs.sites.entry(item).or_default().push(site.clone());
                }
            }
        }
    }
    refs
}
```

Append the data side of R-SCOPE-017 (P5 raises `HN-C013` from it; a `def` carrying `home=<scope>` must name an ancestor-or-self of the scope of every implementation site):

```rust
use fairlead_syntax::ids::ScopeSlug;

pub fn home_mismatches(reg: &Registry) -> Vec<(ItemId, Site, ScopeId)> {
    let mut out = Vec::new();
    for (id, item) in &reg.items {
        let Some(FieldValue::Scalar(h)) = item.fields.get("home") else { continue };
        let Ok(slug) = ScopeSlug::parse(h) else { continue };
        let home = ScopeId(slug);
        for site in reg.sites_of(id) {
            if !reg.table.is_ancestor_or_self(&home, &site.scope) {
                out.push((id.clone(), site.clone(), home.clone()));
            }
        }
    }
    out
}
```
(import `crate::registry::Registry` in `item.rs`.)

`registry/mod.rs`: add `Registry::index`, `lookup`, `resolve_target`, and set `edges`/`sites` in `resolve.rs`:

```rust
impl Registry {
    pub(crate) fn index(&self) -> item::Index<'_> {
        item::Index { table: &self.table, kinds: &self.kinds, items: &self.items }
    }
    pub fn lookup(&self, from: &ScopeId, r: &fairlead_syntax::ids::Id) -> Lookup {
        self.index().lookup(from, r)
    }
    pub fn resolve_target(&self, from: &ScopeId, text: &str) -> Option<(ItemId, Option<String>)> {
        self.index().resolve_target(from, text)
    }
}
```
```rust
    // resolve.rs, after collect_items:
    let idx = Index { table: &table, kinds: &kinds, items: &decls.items };
    let refs = collect_refs(&idx, input.root.folder.as_str(), input.corpus, &mut diags);
    // struct literal: edges: refs.edges, sites: refs.sites,
```

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_syn_025 r_scope_024 r_arch_062 r_file_037 r_syn_024 r_scope_017`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): lookup, use edges and def sites"
```

---

### Task 7: Override construction, targets, fields, merge types, `OvrId` (R-SCOPE-022, 040, 041, R-SYN-013, R-CHK-026)

**Files:**
- Modify: `crates/fairlead-core/src/registry/ovr.rs`, `view.rs`, `resolve.rs`, `mod.rs`
- Create: `crates/fairlead-core/tests/registry_overrides.rs`

**Interfaces:**

Consumes: `Index::resolve_target` (Task 6), `Directive.args`, `OvrDecl` (`target, op, applies, set, why, owner, exp, base`), `ControlFile.ovrs`, `hash_raw`, `Fingerprint::from_hash`, `codes::{HN_S009, HN_C001}`.

Produces (contract exact):
```rust
pub fn ovr_fingerprint(target: &ItemId, scope: &ScopeId, why: &str, owner: &str, exp: Option<&jiff::civil::Date>) -> Fingerprint;
impl Registry { pub fn overrides(&self) -> &[Override]; }     // sorted by (target, scope, at)
```
Internal, extended by Tasks 9 to 11:
```rust
pub(crate) enum OvrExtent { File(RepoPath), Scope(Option<String>) }
pub(crate) struct RawOvr { scope, at: Location, target_text: String, extent: OvrExtent, op_text: Option<String>,
    set: BTreeMap<String, FieldChange>, why, owner, exp: Option<String>, until: bool, base, pin: Option<String>, text: String }
pub(crate) fn collect_raw(idx: &Index<'_>, corpus: &Corpus, diags: &mut Vec<Diagnostic>) -> BTreeMap<ScopeId, Vec<RawOvr>>;
pub(crate) struct Eval<'a> { idx: &'a Index<'a>, as_of: AsOf, grants: &'a [Allow], approvals: &'a [Approval], waiver: WaiverPolicy, horizon_days: i64 }
pub(crate) struct OvrMeta { applied: bool, whole: bool, extent: OvrExtent, used_grants: Vec<String> }
pub(crate) struct Evaluated { ovr: Override, meta: OvrMeta }
pub(crate) fn evaluate_scope(ev: &Eval<'_>, scope: &ScopeId, working: &mut EffectiveView,
                             raws: Vec<RawOvr>, diags: &mut Vec<Diagnostic>) -> Vec<Evaluated>;
```
Semantics fixed here: the directive form targets `Id` text, the table form `target`; `kind.name.field` peels the field (P4-3 alias applies); `field=value` replaces, `field+=a,b` appends, table `op = "add"` appends and `op = "remove"` lists the values to remove (P4-9). A key that is not a field of the target is `HN-S009` and the override is `Inert(UnknownField)`. A target that does not resolve yields no `Override` (P4-11).

- [ ] **Step 1: Write the failing tests** `crates/fairlead-core/tests/registry_overrides.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::diag::codes::HN_S009;
use fairlead_core::registry::{FieldChange, FieldValue, InertReason, OvrOp, OvrState};
use support::*;

fn base() -> Kit {
    Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button layer=atom size=md kw=ui", "def tok.primary applies=color"]))
        .scope("apps/web", "web", &child_toml("web", ""))
}

#[test]
fn r_scope_040__ovr_names_target_op_and_values() {
    let reg = base()
        .file("apps/web/src/a.ts", "// fairlead: ovr cmp.button op=set layer=molecule\nexport const a = 1;\n")
        .resolve();
    let o = &reg.overrides()[0];
    assert_eq!(o.target, iid("root", "cmp.button"));
    assert_eq!(o.op, OvrOp::Set);
    assert_eq!(o.declared_in, sid("web"));
    assert_eq!(o.set["layer"], FieldChange::Replace(FieldValue::Scalar("molecule".into())));
    assert_eq!(o.state, OvrState::Effective); // open policy needs nothing (R-SCOPE-031)
}

#[test]
fn r_scope_041__ovr_is_file_local_or_a_scope_level_table() {
    let ctl = child_toml("web", &ovr_table("cmp.button", "op = \"set\"\napplies = \"src/**\"\nsize = \"lg\""));
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
        .scope("apps/web", "web", &ctl)
        .file("apps/web/src/a.ts", "// fairlead: ovr cmp.button size=sm\nexport const a = 1;\n")
        .resolve();
    let table: Vec<_> = reg.overrides().iter().filter(|o| o.applies.is_some()).collect();
    let local: Vec<_> = reg.overrides().iter().filter(|o| o.applies.is_none()).collect();
    assert_eq!((table.len(), local.len()), (1, 1));
    assert_eq!(table[0].applies.as_deref(), Some("src/**"));
    assert_eq!(local[0].at.path.as_str(), "apps/web/src/a.ts");
}

#[test]
fn r_syn_013__append_marker_remove_op_and_unknown_field() {
    let reg = base()
        .file(
            "apps/web/src/a.ts",
            "// fairlead: ovr cmp.button kw+=a,b\n// fairlead: ovr cmp.button nosuch=1\nexport const a = 1;\n",
        )
        .resolve();
    let app = reg.overrides().iter().find(|o| o.set.contains_key("kw")).unwrap();
    assert_eq!(app.set["kw"], FieldChange::Append(vec!["a".into(), "b".into()]));
    let bad = reg.overrides().iter().find(|o| o.set.contains_key("nosuch")).unwrap();
    assert_eq!(bad.state, OvrState::Inert(InertReason::UnknownField));
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_S009));
}

#[test]
fn r_scope_040__target_may_name_a_field_and_a_dangling_target_makes_no_override() {
    let reg = base()
        .file(
            "apps/web/src/a.ts",
            "// fairlead: ovr tok.primary.value value=#ffffff\n// fairlead: ovr cmp.nothere size=sm\nexport const a = 1;\n",
        )
        .resolve();
    assert_eq!(reg.overrides().len(), 1);
    assert_eq!(reg.overrides()[0].field.as_deref(), Some("value"));
    assert_eq!(reg.resolve_target(&sid("web"), "tok.primary.value"), Some((iid("root", "tok.primary"), Some("value".into()))));
    assert_eq!(reg.resolve_target(&sid("web"), "cmp.nothere"), None);
}

#[test]
fn r_chk_026__ovr_id_binds_to_content_not_to_the_file_path() {
    let line = "// fairlead: ovr cmp.button size=sm why=\"dense\" owner=@andy exp=2026-12-31\nexport const a = 1;\n";
    let a = base().file("apps/web/src/a.ts", line).resolve();
    let b = base().file("apps/web/lib/moved/b.ts", line).resolve();
    assert_eq!(a.overrides()[0].id, b.overrides()[0].id);
    let c = base().file("apps/web/src/a.ts", &line.replace("2026-12-31", "2027-01-31")).resolve();
    assert_ne!(a.overrides()[0].id, c.overrides()[0].id); // re-dating changes the fingerprint
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core -- r_scope_040 r_scope_041 r_syn_013__append r_chk_026`
Expected: FAIL: index out of bounds, `reg.overrides()` is empty (no override is collected yet).

- [ ] **Step 3: Implement.** Append to `registry/ovr.rs` (add imports: `std::collections::BTreeSet`, `fairlead_syntax::diag::Severity`, `fairlead_syntax::directive::{known_keys, ArgValue, Verb}`, `crate::config::{Approval, ControlFile, OvrDecl, WaiverPolicy}`, `crate::extract::Corpus`, `crate::hash::{hash_raw, ContentHash}`, `crate::time::AsOf`, `crate::path::RepoPath`, `crate::diag::{codes, Diagnostic, Location}`, `crate::registry::{Dg, grant::Allow, item::{...}, scope::ScopeState, view::EffectiveView}`):

```rust
pub fn ovr_fingerprint(
    target: &ItemId,
    scope: &ScopeId,
    why: &str,
    owner: &str,
    exp: Option<&jiff::civil::Date>,
) -> Fingerprint {
    let why = why.split_whitespace().collect::<Vec<_>>().join(" ");
    let exp = exp.map(|d| d.to_string()).unwrap_or_default();
    let text = format!("{}\u{1f}{}\u{1f}{why}\u{1f}{owner}\u{1f}{exp}", target.to_id(), scope.0.as_str());
    Fingerprint::from_hash(&hash_raw(text.as_bytes())) // Verify: P3 constructor, first 12 base32 chars
}

pub(crate) fn op_name(o: OvrOp) -> &'static str {
    match o {
        OvrOp::Loosen => "loosen",
        OvrOp::Tighten => "tighten",
        OvrOp::Set => "set",
        OvrOp::Replace => "replace",
        OvrOp::Add => "add",
        OvrOp::Remove => "remove",
    }
}

pub(crate) fn parse_op(s: &str) -> Option<OvrOp> {
    [OvrOp::Loosen, OvrOp::Tighten, OvrOp::Set, OvrOp::Replace, OvrOp::Add, OvrOp::Remove]
        .into_iter()
        .find(|o| op_name(*o) == s)
}

#[derive(Clone, Debug)]
pub(crate) enum OvrExtent {
    File(RepoPath),
    Scope(Option<String>),
}

#[derive(Clone, Debug)]
pub(crate) struct RawOvr {
    pub(crate) scope: ScopeId,
    pub(crate) at: Location,
    pub(crate) target_text: String,
    pub(crate) extent: OvrExtent,
    pub(crate) op_text: Option<String>,
    pub(crate) set: BTreeMap<String, FieldChange>,
    pub(crate) why: Option<String>,
    pub(crate) owner: Option<String>,
    pub(crate) exp: Option<String>,
    pub(crate) until: bool,
    pub(crate) base: Option<String>,
    pub(crate) pin: Option<String>,
    pub(crate) text: String,
}

impl RawOvr {
    fn new(scope: &ScopeId, at: Location, target_text: String, extent: OvrExtent, text: String) -> RawOvr {
        RawOvr {
            scope: scope.clone(), at, target_text, extent, op_text: None, set: BTreeMap::new(),
            why: None, owner: None, exp: None, until: false, base: None, pin: None, text,
        }
    }

    fn absorb(&mut self, key: &str, append: bool, v: &ArgValue) {
        match key {
            "why" => self.why = Some(arg_scalar(v)),
            "owner" => self.owner = Some(arg_scalar(v)),
            "exp" => self.exp = Some(arg_scalar(v)),
            "until" => self.until = true,
            "op" => self.op_text = Some(arg_scalar(v)),
            "base" => self.base = Some(arg_scalar(v)),
            "x-pin" => self.pin = Some(arg_scalar(v)),
            k if k.starts_with("x-") => {}
            k => {
                let change = if append {
                    FieldChange::Append(arg_items(v))
                } else {
                    FieldChange::Replace(match v {
                        ArgValue::List(l) => FieldValue::List(l.clone()),
                        other => FieldValue::Scalar(arg_scalar(other)),
                    })
                };
                self.set.insert(k.to_string(), change);
            }
        }
    }
}

pub(crate) fn collect_raw(
    idx: &Index<'_>,
    corpus: &Corpus,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<ScopeId, Vec<RawOvr>> {
    let mut out: BTreeMap<ScopeId, Vec<RawOvr>> = BTreeMap::new();
    for (path, rec) in &corpus.records {
        let Some(sid) = idx.table.scope_of(path) else { continue };
        if !idx.table.is_resolved(sid) {
            continue;
        }
        let Some(ex) = rec.extraction.as_ref() else { continue };
        for d in ex.directives.iter().filter(|d| d.verb == Verb::Ovr) {
            let Some(id) = d.ids.first() else { continue };
            let text = fairlead_syntax::extract::render_directive_body(d);
            if !idx.kinds.is_declared(idx.table, sid, id) {
                diags.push(undeclared_kind(idx.kinds, idx.table, sid, id, path, d.span, &text));
                continue;
            }
            let at = Location { path: path.clone(), line: d.span.line, col: d.span.col };
            let mut r = RawOvr::new(sid, at, id.to_string(), OvrExtent::File(path.clone()), text);
            for a in &d.args {
                r.absorb(&a.key, matches!(a.op, ArgOp::Append), &a.value);
            }
            out.entry(sid.clone()).or_default().push(r);
        }
    }
    for scope in idx.table.in_depth_order() {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else { continue };
        for o in &cf.ovrs {
            let parsed = Id::parse(&normalise_rules_alias(&o.target));
            let Ok(id) = parsed else {
                diags.push(
                    Dg::new(codes::HN_C001, Severity::Error, &cf.path, 1, 1)
                        .scope(&scope.id)
                        .msg(format!("[[ovr]] target \"{}\" is not a valid ID", o.target))
                        .hint("write the target as kind.name or kind.name.field")
                        .done(),
                );
                continue;
            };
            let at = Location { path: cf.path.clone(), line: 0, col: 0 };
            if !idx.kinds.is_declared(idx.table, &scope.id, &id) {
                diags.push(undeclared_kind(idx.kinds, idx.table, &scope.id, &id, &cf.path, fairlead_syntax::diag::Span { start: 0, end: 0, line: 0, col: 0 }, &o.target));
                continue;
            }
            let mut r = RawOvr::new(&scope.id, at, id.to_string(), OvrExtent::Scope(o.applies.clone()), o.target.clone());
            r.op_text = Some(op_name(o.op).to_string());
            r.why = o.why.clone();
            r.owner = o.owner.clone();
            r.exp = o.exp.clone();
            r.base = o.base.clone();
            for (k, v) in &o.set {
                r.absorb(k, o.op == OvrOp::Add, v);
            }
            out.entry(scope.id.clone()).or_default().push(r);
        }
    }
    out
}

// --- evaluation -----------------------------------------------------------------------------

pub(crate) struct Eval<'a> {
    pub(crate) idx: &'a Index<'a>,
    pub(crate) as_of: AsOf,
    pub(crate) grants: &'a [Allow],
    pub(crate) approvals: &'a [Approval],
    pub(crate) waiver: WaiverPolicy,
    pub(crate) horizon_days: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct OvrMeta {
    pub(crate) applied: bool,
    pub(crate) whole: bool,
    pub(crate) extent: OvrExtent,
    pub(crate) used_grants: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct Evaluated {
    pub(crate) ovr: Override,
    pub(crate) meta: OvrMeta,
}

struct Pre {
    raw: RawOvr,
    target: ItemId,
    field: Option<String>,
    id: OvrId,
    base: Option<OvrId>,
    op: OvrOp,
    touched: BTreeSet<String>,
}

struct Verdict {
    state: OvrState,
    approval: ApprovalStatus,
    tighten: bool,
    used_grants: Vec<String>,
}

fn policy_text(p: crate::config::Policy) -> &'static str {
    match p {
        crate::config::Policy::Open => "open",
        crate::config::Policy::NeedsException => "needs-exception",
        crate::config::Policy::Sealed => "sealed",
    }
}

fn field_text(v: Option<&FieldValue>) -> String {
    match v {
        Some(FieldValue::Scalar(s)) => s.clone(),
        Some(FieldValue::List(l)) => l.join(","),
        None => String::new(),
    }
}

/// P4-7: pin of the declared form of the touched fields.
fn target_pin(item: Option<&Item>, target: &ItemId, touched: &BTreeSet<String>) -> ContentHash {
    let mut s = format!("{}", target.to_id());
    if let Some(it) = item {
        s.push('|');
        s.push_str(policy_text(it.policy));
        for k in touched {
            s.push_str(&format!("|{k}={}", field_text(it.fields.get(k))));
        }
    }
    hash_raw(s.as_bytes())
}

fn field_allowed(ev: &Eval<'_>, item: &Item, key: &str) -> bool {
    if item.fields.contains_key(key) {
        return true;
    }
    let kind = item.id.kind.as_str();
    kind != "rule"
        && (known_keys(Verb::Def).contains(&key)
            || ev.idx.kinds.decl(ev.idx.table, &item.id.scope, kind).is_some_and(|k| k.keys.contains_key(key)))
}

fn judge(ev: &Eval<'_>, p: &Pre, item: Option<&Item>, _working: &EffectiveView) -> Verdict {
    let v = |state| Verdict { state, approval: ApprovalStatus::NotRequired, tighten: false, used_grants: Vec::new() };
    // [T9: floor, outside subtree, non-delegable]
    if let Some(it) = item {
        if p.touched.iter().any(|k| !field_allowed(ev, it, k)) {
            return v(OvrState::Inert(InertReason::UnknownField));
        }
    }
    // [T11: duplicate]
    // [T9, T10: expiry shapes, tighten, policy, owner, expiry, stale]
    v(OvrState::Effective)
}

pub(crate) fn evaluate_scope(
    ev: &Eval<'_>,
    scope: &ScopeId,
    working: &mut EffectiveView,
    raws: Vec<RawOvr>,
    diags: &mut Vec<Diagnostic>,
) -> Vec<Evaluated> {
    let mut pres: Vec<Pre> = Vec::new();
    for raw in raws {
        let Some((target, field)) = ev.idx.resolve_target(scope, &raw.target_text) else { continue }; // P4-11
        let exp = raw.exp.as_deref().and_then(|s| s.parse::<jiff::civil::Date>().ok());
        let id = OvrId(ovr_fingerprint(
            &target,
            scope,
            raw.why.as_deref().unwrap_or(""),
            raw.owner.as_deref().unwrap_or(""),
            exp.as_ref(),
        ));
        let base = raw.base.as_deref().and_then(|b| b.parse::<Fingerprint>().ok()).map(OvrId);
        let op = raw.op_text.as_deref().and_then(parse_op).unwrap_or(OvrOp::Set);
        let mut touched: BTreeSet<String> = raw.set.keys().cloned().collect();
        touched.extend(field.clone());
        pres.push(Pre { raw, target, field, id, base, op, touched });
    }
    pres.sort_by(|a, b| {
        (a.target.to_id().to_string(), a.raw.at.path.as_str(), a.raw.at.line, a.raw.at.col)
            .cmp(&(b.target.to_id().to_string(), b.raw.at.path.as_str(), b.raw.at.line, b.raw.at.col))
    });
    let mut out = Vec::new();
    for p in &pres {
        let item = ev.idx.items.get(&p.target);
        let verdict = judge(ev, p, item, working);
        if verdict.state == OvrState::Inert(InertReason::UnknownField) {
            diags.push(
                Dg::new(codes::HN_S009, Severity::Error, &p.raw.at.path, p.raw.at.line, p.raw.at.col)
                    .scope(scope)
                    .msg(format!("`ovr {}` names a field the item does not have", p.raw.target_text))
                    .hint("use a field of the target (see `fairlead explain` for the kind), or declare it under [kinds.<k>.keys]")
                    .text(p.raw.text.as_str())
                    .done(),
            );
        }
        let exp = p.raw.exp.as_deref().and_then(|s| s.parse::<jiff::civil::Date>().ok());
        let applies = match &p.raw.extent {
            OvrExtent::Scope(a) => a.clone(),
            OvrExtent::File(_) => None,
        };
        let whole = matches!(&p.raw.extent, OvrExtent::Scope(None))
            || matches!(&p.raw.extent, OvrExtent::Scope(Some(a)) if a == "**");
        let applied = verdict.state == OvrState::Effective && verdict.approval != ApprovalStatus::Missing;
        out.push(Evaluated {
            ovr: Override {
                id: p.id.clone(),
                target: p.target.clone(),
                field: p.field.clone(),
                op: p.op,
                set: p.raw.set.clone(),
                why: p.raw.why.clone(),
                owner: p.raw.owner.clone(),
                exp,
                base: p.base.clone(),
                declared_in: scope.clone(),
                applies,
                at: p.raw.at.clone(),
                state: verdict.state,
                approval: verdict.approval,
                target_hash: Some(target_pin(item, &p.target, &p.touched)),
            },
            meta: OvrMeta { applied, whole, extent: p.raw.extent.clone(), used_grants: verdict.used_grants },
        });
        let _ = verdict.tighten;
    }
    out
}
```

In `registry/view.rs` replace `build_views` with the loop that calls `evaluate_scope` (the fold of applied overrides arrives in Task 12; here `working` is only read):

```rust
pub(crate) struct Built {
    pub(crate) views: BTreeMap<ScopeId, EffectiveView>,
    pub(crate) evals: Vec<Evaluated>,
}

pub(crate) fn build_views(
    ev: &Eval<'_>,
    locs: &BTreeMap<ItemId, Location>,
    mut raws: BTreeMap<ScopeId, Vec<RawOvr>>,
    diags: &mut Vec<Diagnostic>,
) -> Built {
    let (table, items) = (ev.idx.table, ev.idx.items);
    let mut views: BTreeMap<ScopeId, EffectiveView> = BTreeMap::new();
    let mut evals = Vec::new();
    for scope in table.in_depth_order() {
        let mut v = empty_view(&scope.id, table);
        if scope.state == ScopeState::Resolved {
            if let Some(p) = scope.parent.as_ref().and_then(|p| views.get(p)) {
                v.items = p.items.clone();
            }
            for (id, item) in items.iter().filter(|(k, _)| k.scope == scope.id) {
                if let Some(loc) = locs.get(id) {
                    v.items.insert(id.clone(), effective_item(item, loc));
                }
            }
            let mine = raws.remove(&scope.id).unwrap_or_default();
            evals.extend(evaluate_scope(ev, &scope.id, &mut v, mine, diags));
        }
        views.insert(scope.id.clone(), v);
    }
    Built { views, evals }
}
```
(imports: `crate::registry::ovr::{evaluate_scope, Eval, Evaluated, RawOvr}`.)

`registry/mod.rs`: add `pub(crate) meta: Vec<ovr::OvrMeta>` to `Registry`. `registry/resolve.rs`, after `collect_refs`:

```rust
    let raws = collect_raw(&idx, input.corpus, &mut diags);
    let waiver = table.root_control().map(|c| c.policies.waiver.clone()).unwrap_or(WaiverPolicy {
        max_exp_days: 90, budget: 25, max_renewals: 2, approval_required: true,
    });
    let ev = Eval {
        idx: &idx,
        as_of: input.as_of,
        grants: &[],
        approvals: &input.facts.approvals.approvals,
        horizon_days: i64::from(waiver.max_exp_days),
        waiver,
    };
    let built = view::build_views(&ev, &decls.locs, raws, &mut diags);
    let mut evals = built.evals;
    evals.sort_by(|a, b| {
        (&a.ovr.target, &a.ovr.declared_in, a.ovr.at.path.as_str(), a.ovr.at.line, a.ovr.at.col, &a.ovr.id)
            .cmp(&(&b.ovr.target, &b.ovr.declared_in, b.ovr.at.path.as_str(), b.ovr.at.line, b.ovr.at.col, &b.ovr.id))
    });
    let (overrides, meta): (Vec<_>, Vec<_>) = evals.into_iter().map(|e| (e.ovr, e.meta)).unzip();
    // struct literal: overrides, meta, views: built.views,
```
Note `idx` borrows `decls.items`, so move `decls.items` into the `Registry` only after the last use of `idx`/`ev` (re-order the struct literal at the end of `resolve`). Remove the earlier `view::build_views(&table, ...)` call.

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_scope_040 r_scope_041 r_syn_013 r_chk_026`
Expected: PASS (5 tests). If `r_scope_041` finds the table override with `applies = None`, `OvrDecl.applies` is not populated from the inline `applies` key: Verify the table field names with P3 and fix `ovr_table` in the kit only.

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): override construction, targets, fields and OvrId"
```

---

### Task 8: Grants, escalations, root-only vocabulary and proposals (R-SCOPE-036, 037, 053, 058)

**Files:**
- Modify: `crates/fairlead-core/src/registry/grant.rs`, `registry/mod.rs`, `registry/resolve.rs`
- Create: `crates/fairlead-core/tests/registry_grants.rs`

**Interfaces:**

Consumes: `ControlFile.{allows, reqs, violations}`, `AllowDecl`, `ReqDecl`, `Index::resolve_target`, `ScopeTable::is_ancestor_or_self`, `codes::{HN_C001, HN_C003}`.

Produces (contract exact): `Registry::grants() -> &[Allow]`, `Registry::requests() -> &[Req]`. Additive: `Registry::proposals() -> &[Proposal]`.
```rust
pub struct Proposal { pub from: ScopeId, pub to: ScopeId, pub target: String, pub why: String }
pub(crate) fn collect_grants(idx: &Index<'_>, as_of: AsOf, diags: &mut Vec<Diagnostic>) -> Vec<Allow>;
pub(crate) fn matching_grants<'a>(grants: &'a [Allow], table: &ScopeTable, as_of: AsOf, target: &ItemId,
                                  touched: &BTreeSet<String>, declared_in: &ScopeId) -> Vec<&'a Allow>;
pub(crate) fn collect_requests(table: &ScopeTable, diags: &mut Vec<Diagnostic>) -> Vec<Req>;
pub(crate) fn collect_proposals(table: &ScopeTable, requests: &[Req]) -> Vec<Proposal>;
```
Rules: a grant has `id = "<scope>:allow.<n>"` (n = 1-based rank of the grant among that scope's grants ordered by content, so the id does not depend on textual order), is `valid` only when authored in the target's home scope and every grantee lies below the granting scope; an invalid grant is kept in `grants()` with `valid = false`, never matches, and raises `HN-C003` (escalate with `[[req]]`). A grant matches an override iff valid, unexpired at `as_of`, same target, the override touches only the granted field (when one is named), and the override's scope is a grantee or below it. A `[[req]]` is never effective; its `to` must be a proper ancestor. A non-root `HN-C003` violation that P3 recorded and that no `[[req]]` already covers becomes a `Proposal` to the root.

- [ ] **Step 1: Write the failing tests** `crates/fairlead-core/tests/registry_grants.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::diag::codes::HN_C003;
use fairlead_core::registry::ScopeState;
use support::*;

const ALLOW: &str = "[[allow]]\ntarget = \"rules.r1\"\nfield = \"sev\"\nfor = [\"web\"]\nwhy = \"migrating\"\nexp = \"2027-03-31\"\n";

fn kit(root_extra: &str) -> Kit {
    Kit::new(&root_toml(&format!("\n{}{}", rule("r1", "error", "enforce", false), root_extra)))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/web/inner", "inner", &child_toml("inner", ""))
}

#[test]
fn r_scope_036__allow_in_the_home_scope_is_valid() {
    let reg = kit(ALLOW).resolve();
    let g = &reg.grants()[0];
    assert!(g.valid);
    assert_eq!(g.id, "root:allow.1");
    assert_eq!((g.granting.clone(), g.target.clone()), (sid("root"), iid("root", "rule.r1")));
    assert_eq!(g.field.as_deref(), Some("sev"));
    assert_eq!(g.for_scopes, vec![sid("web")]);
}

#[test]
fn r_scope_058__grant_authored_outside_the_home_scope_is_invalid_and_not_re_delegable() {
    let forward = "[[allow]]\ntarget = \"rule.r1\"\nfor = [\"inner\"]\nwhy = \"forwarded\"\nexp = \"2027-03-31\"\n";
    let reg = Kit::new(&root_toml(&format!("\n{}", rule("r1", "error", "enforce", false))))
        .scope("apps/web", "web", &child_toml("web", forward))
        .scope("apps/web/inner", "inner", &child_toml("inner", ""))
        .resolve();
    assert_eq!(reg.grants().len(), 1);
    assert!(!reg.grants()[0].valid);
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_C003 && d.location.path.as_str() == "apps/web/fairlead/control.toml"));
}

#[test]
fn r_scope_053__req_is_kept_sorted_and_never_effective() {
    let req = "[[req]]\nto = \"root\"\ntarget = \"layer-ranks.widget\"\nset = \"2\"\nwhy = \"needs a rank\"\nblocking = true\n";
    let bad = "[[req]]\nto = \"inner\"\ntarget = \"x.y\"\nwhy = \"sideways\"\n";
    let reg = kit("")
        .scope("apps/admin", "admin", &child_toml("admin", &format!("{req}{bad}")))
        .resolve();
    assert_eq!(reg.requests().len(), 1); // `to` must be a proper ancestor
    let r = &reg.requests()[0];
    assert_eq!((r.from.clone(), r.to.clone(), r.blocking), (sid("admin"), sid("root"), true));
    assert_eq!(r.set.as_deref(), Some("2"));
    assert_eq!(reg.view(&sid("admin")).items.len(), reg.view(&sid("root")).items.len());
}

#[test]
fn r_scope_037__child_cannot_set_vocabulary_keys_and_gets_an_escalation_proposal() {
    let child = child_toml("web", "[settings]\nowners = [\"@x\"]\n\n[policies]\n\"rules.*\" = \"open\"\n");
    let reg = Kit::new(&root_toml(&format!("\n{}", rule("r1", "error", "enforce", false))))
        .scope("apps/web", "web", &child)
        .resolve();
    assert_eq!(reg.scope(&sid("web")).unwrap().state, ScopeState::Resolved);
    assert!(reg.scope(&sid("web")).unwrap().owners.is_empty()); // owners are root-only in 0.1
    assert_eq!(reg.item(&iid("root", "rule.r1")).unwrap().policy, fairlead_core::config::Policy::Sealed);
    let p = reg.proposals();
    assert_eq!(p.len(), 1);
    assert_eq!((p[0].from.clone(), p[0].to.clone()), (sid("web"), sid("root")));
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core -- r_scope_036 r_scope_058 r_scope_053 r_scope_037`
Expected: FAIL: `index out of bounds: the len is 0` (no grants collected) and `no method named proposals found`.

- [ ] **Step 3: Implement.** Replace the body of `registry/grant.rs` after the two structs:

```rust
use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::diag::Severity;
use fairlead_syntax::ids::ScopeSlug;

use crate::diag::{codes, Diagnostic};
use crate::registry::item::Index;
use crate::registry::scope::{ScopeState, ScopeTable};
use crate::registry::Dg;
use crate::time::AsOf;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Proposal {
    pub from: ScopeId,
    pub to: ScopeId,
    pub target: String,
    pub why: String,
}

fn scope_id(s: &str) -> Option<ScopeId> {
    ScopeSlug::parse(s).ok().map(ScopeId)
}

pub(crate) fn collect_grants(idx: &Index<'_>, as_of: AsOf, diags: &mut Vec<Diagnostic>) -> Vec<Allow> {
    let _ = as_of; // expiry is judged at match time so an expired grant stays visible
    let mut out = Vec::new();
    for scope in idx.table.in_depth_order() {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else { continue };
        for a in &cf.allows {
            let bad = |msg: String| {
                Dg::new(codes::HN_C001, Severity::Error, &cf.path, 1, 1)
                    .scope(&scope.id)
                    .msg(msg)
                    .hint("fix the [[allow]] table")
                    .text(a.target.as_str())
                    .done()
            };
            let Some((target, suffix)) = idx.resolve_target(&scope.id, &a.target) else {
                diags.push(bad(format!("[[allow]] target \"{}\" does not resolve", a.target)));
                continue;
            };
            let Ok(exp) = a.exp.parse::<jiff::civil::Date>() else {
                diags.push(bad(format!("[[allow]] exp \"{}\" is not an ISO date", a.exp)));
                continue;
            };
            let mut grantees: Vec<ScopeId> = Vec::new();
            for s in &a.for_scopes {
                match scope_id(s).filter(|g| idx.table.get(g).is_some()) {
                    Some(g) => grantees.push(g),
                    None => diags.push(bad(format!("[[allow]] grantee \"{s}\" is not a scope"))),
                }
            }
            grantees.sort();
            grantees.dedup();
            let below = grantees.iter().all(|g| *g != scope.id && idx.table.is_ancestor_or_self(&scope.id, g));
            let valid = scope.id == target.scope && below;
            if !valid {
                diags.push(
                    Dg::new(codes::HN_C003, Severity::Error, &cf.path, 1, 1)
                        .scope(&scope.id)
                        .msg("an [[allow]] is valid only in the home scope of its target and only for scopes below it")
                        .hint("escalate with a [[req]] to the home scope (`fairlead new escalation`)")
                        .text(a.target.as_str())
                        .done(),
                );
            }
            out.push(Allow {
                id: String::new(), // numbered after sorting, below
                granting: scope.id.clone(),
                target,
                field: a.field.clone().or(suffix),
                for_scopes: grantees,
                why: a.why.clone(),
                exp,
                valid,
            });
        }
    }
    out.sort_by(|a, b| {
        (&a.granting, &a.target, &a.field, &a.for_scopes, &a.why, a.exp)
            .cmp(&(&b.granting, &b.target, &b.field, &b.for_scopes, &b.why, b.exp))
    });
    let mut seen: BTreeMap<ScopeId, usize> = BTreeMap::new();
    for g in &mut out {
        let n = seen.entry(g.granting.clone()).or_insert(0);
        *n += 1;
        g.id = format!("{}:allow.{n}", g.granting.0.as_str());
    }
    out
}

pub(crate) fn matching_grants<'a>(
    grants: &'a [Allow],
    table: &ScopeTable,
    as_of: AsOf,
    target: &ItemId,
    touched: &BTreeSet<String>,
    declared_in: &ScopeId,
) -> Vec<&'a Allow> {
    grants
        .iter()
        .filter(|g| {
            g.valid
                && g.exp >= as_of.0
                && g.target == *target
                && g.field.as_ref().is_none_or(|f| touched.iter().all(|k| k == f))
                && g.for_scopes.iter().any(|s| table.is_ancestor_or_self(s, declared_in))
        })
        .collect()
}

pub(crate) fn collect_requests(table: &ScopeTable, diags: &mut Vec<Diagnostic>) -> Vec<Req> {
    let mut out = Vec::new();
    for scope in table.in_depth_order() {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else { continue };
        for r in &cf.reqs {
            let to = scope_id(&r.to)
                .filter(|t| *t != scope.id && table.get(t).is_some() && table.is_ancestor_or_self(t, &scope.id));
            let Some(to) = to else {
                diags.push(
                    Dg::new(codes::HN_C001, Severity::Error, &cf.path, 1, 1)
                        .scope(&scope.id)
                        .msg(format!("[[req]] to \"{}\" is not an ancestor scope", r.to))
                        .hint("name an ancestor scope id in `to`")
                        .text(r.target.as_str())
                        .done(),
                );
                continue;
            };
            out.push(Req {
                from: scope.id.clone(),
                to,
                target: r.target.clone(),
                set: r.set.clone(),
                why: r.why.clone(),
                blocking: r.blocking,
                at: Location { path: cf.path.clone(), line: 0, col: 0 },
            });
        }
    }
    out.sort_by(|a, b| (&a.from, &a.to, &a.target, &a.why).cmp(&(&b.from, &b.to, &b.target, &b.why)));
    out
}

pub(crate) fn collect_proposals(table: &ScopeTable, requests: &[Req]) -> Vec<Proposal> {
    let mut out = Vec::new();
    for scope in table.in_depth_order().into_iter().filter(|s| s.id != table.root) {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else { continue };
        for v in cf.violations.iter().filter(|v| v.code == codes::HN_C003) {
            let target = if v.text.is_empty() { v.message.clone() } else { v.text.clone() };
            if requests.iter().any(|r| r.from == scope.id && r.target == target) {
                continue;
            }
            out.push(Proposal {
                from: scope.id.clone(),
                to: table.root.clone(),
                why: format!("scope \"{}\" cannot set \"{target}\" (non-delegable); the root decides", scope.id.0.as_str()),
                target,
            });
        }
    }
    out.sort_by(|a, b| (&a.from, &a.target).cmp(&(&b.from, &b.target)));
    out
}
```

`mod.rs`: add `pub use grant::Proposal;`, field `pub(crate) proposals: Vec<Proposal>`, accessor `pub fn proposals(&self) -> &[Proposal] { &self.proposals }`. `resolve.rs`: before building `Eval`:

```rust
    let grants = collect_grants(&idx, input.as_of, &mut diags);
    let requests = collect_requests(&table, &mut diags);
    let proposals = collect_proposals(&table, &requests);
    // Eval { grants: &grants, .. }; struct literal: grants, requests, proposals,
```
Imports needed in `grant.rs`: `crate::diag::Location`, `crate::registry::item::ItemId`, `crate::registry::scope::ScopeId` (already at the top from Task 1).

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_scope_036 r_scope_058 r_scope_053 r_scope_037`
Expected: PASS. If `r_scope_037` shows no proposal, P3 did not record a `HN-C003` violation for a child `[settings] owners`/`[policies]`; Verify against P3 and change only the child text (any key P3 reports as `HN-C003`).

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): allow grants, requests and escalation proposals"
```

---

### Task 9: Policy gating, tightening, grants at work, floor and subtree limits (R-SCOPE-031 to 035, 038, R-SYN-011, R-CHK-006)

**Files:**
- Modify: `crates/fairlead-core/src/registry/ovr.rs`, `registry/resolve.rs`
- Modify: `crates/fairlead-core/tests/registry_overrides.rs`

**Interfaces:**

Consumes: `matching_grants` (Task 8), `FLOOR_RULES` (Task 6), `Item.policy`, `field_allowed`, `codes::{HN_S016, HN_S012, HN_C003, HN_S009}`.

Produces: no new signature; `Override.state` now reflects policy. Precedence of reasons, first match wins: `FloorRule`, `OutsideSubtree`, `NonDelegable`, `UnknownField`, `DuplicateOvr` (Task 11), `VersionExpiry`, then (after tightening is decided) `SealedNoAllow`, `Incomplete`, then (Task 10) `UnknownOwner`, `Expired`, `Stale`, else `Effective`. Tightening is a change whose every touched field moves a root-ordered scale (`sev`, `stage` of a rule; a rule `glob` append; a rule `exclude` removal) strictly toward the stricter slot; it is effective on every policy without `why`/`owner`/`exp`. Open: nothing required (R-SCOPE-031; chapter 12's blanket "`ovr` MUST carry why" is overridden here, see Contract issues). NeedsException, and Sealed with a matching grant: `why`, `owner`, `exp` are required (R-SCOPE-032/033).

- [ ] **Step 1: Write the failing tests.** Append to `tests/registry_overrides.rs`:

```rust
use fairlead_core::diag::codes::{HN_C003, HN_S016};

const ALLOW: &str = "[[allow]]\ntarget = \"rules.r1\"\nfield = \"sev\"\nfor = [\"web\"]\nwhy = \"migrating\"\nexp = \"2027-03-31\"\n";

fn rooted(extra: &str) -> Kit {
    Kit::new(&root_toml(&format!("\n{}{extra}", rule("r1", "warn", "observe", false))))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md", "def tok.primary applies=color"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/admin/fairlead/docs/c.md", &doc(&["def cmp.card"]))
}

fn with_lines(kit: Kit, lines: &[&str]) -> Registry {
    kit.file("apps/web/src/a.ts", &format!("{}\nexport const a = 1;\n", lines.join("\n"))).resolve()
}

fn state_of(reg: &Registry, field: &str) -> OvrState {
    reg.overrides().iter().find(|o| o.set.contains_key(field)).unwrap().state
}

use fairlead_core::registry::Registry;

#[test]
fn r_scope_031__open_ovr_needs_no_reason() {
    let reg = with_lines(rooted(""), &["// fairlead: ovr cmp.button size=sm"]);
    assert_eq!(state_of(&reg, "size"), OvrState::Effective);
}

#[test]
fn r_scope_032__needs_exception_ovr_without_exp_is_inert_and_hn_s016() {
    let incomplete = with_lines(rooted(""), &["// fairlead: ovr tok.primary value=#fff why=\"brand\" owner=@andy"]);
    assert_eq!(state_of(&incomplete, "value"), OvrState::Inert(InertReason::Incomplete));
    assert!(incomplete.diagnostics().iter().any(|d| d.code == HN_S016));
    let complete = with_lines(rooted(""), &["// fairlead: ovr tok.primary value=#fff why=\"brand\" owner=@andy exp=2026-12-31"]);
    assert_eq!(state_of(&complete, "value"), OvrState::Effective);
}

#[test]
fn r_scope_033__sealed_ovr_is_inert_until_a_matching_allow_exists() {
    let line = "// fairlead: ovr rule.r1 sev=info why=\"x\" owner=@andy exp=2026-12-31";
    let without = with_lines(rooted(""), &[line]);
    assert_eq!(state_of(&without, "sev"), OvrState::Inert(InertReason::SealedNoAllow));
    let with = with_lines(rooted(ALLOW), &[line]);
    assert_eq!(state_of(&with, "sev"), OvrState::Effective);
    // a grant covers its grantee only: the sibling stays inert
    let sib = rooted(ALLOW).file("apps/admin/src/a.ts", &format!("{line}\nexport const a = 1;\n")).resolve();
    let o = sib.overrides().iter().find(|o| o.declared_in == sid("admin")).unwrap();
    assert_eq!(o.state, OvrState::Inert(InertReason::SealedNoAllow));
}

#[test]
fn r_scope_034__tightening_is_free_on_every_policy_loosening_is_not() {
    let up = with_lines(rooted(""), &["// fairlead: ovr rule.r1 sev=error stage=enforce"]);
    assert_eq!(state_of(&up, "sev"), OvrState::Effective); // warn -> error, no why/owner/exp
    let down = with_lines(rooted(""), &["// fairlead: ovr rule.r1 sev=info"]);
    assert_eq!(state_of(&down, "sev"), OvrState::Inert(InertReason::SealedNoAllow));
}

#[test]
fn r_scope_035__a_child_cannot_change_the_policy_of_an_inherited_item() {
    let reg = with_lines(rooted(""), &["// fairlead: ovr cmp.button policy=sealed"]);
    assert_eq!(state_of(&reg, "policy"), OvrState::Inert(InertReason::NonDelegable));
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_C003));
}

#[test]
fn r_scope_038__inert_lines_never_abort_the_valid_ones() {
    let reg = with_lines(
        rooted(""),
        &[
            "// fairlead: ovr rule.secret-like sev=info why=\"x\" owner=@andy exp=2026-12-31",
            "// fairlead: ovr admin:cmp.card size=sm",
            "// fairlead: ovr cmp.button nosuch=1",
            "// fairlead: ovr cmp.button kw+=a",
        ],
    );
    assert_eq!(state_of(&reg, "sev"), OvrState::Inert(InertReason::FloorRule));
    let outside = reg.overrides().iter().find(|o| o.target == iid("admin", "cmp.card")).unwrap();
    assert_eq!(outside.state, OvrState::Inert(InertReason::OutsideSubtree));
    assert_eq!(state_of(&reg, "nosuch"), OvrState::Inert(InertReason::UnknownField));
    assert_eq!(state_of(&reg, "kw"), OvrState::Effective);
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core -- r_scope_031 r_scope_032 r_scope_033 r_scope_034 r_scope_035 r_scope_038`
Expected: FAIL: `r_scope_032` left `Effective` right `Inert(Incomplete)` (Task 7's `judge` only knows `UnknownField`).

- [ ] **Step 3: Implement.** In `registry/ovr.rs` replace `Verdict` and `judge`, and the `HN_S009` block of `evaluate_scope`. Imports to add: `fairlead_syntax::diag::DiagCode`, `crate::config::Policy`, `crate::registry::grant::matching_grants`.

```rust
struct Verdict {
    state: OvrState,
    approval: ApprovalStatus,
    tighten: bool,
    used_grants: Vec<String>,
    diag: Option<(DiagCode, String, &'static str)>,
}

impl Verdict {
    fn inert(reason: InertReason, diag: Option<(DiagCode, String, &'static str)>) -> Verdict {
        Verdict { state: OvrState::Inert(reason), approval: ApprovalStatus::NotRequired, tighten: false, used_grants: Vec::new(), diag }
    }
}

fn looks_like_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn rank(field: &str, v: &str) -> Option<u8> {
    match (field, v) {
        ("sev", "info") | ("stage", "observe") => Some(0),
        ("sev", "warn") | ("stage", "warn") => Some(1),
        ("sev", "error") | ("stage", "enforce") => Some(2),
        _ => None,
    }
}

/// R-SCOPE-034, R-CHK-006: every touched field moves toward the stricter slot.
fn tightening(it: &Item, working: &EffectiveView, p: &Pre) -> bool {
    if it.id.kind.as_str() != "rule" || p.raw.set.is_empty() {
        return false;
    }
    let cur = working.items.get(&it.id);
    p.raw.set.iter().all(|(k, ch)| match (k.as_str(), ch) {
        ("sev" | "stage", FieldChange::Replace(FieldValue::Scalar(new))) => {
            let now = cur.and_then(|c| c.fields.get(k)).map(|f| &f.value).or(it.fields.get(k));
            match (now, rank(k, new)) {
                (Some(FieldValue::Scalar(c)), Some(n)) => rank(k, c).is_some_and(|cr| n > cr),
                _ => false,
            }
        }
        ("glob", FieldChange::Append(_)) => true,
        ("exclude", _) => p.op == OvrOp::Remove,
        _ => false,
    })
}

fn judge(ev: &Eval<'_>, p: &Pre, item: Option<&Item>, working: &EffectiveView, dup: bool) -> Verdict {
    let table = ev.idx.table;
    let scope = &p.raw.scope;
    let is_floor = p.target.kind.as_str() == "rule" && FLOOR_RULES.contains(&p.target.name.as_str());
    let Some(it) = item.filter(|_| !is_floor) else {
        return Verdict::inert(InertReason::FloorRule, None); // HN-X003 is derived by P5
    };
    if !table.is_ancestor_or_self(&it.id.scope, scope) {
        return Verdict::inert(
            InertReason::OutsideSubtree,
            Some((codes::HN_C003, format!("`ovr {}` targets an item outside this scope's ancestry", p.raw.target_text),
                "exceptions never escape their subtree; escalate with a [[req]]")),
        );
    }
    // [T11: applies glob escaping the subtree]
    if p.touched.contains("policy") {
        return Verdict::inert(
            InertReason::NonDelegable,
            Some((codes::HN_C003, "only the home scope may set an item's policy".to_string(),
                "set `policy=` on the def in the home scope, or escalate with a [[req]]")),
        );
    }
    if p.touched.iter().any(|k| !field_allowed(ev, it, k)) {
        return Verdict::inert(
            InertReason::UnknownField,
            Some((codes::HN_S009, format!("`ovr {}` names a field the item does not have", p.raw.target_text),
                "use a field of the target or declare it under [kinds.<k>.keys]")),
        );
    }
    let _ = dup; // [T11: duplicate]
    let exp_text = p.raw.exp.as_deref();
    if p.raw.until || exp_text.is_some_and(looks_like_version) {
        return Verdict::inert(
            InertReason::VersionExpiry,
            Some((codes::HN_S012, "version-based expiry needs seals (0.2); the override is dropped".to_string(),
                "write exp=YYYY-MM-DD")),
        );
    }
    let bad_date = exp_text.is_some_and(|s| s.parse::<jiff::civil::Date>().is_err());
    let bad_op = p.raw.op_text.as_deref().is_some_and(|s| parse_op(s).is_none());
    if bad_date || bad_op {
        return Verdict::inert(
            InertReason::Incomplete,
            Some((codes::HN_S016, format!("`ovr {}` has an invalid exp or op", p.raw.target_text),
                "exp is an ISO date; op is loosen|tighten|set|replace|add|remove")),
        );
    }
    let tighten = tightening(it, working, p);
    let mut used = Vec::new();
    let mut need_complete = false;
    if !tighten {
        match it.policy {
            Policy::Open => {}
            Policy::NeedsException => need_complete = true,
            Policy::Sealed => {
                let g = matching_grants(ev.grants, table, ev.as_of, &p.target, &p.touched, scope);
                if g.is_empty() {
                    return Verdict::inert(InertReason::SealedNoAllow, None); // HN-X005 is derived by P5
                }
                used = g.iter().map(|a| a.id.clone()).collect();
                need_complete = true;
            }
        }
    }
    if need_complete {
        let have = [("why", p.raw.why.is_some()), ("owner", p.raw.owner.is_some()), ("exp", exp_text.is_some())];
        let missing: Vec<&str> = have.iter().filter(|t| !t.1).map(|t| t.0).collect();
        if !missing.is_empty() {
            return Verdict::inert(
                InertReason::Incomplete,
                Some((codes::HN_S016, format!("`ovr {}` is missing {}", p.raw.target_text, missing.join(", ")),
                    "add why=, owner= and exp=YYYY-MM-DD")),
            );
        }
    }
    // [T10: owner, expiry, stale, approval]
    Verdict { state: OvrState::Effective, approval: ApprovalStatus::NotRequired, tighten, used_grants: used, diag: None }
}
```

In `evaluate_scope` replace the `if verdict.state == ...UnknownField {...}` block with:

```rust
        if let Some((code, msg, hint)) = &verdict.diag {
            diags.push(
                Dg::new(*code, Severity::Error, &p.raw.at.path, p.raw.at.line, p.raw.at.col)
                    .scope(scope)
                    .msg(msg.clone())
                    .hint(*hint)
                    .text(p.raw.text.as_str())
                    .done(),
            );
        }
```

Wire the grants into `resolve.rs`: build `grants` before `Eval` (Task 8) and set `grants: &grants`.

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_scope_031 r_scope_032 r_scope_033 r_scope_034 r_scope_035 r_scope_038 r_scope_040 r_scope_041 r_syn_013`
Expected: PASS. If `r_scope_034` shows `Inert(Incomplete)` for the tightening line, `tightening` compared against a stale base: check that `working` already holds the rule item (Task 4 `build_views` seeds it before `evaluate_scope`).

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): override policy gating, tightening, grants, floor and subtree limits"
```

---

### Task 10: Owners, expiry, approvals, horizon, stale pin (R-SCOPE-045, 046, 047, 049, R-CHK-025, 026, 053, R-AGT-012, R-SYN-011)

**Files:**
- Modify: `crates/fairlead-core/src/registry/ovr.rs`, `registry/resolve.rs`
- Modify: `crates/fairlead-core/tests/registry_overrides.rs`

**Interfaces:**

Consumes: `Approval.{fp, renewal}`, `WaiverPolicy.{approval_required, max_exp_days}`, `ExceptionsConfig.max_days`, `Scope.owners`, `base32`.

Produces:
```rust
pub fn max_horizon_days(root: Option<&ControlFile>) -> i64;    // min(policies.waiver.max-exp, exceptions.max-days), default 90 (C-18)
```
`Override.approval` is `NotRequired` for Open and for tightening, else (needs-exception, or sealed with a matching grant, when `approval_required`) `Approved { renewal }` if an `Approval.fp` equals `Override.id.0` (highest renewal), else `Missing`. `UnknownOwner` when `owner=` is no handle of the chain's `owners` (HN-X012). `Expired` when `exp < as_of` (`exp == as_of` still counts). `Stale` when the override carries `x-pin=<12 base32>` that differs from the pin of the target's declared form now (P4-7, `Override.target_hash`). An override changes values only if `state == Effective`, `approval != Missing` and `exp` is within the horizon of `as_of` (P4-6); that last gate is applied by the view fold (Task 12) through `OvrMeta.applied`.

- [ ] **Step 1: Write the failing tests.** Append to `tests/registry_overrides.rs`:

```rust
use fairlead_core::diag::codes::{HN_S012, HN_X012};
use fairlead_core::registry::{ovr_fingerprint, ApprovalStatus};

fn tok_line(exp: &str, owner: &str, why: &str) -> String {
    format!("// fairlead: ovr tok.primary value=#fff why=\"{why}\" owner={owner} exp={exp}")
}

fn tok_state(kit: Kit, line: &str) -> (OvrState, ApprovalStatus) {
    let reg = with_lines(kit, &[line]);
    let o = &reg.overrides()[0];
    (o.state, o.approval)
}

#[test]
fn r_scope_046__expired_exception_is_ineffective_and_the_boundary_day_is_live() {
    let (s, _) = tok_state(rooted(""), &tok_line("2026-10-08", "@andy", "brand"));
    assert_eq!(s, OvrState::Expired);
    let (s, _) = tok_state(rooted(""), &tok_line("2026-10-09", "@andy", "brand"));
    assert_eq!(s, OvrState::Effective); // exp == as_of is not yet expired
}

#[test]
fn r_scope_045__version_shaped_expiry_is_inert_with_hn_s012() {
    let ctl = child_toml("web", &ovr_table("tok.primary", "value = \"#fff\"\nwhy = \"x\"\nowner = \"@andy\"\nexp = \"1.3.0\""));
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def tok.primary applies=color"]))
        .scope("apps/web", "web", &ctl)
        .resolve();
    assert_eq!(reg.overrides()[0].state, OvrState::Inert(InertReason::VersionExpiry));
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_S012));
}

#[test]
fn r_chk_053__owner_must_be_a_handle_in_owners() {
    let reg = with_lines(rooted(""), &[&tok_line("2026-12-31", "@nobody", "brand")]);
    assert_eq!(reg.overrides()[0].state, OvrState::Inert(InertReason::UnknownOwner));
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_X012));
}

#[test]
fn r_chk_025__needs_exception_waiver_needs_a_matching_approval() {
    let target = iid("root", "tok.primary");
    let (s, a) = tok_state(rooted(""), &tok_line("2026-12-31", "@andy", "brand"));
    assert_eq!((s, a), (OvrState::Effective, ApprovalStatus::Missing));
    let fp = ovr_fingerprint(&target, &sid("web"), "brand", "@andy", Some(&date("2026-12-31")));
    let (s, a) = tok_state(rooted("").approve(fp, &target, 0), &tok_line("2026-12-31", "@andy", "brand"));
    assert_eq!((s, a), (OvrState::Effective, ApprovalStatus::Approved { renewal: 0 }));
    let (_, a) = tok_state(rooted(""), "// fairlead: ovr cmp.button size=sm");
    assert_eq!(a, ApprovalStatus::NotRequired); // open
}

#[test]
fn r_chk_026__re_dating_a_waiver_invalidates_its_approval() {
    let target = iid("root", "tok.primary");
    let old = ovr_fingerprint(&target, &sid("web"), "brand", "@andy", Some(&date("2026-12-31")));
    let (_, a) = tok_state(rooted("").approve(old, &target, 0), &tok_line("2027-01-05", "@andy", "brand"));
    assert_eq!(a, ApprovalStatus::Missing);
}

#[test]
fn r_agt_012__renewals_are_counted_in_the_approval_status() {
    let target = iid("root", "tok.primary");
    let fp = ovr_fingerprint(&target, &sid("web"), "brand", "@andy", Some(&date("2026-12-31")));
    let (_, a) = tok_state(rooted("").approve(fp, &target, 2), &tok_line("2026-12-31", "@andy", "brand"));
    assert_eq!(a, ApprovalStatus::Approved { renewal: 2 });
}

#[test]
fn r_scope_049__prose_claiming_approval_is_data() {
    let (_, a) = tok_state(rooted(""), &tok_line("2026-12-31", "@andy", "approved by lead -- ok by lead"));
    assert_eq!(a, ApprovalStatus::Missing);
}

#[test]
fn r_syn_011__why_owner_exp_are_required_only_where_the_policy_asks() {
    let open = with_lines(rooted(""), &["// fairlead: ovr cmp.button size=sm"]);
    assert_eq!(open.overrides()[0].state, OvrState::Effective);
    let tok = with_lines(rooted(""), &["// fairlead: ovr tok.primary value=#fff why=\"b\" exp=2026-12-31"]); // no owner
    assert_eq!(tok.overrides()[0].state, OvrState::Inert(InertReason::Incomplete));
    let sealed = with_lines(rooted(ALLOW), &["// fairlead: ovr rule.r1 sev=info why=\"x\" exp=2026-12-31"]);
    assert_eq!(sealed.overrides()[0].state, OvrState::Inert(InertReason::Incomplete));
}

#[test]
fn r_scope_047__pin_mismatch_marks_the_exception_stale() {
    let line = |pin: &str| format!("// fairlead: ovr rule.r1 sev=error x-pin={pin}");
    let probe = with_lines(rooted(""), &[&line("aaaaaaaaaaaa")]);
    assert_eq!(probe.overrides()[0].state, OvrState::Stale);
    let pin = fairlead_core::hash::base32(&probe.overrides()[0].target_hash.as_ref().unwrap().0, 12);
    let ok = with_lines(rooted(""), &[&line(&pin)]);
    assert_eq!(ok.overrides()[0].state, OvrState::Effective);
    // the ancestor changes the declared form: the pinned exception goes stale, it is not deleted
    let changed = Kit::new(&root_toml(&format!("\n{}", rule("r1", "info", "observe", false))))
        .scope("apps/web", "web", &child_toml("web", ""))
        .file("apps/web/src/a.ts", &format!("{}\nexport const a = 1;\n", line(&pin)))
        .resolve();
    assert_eq!(changed.overrides()[0].state, OvrState::Stale);
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core -- r_scope_046 r_scope_045 r_chk_053 r_chk_025 r_chk_026 r_agt_012 r_scope_047`
Expected: FAIL: `r_scope_046` left `Effective` right `Expired`.

- [ ] **Step 3: Implement.** In `registry/ovr.rs` add `Verdict::plain` and replace the `// [T10: ...]` anchor in `judge` with the block below; imports: `crate::hash::base32`, `crate::config::ControlFile`.

```rust
impl Verdict {
    fn plain(state: OvrState) -> Verdict {
        Verdict { state, approval: ApprovalStatus::NotRequired, tighten: false, used_grants: Vec::new(), diag: None }
    }
}

pub fn max_horizon_days(root: Option<&ControlFile>) -> i64 {
    let a = root.map(|c| i64::from(c.policies.waiver.max_exp_days)).unwrap_or(90);
    let b = root.and_then(|c| c.exceptions.max_days).map(i64::from).unwrap_or(a);
    a.min(b)
}

fn beyond_horizon(ev: &Eval<'_>, exp: Option<jiff::civil::Date>) -> bool {
    exp.is_some_and(|d| {
        d.since(ev.as_of.0).map(|s| i64::from(s.get_days())).unwrap_or(i64::MAX) > ev.horizon_days
    }) // Verify: jiff 0.2 Date::since returns Result<Span, Error>; Span::get_days() -> i32
}
```

```rust
    // [T10] owner, expiry, stale, approval
    if let Some(owner) = p.raw.owner.as_deref().filter(|_| !tighten) {
        let known = table
            .chain_ids(scope)
            .into_iter()
            .filter_map(|s| table.get(s))
            .any(|s| s.owners.iter().any(|o| o.handle == owner));
        if !known {
            return Verdict::inert(
                InertReason::UnknownOwner,
                Some((codes::HN_X012, format!("owner \"{owner}\" is not a handle in `owners`"),
                    "use a handle from the root [settings] owners list")),
            );
        }
    }
    let exp = exp_text.and_then(|s| s.parse::<jiff::civil::Date>().ok());
    if exp.is_some_and(|d| d < ev.as_of.0) {
        return Verdict::plain(OvrState::Expired); // HN-X001 is derived by P5
    }
    if let Some(pin) = p.raw.pin.as_deref() {
        let now = base32(&target_pin(Some(it), &p.target, &p.touched).0, 12);
        if pin != now {
            return Verdict::plain(OvrState::Stale); // HN-X010 is derived by P5
        }
    }
    let approval = if need_complete && ev.waiver.approval_required {
        match ev.approvals.iter().filter(|a| a.fp == p.id.0).map(|a| a.renewal).max() {
            Some(renewal) => ApprovalStatus::Approved { renewal },
            None => ApprovalStatus::Missing,
        }
    } else {
        ApprovalStatus::NotRequired
    };
    Verdict { state: OvrState::Effective, approval, tighten, used_grants: used, diag: None }
```
(delete the trailing `Verdict { state: OvrState::Effective, approval: ApprovalStatus::NotRequired, ... }` line of Task 9.)

In `evaluate_scope` change the `applied` line to:

```rust
        let applied = verdict.state == OvrState::Effective
            && verdict.approval != ApprovalStatus::Missing
            && !beyond_horizon(ev, exp);
```
and in `resolve.rs` set `horizon_days: max_horizon_days(table.root_control())` in `Eval` (replacing the Task 7 `i64::from(waiver.max_exp_days)`).

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_scope_046 r_scope_045 r_chk_053 r_chk_025 r_chk_026 r_agt_012 r_scope_047 r_scope_03`
Expected: PASS. If the first `r_scope_047` assertion fails, `x-pin` was consumed by P2 as an unknown key: Verify the extractor keeps `x-` args (R-SYN-009).

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): owners, expiry, approvals, horizon and stale pins"
```

---

### Task 11: Overlapping exceptions and `applies` containment (R-SCOPE-025, 064)

**Files:**
- Modify: `crates/fairlead-core/src/registry/ovr.rs`
- Modify: `crates/fairlead-core/tests/registry_overrides.rs`

**Interfaces:**

Consumes: `RawOvr.extent`, `Pre.touched`, `Pre.base`, `is_within` (Task 2), `codes::{HN_X014, HN_C001}`.

Produces: no new signature. Two overrides of one scope overlap iff they have the same target, share a touched field and their extents overlap (same file; a file and a scope-level glob that may cover it; two globs whose literal directory prefixes nest, or either is whole-scope). Overlap is a conservative over-approximation. Both become `Inert(DuplicateOvr)` with `HN-X014` unless one carries `base=` naming the other. Overrides in sibling scopes never meet (each evaluates inside its own scope). An `applies` glob with a `..` segment, a leading `/`, a `\` or a `:` is `Inert(OutsideSubtree)` with `HN-C001`.

- [ ] **Step 1: Write the failing tests.** Append to `tests/registry_overrides.rs`:

```rust
use fairlead_core::diag::codes::{HN_C001, HN_X014};

#[test]
fn r_scope_064__two_overlapping_ovrs_of_one_field_are_both_inert_hn_x014() {
    let reg = with_lines(rooted(""), &["// fairlead: ovr cmp.button size=sm", "// fairlead: ovr cmp.button size=lg"]);
    assert!(reg.overrides().iter().all(|o| o.state == OvrState::Inert(InertReason::DuplicateOvr)));
    assert_eq!(reg.diagnostics().iter().filter(|d| d.code == HN_X014).count(), 2);
}

#[test]
fn r_scope_064__a_base_chain_is_not_a_duplicate_and_siblings_never_conflict() {
    let target = iid("root", "cmp.button");
    let first = ovr_fingerprint(&target, &sid("web"), "a", "", None);
    let chained = format!("// fairlead: ovr cmp.button size=lg why=\"b\" base={first}");
    let reg = with_lines(rooted(""), &["// fairlead: ovr cmp.button size=sm why=\"a\"", &chained]);
    assert!(reg.overrides().iter().all(|o| o.state == OvrState::Effective));
    let line = "// fairlead: ovr cmp.button size=sm\nexport const a = 1;\n";
    let sib = rooted("").file("apps/web/src/a.ts", line).file("apps/admin/src/a.ts", line).resolve();
    assert_eq!(sib.overrides().len(), 2);
    assert!(sib.overrides().iter().all(|o| o.state == OvrState::Effective));
}

#[test]
fn r_scope_025__applies_glob_cannot_leave_the_owning_subtree() {
    let table = |applies: &str| {
        child_toml("web", &ovr_table("cmp.button", &format!("size = \"sm\"\napplies = \"{applies}\"")))
    };
    let run = |applies: &str| {
        Kit::new(&root_toml(""))
            .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
            .scope("apps/web", "web", &table(applies))
            .resolve()
    };
    let bad = run("../**");
    assert_eq!(bad.overrides()[0].state, OvrState::Inert(InertReason::OutsideSubtree));
    assert!(bad.diagnostics().iter().any(|d| d.code == HN_C001));
    assert_eq!(run("/etc/**").overrides()[0].state, OvrState::Inert(InertReason::OutsideSubtree));
    assert_eq!(run("src/**").overrides()[0].state, OvrState::Effective);
}

#[test]
fn r_scope_064__disjoint_globs_in_one_scope_are_independent() {
    let two = format!(
        "{}{}",
        ovr_table("cmp.button", "size = \"sm\"\napplies = \"src/**\""),
        ovr_table("cmp.button", "size = \"lg\"\napplies = \"docs/**\"")
    );
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
        .scope("apps/web", "web", &child_toml("web", &two))
        .resolve();
    assert!(reg.overrides().iter().all(|o| o.state == OvrState::Effective));
}
```
(`Fingerprint` must implement `Display`; Verify with P3, otherwise use its `as_str()`.)

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core -- r_scope_064 r_scope_025`
Expected: FAIL: `r_scope_064__two_overlapping...` assertion false (states stay `Effective`).

- [ ] **Step 3: Implement.** In `registry/ovr.rs`:

```rust
fn literal_prefix(g: &str) -> &str {
    let end = g.find(['*', '?', '[', '{']).unwrap_or(g.len());
    let head = g.get(..end).unwrap_or("");
    match head.rfind('/') {
        Some(i) if end < g.len() => head.get(..i).unwrap_or(""),
        _ => head,
    }
}

fn glob_escapes(g: &str) -> bool {
    g.starts_with('/') || g.contains('\\') || g.contains(':') || g.split('/').any(|s| s == "..")
}

fn extents_overlap(a: &OvrExtent, b: &OvrExtent, dir: &str) -> bool {
    let rel = |p: &RepoPath| -> String {
        p.as_str().strip_prefix(dir).map(|r| r.trim_start_matches('/')).unwrap_or(p.as_str()).to_string()
    };
    let whole = |g: &Option<String>| g.as_deref().is_none_or(|g| g == "**");
    match (a, b) {
        (OvrExtent::File(x), OvrExtent::File(y)) => x == y,
        (OvrExtent::File(x), OvrExtent::Scope(g)) | (OvrExtent::Scope(g), OvrExtent::File(x)) => {
            whole(g) || g.as_deref().is_some_and(|g| is_within(&rel(x), literal_prefix(g)))
        }
        (OvrExtent::Scope(x), OvrExtent::Scope(y)) => {
            if whole(x) || whole(y) {
                return true;
            }
            let (px, py) = (literal_prefix(x.as_deref().unwrap_or("")), literal_prefix(y.as_deref().unwrap_or("")));
            is_within(px, py) || is_within(py, px)
        }
    }
}

/// R-SCOPE-064: `true` for every override that conflicts with another one of the same scope.
fn overlap_flags(pres: &[Pre], dir: &str) -> Vec<bool> {
    let mut flags = vec![false; pres.len()];
    for (i, a) in pres.iter().enumerate() {
        for (j, b) in pres.iter().enumerate().skip(i + 1) {
            let chained = a.base.as_ref() == Some(&b.id) || b.base.as_ref() == Some(&a.id);
            if a.target == b.target
                && a.touched.intersection(&b.touched).next().is_some()
                && !chained
                && extents_overlap(&a.raw.extent, &b.raw.extent, dir)
            {
                for k in [i, j] {
                    if let Some(f) = flags.get_mut(k) {
                        *f = true;
                    }
                }
            }
        }
    }
    flags
}
```
(add `use crate::registry::scope::is_within;`). In `judge`, replace the two anchors:

```rust
    if let OvrExtent::Scope(Some(g)) = &p.raw.extent {
        if glob_escapes(g) {
            return Verdict::inert(
                InertReason::OutsideSubtree,
                Some((codes::HN_C001, format!("applies glob \"{g}\" leaves the owning subtree"),
                    "write the glob relative to this scope, without .. or a leading /")),
            );
        }
    }
```
```rust
    if dup {
        return Verdict::inert(
            InertReason::DuplicateOvr,
            Some((codes::HN_X014, format!("another `ovr` of {} in this scope overlaps this one", p.raw.target_text),
                "merge them, or chain the newer one with base=<exception id>")),
        );
    }
```
and delete `let _ = dup;`. In `evaluate_scope`, before the loop `for p in &pres {` compute and iterate with the flags:

```rust
    let dir = ev.idx.table.get(scope).map(|s| s.dir.as_str().trim_matches('/').to_string()).unwrap_or_default();
    let flags = overlap_flags(&pres, &dir);
    for (p, dup) in pres.iter().zip(flags) {
        // ... judge(ev, p, item, working, dup)
```

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core -- r_scope_064 r_scope_025 r_scope_041 r_scope_03`
Expected: PASS (`r_scope_041` still passes: it asserts counts and paths only).

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): DUP-OVR detection and applies containment"
```

---

### Task 12: Effective views, fold order, provenance, `view_at`, conventions, digest (R-SCOPE-020, 021, 022, 027, R-ARCH-040, 041, 042, 043, R-SCOPE-018 consumption)

**Files:**
- Modify: `crates/fairlead-core/src/registry/view.rs`, `ovr.rs`, `mod.rs`, `resolve.rs`
- Create: `crates/fairlead-core/tests/registry_views.rs`

**Interfaces:**

Consumes: `Evaluated`, `OvrMeta`, `Override.{set, op, id}`, `ConventionDecl.{glob, action}`, `globset::GlobBuilder`, `ContentHash`.

Produces (contract): `Registry::view(&self, &ScopeId) -> &EffectiveView` with provenance on every field. Additive:
```rust
impl Registry {
    pub fn view_at(&self, path: &RepoPath) -> EffectiveView;                       // E(path): file-local and glob-limited ovrs overlaid last
    pub fn glob_matches(&self, scope: &ScopeId, glob: &str, path: &RepoPath) -> bool; // scope-relative, [[convention]] transparent segments invisible
}
pub(crate) fn apply_ovr(view: &mut EffectiveView, o: &Override, grants: &[String]);
pub(crate) fn visible_rel(table: &ScopeTable, scope: &ScopeId, path: &RepoPath) -> String;
pub(crate) fn own_digests(table: &ScopeTable, corpus: &Corpus) -> BTreeMap<ScopeId, ContentHash>;
pub fn view_signature(view: &EffectiveView) -> BTreeMap<String, BTreeMap<String, FieldValue>>; // bare "kind.name" -> field values
```
Fold order (R-ARCH-040): chain root to nearest; per scope its declared items, then its overrides, base-chained ones after their base, then bytewise by target ID and source path. `view(scope)` applies only whole-scope, effective, approved, in-horizon overrides (P4-4, P4-6). Merge by field type (R-SCOPE-022): scalar replaced; list appended (`k+=`), no duplicates; `op = remove` removes the listed values; appending to a scalar is `Inert(UnknownField)` plus `HN-S009`. `authored_digest` = blake3 over the scope id, its control file hash and (path, hash) of its files that carry `def`/`ovr`/`use`, so editing one scope changes its digest and no other (R-SCOPE-027).

- [ ] **Step 1: Write the failing tests** `crates/fairlead-core/tests/registry_views.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::registry::{ovr_fingerprint, FieldValue, OvrState};
use support::*;

fn scal(s: &str) -> FieldValue {
    FieldValue::Scalar(s.into())
}

fn kit(root_extra: &str, web: &str) -> Kit {
    Kit::new(&root_toml(&format!("\n{}{root_extra}", rule("r1", "warn", "observe", false))))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md kw=ui", "def tok.primary applies=color value=#111"]))
        .scope("apps/web", "web", &child_toml("web", web))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
}

#[test]
fn r_scope_021__child_view_folds_the_chain_root_first_and_siblings_stay_apart() {
    let tighten = ovr_table("rule.r1", "sev = \"error\"");
    let reg = kit("", &tighten).resolve();
    let sev = |s: &str| reg.view(&sid(s)).items[&iid("root", "rule.r1")].fields["sev"].value.clone();
    assert_eq!((sev("root"), sev("web"), sev("admin")), (scal("warn"), scal("error"), scal("warn")));
    let f = &reg.view(&sid("web")).items[&iid("root", "rule.r1")].fields["sev"];
    assert_eq!(f.provenance.home, sid("root"));
    assert_eq!(f.provenance.declared_at.path.as_str(), "fairlead/control.toml");
    assert_eq!(f.provenance.applied, vec![reg.overrides()[0].id.clone()]);
    assert_eq!(reg.view(&sid("web")).chain, vec![sid("root"), sid("web")]);
}

#[test]
fn r_scope_022__merge_by_field_type_and_provenance_lists_grants() {
    let tables = format!(
        "{}{}",
        ovr_table("cmp.button", "op = \"add\"\nkw = [\"a\", \"ui\"]"),
        ovr_table("cmp.button", "size = \"lg\"")
    );
    let reg = kit("", &tables).resolve();
    let it = &reg.view(&sid("web")).items[&iid("root", "cmp.button")];
    assert_eq!(it.fields["size"].value, scal("lg")); // scalar replaced
    assert_eq!(it.fields["kw"].value, FieldValue::List(vec!["ui".into(), "a".into()])); // appended, no duplicate
    let remove = ovr_table("cmp.button", "op = \"remove\"\nkw = [\"ui\"]");
    let reg = kit("", &remove).resolve();
    assert_eq!(reg.view(&sid("web")).items[&iid("root", "cmp.button")].fields["kw"].value, FieldValue::List(vec![]));
    // sealed + allow + approval: the grant id is recorded as provenance
    let allow = "[[allow]]\ntarget = \"rules.r1\"\nfield = \"sev\"\nfor = [\"web\"]\nwhy = \"m\"\nexp = \"2027-03-31\"\n";
    let line = ovr_table("rule.r1", "sev = \"info\"\nwhy = \"m\"\nowner = \"@andy\"\nexp = \"2026-12-31\"");
    let fp = ovr_fingerprint(&iid("root", "rule.r1"), &sid("web"), "m", "@andy", Some(&date("2026-12-31")));
    let reg = kit(allow, &line).approve(fp, &iid("root", "rule.r1"), 0).resolve();
    let f = &reg.view(&sid("web")).items[&iid("root", "rule.r1")].fields["sev"];
    assert_eq!((f.value.clone(), f.provenance.grants.clone()), (scal("info"), vec!["root:allow.1".to_string()]));
}

#[test]
fn r_arch_041__unapproved_or_beyond_horizon_overrides_do_not_change_values() {
    let tok = |exp: &str| ovr_table("tok.primary", &format!("value = \"#fff\"\nwhy = \"b\"\nowner = \"@andy\"\nexp = \"{exp}\""));
    let value = |r: &fairlead_core::registry::Registry| r.view(&sid("web")).items[&iid("root", "tok.primary")].fields["value"].value.clone();
    let t = iid("root", "tok.primary");
    let fp = |exp: &str| ovr_fingerprint(&t, &sid("web"), "b", "@andy", Some(&date(exp)));
    assert_eq!(value(&kit("", &tok("2026-12-31")).resolve()), scal("#111")); // no approval
    assert_eq!(value(&kit("", &tok("2026-12-31")).approve(fp("2026-12-31"), &t, 0).resolve()), scal("#fff"));
    let far = kit("", &tok("2027-06-01")).approve(fp("2027-06-01"), &t, 0).resolve();
    assert_eq!(far.overrides()[0].state, OvrState::Effective);
    assert_eq!(value(&far), scal("#111")); // beyond the 90-day horizon (R-AGT-012)
}

#[test]
fn r_scope_021__file_local_ovr_is_overlaid_last_by_view_at_only() {
    let reg = kit("", "")
        .file("apps/web/src/a.ts", "// fairlead: ovr cmp.button size=sm\nexport const a = 1;\n")
        .file("apps/web/src/b.ts", "export const b = 1;\n")
        .resolve();
    let size = |v: fairlead_core::registry::EffectiveView| v.items[&iid("root", "cmp.button")].fields["size"].value.clone();
    assert_eq!(size(reg.view(&sid("web")).clone()), scal("md"));
    assert_eq!(size(reg.view_at(&path("apps/web/src/a.ts"))), scal("sm"));
    assert_eq!(size(reg.view_at(&path("apps/web/src/b.ts"))), scal("md"));
}

#[test]
fn r_scope_018__transparent_convention_hides_route_group_segments_for_applies_matching() {
    let conv = "[[convention]]\nglob = \"app/(*)\"\ntransparent = true\n";
    let plain = Kit::new(&root_toml("")).resolve();
    let with = Kit::new(&root_toml(&format!("\n{conv}"))).resolve();
    let p = path("app/(app)/page.tsx");
    assert!(!plain.glob_matches(&sid("root"), "app/page.tsx", &p));
    assert!(with.glob_matches(&sid("root"), "app/page.tsx", &p));
    assert!(with.glob_matches(&sid("root"), "app/**/*.tsx", &p));
}

#[test]
fn r_scope_027__digest_changes_only_inside_the_edited_subtree() {
    let a = kit("", "").file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.card"])).resolve();
    let b = kit("", "").file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.card sum=\"x\""])).resolve();
    let d = |r: &fairlead_core::registry::Registry, s: &str| r.view(&sid(s)).authored_digest.clone();
    assert_ne!(d(&a, "web"), d(&b, "web"));
    assert_eq!(d(&a, "admin"), d(&b, "admin"));
    assert_eq!(d(&a, "root"), d(&b, "root"));
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core --test registry_views`
Expected: FAIL to compile (`no method named view_at`), then at runtime `r_scope_021__child_view...` left `warn` right `error` (Task 7/9 never apply overrides).

- [ ] **Step 3: Implement.** Append to `registry/view.rs` (imports: `std::collections::BTreeSet`, `globset::GlobBuilder`, `fairlead_syntax::directive::Verb`, `crate::config::ConventionAction`, `crate::extract::Corpus`, `crate::path::RepoPath`, `crate::registry::ovr::{FieldChange, OvrOp, Override}`, `crate::registry::item::FieldValue`, `crate::hash::{hash_raw, ContentHash}`); make `effective_item` `pub(crate)`:

```rust
fn merge(cur: Option<&FieldValue>, ch: &FieldChange, op: OvrOp) -> Option<FieldValue> {
    let drop_from = |l: &Vec<String>, rm: &[String]| FieldValue::List(l.iter().filter(|x| !rm.contains(x)).cloned().collect());
    match (ch, op) {
        (FieldChange::Replace(FieldValue::List(rm)), OvrOp::Remove) => match cur {
            Some(FieldValue::List(l)) => Some(drop_from(l, rm)),
            _ => None,
        },
        (FieldChange::Replace(FieldValue::Scalar(rm)), OvrOp::Remove) => match cur {
            Some(FieldValue::List(l)) => Some(drop_from(l, std::slice::from_ref(rm))),
            _ => None,
        },
        (FieldChange::Replace(v), _) => Some(v.clone()),
        (FieldChange::Append(items), _) => match cur {
            None => Some(FieldValue::List(items.clone())),
            Some(FieldValue::List(l)) => {
                let mut l = l.clone();
                for i in items {
                    if !l.contains(i) {
                        l.push(i.clone());
                    }
                }
                Some(FieldValue::List(l))
            }
            Some(FieldValue::Scalar(_)) => None,
        },
    }
}

/// Folds one effective override into a view, recording provenance (R-ARCH-042).
pub(crate) fn apply_ovr(view: &mut EffectiveView, o: &Override, grants: &[String]) {
    let Some(item) = view.items.get_mut(&o.target) else { return };
    for (key, change) in &o.set {
        let cur = item.fields.get(key).map(|f| f.value.clone());
        let Some(new) = merge(cur.as_ref(), change, o.op) else { continue };
        let entry = item.fields.entry(key.clone()).or_insert_with(|| EffectiveField {
            value: new.clone(),
            provenance: Provenance {
                home: o.target.scope.clone(),
                declared_at: o.at.clone(),
                applied: Vec::new(),
                grants: Vec::new(),
            },
        });
        entry.value = new;
        entry.provenance.applied.push(o.id.clone());
        for g in grants {
            if !entry.provenance.grants.contains(g) {
                entry.provenance.grants.push(g.clone());
            }
        }
    }
}

/// Path below the owning scope with `[[convention]] transparent` segments removed (R-SCOPE-018).
pub(crate) fn visible_rel(table: &ScopeTable, scope: &ScopeId, path: &RepoPath) -> String {
    let segs: Vec<&str> = path.as_str().split('/').collect();
    let seg_count = |d: &str| if d.is_empty() { 0 } else { d.split('/').count() };
    let mut hidden: BTreeSet<usize> = BTreeSet::new();
    for sid in table.chain_ids(scope) {
        let Some(sc) = table.get(sid) else { continue };
        let Some(cf) = sc.control.as_ref() else { continue };
        let nb = seg_count(sc.dir.as_str().trim_matches('/'));
        for c in cf.conventions.iter().filter(|c| matches!(c.action, ConventionAction::Transparent)) {
            let Ok(g) = GlobBuilder::new(&c.glob).literal_separator(true).build() else { continue };
            let m = g.compile_matcher();
            for end in (nb + 1)..=segs.len() {
                let prefix = segs.get(nb..end).map(|s| s.join("/")).unwrap_or_default();
                if m.is_match(&prefix) {
                    hidden.insert(end - 1);
                }
            }
        }
    }
    let owner_n = table.get(scope).map(|s| seg_count(s.dir.as_str().trim_matches('/'))).unwrap_or(0);
    let kept: Vec<&str> =
        segs.iter().enumerate().filter(|(i, _)| *i >= owner_n && !hidden.contains(i)).map(|(_, s)| *s).collect();
    kept.join("/")
}

pub(crate) fn own_digests(table: &ScopeTable, corpus: &Corpus) -> BTreeMap<ScopeId, ContentHash> {
    let mut parts: BTreeMap<ScopeId, Vec<u8>> = BTreeMap::new();
    for s in table.scopes.values() {
        let mut b = s.id.0.as_str().as_bytes().to_vec();
        if let Some(c) = &s.control {
            b.extend(c.content_hash.0);
        }
        parts.insert(s.id.clone(), b);
    }
    for (path, rec) in &corpus.records {
        let Some(sid) = table.scope_of(path) else { continue };
        let authored = rec
            .extraction
            .as_ref()
            .is_some_and(|e| e.directives.iter().any(|d| matches!(d.verb, Verb::Def | Verb::Ovr | Verb::Use)));
        if let (true, Some(b)) = (authored, parts.get_mut(sid)) {
            b.extend(path.as_str().as_bytes());
            b.extend(rec.hash.0);
        }
    }
    parts.into_iter().map(|(k, v)| (k, hash_raw(&v))).collect()
}

/// Bare `kind.name` -> field values; compares views across scope layouts (R-SCOPE-012).
pub fn view_signature(view: &EffectiveView) -> BTreeMap<String, BTreeMap<String, FieldValue>> {
    view.items
        .iter()
        .map(|(id, it)| {
            let fields = it.fields.iter().map(|(k, f)| (k.clone(), f.value.clone())).collect();
            (format!("{}.{}", id.kind.as_str(), id.name.as_str()), fields)
        })
        .collect()
}
```

`build_views` gets a `digests: &BTreeMap<ScopeId, ContentHash>` parameter (third, after `locs`; the signature becomes `build_views(ev, locs, digests, raws, diags)`) and sets `v.authored_digest = digests.get(&scope.id).cloned().unwrap_or_else(|| hash_raw(b""))`.

In `registry/ovr.rs`: (a) change the `pres.sort_by` key to start with `a.base.is_some()` so a base precedes the override that names it; (b) after the `UnknownField` check in `judge` add the scalar-append guard; (c) in `evaluate_scope` build the `Override` first, fold it into `working`, then push:

```rust
    let appends_to_scalar = p.raw.set.iter().any(|(k, c)| {
        matches!(c, FieldChange::Append(_))
            && matches!(working.items.get(&it.id).and_then(|i| i.fields.get(k)).map(|f| &f.value), Some(FieldValue::Scalar(_)))
    });
    if appends_to_scalar {
        return Verdict::inert(
            InertReason::UnknownField,
            Some((codes::HN_S009, format!("`ovr {}` appends to a scalar field", p.raw.target_text), "use k=v to replace a scalar")),
        );
    }
```
```rust
        let ovr = Override { /* the literal of Task 7, unchanged */ };
        if applied && whole {
            crate::registry::view::apply_ovr(working, &ovr, &verdict.used_grants);
        }
        out.push(Evaluated { ovr, meta: OvrMeta { applied, whole, extent: p.raw.extent.clone(), used_grants: verdict.used_grants } });
```
(`whole` and `applied` are computed before `ovr`; remove the `let _ = verdict.tighten;` line.)

In `registry/mod.rs` add:

```rust
impl Registry {
    pub fn glob_matches(&self, scope: &ScopeId, glob: &str, path: &RepoPath) -> bool {
        let rel = view::visible_rel(&self.table, scope, path);
        globset::GlobBuilder::new(glob)
            .literal_separator(true)
            .build()
            .map(|g| g.compile_matcher().is_match(&rel))
            .unwrap_or(false) // Verify: globset 0.4 GlobBuilder::literal_separator
    }

    fn covers(&self, o: &Override, m: &ovr::OvrMeta, path: &RepoPath) -> bool {
        m.whole
            || match &m.extent {
                ovr::OvrExtent::File(p) => p == path,
                ovr::OvrExtent::Scope(g) => g.as_deref().is_some_and(|g| self.glob_matches(&o.declared_in, g, path)),
            }
    }

    /// E(path): declared items of the chain, then each scope's applicable overrides (R-SCOPE-021).
    pub fn view_at(&self, path: &RepoPath) -> EffectiveView {
        let scope = self.table.scope_of(path).cloned().unwrap_or_else(|| self.table.root.clone());
        let mut v = view::empty_view(&scope, &self.table);
        if let Some(own) = self.views.get(&scope) {
            v.authored_digest = own.authored_digest.clone();
        }
        if !self.table.is_resolved(&scope) {
            return v;
        }
        for sid in v.chain.clone() {
            for (id, item) in self.items.iter().filter(|(k, _)| k.scope == sid) {
                if let Some(loc) = self.locs.get(id) {
                    v.items.insert(id.clone(), view::effective_item(item, loc));
                }
            }
            let mut mine: Vec<(&Override, &ovr::OvrMeta)> =
                self.overrides.iter().zip(&self.meta).filter(|(o, _)| o.declared_in == sid).collect();
            mine.sort_by_key(|(o, _)| o.base.is_some()); // stable: bases first
            for (o, m) in mine {
                if m.applied && self.covers(o, m, path) {
                    view::apply_ovr(&mut v, o, &m.used_grants);
                }
            }
        }
        v
    }
}
```
`resolve.rs`: `let digests = view::own_digests(&table, input.corpus);` and pass `&digests` to `build_views`.

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core --test registry_views` then `cargo test -p fairlead-core --test registry_overrides`
Expected: PASS. If `r_scope_022` shows `kw = ["ui"]` unchanged, P3 stored the table's list under `set` as `ArgValue::Word("a,ui")`; Verify the `OvrDecl.set` mapping of TOML arrays and adjust the kit helper only.

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): effective views, provenance, view_at, transparent conventions, digests"
```

---

### Task 13: Exception ledger on read, propagation table, file-item extends (R-SCOPE-042, 043, 060, R-FILE-043, 052)

**Files:**
- Modify: `crates/fairlead-core/src/registry/ledger.rs`, `registry/item.rs`, `registry/mod.rs`
- Modify: `crates/fairlead-core/tests/registry_views.rs`

**Interfaces:**

Consumes: `Registry.overrides`, `Scope.depth`, `ScopeTable::is_ancestor_or_self`, `Item.fields["kind"]`, `Origin::ControlTable`.

Produces (contract exact): `Registry::ledger(&self, scope: &ScopeId) -> LedgerView`. Additive:
```rust
pub enum LineClass { Def, Use, Ovr, Flag, Env, Allow, Req, ControlKey, NoteTail }
pub struct Propagation { pub down: bool, pub up: bool }
pub fn propagation(c: LineClass) -> Propagation;                 // the matrix of R-SCOPE-060
impl Registry { pub fn nearest_file_item(&self, scope: &ScopeId, file_kind: &str) -> Option<&ItemId>; }
```
`ledger(X)` holds every override declared strictly below X whose target's home is X or above X (so it appears in the ledger of each scope from `parent(declared_in)` up to the home, inclusive, and nowhere else); `depth = depth(declared_in) - depth(X)`; every state is listed (an `Expired` record stays visible). Nothing is stored: it is computed from `overrides()` (R-SCOPE-044 in 0.1). `nearest_file_item` returns the nearest `[[file]]` item of that file kind on the chain (R-FILE-052).

- [ ] **Step 1: Write the failing tests.** Append to `tests/registry_views.rs`:

```rust
use fairlead_core::registry::ledger::{propagation, LineClass};

fn deep() -> Kit {
    Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
        .scope("apps", "apps", &child_toml("apps", ""))
        .file("apps/fairlead/docs/c.md", &doc(&["def cmp.card size=md"]))
        .scope("apps/web", "web", &child_toml("web", &ovr_table("cmp.button", "size = \"lg\"")))
        .scope("apps/web/inner", "inner", &child_toml("inner", &ovr_table("cmp.button", "size = \"sm\"")))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
}

#[test]
fn r_scope_043__ledger_lists_an_ovr_from_the_parent_of_its_scope_up_to_the_home() {
    let reg = deep().resolve();
    let l = |s: &str| reg.ledger(&sid(s)).entries;
    let (root, apps, web, admin) = (l("root"), l("apps"), l("web"), l("admin"));
    assert_eq!(root.iter().map(|e| (e.varied_in.0.as_str().to_string(), e.depth)).collect::<Vec<_>>(),
        vec![("web".to_string(), 2), ("inner".to_string(), 3)]);
    assert_eq!(apps.iter().map(|e| e.depth).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(web.len(), 1); // only the deeper `inner` record: nothing is listed inside its own scope
    assert!(admin.is_empty()); // never sideways
    assert!(root.iter().all(|e| e.home == sid("root") && e.target == iid("root", "cmp.button")));
}

#[test]
fn r_scope_043__nothing_is_listed_above_the_home_scope() {
    let reg = deep()
        .file("apps/web/src/a.ts", "// fairlead: ovr cmp.card size=lg\nexport const a = 1;\n")
        .resolve();
    assert!(reg.ledger(&sid("root")).entries.iter().all(|e| e.target != iid("apps", "cmp.card")));
    assert_eq!(reg.ledger(&sid("apps")).entries.iter().filter(|e| e.target == iid("apps", "cmp.card")).count(), 1);
}

#[test]
fn r_scope_042__one_record_two_readings() {
    let reg = deep().resolve();
    let size = |s: &str| reg.view(&sid(s)).items[&iid("root", "cmp.button")].fields["size"].value.clone();
    assert_eq!(size("web"), scal("lg")); // inside: the patched value is the effective principle
    assert_eq!(size("root"), scal("md")); // outside: unchanged, and the ledger says who varied
    assert!(reg.ledger(&sid("root")).entries.iter().any(|e| e.varied_in == sid("web") && e.state == OvrState::Effective));
}

#[test]
fn r_scope_060__direction_is_a_property_of_the_line_class() {
    let p = |c| {
        let x = propagation(c);
        (x.down, x.up)
    };
    assert_eq!(p(LineClass::Def), (true, true));
    assert_eq!(p(LineClass::Ovr), (true, true));
    assert_eq!(p(LineClass::Flag), (true, false));
    assert_eq!(p(LineClass::Allow), (true, false));
    assert_eq!(p(LineClass::Req), (false, true));
    assert_eq!(p(LineClass::NoteTail), (false, false));
}

#[test]
fn r_file_052__extends_target_is_the_nearest_file_item_and_rebinds_when_one_is_declared() {
    let file = |id: &str, extends: &str| format!("[[file]]\nid = \"{id}\"\nkind = \"stylesheet\"\npath = \"app/x.css\"\n{extends}\n");
    let root = file("file.styles", "");
    let before = Kit::new(&root_toml(&format!("\n{root}"))).scope("apps/web", "web", &child_toml("web", "")).resolve();
    assert_eq!(before.nearest_file_item(&sid("web"), "stylesheet"), Some(&iid("root", "file.styles")));
    let own = file("file.web-styles", "extends = \"root:file.styles\"");
    let after = Kit::new(&root_toml(&format!("\n{root}"))).scope("apps/web", "web", &child_toml("web", &own)).resolve();
    assert_eq!(after.nearest_file_item(&sid("web"), "stylesheet"), Some(&iid("web", "file.web-styles")));
    assert_eq!(after.nearest_file_item(&sid("root"), "stylesheet"), Some(&iid("root", "file.styles")));
    let item = after.item(&iid("web", "file.web-styles")).unwrap();
    assert_eq!(item.fields["extends"], scal("root:file.styles"));
    // R-FILE-043: the relationship resolves through the managerial declaration
    assert!(after.resolve_target(&sid("web"), "root:file.styles").is_some());
}

#[test]
fn r_file_043__an_in_file_anchor_never_creates_a_file_item() {
    let reg = Kit::new(&root_toml(""))
        .file("app/globals.css", "/* fairlead: def file.fake */\n:root{}\n")
        .resolve();
    assert!(reg.nearest_file_item(&sid("root"), "stylesheet").is_none());
    assert!(reg.items().all(|i| i.id.kind.as_str() != "file"));
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core --test registry_views -- r_scope_043 r_scope_042 r_scope_060 r_file`
Expected: FAIL to compile, `unresolved import registry::ledger::propagation` and `no method named ledger`.

- [ ] **Step 3: Implement.** Append to `registry/ledger.rs`:

```rust
use crate::registry::{Origin, Registry};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LineClass {
    Def,
    Use,
    Ovr,
    Flag,
    Env,
    Allow,
    Req,
    ControlKey,
    NoteTail,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Propagation {
    pub down: bool, // computed at read time into views of the subtree
    pub up: bool,   // fact visible to ancestors
}

/// The propagation matrix of R-SCOPE-060 for 0.1.
pub fn propagation(c: LineClass) -> Propagation {
    let (down, up) = match c {
        LineClass::Def | LineClass::Use | LineClass::Ovr | LineClass::ControlKey => (true, true),
        LineClass::Flag | LineClass::Env | LineClass::Allow => (true, false),
        LineClass::Req => (false, true),
        LineClass::NoteTail => (false, false),
    };
    Propagation { down, up }
}

impl Registry {
    /// Computed on read (R-SCOPE-044): overrides declared strictly below `scope` whose home is `scope` or above.
    pub fn ledger(&self, scope: &ScopeId) -> LedgerView {
        let Some(x) = self.table.get(scope) else {
            return LedgerView { for_scope: scope.clone(), entries: Vec::new() };
        };
        let mut entries: Vec<LedgerEntry> = Vec::new();
        for o in &self.overrides {
            let Some(s) = self.table.get(&o.declared_in) else { continue };
            let below = o.declared_in != *scope && self.table.is_ancestor_or_self(scope, &o.declared_in);
            if below && self.table.is_ancestor_or_self(&o.target.scope, scope) {
                entries.push(LedgerEntry {
                    ovr: o.id.clone(),
                    target: o.target.clone(),
                    home: o.target.scope.clone(),
                    varied_in: o.declared_in.clone(),
                    depth: s.depth.saturating_sub(x.depth),
                    why: o.why.clone(),
                    owner: o.owner.clone(),
                    exp: o.exp,
                    state: o.state,
                });
            }
        }
        entries.sort_by(|a, b| (&a.target, a.depth, &a.varied_in, &a.ovr).cmp(&(&b.target, b.depth, &b.varied_in, &b.ovr)));
        LedgerView { for_scope: scope.clone(), entries }
    }

    /// R-FILE-052: nearest `[[file]]` item of a file kind along the chain, nearest scope first.
    pub fn nearest_file_item(&self, scope: &ScopeId, file_kind: &str) -> Option<&ItemId> {
        self.table.chain_ids(scope).into_iter().rev().find_map(|s| {
            self.items
                .iter()
                .filter(|(id, it)| {
                    id.scope == *s
                        && matches!(it.origin, Origin::ControlTable { .. })
                        && matches!(it.fields.get("kind"), Some(FieldValue::Scalar(k)) if k == file_kind)
                })
                .map(|(id, _)| id)
                .next()
        })
    }
}
```
(imports at the top of `ledger.rs`: `crate::registry::item::FieldValue`.)

Sort note: the ledger orders by target, then distance, then scope, so the order is reproducible and independent of override order (R-ARCH-060).

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core --test registry_views`
Expected: PASS (all of Task 12 and Task 13).

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): exception ledger on read, propagation matrix, file-item rebinding"
```

---

### Task 14: Eligibility, hoisting, promotion and demotion model (R-SCOPE-012; data for R-SCOPE-011, 013, 014)

**Files:**
- Modify: `crates/fairlead-core/src/registry/scope.rs`, `registry/ovr.rs`
- Create: `crates/fairlead-core/tests/registry_restructure.rs`, `crates/fairlead-core/tests/prop_promote_demote_roundtrip.rs`

**Interfaces:**

Consumes: `Registry` accessors, `ControlFile.ovrs`, `OvrDecl`, `EligibilityConfig{min_lines,min_files}`, `ManifestHints.workspace_members`, `ovr_fingerprint`, `AsOf`.

Produces (additive; pure data, no `Plan`, no I/O; P5 raises `HN-N002` and `HN-C009`, P7 turns the plans into writes, C-9):
```rust
pub enum EligibilityReason { FileLocalLines { files: u32 }, ManifestMember }
pub struct EligibilityHint { pub dir: RepoPath, pub reason: EligibilityReason }
pub fn scope_eligibility(reg: &Registry, cfg: &EligibilityConfig, hints: &ManifestHints) -> Vec<EligibilityHint>;
pub struct HoistGroup { pub dir: RepoPath, pub target: ItemId, pub applies: String, pub body: String, pub sites: Vec<(RepoPath, u32)> }
pub fn hoist_candidates(reg: &Registry, dir: &RepoPath, min_files: u32) -> Vec<HoistGroup>;   // ovr.rs
pub struct Tombstone { pub id: ScopeSlug, pub former_path: RepoPath, pub written_in: ScopeId, pub date: jiff::civil::Date }
pub struct PromotePlan { pub ancestor: ScopeId, pub new_scope: ScopeId, pub dir: RepoPath, pub moved: Vec<OvrDecl>, pub kept: Vec<OvrDecl>, pub rebinds: Vec<(OvrId, OvrId)> }
pub enum PromoteRefusal { NotBelowAncestor, ScopeExists, IdInUse, AncestorUnresolved }
pub fn promote_model(reg: &Registry, ancestor: &ScopeId, dir: &RepoPath, new_id: &ScopeSlug) -> Result<PromotePlan, PromoteRefusal>;
pub struct DemotePlan { pub scope: ScopeId, pub into: ScopeId, pub tombstone: Tombstone, pub moved: Vec<OvrDecl>,
                        pub items_moved: Vec<ItemId>, pub reparented: Vec<ScopeId>, pub rebinds: Vec<(OvrId, OvrId)> }
pub enum DemoteRefusal { Root, Unresolved, DuplicateId(ItemId) }
pub fn demote_model(reg: &Registry, scope: &ScopeId, as_of: AsOf) -> Result<DemotePlan, DemoteRefusal>;
```
Semantics: eligibility counts file-local lines (`use` edges, code `def` sites, file-local `ovr`s) per file; a non-scope folder with at least `min_files` files of at least `min_lines` lines is a hint (info only, never automatic), as is a manifest member that has no scope. Promotion moves the ancestor's `[[ovr]]` tables whose `applies` is `<rel>/**` (becomes whole-scope) or `<rel>/<glob>` (becomes `<glob>`); demotion is the exact inverse. Approvals bind to a fingerprint that contains the scope id (R-CHK-026), so each moved override gets a new id: `rebinds` lists `(old, new)` pairs and the equivalence invariant is stated for values, not for approvals (Contract issues). Demotion refuses when a moved item's `kind.name` already exists in the target's ancestry or elsewhere in its subtree (R-SCOPE-014).

- [ ] **Step 1: Write the failing tests** `crates/fairlead-core/tests/registry_restructure.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::config::EligibilityConfig;
use fairlead_core::manifest::ManifestHints;
use fairlead_core::registry::ovr::hoist_candidates;
use fairlead_core::registry::scope::{
    demote_model, promote_model, scope_eligibility, DemoteRefusal, EligibilityReason, PromoteRefusal,
};
use fairlead_syntax::ids::ScopeSlug;
use support::*;

fn use_lines(n: usize) -> String {
    format!("{}export const a = 1;\n", "// fairlead: use cmp.button\n".repeat(n))
}

#[test]
fn r_scope_011__busy_folder_and_manifest_member_are_eligible_hints_only() {
    let mut kit = Kit::new(&root_toml("")).file("fairlead/docs/c.md", &doc(&["def cmp.button"]));
    for f in ["a", "b", "c"] {
        kit = kit.file(&format!("src/busy/{f}.ts"), &use_lines(5));
    }
    kit = kit.file("src/thin/a.ts", &use_lines(5)).file("src/thin/b.ts", &use_lines(5));
    let reg = kit.resolve();
    let mut hints = ManifestHints { workspace_members: Default::default(), default_excludes: vec![] };
    hints.workspace_members.insert(path("crates/core"));
    let cfg = EligibilityConfig { min_lines: 5, min_files: 3 };
    let got: Vec<_> = scope_eligibility(&reg, &cfg, &hints).into_iter().map(|h| (h.dir.as_str().to_string(), h.reason)).collect();
    assert_eq!(got, vec![
        ("crates/core".to_string(), EligibilityReason::ManifestMember),
        ("src/busy".to_string(), EligibilityReason::FileLocalLines { files: 3 }),
    ]);
}

#[test]
fn r_scope_013__identical_file_local_ovrs_in_several_files_are_hoist_candidates() {
    let line = "// fairlead: ovr cmp.button size=sm\nexport const a = 1;\n";
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .file("apps/web/src/a.ts", line)
        .file("apps/web/src/b.ts", line)
        .file("apps/web/src/c.ts", "// fairlead: ovr cmp.button size=lg\nexport const c = 1;\n")
        .resolve();
    let g = hoist_candidates(&reg, &path("apps/web/src"), 2);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].applies, "src/**");
    assert_eq!(g[0].sites.len(), 2);
    assert!(g[0].body.contains("size=sm"));
}

fn promote_kit() -> Kit {
    let tables = format!(
        "{}{}",
        ovr_table("cmp.button", "applies = \"web/**\"\nsize = \"lg\""),
        ovr_table("cmp.card", "applies = \"docs/**\"\nsize = \"sm\"")
    );
    Kit::new(&root_toml(&format!("\n{tables}")))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md", "def cmp.card size=md"]))
}

#[test]
fn r_scope_012__promote_moves_matching_ovrs_demote_restores_them() {
    let reg = promote_kit().resolve();
    let slug = ScopeSlug::parse("web").unwrap();
    let plan = promote_model(&reg, &sid("root"), &path("web"), &slug).unwrap();
    assert_eq!(plan.moved.len(), 1);
    assert_eq!(plan.moved[0].applies, None); // `web/**` became the whole new scope
    assert_eq!(plan.kept.len(), 1);
    assert_eq!(plan.rebinds.len(), 1);
    assert!(matches!(promote_model(&reg, &sid("root"), &path("docs"), &ScopeSlug::parse("root").unwrap()), Err(PromoteRefusal::IdInUse)));
    // after the move the scope exists; demoting it re-prefixes the globs
    let after = promote_kit().scope("web", "web", &child_toml("web", &ovr_table("cmp.button", "size = \"lg\""))).resolve();
    let d = demote_model(&after, &sid("web"), after.as_of()).unwrap();
    assert_eq!(d.into, sid("root"));
    assert_eq!(d.moved[0].applies.as_deref(), Some("web/**"));
    assert_eq!((d.tombstone.id.as_str(), d.tombstone.former_path.as_str()), ("web", "web"));
    assert_eq!(d.tombstone.written_in, sid("root"));
}

#[test]
fn r_scope_014__demotion_refuses_a_duplicate_id_and_never_the_root() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.card"]))
        .file("apps/admin/fairlead/docs/c.md", &doc(&["def cmp.card"]))
        .resolve();
    assert_eq!(demote_model(&reg, &sid("web"), reg.as_of()).unwrap_err(), DemoteRefusal::DuplicateId(iid("web", "cmp.card")));
    assert_eq!(demote_model(&reg, &sid("root"), reg.as_of()).unwrap_err(), DemoteRefusal::Root);
    assert!(matches!(promote_model(&reg, &sid("root"), &path("apps/web"), &ScopeSlug::parse("x").unwrap()), Err(PromoteRefusal::ScopeExists)));
}
```
and `crates/fairlead-core/tests/prop_promote_demote_roundtrip.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::config::OvrDecl;
use fairlead_core::registry::ovr::OvrOp;
use fairlead_core::registry::scope::{demote_model, promote_model};
use fairlead_core::registry::view::view_signature;
use fairlead_syntax::directive::ArgValue;
use fairlead_syntax::ids::ScopeSlug;
use proptest::prelude::*;
use support::*;

const TARGETS: [&str; 2] = ["cmp.button", "cmp.card"];
const FIELDS: [&str; 2] = ["size", "layer"];
const VALUES: [&str; 3] = ["a", "b", "c"];
const GLOBS: [&str; 2] = ["web/**", "web/src/**"];
const PATHS: [&str; 3] = ["web/src/a.rs", "web/lib/b.rs", "docs/c.rs"];

fn op_str(o: OvrOp) -> &'static str {
    match o {
        OvrOp::Loosen => "loosen",
        OvrOp::Tighten => "tighten",
        OvrOp::Set => "set",
        OvrOp::Replace => "replace",
        OvrOp::Add => "add",
        OvrOp::Remove => "remove",
    }
}

fn render(d: &OvrDecl) -> String {
    let mut s = format!("[[ovr]]\ntarget = \"{}\"\nop = \"{}\"\n", d.target, op_str(d.op));
    if let Some(a) = &d.applies {
        s.push_str(&format!("applies = \"{a}\"\n"));
    }
    for (k, v) in &d.set {
        let v = match v {
            ArgValue::Word(w) | ArgValue::Quoted(w) => format!("\"{w}\""),
            ArgValue::List(l) => format!("[{}]", l.iter().map(|x| format!("\"{x}\"")).collect::<Vec<_>>().join(", ")),
        };
        s.push_str(&format!("{k} = {v}\n"));
    }
    s.push('\n');
    s
}

fn docs() -> String {
    doc(&["def cmp.button size=md layer=atom", "def cmp.card size=md layer=atom"])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn prop_promote_demote_roundtrip(specs in proptest::collection::vec((0..2usize, 0..2usize, 0..3usize, 0..2usize), 1..5)) {
        let tables: String = specs.iter().map(|(t, f, v, g)| {
            ovr_table(TARGETS[*t], &format!("applies = \"{}\"\n{} = \"{}\"", GLOBS[*g], FIELDS[*f], VALUES[*v]))
        }).collect();
        let a = Kit::new(&root_toml(&format!("\n{tables}"))).file("fairlead/docs/c.md", &docs()).resolve();
        let original: Vec<OvrDecl> = a.scope(&sid("root")).unwrap().control.as_ref().unwrap().ovrs.clone();

        let plan = promote_model(&a, &sid("root"), &path("web"), &ScopeSlug::parse("web").unwrap()).unwrap();
        prop_assert!(plan.kept.is_empty());
        let kept: String = plan.kept.iter().map(render).collect();
        let moved: String = plan.moved.iter().map(render).collect();
        let b = Kit::new(&root_toml(&format!("\n{kept}")))
            .file("fairlead/docs/c.md", &docs())
            .scope("web", "web", &child_toml("web", &moved))
            .resolve();
        for p in PATHS {
            prop_assert_eq!(view_signature(&a.view_at(&path(p))), view_signature(&b.view_at(&path(p))));
        }

        let back = demote_model(&b, &sid("web"), b.as_of()).unwrap();
        // Verify: an `[[ovr]]` without `op` parses to OvrOp::Set; values are compared as text.
        let key = |d: &OvrDecl| {
            let set: Vec<(String, String)> = d.set.iter().map(|(k, v)| (k.clone(), match v {
                ArgValue::Word(s) | ArgValue::Quoted(s) => s.clone(),
                ArgValue::List(l) => l.join(","),
            })).collect();
            format!("{}|{:?}|{:?}", d.target, d.applies, set)
        };
        let mut got: Vec<String> = back.moved.iter().map(key).collect();
        let mut want: Vec<String> = original.iter().map(key).collect();
        got.sort();
        want.sort();
        prop_assert_eq!(got, want);
    }
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core --test registry_restructure --test prop_promote_demote_roundtrip`
Expected: FAIL to compile, `unresolved import registry::scope::promote_model`.

- [ ] **Step 3: Implement.** Append to `registry/scope.rs` (imports: `crate::config::{EligibilityConfig, OvrDecl}`, `crate::manifest::ManifestHints`, `crate::hash::*`, `crate::registry::{ovr::ovr_fingerprint, ItemId, OvrId, Registry}`, `crate::time::AsOf`):

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EligibilityReason {
    FileLocalLines { files: u32 },
    ManifestMember,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EligibilityHint {
    pub dir: RepoPath,
    pub reason: EligibilityReason,
}

/// R-SCOPE-011, R-SCOPE-016: hints only, never automatic; the manifest never overrides a declaration.
pub fn scope_eligibility(reg: &Registry, cfg: &EligibilityConfig, hints: &ManifestHints) -> Vec<EligibilityHint> {
    let mut lines: BTreeMap<&str, u32> = BTreeMap::new();
    for p in reg.edges.iter().map(|e| &e.from.path)
        .chain(reg.sites.values().flatten().map(|s| &s.path))
        .chain(reg.overrides.iter().zip(&reg.meta).filter(|(_, m)| matches!(m.extent, crate::registry::ovr::OvrExtent::File(_))).map(|(o, _)| &o.at.path))
    {
        *lines.entry(p.as_str()).or_insert(0) += 1;
    }
    let mut per_dir: BTreeMap<&str, u32> = BTreeMap::new();
    for (file, _) in lines.iter().filter(|(_, n)| **n >= cfg.min_lines) {
        if let Some(d) = parent_dir(file) {
            *per_dir.entry(d).or_insert(0) += 1;
        }
    }
    let mut out: BTreeMap<String, EligibilityReason> = BTreeMap::new();
    for (d, files) in per_dir {
        if files >= cfg.min_files && !d.is_empty() && !reg.table.by_dir.contains_key(d) {
            out.insert(d.to_string(), EligibilityReason::FileLocalLines { files });
        }
    }
    for m in &hints.workspace_members {
        if !reg.table.by_dir.contains_key(m.as_str()) {
            out.entry(m.as_str().to_string()).or_insert(EligibilityReason::ManifestMember);
        }
    }
    out.into_iter().filter_map(|(d, reason)| RepoPath::new(&d).ok().map(|dir| EligibilityHint { dir, reason })).collect()
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tombstone {
    pub id: ScopeSlug,
    pub former_path: RepoPath,
    pub written_in: ScopeId,
    pub date: jiff::civil::Date,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PromotePlan {
    pub ancestor: ScopeId,
    pub new_scope: ScopeId,
    pub dir: RepoPath,
    pub moved: Vec<OvrDecl>,
    pub kept: Vec<OvrDecl>,
    pub rebinds: Vec<(OvrId, OvrId)>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PromoteRefusal {
    NotBelowAncestor,
    ScopeExists,
    IdInUse,
    AncestorUnresolved,
}

fn rel_dir<'a>(dir: &'a str, base: &str) -> Option<&'a str> {
    if base.is_empty() {
        return Some(dir);
    }
    dir.strip_prefix(base)?.strip_prefix('/')
}

/// Old and new `OvrId` of a declaration that moves from scope `from` to scope `to` (R-CHK-026).
fn rebind(reg: &Registry, resolve_from: &ScopeId, d: &OvrDecl, from: &ScopeId, to: &ScopeId) -> Option<(OvrId, OvrId)> {
    let (target, _) = reg.resolve_target(resolve_from, &d.target)?;
    let exp = d.exp.as_deref().and_then(|s| s.parse::<jiff::civil::Date>().ok());
    let fp = |s: &ScopeId| {
        OvrId(ovr_fingerprint(&target, s, d.why.as_deref().unwrap_or(""), d.owner.as_deref().unwrap_or(""), exp.as_ref()))
    };
    Some((fp(from), fp(to)))
}

pub fn promote_model(
    reg: &Registry,
    ancestor: &ScopeId,
    dir: &RepoPath,
    new_id: &ScopeSlug,
) -> Result<PromotePlan, PromoteRefusal> {
    let anc = reg.table.get(ancestor).filter(|s| s.state == ScopeState::Resolved).ok_or(PromoteRefusal::AncestorUnresolved)?;
    let cf = anc.control.as_ref().ok_or(PromoteRefusal::AncestorUnresolved)?;
    let rel = rel_dir(dir.as_str(), anc.dir.as_str().trim_matches('/')).filter(|r| !r.is_empty()).ok_or(PromoteRefusal::NotBelowAncestor)?;
    if reg.table.by_dir.contains_key(dir.as_str()) {
        return Err(PromoteRefusal::ScopeExists);
    }
    let new_scope = ScopeId(new_id.clone());
    if reg.table.get(&new_scope).is_some() {
        return Err(PromoteRefusal::IdInUse);
    }
    let (mut moved, mut kept, mut rebinds) = (Vec::new(), Vec::new(), Vec::new());
    for d in &cf.ovrs {
        let rest = d.applies.as_deref().and_then(|a| a.strip_prefix(rel)).and_then(|a| a.strip_prefix('/'));
        match rest {
            Some(rest) => {
                let mut m = d.clone();
                m.applies = if rest == "**" { None } else { Some(rest.to_string()) };
                rebinds.extend(rebind(reg, ancestor, d, ancestor, &new_scope));
                moved.push(m);
            }
            None => kept.push(d.clone()),
        }
    }
    Ok(PromotePlan { ancestor: ancestor.clone(), new_scope, dir: dir.clone(), moved, kept, rebinds })
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DemotePlan {
    pub scope: ScopeId,
    pub into: ScopeId,
    pub tombstone: Tombstone,
    pub moved: Vec<OvrDecl>,
    pub items_moved: Vec<ItemId>,
    pub reparented: Vec<ScopeId>,
    pub rebinds: Vec<(OvrId, OvrId)>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DemoteRefusal {
    Root,
    Unresolved,
    DuplicateId(ItemId),
}

/// R-SCOPE-014: tombstone data plus the inverse of `promote_model`; refuses on duplicate IDs.
pub fn demote_model(reg: &Registry, scope: &ScopeId, as_of: AsOf) -> Result<DemotePlan, DemoteRefusal> {
    let s = reg.table.get(scope).ok_or(DemoteRefusal::Unresolved)?;
    let into = s.parent.clone().ok_or(DemoteRefusal::Root)?;
    let cf = s.control.as_ref().filter(|_| s.state == ScopeState::Resolved).ok_or(DemoteRefusal::Unresolved)?;
    let anc = reg.table.get(&into).ok_or(DemoteRefusal::Unresolved)?;
    let rel = rel_dir(s.dir.as_str(), anc.dir.as_str().trim_matches('/')).unwrap_or("").to_string();
    let items_moved: Vec<ItemId> = reg.items.keys().filter(|k| k.scope == *scope).cloned().collect();
    for m in &items_moved {
        let clash = reg.items.keys().find(|k| {
            k.scope != *scope && k.kind == m.kind && k.name == m.name
                && (reg.table.is_ancestor_or_self(&k.scope, &into) || reg.table.is_ancestor_or_self(&into, &k.scope))
                && !reg.table.is_ancestor_or_self(scope, &k.scope)
        });
        if clash.is_some() {
            return Err(DemoteRefusal::DuplicateId(m.clone()));
        }
    }
    let mut moved = Vec::new();
    let mut rebinds = Vec::new();
    for d in &cf.ovrs {
        let mut m = d.clone();
        m.applies = Some(match d.applies.as_deref() {
            None | Some("**") => format!("{rel}/**"),
            Some(a) => format!("{rel}/{a}"),
        });
        rebinds.extend(rebind(reg, scope, d, scope, &into));
        moved.push(m);
    }
    let reparented = reg.table.scopes.values().filter(|c| c.parent.as_ref() == Some(scope)).map(|c| c.id.clone()).collect();
    Ok(DemotePlan {
        scope: scope.clone(),
        into: into.clone(),
        tombstone: Tombstone { id: scope.0.clone(), former_path: s.dir.clone(), written_in: into, date: as_of.0 },
        moved,
        items_moved,
        reparented,
        rebinds,
    })
}
```
(`reg.meta` and `reg.table` are `pub(crate)`; if the borrow checker rejects the chained iterator in `scope_eligibility`, split it into three `for` loops incrementing the same map.)

Append to `registry/ovr.rs`:

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HoistGroup {
    pub dir: RepoPath,
    pub target: ItemId,
    pub applies: String,
    pub body: String,
    pub sites: Vec<(RepoPath, u32)>,
}

fn change_text(k: &str, c: &FieldChange) -> String {
    match c {
        FieldChange::Replace(FieldValue::Scalar(s)) => format!("{k}={s}"),
        FieldChange::Replace(FieldValue::List(l)) => format!("{k}={}", l.join(",")),
        FieldChange::Append(l) => format!("{k}+={}", l.join(",")),
    }
}

/// R-SCOPE-013 data for `new scope --hoist` (P7): identical file-local `ovr`s in at least `min_files` files of `dir`.
pub fn hoist_candidates(reg: &crate::registry::Registry, dir: &RepoPath, min_files: u32) -> Vec<HoistGroup> {
    let mut groups: BTreeMap<String, HoistGroup> = BTreeMap::new();
    for (o, m) in reg.overrides.iter().zip(&reg.meta) {
        let OvrExtent::File(p) = &m.extent else { continue };
        if p.as_str().rsplit_once('/').map(|(d, _)| d) != Some(dir.as_str()) {
            continue;
        }
        let mut body = format!("ovr {} op={}", o.target.to_id(), op_name(o.op));
        for (k, c) in &o.set {
            body.push(' ');
            body.push_str(&change_text(k, c));
        }
        for (k, v) in [("why", &o.why), ("owner", &o.owner)] {
            if let Some(v) = v {
                body.push_str(&format!(" {k}=\"{v}\""));
            }
        }
        if let Some(e) = o.exp {
            body.push_str(&format!(" exp={e}"));
        }
        let owner_dir = reg.table.get(&o.declared_in).map(|s| s.dir.as_str().trim_matches('/').to_string()).unwrap_or_default();
        let rel = dir.as_str().strip_prefix(owner_dir.as_str()).map(|r| r.trim_start_matches('/')).unwrap_or(dir.as_str());
        let g = groups.entry(body.clone()).or_insert_with(|| HoistGroup {
            dir: dir.clone(),
            target: o.target.clone(),
            applies: format!("{rel}/**"),
            body,
            sites: Vec::new(),
        });
        g.sites.push((p.clone(), o.at.line));
    }
    groups.into_values().filter(|g| g.sites.len() as u32 >= min_files).collect()
}
```
(`OvrExtent` fields `reg.table`, `reg.meta` are `pub(crate)`, reachable from the same crate.)

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p fairlead-core --test registry_restructure --test prop_promote_demote_roundtrip`
Expected: PASS. A proptest failure prints a minimal spec list: the usual cause is two overlapping specs becoming non-overlapping after the move; fix `extents_overlap`, not the test.

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): eligibility, hoisting, promotion and demotion model"
```

---

### Task 15: Order independence property, final `resolve()` wiring, worked example of chapter 10 (R-SCOPE-020, 042, 049, R-ARCH-040, 043)

**Files:**
- Modify: `crates/fairlead-core/src/registry/resolve.rs` (final listing below)
- Create: `crates/fairlead-core/tests/prop_resolution_order_independent.rs`, `crates/fairlead-core/tests/registry_scenario.rs`

**Interfaces:**

Consumes: everything above. Produces nothing new; this task proves the contract property `prop_resolution_order_independent` (same declarations in any textual order, any root selection, give the same views, ledgers, override states and diagnostics) and pins the end-to-end reading of the worked example in chapter 10.

- [ ] **Step 1: Write the failing tests.** `crates/fairlead-core/tests/prop_resolution_order_independent.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::registry::{ApprovalStatus, OvrState};
use proptest::prelude::*;
use support::*;

const FIELDS: [(&str, [&str; 3]); 2] = [("sev", ["info", "warn", "error"]), ("stage", ["observe", "warn", "enforce"])];

fn permute<T: Clone>(v: &[T], seed: u64) -> Vec<T> {
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by_key(|i| seed.wrapping_mul(0x9E37_79B9).wrapping_add((*i as u64).wrapping_mul(40_503)) % 1_000_003);
    idx.into_iter().filter_map(|i| v.get(i).cloned()).collect()
}

fn build(ovrs: &[String], allows: &[String], search: bool) -> fairlead_core::registry::Registry {
    let rules = format!(
        "{}{}{}{}",
        rule("r1", "warn", "observe", false),
        rule("r2", "error", "enforce", false),
        rule("r3", "warn", "warn", true),
        allows.concat()
    );
    let kit = Kit::new(&root_toml(&format!("\n{rules}")))
        .scope("apps/web", "web", &child_toml("web", &ovrs.concat()))
        .scope("apps/admin", "admin", &child_toml("admin", ""));
    if search { kit.search_source().resolve() } else { kit.resolve() }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn prop_resolution_order_independent(
        specs in proptest::collection::vec((0..3usize, 0..2usize, 0..3usize, any::<bool>()), 1..6),
        grants in proptest::collection::vec((0..3usize, 0..2usize), 0..4),
        seed in any::<u64>(),
    ) {
        let ovrs: Vec<String> = specs.iter().map(|(r, f, v, complete)| {
            let (field, values) = FIELDS[*f];
            let meta = if *complete { "\nwhy = \"w\"\nowner = \"@andy\"\nexp = \"2026-12-31\"" } else { "" };
            ovr_table(&format!("rule.r{}", r + 1), &format!("{field} = \"{}\"{meta}", values[*v]))
        }).collect();
        let allows: Vec<String> = grants.iter().map(|(r, f)| format!(
            "[[allow]]\ntarget = \"rules.r{}\"\nfield = \"{}\"\nfor = [\"web\"]\nwhy = \"g\"\nexp = \"2027-03-31\"\n\n", r + 1, FIELDS[*f].0
        )).collect();
        let a = build(&ovrs, &allows, false);
        let b = build(&permute(&ovrs, seed), &permute(&allows, seed ^ 7), true); // also another root selection
        for s in ["root", "web", "admin"] {
            prop_assert_eq!(&a.view(&sid(s)).items, &b.view(&sid(s)).items);
            prop_assert_eq!(a.ledger(&sid(s)).entries, b.ledger(&sid(s)).entries);
        }
        let states = |r: &fairlead_core::registry::Registry| -> Vec<(String, OvrState, ApprovalStatus)> {
            let mut v: Vec<_> = r.overrides().iter().map(|o| (format!("{:?}", o.id), o.state, o.approval)).collect();
            v.sort_by(|x, y| x.0.cmp(&y.0));
            v
        };
        prop_assert_eq!(states(&a), states(&b));
        prop_assert_eq!(a.diagnostics(), b.diagnostics());
    }
}
```

`crates/fairlead-core/tests/registry_scenario.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::config::Policy;
use fairlead_core::registry::{ovr_fingerprint, ApprovalStatus, FieldValue, OvrState};
use support::*;

#[test]
fn r_scope_042__worked_example_of_chapter_10_reads_both_ways() {
    let line = "// fairlead: ovr rule.no-inline-style op=loosen sev=info why=\"chart lib positions via style attr\" owner=@andy exp=2026-12-31\nexport const L = 1;\n";
    let target = iid("root", "rule.no-inline-style");
    let fp = ovr_fingerprint(&target, &sid("apps-console"), "chart lib positions via style attr", "@andy", Some(&date("2026-12-31")));
    let build = |k: Kit| {
        k.scope("apps/console", "apps-console", &child_toml("apps-console", ""))
            .scope("apps/admin", "apps-admin", &child_toml("apps-admin", ""))
            .file("apps/console/src/charts/line.tsx", line)
            .approve(fp.clone(), &target, 0)
            .resolve()
    };
    let reg = build(Kit::new(&root_toml(&format!("\n{}", rule("no-inline-style", "error", "enforce", true)))));
    assert!(reg.diagnostics().is_empty(), "{:?}", reg.diagnostics());

    let inside = reg.view_at(&path("apps/console/src/charts/line.tsx"));
    let f = &inside.items[&target].fields["sev"];
    assert_eq!(f.value, FieldValue::Scalar("info".into()));
    assert_eq!(f.provenance.home, sid("root"));
    assert_eq!(inside.items[&target].policy, Policy::NeedsException);
    assert_eq!(f.provenance.applied, vec![reg.overrides()[0].id.clone()]);

    let outside = reg.view_at(&path("apps/admin/src/a.tsx"));
    assert_eq!(outside.items[&target].fields["sev"].value, FieldValue::Scalar("error".into()));

    let l = reg.ledger(&sid("root")).entries;
    assert_eq!(l.len(), 1);
    assert_eq!((l[0].varied_in.clone(), l[0].depth, l[0].state), (sid("apps-console"), 1, OvrState::Effective));
    assert_eq!((l[0].owner.as_deref(), l[0].exp), (Some("@andy"), Some(date("2026-12-31"))));
    assert_eq!(reg.overrides()[0].approval, ApprovalStatus::Approved { renewal: 0 });
}
```

Append these tests to `registry_scenario.rs` (one per remaining rule of the fold: R-SCOPE-020, R-ARCH-040, 042, 043):

```rust
fn world(search: bool) -> fairlead_core::registry::Registry {
    let k = Kit::new(&root_toml(&format!("
{}", rule("r1", "warn", "observe", false))))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
        .scope("apps/web", "web", &child_toml("web", &ovr_table("rule.r1", "sev = \"error\"")))
        .file("apps/web/src/a.ts", "// fairlead: ovr cmp.button size=sm
export const a = 1;
");
    if search { k.search_source().resolve() } else { k.resolve() }
}

#[test]
fn r_scope_020__resolution_is_a_pure_function_of_the_authored_files() {
    let (a, b) = (world(false), world(true));
    for s in ["root", "web"] {
        assert_eq!(a.view(&sid(s)).items, b.view(&sid(s)).items);
    }
    assert_eq!(a.diagnostics(), b.diagnostics());
}

#[test]
fn r_arch_043__invocation_directory_only_selects_a_cone_never_the_result() {
    let (a, b) = (world(false), world(true));
    let p = path("apps/web/src/a.ts");
    assert_eq!(a.view_at(&p).items, b.view_at(&p).items);
    assert_eq!(a.ledger(&sid("root")).entries, b.ledger(&sid("root")).entries);
}

#[test]
fn r_arch_042__provenance_names_home_declaration_site_and_applied_overrides() {
    let reg = world(false);
    let f = &reg.view(&sid("web")).items[&iid("root", "rule.r1")].fields["sev"];
    assert_eq!(f.provenance.home, sid("root"));
    assert_eq!(f.provenance.declared_at.path.as_str(), "fairlead/control.toml");
    assert_eq!(f.provenance.applied.len(), 1);
    assert!(reg.view(&sid("root")).items[&iid("root", "rule.r1")].fields["sev"].provenance.applied.is_empty());
}

#[test]
fn r_arch_040__base_chained_overrides_fold_base_first_in_any_textual_order() {
    let target = iid("root", "cmp.button");
    let first = ovr_fingerprint(&target, &sid("web"), "a", "", None);
    let one = ovr_table("cmp.button", "size = \"sm\"
why = \"a\"");
    let two = ovr_table("cmp.button", &format!("size = \"lg\"
why = \"b\"
base = \"{first}\""));
    let size = |tables: String| {
        let reg = Kit::new(&root_toml(""))
            .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
            .scope("apps/web", "web", &child_toml("web", &tables))
            .resolve();
        reg.view(&sid("web")).items[&target].fields["size"].value.clone()
    };
    assert_eq!(size(format!("{one}{two}")), FieldValue::Scalar("lg".into()));
    assert_eq!(size(format!("{two}{one}")), FieldValue::Scalar("lg".into()));
}
```

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p fairlead-core --test prop_resolution_order_independent --test registry_scenario`
Expected: the scenario test fails to compile on the `Policy` import path; after fixing it, both pass unless a Task left a dependence on input order. The usual finding is a missing tie-break: `evals.sort_by` in `resolve` and `pres.sort_by` in `evaluate_scope` must end with `&id`/`text` so equal keys cannot keep input order.

- [ ] **Step 3: Make `resolve.rs` final** (this is the complete function after all tasks; compare with your edits):

```rust
pub fn resolve(input: &ResolveInput<'_>) -> Registry {
    let mut diags: Vec<Diagnostic> = Vec::new();
    let folder = input.root.folder.as_str();
    let table = build_scope_table(input.root, input.controls, input.facts, &mut diags);
    let kinds = KindTable::build(&table, &mut diags);
    let docs = doc_cands(&table, &kinds, folder, input.corpus, &mut diags);
    let decls = collect_items(&table, docs, &mut diags);
    let idx = Index { table: &table, kinds: &kinds, items: &decls.items };
    let refs = collect_refs(&idx, folder, input.corpus, &mut diags);
    let grants = collect_grants(&idx, input.as_of, &mut diags);
    let requests = collect_requests(&table, &mut diags);
    let proposals = collect_proposals(&table, &requests);
    let raws = collect_raw(&idx, input.corpus, &mut diags);
    let waiver = table.root_control().map(|c| c.policies.waiver.clone()).unwrap_or(WaiverPolicy {
        max_exp_days: 90, budget: 25, max_renewals: 2, approval_required: true,
    });
    let ev = Eval {
        idx: &idx,
        as_of: input.as_of,
        grants: &grants,
        approvals: &input.facts.approvals.approvals,
        horizon_days: max_horizon_days(table.root_control()),
        waiver,
    };
    let digests = view::own_digests(&table, input.corpus);
    let built = view::build_views(&ev, &decls.locs, &digests, raws, &mut diags);
    let mut evals = built.evals;
    evals.sort_by(|a, b| {
        (&a.ovr.target, &a.ovr.declared_in, a.ovr.at.path.as_str(), a.ovr.at.line, a.ovr.at.col, &a.ovr.id)
            .cmp(&(&b.ovr.target, &b.ovr.declared_in, b.ovr.at.path.as_str(), b.ovr.at.line, b.ovr.at.col, &b.ovr.id))
    });
    let (overrides, meta): (Vec<_>, Vec<_>) = evals.into_iter().map(|e| (e.ovr, e.meta)).unzip();
    diags.sort_by(|a, b| {
        (a.location.path.as_str(), a.location.line, a.location.col, a.code, a.message.as_str())
            .cmp(&(b.location.path.as_str(), b.location.line, b.location.col, b.code, b.message.as_str()))
    });
    let empty_view = view::empty_view(&table.root, &table);
    Registry {
        table, kinds, items: decls.items, locs: decls.locs, overrides, meta, grants, requests, proposals,
        edges: refs.edges, sites: refs.sites, views: built.views, diagnostics: diags, as_of: input.as_of, empty_view,
    }
}
```
In `evaluate_scope` make the `pres.sort_by` key `(a.base.is_some(), target text, path, line, col, a.raw.text.as_str(), a.id)`.

- [ ] **Step 4: Run the whole phase, expect PASS**

Run: `cargo test -p fairlead-core registry` then `cargo test -p fairlead-core --test prop_resolution_order_independent --test prop_promote_demote_roundtrip --test registry_scenario`
Expected: all pass. Then `cargo clippy -p fairlead-core --all-targets -- -D warnings` and `cargo fmt --all --check` clean.

- [ ] **Step 5: Commit**

```bash
git add crates/fairlead-core && git commit -m "feat(registry): final resolve wiring, order independence property, chapter 10 scenario"
```

---

## Traceability

Every P4 rule of contract Section 6 (36 SCOPE, 3 FILE, 6 SYN, 4 CHK, 1 AGT, 7 ARCH = 57), the task that implements it and the tests whose names start with the rule id. Support tests for rules owned by P5/P7 (SCOPE-011, 013, 014, 018, 039) are named in the last paragraph.

| Rule | Task | Test functions |
|---|---|---|
| R-SCOPE-001 | 2 | `r_scope_001__scope_id_is_the_stored_slug_not_the_path` |
| R-SCOPE-002 | 2 | `r_scope_002__identity_survives_a_directory_move` |
| R-SCOPE-003 | 2 | `r_scope_003__membership_is_nearest_ancestor_or_self` |
| R-SCOPE-005 | 2, 3 | `r_scope_005__duplicate_scope_id_is_hn_c007_and_the_loser_is_unresolved`, `r_scope_005__scope_missing_from_parent_shard_is_hn_c008` |
| R-SCOPE-012 | 14 | `r_scope_012__promote_moves_matching_ovrs_demote_restores_them`, `prop_promote_demote_roundtrip` |
| R-SCOPE-015 | 2 | `r_scope_015__leaf_folder_without_managerial_folder_stays_implicit` |
| R-SCOPE-020 | 15 | `r_scope_020__resolution_is_a_pure_function_of_the_authored_files`, `prop_resolution_order_independent` |
| R-SCOPE-021 | 12 | `r_scope_021__child_view_folds_the_chain_root_first_and_siblings_stay_apart`, `r_scope_021__file_local_ovr_is_overlaid_last_by_view_at_only` |
| R-SCOPE-022 | 12 | `r_scope_022__merge_by_field_type_and_provenance_lists_grants` |
| R-SCOPE-023 | 5 | `r_scope_023__child_doc_def_of_an_inherited_id_is_hn_r005` |
| R-SCOPE-024 | 5, 6 | `r_scope_024__siblings_may_reuse_a_bare_name`, `r_scope_024__ancestor_into_descendant_reference_needs_a_qualifier` |
| R-SCOPE-025 | 11 | `r_scope_025__applies_glob_cannot_leave_the_owning_subtree` |
| R-SCOPE-026 | 4 | `r_scope_026__ancestor_declaration_wins_over_a_descendant_redefinition_hn_r005` |
| R-SCOPE-027 | 12 | `r_scope_027__digest_changes_only_inside_the_edited_subtree` |
| R-SCOPE-030 | 4, 5 | `r_scope_030__rules_default_to_sealed_and_patterns_refine`, `r_scope_030__def_policy_key_refines_the_item_policy` |
| R-SCOPE-031 | 9 | `r_scope_031__open_ovr_needs_no_reason` |
| R-SCOPE-032 | 9 | `r_scope_032__needs_exception_ovr_without_exp_is_inert_and_hn_s016` |
| R-SCOPE-033 | 9 | `r_scope_033__sealed_ovr_is_inert_until_a_matching_allow_exists` |
| R-SCOPE-034 | 9 | `r_scope_034__tightening_is_free_on_every_policy_loosening_is_not` |
| R-SCOPE-035 | 9 | `r_scope_035__a_child_cannot_change_the_policy_of_an_inherited_item` |
| R-SCOPE-036 | 8 | `r_scope_036__allow_in_the_home_scope_is_valid` |
| R-SCOPE-037 | 8 | `r_scope_037__child_cannot_set_vocabulary_keys_and_gets_an_escalation_proposal` |
| R-SCOPE-038 | 9 | `r_scope_038__inert_lines_never_abort_the_valid_ones` |
| R-SCOPE-040 | 7 | `r_scope_040__ovr_names_target_op_and_values`, `r_scope_040__target_may_name_a_field_and_a_dangling_target_makes_no_override` |
| R-SCOPE-041 | 7 | `r_scope_041__ovr_is_file_local_or_a_scope_level_table` |
| R-SCOPE-043 | 13 | `r_scope_043__ledger_lists_an_ovr_from_the_parent_of_its_scope_up_to_the_home`, `r_scope_043__nothing_is_listed_above_the_home_scope` |
| R-SCOPE-045 | 10 | `r_scope_045__version_shaped_expiry_is_inert_with_hn_s012` |
| R-SCOPE-046 | 10 | `r_scope_046__expired_exception_is_ineffective_and_the_boundary_day_is_live` |
| R-SCOPE-047 | 10 | `r_scope_047__pin_mismatch_marks_the_exception_stale` |
| R-SCOPE-049 | 10 | `r_scope_049__prose_claiming_approval_is_data` |
| R-SCOPE-053 | 8 | `r_scope_053__req_is_kept_sorted_and_never_effective` |
| R-SCOPE-054 | 3 | `r_scope_054__unparseable_control_leaves_the_subtree_unresolved`, `r_scope_054__scope_in_sparse_absent_dir_is_absent_from_tree` |
| R-SCOPE-055 | 3 | `r_scope_055__diagnostics_do_not_depend_on_how_the_root_was_selected` |
| R-SCOPE-058 | 8 | `r_scope_058__grant_authored_outside_the_home_scope_is_invalid_and_not_re_delegable` |
| R-SCOPE-060 | 13 | `r_scope_060__direction_is_a_property_of_the_line_class` |
| R-SCOPE-064 | 11 | `r_scope_064__two_overlapping_ovrs_of_one_field_are_both_inert_hn_x014`, `r_scope_064__a_base_chain_is_not_a_duplicate_and_siblings_never_conflict`, `r_scope_064__disjoint_globs_in_one_scope_are_independent` |
| R-FILE-037 | 6 | `r_file_037__def_in_a_leaf_folder_belongs_to_the_nearest_ancestor_scope` |
| R-FILE-043 | 13 | `r_file_043__an_in_file_anchor_never_creates_a_file_item` |
| R-FILE-052 | 13 | `r_file_052__extends_target_is_the_nearest_file_item_and_rebinds_when_one_is_declared` |
| R-SYN-011 | 9, 10 | `r_syn_011__why_owner_exp_are_required_only_where_the_policy_asks` |
| R-SYN-013 | 5, 7 | `r_syn_013__unknown_def_key_is_hn_s011_warn_and_error_under_bang`, `r_syn_013__append_marker_remove_op_and_unknown_field` |
| R-SYN-024 | 4, 5, 6 | `r_syn_024__child_cannot_redefine_an_inherited_kind`, `r_syn_024__undeclared_kind_is_hn_s002_with_declared_kinds_and_a_suggestion`, `r_syn_024__use_of_an_undeclared_kind_is_hn_s002_and_makes_no_edge` |
| R-SYN-025 | 6 | `r_syn_025__unqualified_reference_resolves_to_the_nearest_declaring_scope` |
| R-SYN-026 | 5 | `r_syn_026__two_doc_defs_of_one_id_in_one_scope_is_hn_d001` (HN-R005 half: R-SCOPE-023/026 tests) |
| R-SYN-033 | 3, 4 | `r_syn_033__unresolved_scope_has_no_view_and_never_inherits_the_ancestor_rules` |
| R-CHK-006 | 4, 9 | `r_chk_006__waivable_rule_is_needs_exception`, tightening: `r_scope_034__...` |
| R-CHK-025 | 10 | `r_chk_025__needs_exception_waiver_needs_a_matching_approval` |
| R-CHK-026 | 7, 10 | `r_chk_026__ovr_id_binds_to_content_not_to_the_file_path`, `r_chk_026__re_dating_a_waiver_invalidates_its_approval` |
| R-CHK-053 | 10 | `r_chk_053__owner_must_be_a_handle_in_owners` |
| R-AGT-012 | 10, 12 | `r_agt_012__renewals_are_counted_in_the_approval_status`, `r_arch_041__unapproved_or_beyond_horizon_overrides_do_not_change_values` |
| R-ARCH-040 | 15 | `r_arch_040__base_chained_overrides_fold_base_first_in_any_textual_order` |
| R-ARCH-041 | 9, 12 | `r_scope_038__inert_lines_never_abort_the_valid_ones`, `r_arch_041__unapproved_or_beyond_horizon_overrides_do_not_change_values` |
| R-ARCH-042 | 15 | `r_arch_042__provenance_names_home_declaration_site_and_applied_overrides` |
| R-ARCH-043 | 3, 15 | `r_arch_043__invocation_directory_only_selects_a_cone_never_the_result` |
| R-ARCH-061 | 1 | `r_arch_061__registry_types_are_send_sync_plain_data` |
| R-ARCH-062 | 6 | `r_arch_062__bare_reference_with_two_candidates_is_ambiguous_hn_r007`, `r_arch_062__missing_reference_lists_the_nearest_ids_by_edit_distance` |
| R-ARCH-104 | 4 | `r_arch_104__error_in_one_scope_does_not_hide_an_unrelated_scope` |

Support tests for rules owned elsewhere: `r_scope_017__home_must_be_an_ancestor_of_every_implementation_site` (Task 6, data for `HN-C013`), `r_scope_011__busy_folder_and_manifest_member_are_eligible_hints_only`, `r_scope_013__identical_file_local_ovrs_in_several_files_are_hoist_candidates`, `r_scope_014__demotion_refuses_a_duplicate_id_and_never_the_root` (Task 14, data for P5/P7), `r_scope_018__transparent_convention_hides_route_group_segments_for_applies_matching` (Task 12, consumption of P3's `[[convention]]`), `r_scope_042__one_record_two_readings` and `r_scope_042__worked_example_of_chapter_10_reads_both_ways` (Tasks 13, 15; the data behind P7's `explain`), `r_scope_038`'s floor line (`InertReason::FloorRule`, R-SCOPE-039 is P5's).

## Contract issues

Everything here is a gap or conflict found while writing this plan. Each item names the choice this plan made so the contract owner can ratify or overrule it.

1. **`gen` is a keyword in edition 2024.** `Layout { gen: String }` (4.6) and `GenKind`'s directory names cannot be written without `r#gen`; this plan uses `layout.r#gen`. Rename the field (`gen_dir`) in the contract.
2. **Unspecified upstream constructors.** The registry needs `Fingerprint::from_hash(&ContentHash)`, `Display` and `FromStr` for `Fingerprint`, `Ord` on `ScopeSlug`/`Kind`/`Name`, `as_str` on `Kind`/`Name`, `StyleKinds: Default`, and `RepoPath::new("")` as the root directory (all marked `Verify:`). `ManifestHints` lives in `manifest`, not `facts` (4.4 vs 3).
3. **No path-level view.** Contract 4.8 has `view(scope)` only, yet R-SCOPE-021 overlays file-local lines and `explain @ path` needs them. Added `view_at(path)`, `glob_matches`, `resolve_target`, `proposals`, `nearest_file_item` (listed under "P4 additions"). `RuleSet::applies` (P5) should call `glob_matches` so `[[convention]] transparent` is honoured.
4. **Declaration versus site.** The contract does not say how `resolve` tells a doc `def` from a code `def` (P4-1). Chosen: Markdown, frontmatter, or a path inside a managerial folder declares; everything else is a site.
5. **Membership shard format.** `gen/members` has no line format in the spec. Chosen (P4-2): first token the scope id, optional `path=<dir>`; an absent shard means unregistered. P5's `GenKind::Members` generator must write exactly that, otherwise every scope raises `HN-C008`.
6. **R-SYN-011 versus R-SCOPE-031.** Chapter 12 says every `ovr` carries `why`; chapter 10 says `open` needs no reason. This plan follows chapter 10 (`open` requires nothing). If chapter 12 wins, add `why` to the `Open` branch of `judge`.
7. **R-SCOPE-012 versus R-CHK-026.** The approval fingerprint contains the scope id, so a promoted or demoted `ovr` gets a new `OvrId` and loses its approval; a `[[allow]]` that names the new scope cannot exist before promotion. The equivalence invariant therefore holds for values of `open` items and tightening overrides (what `prop_promote_demote_roundtrip` states); `PromotePlan.rebinds`/`DemotePlan.rebinds` list the approvals P7 must re-record.
8. **No authored pin for EXC-STALE.** R-SCOPE-047 wants a pinned target hash but 12 lists no such key. Chosen: user-extension key `x-pin=<12 base32>` (P4-7); P7's `sync` must write it. A pinned `ovr` whose target vanished is a dangling target (no `Override`, P4-11), so `HN-X010` for "vanished" needs P5 to treat a pinned dangling `ovr` as stale rather than `HN-R001`.
9. **Codes emitted beyond the 4.9 table.** P4 also emits `HN-C001` (bad `[[ovr]]`/`[[allow]]` target, bad `applies` glob, bad `[[req]] to`, bad file-item id), `HN-C003` (child kind redefinition, invalid `[[allow]]`, ovr of `policy`, ovr outside the ancestry), `HN-C013` (qualified `def` in another scope's doc) and `HN-S012` (version-shaped `exp` in a `[[ovr]]` table; P2 covers the directive form). Dangling `HN-R001` is not emitted: P5 derives it from `edges()` (`resolved == None`, skipping `Lookup::Ambiguous`) and from `resolve_target` for `ovr` targets, skipping ovrs that appear in `overrides()` (floor targets resolve).
10. **Floor rule names.** The spec lists floor codes, not floor rule IDs. `FLOOR_RULES` (P4-8) mirrors the mnemonics; P5's `is_floor` must stay in sync. `Registry::item()` returns `None` for such a target.
11. **Field defaults and scales.** Rule items carry `sev` (default `error`), `stage` (default `observe`, R-CHK-009), `glob`, `exclude`; the spec gives no default `sev`. `KeyDecl` has no order, so only those four rule fields are root-ordered scales in 0.1 (R-SCOPE-034).
12. **Policy patterns spell `rules.`.** Chapter 10's examples write `"rules.*"` and `target = "rules.max-lines"` while the kind is `rule` (P4-3).
13. **`Override.state` has no unapproved/over-horizon reason.** Both stay `Effective` (P4-6) and are visible as `approval == Missing` and `exp` beyond `max_horizon_days(root)`, which P5 uses for `HN-X002`/`HN-X006`.
14. **Edge case 13 of chapter 10** lists `src/**` among rejected globs; this plan rejects only globs that contain `..`, start with `/`, or contain `\` or `:`, since a relative glob cannot name a path outside its scope.
15. **`Lookup` for a lone sideways item.** A bare reference that is declared in exactly one non-chain scope is `Missing` with the qualified ID first (R-SCOPE-024: ancestor to descendant must be qualified); `Ambiguous` needs two or more.
16. **`OvrDecl` shape.** The kit assumes field assignments are inline keys of `[[ovr]]` (chapter 12: "the keys of the `ovr` directive"); if P3 chose a sub-table, only `support::ovr_table` changes.

## Execution handoff

Plan complete and saved to `docs/plans/0.1/p4-registry.md`. Two execution options:

1. **Subagent-driven (recommended).** One fresh subagent per task, review between tasks. Run the tasks strictly in order: they edit the same files (`ovr.rs` through Tasks 7 to 11, `view.rs` through Tasks 4, 7, 12, `resolve.rs` every task), so do not parallelise them.
2. **Inline execution** with `superpowers:executing-plans`, one commit per task.

Before Task 1, spend ten minutes on the "Assumed upstream API" table: write a scratch test that touches each listed item and fix names in `tests/support/mod.rs` and in the `Verify:` lines only; never rename a contract type. P4 starts after P2 and P3 have landed their types (contract Section 6). Consumers: P5 (`check`) reads `Registry::{overrides, edges, view, ledger, diagnostics, resolve_target, glob_matches}` and the restructure helpers; P6 reads `view`, `ledger`, `requests`, `proposals`; P7 reads `promote_model`/`demote_model`/`hoist_candidates`/`scope_eligibility` and writes `x-pin`; P8's `pipeline::resolve_loaded` is a one-line call to `resolve`. Gate for the phase: `cargo test -p fairlead-core registry`, the three property/scenario targets, `cargo clippy -p fairlead-core --all-targets -- -D warnings`, `cargo fmt --all --check`, and `cargo test -p fairlead-core -- --test-threads=1` giving the same results as the default run.
