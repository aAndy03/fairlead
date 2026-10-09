# Scopes and Inheritance
> Status: draft · Milestone: mixed · Decisions: D-02, D-03, D-10, D-11, D-12, D-14, D-18, D-26, D-31, D-33, D-34, D-37, D-46 · Related: 01-glossary.md, 11-files-and-classes.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 15-cli.md, 20-versions-and-seal.md, 32-portability-and-restructure.md

## Purpose
A codebase is a tree of folders. Fairlead lets any folder carry a managerial folder (`fairlead/`) that adds rules, items and exceptions for its subtree, while everything not stated locally is inherited from the nearest ancestor. This chapter defines the scope tree, how the effective view of a path is resolved, who may change what (override policies), how exceptions are recorded and read from both sides, what control files at every level may say, and what moves up or down the tree.

## Concepts
- **Scope.** A directory `D` whose managerial folder `D/fairlead/` holds a control file with `[fairlead] scope = "<slug>"`. A scope is **eligible, not mandatory** for any folder (D-11).
- **Scope id.** A STORED slug (`[a-z0-9]+(-[a-z0-9]+)*`, ASCII, at most 32 chars) in the control file. Identity is the id, never the path. **Membership** is path-derived: a file belongs to exactly one scope, the nearest ancestor-or-self directory that is a scope.
- **Implicit folder.** A folder without a managerial folder. It resolves to its nearest ancestor scope. Lines placed in its files are **file-local**: they affect only that file.
- **Root.** The scope whose control file has `[fairlead] root = true`, found by `--root`, `FAIRLEAD_ROOT`, or the upward search of D-31 (nearest `root = true` only; ordinary managerial folders are skipped).
- **Chain** of a path: the scopes on its path, `[root … nearest]`.
- **Item.** Anything with an ID `[scope:]kind.name` (rule, token, constant, component, doc, file-item). Its **home scope** is where its declaration is authored (a `def` in a doc, or a control-file table). Each item has an override **policy**.
- **Effective view** `E(S)`: the fold of the chain root-to-S, each item annotated with provenance (home, applied `ovr`s, grants). `E(path)` adds the file-local lines of that file. Authored intent is computed at read time, never copied down (D-10).
- **Override policy** (D-02): `open`, `needs-exception`, `sealed`.
- **`ovr`.** A declared override of an inherited item (a directive, or a `[[ovr]]` table for a scope-level exception). **`allow`**: a `[[allow]]` table in the ancestor's control file that makes an override on a `sealed` item effective. **`req`**: a `[[req]]` table proposing an upward escalation (inert until the ancestor acts).
- **Exception ledger.** The generated view of all effective `ovr`s that deviate from an ancestor, with reason, owner, expiry and distance.
- **Tombstone.** A stored record left when a scope is demoted or deleted (see [portability and restructure](32-portability-and-restructure.md)).

## Rules

### Scope tree and identity
- **R-SCOPE-001 [0.1]** A scope MUST declare `scope` in the `[fairlead]` header of its control file. The id MUST be ASCII, match the slug grammar, and be unique in the codebase after case folding.
- **R-SCOPE-002 [0.1]** A scope's identity MUST be its id. No stored fact (ledger entry, grant, exception, alias) MAY be keyed by path. Paths appear only in scope-relative globs, native import paths and prose.
- **R-SCOPE-003 [0.1]** Membership MUST be derived from the path: the nearest ancestor-or-self scope; the only exception is an explicit `own` convention (R-SCOPE-019, [0.2]). A file cannot be in two scopes; cross-cutting groupings use kinds, tags and `applies` globs, never scopes.
- **R-SCOPE-004 [0.1]** The root is the scope named by `--root`, else by `FAIRLEAD_ROOT`, else the nearest ancestor-or-self of the working directory (or of `-C <dir>`) whose managerial folder's control file has `[fairlead] root = true` (D-31). Ordinary managerial folders are skipped. Fairlead MUST NEVER invent a root: none found is exit 2 with the `fairlead init` hint. The version-control top level is never used, so non-VCS trees work the same way. A second `root = true` control file below the chosen root is `ROOT-DUP` (HN-C004) until sub-roots are adopted in 0.3 (D-22).
- **R-SCOPE-005 [0.1]** Two control files with the same scope id are the error `DUP-SCOPE-ID` (HN-C007). A managerial folder not registered in its parent scope's membership shard is the error `SCOPE-UNREGISTERED` (HN-C008); `sync` registers it only on explicit request.
- **R-SCOPE-006 [0.1]** Symbolic links and junctions inside a managerial folder MUST be an error. Fairlead MUST NOT follow links when walking the tree.
- **R-SCOPE-007 [0.1]** Two directories whose names differ only in case MUST raise a load-time error naming both paths, on every platform.

### Scope lifecycle (lazy creation, promotion, demotion)
- **R-SCOPE-010 [0.1]** Fairlead MUST NOT create a scope or a control file implicitly. Materialisation happens only through `fairlead new scope <dir>` (or `fairlead new` when the user places a scope-level item there). A minimal scope is a control file whose only content is the `[fairlead] scope` header; other files appear on first need. Scopes exist only where declared (no discovery by walking); `new scope` warns, without refusing, when the new scope would sit deeper than 3 scopes below the root (the depth hint is advisory: any folder stays eligible, D-11).
- **R-SCOPE-011 [0.1]** The root control file MAY declare eligibility hints in the root `[eligibility]` table: `min-lines` (default 5) file-local lines in at least `min-files` (default 3) files of one folder ([syntax](12-directive-syntax.md) schema index), and further hints such as a workspace member or a folder with its own docs. `check` MUST report `SCOPE-ELIGIBLE` (HN-N002) as info only.
- **R-SCOPE-012 [0.1]** Equivalence invariant: a scope is semantically identical to "its defs and ovrs kept in the nearest ancestor with `applies = "<dir>/**"`". Promotion and demotion MUST therefore be lossless and mechanical, and a promote-then-demote round trip MUST leave the effective view unchanged.
- **R-SCOPE-013 [0.1]** `fairlead new scope --hoist` MUST lift identical file-local lines found in several files of the folder into one scope-level def or ovr with an `applies` glob. `check` SHOULD suggest hoisting when the same `ovr` text occurs in at least N files.
- **R-SCOPE-014 [0.1]** Demotion MUST write a tombstone in the nearest ancestor (scope id kept as alias, former path, date of the seal or `sync` run) and MUST refuse, with a diagnostic, when it would create a duplicate ID in the ancestor. Deleting a registered scope's folder without demotion is the error `SCOPE-VANISHED` (HN-C009).
- **R-SCOPE-015 [0.1]** Leaf folders have no minimum: a leaf MAY stay implicit with file-local lines only. File-local items can never be referenced from other files.
- **R-SCOPE-016 [0.1]** Membership is declared by the folder, never taken from a manifest. Fairlead MAY read Cargo workspace members from `Cargo.toml` (TOML only; fairlead reads no YAML and does not run `cargo metadata`, D-50) as an oracle for the eligibility hints of R-SCOPE-011 and for default excludes, and MUST NEVER edit a manifest (R-CLI-014). Where a declaration and a manifest disagree the declaration wins; a manifest member without a scope yields at most the `SCOPE-ELIGIBLE` (HN-N002) hint.
- **R-SCOPE-017 [0.1]** A declaration file shared by several scopes (a bridge declaration, a shared token contract) MUST live in the managerial folder of its home scope (`contract/` or `docs/`). When the sites that implement it sit in different sibling scopes, the home scope is their nearest common ancestor. A `def` carrying `home=<scope>` MUST name an ancestor-or-self of the scope of every implementation site; otherwise it is `HOME-MISMATCH` (HN-C013).
- **R-SCOPE-018 [0.1]** Framework layouts that path-derived rules mishandle are declared, not guessed, in a `[[convention]]` table (root or any scope, applying to its subtree): `glob` (scope-relative) plus exactly one action. `transparent = true` makes a matching path segment invisible when `applies` and rule globs are matched (Next.js route groups such as `app/(*)`); membership is unchanged because it follows the folder tree. `class = "external-generated"` classes matching files as external-generated (R-FILE-009, for example `**/*.gen.ts`). Test trees such as Rust `tests/` crate roots are members of the scope that holds them and are excluded from rules by the rule's own `exclude` globs.
- **R-SCOPE-019 [0.2]** `[[convention]]` also takes `own = "<scope id>"`: matching files belong to the named scope instead of the nearest ancestor by path. The named scope MUST be a descendant of the declaring scope; one file still has exactly one scope. `sync` MAY scaffold `own` entries from manifest members, only on explicit request. A directory `D/` that is a scope, with a sibling module file `D.<ext>` (Rust `edge.rs` beside `edge/`) and no `own` entry deciding it, is the error `SCOPE-AMBIG` (HN-C014). Code in a manifest member that has no scope of its own belongs to the nearest ancestor scope (an implicit folder).

### Resolution
- **R-SCOPE-020 [0.1]** Given the root, resolution of a path MUST be a pure function of authored files (plus, in 0.3, the lock for foreign boundaries). It MUST NOT depend on the invocation directory, the clock or traversal order; only root selection (R-SCOPE-004) uses the working directory.
- **R-SCOPE-021 [0.1]** Resolution MUST process the chain root to nearest. Per scope the order is: control keys, then `def`s, then `ovr`s. File-local lines are overlaid last.
- **R-SCOPE-022 [0.1]** Merge by field type: scalar = nearest wins if the policy allows; keyed map = union by ID; list/set = child appends, removal needs an explicit `ovr op=remove`, replacement only for lists declared `replaceable`; ordered lists use `before=`/`after=` anchors, never indexes (D-12).
- **R-SCOPE-023 [0.1]** A child redefining an ID visible from its chain with `def` instead of `ovr` MUST be the error `DUP-ID` (HN-R005, D-12), whatever the item's policy. Sealed items forbid shadowing by either form.
- **R-SCOPE-024 [0.1]** Bare IDs MUST be unique along a chain. Siblings MAY reuse a bare name; a reference to a sibling's item MUST be qualified `scope:kind.name`. An unqualified ambiguous reference is `AMBIG` (HN-R007). IDs are chain-unique, not globally unique, to keep names short.
- **R-SCOPE-025 [0.1]** `applies` globs MUST be relative to the scope that owns the line. A glob containing `..`, a leading `/`, a `\` or a `:` MUST be rejected at load; every other relative glob stays inside the owning subtree by construction. A child MAY narrow an inherited glob freely.
- **R-SCOPE-026 [0.1]** An ancestor adding an ID that a descendant already declares MUST win. `check` MUST report the collision on the ancestor's cone and `sync` MUST propose an alias rename for the descendant, updating in-scope references; it MUST NOT apply it unasked.
- **R-SCOPE-027 [0.1]** Per-scope views SHOULD be computed once in a depth-first pass and memoised, keyed by the hash of the chain's authored files. Invalidation MUST stay inside the changed subtree.
- **R-SCOPE-028 [0.1]** Files fairlead never merges (stylesheets, tsconfig, manifests, prose docs) are layered by their native mechanism and checked through the extends contract in [files and classes](11-files-and-classes.md).
- **R-SCOPE-029 [0.2]** A qualified sideways reference (R-SCOPE-024) MUST name an item listed in the owning scope's `provides` (or declared visibility `public`). A target not provided is `NOT-PROVIDED` (HN-R008). In 0.1 only the qualifier is required.

### Override policies (D-02)
- **R-SCOPE-030 [0.1]** Each item MUST have one policy. The root declares it in `[policies]` per key pattern ([syntax](12-directive-syntax.md)); the home scope may refine it per item with the `policy=` key of its `def`. Unset defaults: kind `rule` is `sealed`; every other kind is `needs-exception`. A rule marked `waivable = true` is `needs-exception`.
- **R-SCOPE-031 [0.1]** `open`: the nearest scope's `ovr` wins and needs no reason. It MUST still be counted as variance in the ledger. An `open` target is the one policy where `why` is optional ([syntax](12-directive-syntax.md) R-SYN-011).
- **R-SCOPE-032 [0.1]** `needs-exception`: an `ovr` is effective and recorded iff it carries `why`, `owner` and `exp`. An incomplete `ovr` MUST be inert and reported as `OVR-INCOMPLETE` (HN-S016).
- **R-SCOPE-033 [0.1]** `sealed`: an `ovr` is inert (the ancestor value stays effective, code is measured against it) unless a matching `allow` exists (R-SCOPE-036). With a matching `allow` the item behaves as `needs-exception` for that grantee.
- **R-SCOPE-034 [0.1]** Tightening is free on every policy. A change that moves a root-ordered restrictive scalar (severity, enforcement level, retention length, required slots, environment set) toward the stricter canonical slot is `op=tighten`: effective without `why`/`exp`, still written as an `ovr` line (D-12) and counted. Retention is such a scale: a child may lengthen `hot`, `ttl` and `min-age` but not shorten them ([retention](33-retention-and-compaction.md) R-RET-013). Any other change on a `sealed` item is a loosen/replace and is inert without `allow`.
- **R-SCOPE-035 [0.1]** Only the home scope MAY set or change an item's policy or add `allow`s for it. A child MUST NOT mark an ancestor's item `sealed` or loosen its policy.
- **R-SCOPE-036 [0.1]** A `[[allow]]` table names its `target` (the item, optionally with `field`), its grantee scopes in `for`, and `why` and `exp` (`until` from 0.2). It applies to the grantee and its subtree and is invalid when authored outside the item's home scope.
- **R-SCOPE-037 [0.1]** Vocabulary keys (layer ranks, environment and maturity slots, ID namespace, policy kinds, kind definitions) are root-only and sealed. A child control file setting one MUST be inert and reported as `CTL-DELEG` (HN-C003, non-delegable key); fairlead SHOULD scaffold a `req` for it.
- **R-SCOPE-038 [0.1]** An inert `ovr`, `allow` or control key MUST never abort the run. Valid lines still apply, and the effective view is computed with the inert lines ignored.
- **R-SCOPE-039 [0.1]** Floor rules compiled into the binary MUST NOT be overridable by any policy, `allow` or `ovr` (D-03).

### Exception record and ledger
- **R-SCOPE-040 [0.1]** An `ovr` MUST name its target (`kind.name` or `kind.name.field`), an `op` (`loosen|tighten|set|replace|add|remove`) and the new value. When overriding another `ovr` it MUST carry `base=<exception id>`, set by `new`/`sync`.
- **R-SCOPE-041 [0.1]** An `ovr` MAY be file-local (applies to that file) or scope-level in the control file with `applies` (default: whole scope). The reserved kind `exc` names ledger entries: `ui-sidebar:exc.x1`.
- **R-SCOPE-042 [0.1]** Relativity: one record, two readings. Inside the overriding scope, the patched value IS the effective principle and conforming code produces no drift against the root. Outside, the record is a ledger entry ("varied in scope S"). `explain` MUST print both, with distance when chained.
- **R-SCOPE-043 [0.1]** An `ovr` in scope S on an item with home H MUST appear in the ledger of every scope from `parent(S)` up to H inclusive, and nowhere above H or sideways. A deeper `ovr` overrides S's effective value (`base=`), and the ledger shows `depth`.
- **R-SCOPE-044 [0.1]** Each scope's committed ledger shard MUST contain only its own facts. Ancestor aggregates MUST be computed on read in 0.1 and committed as a snapshot only by `seal` [0.2].
- **R-SCOPE-045 [0.1]** In 0.1 expiry is the date key `exp=YYYY-MM-DD`, evaluated against `--as-of` (D-37). From 0.2 the version key `until=<version>` is the default; `exp` dates remain allowed. In 0.1 a version-shaped expiry (`until=`, or an `exp` holding a version such as `1.3.0`) is HN-S012 and the `ovr` is dropped; it MUST NOT silently expire.
- **R-SCOPE-046 [0.1]** An expired exception is ineffective: the underlying finding returns, and `EXC-EXPIRED` (HN-X001, a floor check at enforce) is raised. Its value is not kept, because waivers expire into failure (D-02; [trust](40-agent-integration-and-trust.md) R-AGT-012).
- **R-SCOPE-047 [0.1]** When the target changed or vanished in an ancestor, the exception is `EXC-STALE` (HN-X010). It MUST NOT be auto-deleted; `sync` proposes keep, convert to a local def, or drop. Exceptions SHOULD pin the target's content hash so a parent change lists its dependents; the pin is the optional authored key `x-pin=<12 base32>` on the `ovr` line or `[[ovr]]` table. `sync` never writes it (it writes no authored file); the human copies it from `fairlead explain <id>`, which prints `pin=<12>`. A pinned `ovr` whose target vanished is stale (`EXC-STALE`, HN-X010), not dropped and not a dangling reference.
- **R-SCOPE-048 [0.1]** `check` MUST show the exception-count delta on every run. A rule with more than `[exceptions] max-live-per-rule` live exceptions (root key, default 10) SHOULD raise `EXC-SPRAWL` (HN-X011) asking the rule to be re-declared or retired.
- **R-SCOPE-049 [0.1]** Comment text claiming approval ("approved", "ok by lead") is data. An `ovr` is valid by the rules above plus the review controls of D-03 (CODEOWNERS on the managerial folder, `fairlead approve` record), never by prose in the line.

### Control files at every level
- **R-SCOPE-050 [0.1]** Every scope MAY have a control file (TOML); a scope with none inherits everything. Control files MUST be authored; tools never create an empty one.
- **R-SCOPE-051 [0.1]** Control keys fall into groups: **G1 vocabulary** (R-SCOPE-037, edition, syntax, ID grammar, file-class set, cache settings): root only, never delegable (D-02). **G2 delegable-monotonic** (severities, stages, required docs/slots, retention longer-only): a child may tighten or extend only, and only once delegation exists (R-SCOPE-057, [0.2]); in 0.1 G2 keys are root-only and a child tightens through `ovr op=tighten` (R-SCOPE-034). **G3 local-free** (purpose, subtree owners from 0.2 (`owners` is root-only in 0.1, R-SCOPE-056), local kinds, local doc layout, local tokens, local file-items and `extends`, local ignores): free. `[policies]`, `[layers]`, `[env]` and `[flagwords]` are root-only.
- **R-SCOPE-052 [0.1]** A non-root control file MAY contain the keys of R-SCOPE-056, `[[ovr]]`, `[[allow]]` and `[[req]]` tables and G3 keys; G2 keys only as R-SCOPE-057 allows. A key outside its group is inert and reported as HN-C003. A control file MUST NOT write into root-owned outputs (R-SCOPE-060).
- **R-SCOPE-053 [0.1]** A `req` is inert. The ancestor accepts it by authoring the corresponding def, `allow` or control change; it rejects it with a one-line reason. `sync` marks resolved requests and reduces them to one-line tombstones. An unresolved `req` MUST show in the ancestor's `context`. A `[[req]]` MAY carry `blocking = true`. From 0.2 an unresolved `req` does not block `seal` by default: `seal` lists it and `check` warns; one older than `version.escalation-grace` seals (default 3) is an error at seal; a `blocking` one blocks the very next seal ([versions](20-versions-and-seal.md) R-VER-058).
- **R-SCOPE-054 [0.1]** Failing closed: an unparseable control file, or a registered scope whose control file is missing, MUST fail `check` for that scope and its whole subtree. The subtree's view is `UNRESOLVED`; fairlead MUST NOT fall back to the ancestor view as if it were verified. `context` shows the error and the ancestor view marked `unverified`.
- **R-SCOPE-055 [0.1]** After a non-root control edit, `check` MUST give identical diagnostics whether run at the child or the root. The invocation directory selects only the cone that `sync` writes into (subtree plus chain).
- **R-SCOPE-056 [0.1]** In 0.1 a non-root control file adds facts and never sets an inherited key. It MAY hold: the `[fairlead] scope` header, `purpose`, `[[file]]`, `[[vendor]]`, `[[ovr]]`, `[[allow]]` (for descendants only), `[[req]]`, `[[convention]]`, `[scan]` `exclude`/`skip`, new namespaced `[kinds.K]` and `[file-kinds.K]`, and new `[rules.R]` declarations (new IDs). Changing the value of an inherited item or key is done with `ovr` (R-SCOPE-034), never by redefining the key. `owners` is a root-only list in 0.1 (a non-root file does not set it; D-51); from 0.2 a declaring scope may add owners for its own subtree, additively.
- **R-SCOPE-057 [0.2]** Delegation of G2 keys uses a root `[delegate]` table: `"<key pattern>" = "tighten"`. A child control file may then set a matching G2 key, monotonically only (R-SCOPE-034); an unlisted key stays non-delegable (HN-C003). Vocabulary keys (G1) cannot be listed in `[delegate]`; they are root-only and declared in the root, not compiled into the binary (D-02).
- **R-SCOPE-058 [0.1]** Grants are not re-delegable. Only the home scope authors an `[[allow]]` (R-SCOPE-035, R-SCOPE-036); a grant already covers the grantee's subtree, and a mid-chain grantee cannot forward it. A scope that needs the same exemption for another branch escalates with a `[[req]]` to the home scope.

### Propagation
- **R-SCOPE-060 [0.1]** Direction is a property of the line class, fixed by the table below, never chosen per line or per file.
- **R-SCOPE-061 [0.1]** `sync` MUST write generated projections from each scope's own effective view into that scope's own folder, and MUST write only a scope's own facts upward. It MUST NOT edit authored files.
- **R-SCOPE-062 [0.1]** Generated output MUST be written per file by temp-write then rename with a journal in `.fairlead/`. `check` MUST report mixed generation stamps across scopes.
- **R-SCOPE-063 [0.1]** A scope root control change MUST mark every dependent descendant's generated files stale (`check`) and let `sync` rebuild them in one deterministic pass. `--scope` limits the write cone.
- **R-SCOPE-064 [0.1]** Two `ovr` records in one scope (directives or `[[ovr]]` tables) with the same target and field whose extents overlap (same file, or overlapping `applies`) are the error `DUP-OVR` (HN-X014); both are inert (fail closed) unless one carries `base=` naming the other. `ovr`s in sibling scopes never overlap, because each applies to its own subtree; they are independent ledger entries, not an error.
- **R-SCOPE-065 [0.1]** `exc` is a built-in reserved kind (R-SCOPE-041). A control file declaring `[kinds.exc]` is rejected at load as an error; the root cannot redefine it and children cannot shadow it.
- **R-SCOPE-066 [0.3]** `extends` and `requires` are separate fields with separate meanings. `extends` is content inheritance between file-items or docs, checked by the extends contract ([files](11-files-and-classes.md) R-FILE-051); `requires` is a dependency declaration generated from `use` and `extends` plus authored additions ([portability](32-portability-and-restructure.md) R-PORT-020). No field carries both modes.

Propagation matrix. DOWN is computed at read time. UP is a fact in a generated shard.

| Line class | Authored in | DOWN | UP |
|---|---|---|---|
| `def` | home scope | into subtree views (read time) | L0 entry in the home scope's registry shard (`sync`) |
| `use` | anywhere | resolved up the chain at read | edge in the generated ref graph (`sync`); dangling = drift |
| `ovr` | child or file | applies to the child subtree (read time) | ledger entry up to the home scope (R-SCOPE-043) |
| `flag` | home scope | consumers' `check` reports use (read time) | nothing |
| `env` | any | narrows monotonically (read time) | env coverage summary at `seal` [0.2] |
| `[[allow]]` | home scope | grant to named scopes (read time) | nothing |
| `[[req]]` | child only | none | to the named ancestor, shown until resolved |
| control keys (TOML) | any scope, per group | per group, G2 tighten only | scope self-facts shard |
| `-- note` tail (any directive) | any | none | none (shown by `explain` only) |

## Examples
Root control file, policies declared once for the whole tree:
```toml
# fairlead/control.toml (root)
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[policies]
"rule.*" = "sealed"          # children may only tighten
"tok.*" = "needs-exception"
"cmp.*" = "open"

[[allow]]                  # grant on a sealed item (home scope authors it)
target = "rule.max-lines"
for    = ["apps-console"]
field  = "limit"
why    = "chart legend needs more lines"
exp    = "2027-03-31"
```
A scope-level and a file-local exception, one reading inside, one outside:
```fairlead
// apps/console/src/charts/line.tsx
// fairlead: ovr rule.no-inline-style op=loosen severity=info why="chart lib positions via style attr" owner=andy exp=2027-03-31
```
```text
explain rule.no-inline-style @ apps/console/src/charts/line.tsx
  effective   severity=info      from apps-console:exc.x1 (root says error)
  policy      needs-exception    home=root
  ledger      varied in apps-console, depth=1, exp=2027-03-31, owner=andy
explain rule.no-inline-style @ apps/admin/src/a.tsx
  effective   severity=error     from root
```
An escalation from a child that may not set a vocabulary key:
```toml
# apps/console/fairlead/control.toml
[fairlead]
scope = "apps-console"

[[req]]
to     = "root"
target = "layer-ranks.widget"
set    = "2"
why = "console needs a rank between molecule and organism"
```

## Edge cases
| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Child `def`s an ID it inherits, no `ovr` | `DUP-ID` (HN-R005); hint: convert to `ovr` or rename | R-SCOPE-023 |
| 2 | Two siblings define the same bare ID | Allowed; ambiguous unqualified use is `AMBIG`; qualified use needs `provides` from 0.2 | R-SCOPE-024, 029 |
| 3 | `ovr` on a sealed item, no `allow` | Inert + diagnostic; code measured against ancestor; `new req` scaffolded | R-SCOPE-033 |
| 4 | Child control sets a vocabulary key (edition, ranks, slots) | Inert HN-C003 (non-delegable key); `req` scaffolded | R-SCOPE-037 |
| 5 | Child lowers severity error to warn | Loosen: needs complete `ovr` (needs-exception) or inert (sealed). Raising warn to error is free | R-SCOPE-034 |
| 6 | Exception chain S then S2 | S2 `ovr` carries `base=` S's id; home ledger shows both with `depth=2` | R-SCOPE-043 |
| 7 | Exception expired while code relies on it | Waiver ineffective; the finding returns; `EXC-EXPIRED` (HN-X001, floor) | R-SCOPE-046 |
| 8 | Ancestor deletes or changes the overridden item | `EXC-STALE`; `sync` proposes keep / local def / drop, never auto | R-SCOPE-047 |
| 9 | Ancestor adds an ID a child already declares | Ancestor wins; collision reported on ancestor cone; alias rename proposed | R-SCOPE-026 |
| 10 | Control file unparseable in mid-chain scope | Subtree `UNRESOLVED`, `check` fails; no silent fallback to ancestor rules | R-SCOPE-054 |
| 11 | `fairlead/` of a registered scope deleted by hand | `SCOPE-VANISHED` (HN-C009; parent shard still lists it); demotion is the sanctioned way | R-SCOPE-014 |
| 12 | Stray `src/ui/fairlead/` created by an agent, or a copied root control file under a subfolder | `SCOPE-UNREGISTERED` (HN-C008) for the stray; a copied root control file is `ROOT-DUP` (HN-C004) from the real root. A run started inside the copy selects it as root (D-31), so CI MUST check from the real root | R-SCOPE-004, 005 |
| 13 | `ovr` glob `../**`, `/abs/**`, a glob with a backslash or `C:x` from a child | Rejected at load (`..`, leading `/`, `\`, `:`); `src/**` is fine and relative to the owning scope; exceptions never escape their subtree | R-SCOPE-025 |
| 14 | One rule collects 100 exceptions across scopes | `EXC-SPRAWL` (HN-X011); re-declare or retire the rule | R-SCOPE-048 |
| 15 | Child `ovr` text says "approved by lead" | Ignored as data; validity comes from policy and D-03 review controls | R-SCOPE-049 |
| 16 | Folders `Ui/` and `ui/` both tracked (created on Linux) | Load-time case-fold collision error on all platforms | R-SCOPE-007 |
| 17 | Control edit at a non-root scope, `check` run at root vs child | Identical diagnostics; only the `sync` write cone differs | R-SCOPE-055 |
| 18 | Scope promoted then demoted | Effective view unchanged (equivalence invariant) | R-SCOPE-012 |
| 19 | Root `open`, scope A `sealed` on same item | Stricter wins; descendants of A cannot loosen | R-SCOPE-035 |
| 20 | Two `ovr` lines for one key in one file or scope | `DUP-OVR` (HN-X014); both inert unless chained with `base=` | R-SCOPE-064 |
| 21 | Same item overridden in two sibling scopes | Allowed: independent ledger entries, no shared view | R-SCOPE-064 |
| 22 | Scope declared where the manifest lists no member, or a manifest member has no scope | Declaration wins; manifest member without scope is at most `SCOPE-ELIGIBLE` (HN-N002); manifests never edited | R-SCOPE-016 |
| 23 | `def` with `home=web` implemented in a scope outside `web` | `HOME-MISMATCH` (HN-C013) | R-SCOPE-017 |
| 24 | Next.js `app/(app)/page.tsx` and `*.gen.ts` | Declared in `[[convention]]` (`transparent`, `external-generated`), not guessed | R-SCOPE-018 |
| 25 | `edge.rs` beside a scope folder `edge/`, no `own` entry | `SCOPE-AMBIG` (HN-C014) error [0.2] | R-SCOPE-019 |
| 26 | Qualified sideways ref to an item not in `provides` | Accepted in 0.1; `NOT-PROVIDED` (HN-R008) from 0.2 | R-SCOPE-029 |
| 27 | Grantee scope tries to forward its `allow` to a sibling branch | Not valid; escalate with `[[req]]` to the home scope | R-SCOPE-058 |
| 28 | Child control file sets a G2 key (for example `sev`) in 0.1 | Inert HN-C003; use `ovr op=tighten`; delegation via `[delegate]` from 0.2 | R-SCOPE-056, R-SCOPE-057 |
| 29 | Control file declares `[kinds.exc]` | Rejected at load | R-SCOPE-065 |

## Resolved questions
- Q10-1 (resolved 2026-10-09): IDs are chain-unique, siblings reuse names with `scope:` qualification; see R-SCOPE-024
- Q10-2 (resolved 2026-10-09, D-34): tightening on a root-ordered scale needs no `allow`, loosening does; see R-SCOPE-034
- Q10-3 (resolved 2026-10-09, D-37): 0.1 date key `exp` with `--as-of`, `until=<version>` from 0.2; see R-SCOPE-045
- Q10-4 (resolved 2026-10-09, D-31): `--root`, `FAIRLEAD_ROOT`, upward search for `root = true` only; see R-SCOPE-004
- Q10-5 (resolved 2026-10-09, D-34): `[[allow]]` and `[[req]]` are control-file tables; see R-SCOPE-036, R-SCOPE-053
- Q10-6 (resolved 2026-10-09): no re-delegation of grants, escalate with `req`; see R-SCOPE-058
- Q10-7 (resolved 2026-10-09): `exc` is a reserved built-in kind; see R-SCOPE-041, R-SCOPE-065
- Q-SCOPE-1 (resolved 2026-10-09): 0.1 child control files add facts only (identity, local tables), inherited keys stay root-only; G2 delegation is 0.2; partly superseded by D-11, D-33, D-34 which require child identity header, `[[allow]]` and `[[req]]` tables in 0.1; see R-SCOPE-056, R-SCOPE-051
- Q-SCOPE-2 (resolved 2026-10-09): folder declares scope, manifests read as an oracle and never edited, declaration wins; see R-SCOPE-016
- Q-SCOPE-3 (resolved 2026-10-09): shared declarations live in the home scope's folder, `home=` checked against implementation sites; see R-SCOPE-017
- Q-SCOPE-4 (resolved 2026-10-09): `extends` and `requires` are two fields; see R-SCOPE-066
- Q-SCOPE-5 (resolved 2026-10-09): policies delegated through a root `[delegate]` table (0.2); vocabulary is root-only in the control file, not compiled in (superseded in part by D-02); see R-SCOPE-057, R-SCOPE-037
- Q-SCOPE-6 (resolved 2026-10-09): duplicate `ovr` in one scope is the error `DUP-OVR` (HN-X014); sibling scopes are independent, not an error; see R-SCOPE-064
- Q-SCOPE-7 (resolved 2026-10-09): qualifier plus `provides` export, `provides` enforcement from 0.2; see R-SCOPE-024, R-SCOPE-029
- Q-SCOPE-8 (resolved 2026-10-09): declared scopes only, `fairlead new scope` is the verb form, depth over 3 is an advisory warning; "root-only in 0.1" superseded by D-06/D-38 (spec covers nested scopes, implementation order root first) and "any folder" by D-11; see R-SCOPE-010
- Q-SCOPE-9 (resolved 2026-10-09): explicit `own` convention for non-owned code, ambiguous layout is an error, 0.2; see R-SCOPE-019
- Q-SCOPE-10 (resolved 2026-10-09): explicit `[[convention]]` list in control; see R-SCOPE-018
