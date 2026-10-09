# Environments, Maturity and Lifecycle
> Status: draft · Milestone: 0.2 · Decisions: D-02, D-12, D-14, D-16, D-18, D-19, D-27, D-33, D-34, D-35, D-37, D-42, D-46, D-49, D-50 · Related: 10-scopes-and-inheritance.md, 11-files-and-classes.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 15-cli.md, 20-versions-and-seal.md, 30-hierarchy-and-engines.md, 31-variation-protocol.md, 33-retention-and-compaction.md, examples/rust-workspace.md

## Purpose
Three independent questions are asked about every ID'd item: where does it exist (environment), how settled is it (maturity), and what is planned for it (lifecycle). This chapter defines each axis, how teams use their own vocabulary on top of fixed ordered slots, the rules fairlead checks between them, how real build gates (Cargo features, `cfg`, TS env flags) are cross-checked for existence only, how lifecycle plans become due dates, and what remains after an item is removed or rejected (tombstones and the graveyard).

## Concepts

| Term | Definition |
|---|---|
| **Environment (env)** | Where an item exists or runs. A set of values from an ordered promotion path declared at the root (order = exposure toward production). |
| **Maturity** | How settled an item is, per env. User words map onto integer ranks. Only rank is compared. |
| **Lifecycle** | Planned and actual state: `proposed -> active -> deprecated -> removed`, plus `rejected` (graveyard). States are fixed; their labels may be aliased. |
| **Slot** | A canonical ordered position. Envs and maturities are user-defined words mapped to slots (D-16). |
| **Binding** | Declared name of the real gate for an env: a Cargo feature, a `cfg` expression, a TS flag token. Checked for existence, never evaluated. |
| **Plan flag** | A `flag` directive whose WORD is the pending intent (`deprecate`, `replace`, `edit`, `remove`) on an `active` or `deprecated` item, per [syntax](12-directive-syntax.md) `flag WORD [ID] [to=ID] [target=]`. |
| **Tombstone** | Permanent record in the scope folder that an ID was removed. Written as a TOML `[[tomb]]` table (not a directive). The ID is reserved forever. |
| **Graveyard entry** | Permanent record of a rejected proposal with match terms and a fingerprint, written as a TOML-frontmatter doc in `docs/graveyard/`. |

Theme variants (light, dark, brand) are not environments and are not a fourth ordered axis here; they are values of a variation axis ([variation](31-variation-protocol.md), R-ENV-037).

The three axes are orthogonal: lifecycle says what is intended, env says where it can be reached, maturity says how much to trust it. A Cargo feature is none of these: it is a compile variant, referenced from a binding.

## Rules

### Vocabulary
- **R-ENV-001** [0.2] The env path, maturity words and lifecycle aliases MUST be declared only in the root control file. These are root-only sealed keys (D-02); a child scope's attempt MUST be reported as an escalation proposal to the ancestor, not applied.
- **R-ENV-002** [0.2] Envs MUST be ordered by exposure, not alphabetically. Maturity words MUST map to ranks (gaps allowed; for example `alpha=0, beta=1, rc=2, stable=3`).
- **R-ENV-003** [0.2] Old words MUST remain valid as aliases forever (`qa -> staging`). A word that is neither canonical nor alias MUST be `H-VOC-UNKNOWN`. `sync --migrate-vocab` MAY rewrite sources to canonical words.
- **R-ENV-004** [0.2] Lifecycle states MUST be fixed (`proposed`, `active`, `deprecated`, `removed`, `rejected`); only their display aliases are configurable, because due dates, tombstones and the graveyard depend on them.
- **R-ENV-005** [0.2] An item with no `in=` MUST default to the nearest scope policy; with none, to all envs. A missing `mat` MUST resolve the same way, else `H-MAT-MISSING` (warn; error when the scope's ramp reaches enforce).
- **R-ENV-006** [0.2] A scope's default maturity (the `mat` it declares for itself in its control file) MUST also set the default docs-first level of that scope ([versions](20-versions-and-seal.md) R-VER-034): the lowest declared maturity rank starts at `observe`, any higher rank at `warn`. An explicit `docs-first.level`, or a level inherited from an ancestor, always wins. There is no separate `ramp` key.

### Env and maturity declarations
- **R-ENV-010** [0.2] Environment and maturity are keys on the `def` of the ID they describe, declared by the kind (`in=<env list>`, `mat=<word>` or `mat=<env>:<word>,...`, optional `bind=`). The `env` verb takes only a bare environment list and gates a region: `env dev` on a file (file form) or `env dev +N`/`+end`...`end` on a region ([syntax](12-directive-syntax.md) R-SYN-015, R-SYN-016 to R-SYN-019).
- **R-ENV-011** [0.2] Per-env maturity MUST satisfy: maturity in a later env <= maturity in an earlier env (`H-MAT-ORDER`). An item cannot be more mature in prod than in dev.
- **R-ENV-012** [0.2] Maturity regression (for example stable to beta) MUST be allowed through a change doc, MUST emit `H-MAT-REGRESS` (warn) and a ledger entry, and MUST NOT retroactively break dependents unless a maturity rule now fails.

### Rules between axes
- **R-ENV-020** [0.2] `rule.maturity-gap` (formerly written `R-MAT-1`, not a legal ID under [syntax](12-directive-syntax.md) R-SYN-007): an item MUST NOT depend on an item of lower maturity rank (stable cannot depend on alpha). The default severity is error when the gap is two or more ranks and warn when one rank; a root MAY instead set one level for the rule (`[rules."rule.maturity-gap"] sev`), which then replaces the gap-based default. An alpha item may depend on anything. The rule is `waivable` (D-02).
- **R-ENV-021** [0.2] External packages MUST carry a declared maturity in the root control file; unlisted externals are `unrated` (info).
- **R-ENV-022** [0.2] `rule.env-leakage` (leakage): an item that reaches env E MUST NOT use an item whose env set excludes E. In 0.1/0.2 this is provable only over declared edges (`use` lines and Cargo/package manifests). A scope MAY add `watch` terms (textual, heuristic, warn) for name mentions.
- **R-ENV-023** [0.2] Reachability MUST be evaluated per env: a dev-only item is unreachable in prod by design, not stale.
- **R-ENV-024** [0.2] Lifecycle edges: an `active` item using a `removed` item MUST be an error (`HN-R004`; a warning while a `replaced-by` alias exists, R-LC-022); using a `deprecated` item MUST warn (`HN-R006`), escalating to error at the due date.

### Cross-checking real gates
- **R-ENV-030** [0.2] `bind=` MUST name the real gate. `check` verifies existence only. Fairlead MUST NOT evaluate `cfg`, feature unification or build scripts, and MUST report the result as "declared + bound + syntactically present", never "excluded from the build". Feature unification (a dependent enabling a dev feature in a prod build) is an accepted, documented limit (D-16): fairlead reports the declared edges only and does not resolve features through `cargo metadata`.
- **R-ENV-031** [0.2] A file or mount point gated by `env dev` MUST mention at least one dev binding. A file mentioning a dev binding with no env declaration MUST be `H-ENV-UNDECLARED` (drift).
- **R-ENV-032** [0.2] Rust: a bound feature MUST exist in the nearest `Cargo.toml` `[features]` and MUST NOT be in `default`; a dev-only crate MUST NOT appear in a prod crate's `[dependencies]`. Only manifest TOML is parsed; no AST (D-27). The latter violation is an error.
- **R-ENV-033** [0.2] TypeScript: `import.meta.env.*` and `process.env.NODE_ENV` tokens MUST match the binding table; a token in no table entry is `H-ENV-UNBOUND` (info). Extending the table, not the code, is the fix.
- **R-ENV-034** [0.2] Runtime feature flags (vendor flag systems) MUST be modelled as `bind=flag:<name>` (info) and checked only against a declared registry file. Fairlead MUST NOT claim their runtime state.
- **R-ENV-035** [0.2] Platform `cfg` and build-script aliases MUST NOT be treated as env. A scope MAY map a platform to an env binding by `bind.<env> = "feature:<X>"` existence only.
- **R-ENV-036** [0.2] Test-only code MUST default to env `test` when the root env path declares that word (or an alias of it) and the code is recognised textually: paths `tests/**`, `**/__tests__/**`, `*.test.*`, `*.spec.*`, `*_test.rs`, or a file containing a `#[cfg(test)]` token. When the code is not recognisable, or no `test` env is declared, a file mentioning a test binding MUST be `H-ENV-UNDECLARED` (warn). Fairlead MUST NOT select a build-time env: it evaluates every declared env in each run (R-ENV-023), and the CI build matrix owns which real builds run.
- **R-ENV-037** [0.2] Theme variants (light, dark, brand) MUST NOT be modelled as envs. A variant word in `in=` or in the env path is `H-VOC-UNKNOWN` (R-ENV-003). Variants are values of a variation axis declared under [31-variation-protocol](31-variation-protocol.md).

### Monotonic restriction and waivers
- **R-ENV-040** [0.2] A child scope MAY only narrow: a subset of envs, an equal or higher maturity floor, equal or stricter severities.
- **R-ENV-041** [0.2] Loosening MUST be a declared exception with `why`, `owner` and `until=<version>`, effective and recorded in the exception ledger (D-02, [seal rules](20-versions-and-seal.md)), and only where the rule is `waivable` or an ancestor has granted an `[[allow]]` for it (D-34). A non-waivable (floor) rule MUST NOT be loosened.
- **R-ENV-042** [0.2] A child asking for a new env (for example `canary`) MUST be refused and filed as an escalation note surfaced in the root's `context`.

### Lifecycle flags and due dates
- **R-LC-001** [0.2] State changes are `proposed`, `active`, `deprecated`, `removed`. Plans are `flag deprecate <ID>`, `flag replace <ID> to=<ID>`, `flag edit <ID> to=<doc>` and `flag remove <ID>`, each with `target=<version>` (the target version; `target` is the flag key of [syntax](12-directive-syntax.md), D-49; `flag` never takes `until=`, which belongs to `ovr`). The reason is the directive note (`-- text`). State (`proposed`, `active`, `deprecated`, `removed`, `rejected`) and plan (the flags) are separate: a flag never changes state until seal applies it, and env and maturity are separate axes (R-ENV-010).
- **R-LC-002** [0.2] `since` MUST NOT be authored by default; seal records it in the ledger. An explicit `since=` is allowed for back-fill.
- **R-LC-003** [0.2] A symbolic target (`next-major`, `next-minor`) MUST be resolved at the seal that first contains the flag against the sealed version before that seal, and the concrete version recorded in the ledger.
- **R-LC-004** [0.2] Due computation: let `T` be the concrete `target=` version and `O` the projected open version. The item is **due** when `O >= T` (never `==`, since a bump can jump over a target). It is **approaching** (info) when `O < T` and `T` lies within `lc.warn-window` minors (default 1) of `O`. Due severity comes from rule `R-LC-DUE` (default error).
- **R-LC-005** [0.2] `by-date=` is opt-in and MAY coexist with a version target; whichever fires first wins. Dates are UTC, day granularity, evaluated against `--as-of`.
- **R-LC-006** [0.2] A due item MUST block `seal` unless: (a) its references are gone and the removal doc is in this seal; (b) an exception with `until` exists; or (c) a `retarget` change doc moves the target, recorded in the ledger. Repeated retargeting MUST raise `H-LC-STALE-PLAN`.
- **R-LC-007** [0.2] Replacement chains (`flag replace ... to=`) MUST resolve transitively to the final `active` ID. Cycles MUST be an error and printed. A replacement MUST be `active` (or `proposed` with warn).
- **R-LC-008** [0.2] References to a deprecated or replaced ID MUST warn until the item is due (R-LC-004) and be an error from the due version on, as `HN-R006` ([checks](13-checks-and-diagnostics.md), D-51); a reference to a `removed` ID is `HN-R004` (R-LC-022), an error unless a `replaced-by` alias exists (R-ENV-024). Existing references are baselined and counted, a reference added beyond the baseline is reported as new, and the count is stored per version in the ledger.
- **R-LC-009** [0.2] Early removal before target MUST need a change doc `kind = "early-removal"`; remaining references are errors.
- **R-LC-010** [0.2] Where a language has its own deprecation attribute (`#[deprecated]`, `@deprecated`), `check` MUST flag a public item with a `deprecate` flag but no attribute and vice versa. The target in the flag is authoritative; native attributes never override it.
- **R-LC-011** [0.2] (D-50) Renaming, merging and splitting IDs MUST be expressed with `flag replace <old> to=<new>` (rename: one flag; merge: one flag per old ID with the same `to`; split: the old ID is replaced by its primary successor and the change doc lists the others), followed by `flag remove` when references are gone. At removal, seal stores the successor as `replaced-by` in the old ID's tombstone (the `by` field, R-LC-020): the alias is permanent, because tombstones are permanent. A reference that still uses the old ID resolves through the alias and is reported (`HN-R004` as a warning, R-LC-022) until it is updated; before removal it is reported as a deprecated reference (R-LC-008). There is no `move --id` verb in 0.1 to 0.3 and no alias-rewriting tool.
- **R-LC-012** [0.2] There is no migration tool for long-lived branches that still use an old ID scheme. After a merge such a branch surfaces as deprecated or removed-ID references (R-LC-008, R-LC-022) and duplicate declarations (`H-ID-COLLISION`), fixed by hand.

### Tombstones
- **R-LC-020** [0.2] On removal, seal MUST write a tombstone into the scope folder as a TOML `[[tomb]]` table with `id`, `removed` (version), `by` (ID; the permanent `replaced-by` alias of a rename or merge, R-LC-011), `why` and, when files were removed, `paths` (the removed paths, for re-introduction matching, R-LC-033), in an append-only file `fairlead/tombstones/<year>.toml` (layout in [files](11-files-and-classes.md); D-16, D-32). Lifecycle removals are written by seal; from 0.2 `sync` also appends a `[[tomb]]` for an ID deleted outside the lifecycle flow (an ID in the committed shard with no declaration left), listed in its plan and flagged for the author to complete ([CLI](15-cli.md) R-CLI-046). A tombstone is never a directive.
- **R-LC-021** [0.2] A removed ID MUST be reserved forever. `new` MUST refuse to create it and `check` MUST report `HN-D003` (tombstoned-ID reuse, [checks](13-checks-and-diagnostics.md)) on any `def` of it.
- **R-LC-022** [0.2] A reference to a tombstoned ID MUST report the alias or removed-ID reference diagnostic of [checks](13-checks-and-diagnostics.md) (`HN-R004`: an error when the removed ID has no alias, a warning while a `replaced-by` alias exists, D-51) with the replacement (the tombstone `by` alias) and the reason, not a generic unknown-ID error.
- **R-LC-023** [0.2] Demoting or deleting a scope MUST leave a scope tombstone (D-11).
- **R-LC-024** [0.2] A tombstoned ID MUST NOT be revived under its old name (R-LC-021). The revive path is a change doc `kind = "revive"` with `revives = "<tombstoned id>"`, valid only with an `[[approval]]` recorded by `fairlead approve` (D-03). It creates a NEW ID formed from the old one with a version suffix (`engine.legacy-diff-v2`); the tombstone is not rewritten and the link is recorded in the ledger at seal. A graveyard entry that matched becomes `revisited` (R-LC-035).

### Graveyard
- **R-LC-030** [0.2] A graveyard entry MUST live in the scope where the proposal was rejected, apply to its whole subtree, and never be compacted away. Fields: `id` (reserved kind `grv`), `state` (`rejected` | `parked` | `revisited`), `at` (version), `what` (1 to 3 lines), `why`, `match` terms (words, globs, regexes), optional `revisit-after` (version).
- **R-LC-031** [0.2] Every entry MUST carry a **fingerprint** computed at creation by seal from the removed or rejected material: (a) normalised identifiers (public symbol names, wire names, route strings, file basenames); (b) min-hash of k=5 token shingles (whitespace/comment-normalised) over the removed files, 32 values. The fingerprint MUST be computed from the removed or rejected files only, once, at the seal that records the removal, and stored; later checks compare candidates to the stored fingerprint and never rescan the repository. A candidate matches when identifier overlap >= 0.6 or shingle similarity >= 0.5 (both thresholds configurable).
- **R-LC-032** [0.2] Proposal time: `new change|feature "<title>"` MUST match whole words of title and body against all `match` terms in the effective graveyard (self and ancestors) and, on a hit, print the entry and require `revisits=<grv-id>` plus a one-line reason in the new doc.
- **R-LC-033** [0.2] Check time: a draft or ready doc matching an entry without `revisits=` MUST yield `H-GRAVE-MATCH`; a new file matching an entry's glob, regex or fingerprint, or a tombstone's `paths` or ID, MUST yield the same, under a new ID too (re-introduction detection). Only new and changed files are compared. Default severity warn; `rejected` may be raised to error per scope.
- **R-LC-034** [0.2] Context time: `context <path>` SHOULD print one line (`graveyard: 3 rejected under here`) and `context "<task>"` SHOULD inject matching entries first, within budget.
- **R-LC-035** [0.2] Un-rejecting MUST be an entry state change to `revisited` linked to the new doc. Entries MUST NOT be deleted.
- **R-LC-036** [0.2] Silencing a match with `revisits=` MUST be logged and counted; repeated silencing raises the entry's `tripwire` count shown by `context`.
- **R-LC-037** [0.2] An `abandoned` change doc SHOULD prompt "graveyard candidate?" in `context`.
- **R-LC-038** [0.2] Authority: `rejected`, `revisited` and `do-not-reintroduce` records are unauthenticated in-repo (owner and agents may share a git author). Protection MUST rely on D-03: mandatory CODEOWNERS review of the managerial folder and `fairlead approve`. The docs MUST say that fairlead cannot verify who rejected.
- **R-LC-040** [0.2] An optional root setting `authority.signed-records = true` (default false) MAY require that the commit introducing a `rejected`, `revisited` or `do-not-reintroduce` record or an `[[approval]]` be signed. `check --ci` verifies it by running `git verify-commit` as an explicit foreground subprocess (D-26) and reports `H-AUTH-UNSIGNED` (warn; error per scope) otherwise. CODEOWNERS (R-LC-038) remains the baseline and is unaffected.
- **R-LC-039** [0.2] Matching is lexical and fingerprint-based; synonyms and rewrites evade it. This residual risk MUST be documented.

## Examples

Root control excerpt:
```toml
[env]
order = ["dev", "staging", "prod"]
[env.alias]
qa = "staging"
local = "dev"

[maturity]
alpha = 0
beta = 1
rc = 2
stable = 3

[lifecycle.alias]
sunsetting = "deprecated"
idea = "proposed"

[bind.dev]
rust = ['feature="devtools"', "cfg(debug_assertions)"]
ts = ["import.meta.env.DEV"]
[bind.prod]
rust = ["cfg(not(debug_assertions))"]
ts = ["import.meta.env.PROD"]

[rules."rule.maturity-gap"]
sev = "error"
waivable = true
```

Directives (`env` and `flag` verbs, D-18):
```fairlead
// fairlead: def engine.verify in=all mat=beta
// fairlead: def component.dev-panel in=dev mat=alpha bind=feature:devtools
// fairlead: env dev -- whole file is dev-only
// fairlead: def engine.export in=dev,staging mat=dev:beta,staging:alpha
// fairlead: flag deprecate engine.legacy-diff to=engine.diff-v2 target=2.0.0 -- quadratic on large files
// fairlead: flag replace component.button to=component.button-v2 target=1.6.0 by-date=2027-03-01
// fairlead: flag edit engine.verify to=d-7k3m9qxa target=1.5.0
// fairlead: ovr rule.maturity-monotonic why="diff-v2 is beta" owner=@pipelines until=0.5.0
```

Tombstone and graveyard entry:
```toml
# crates/engines/fairlead/tombstones/2031.toml (append-only; one file per year)
[[tomb]]
id = "engine.legacy-diff"
removed = "0.5.0"
by = "engine.diff-v2"
why = "v1 diff; quadratic on 100k-line files; replaced (ADR-014)"
```
```toml
+++
id = "grv.dark-toggle-v1"
state = "rejected"
at = "1.4.0"
what = "Per-component dark-mode toggles"
why = "Token drift across 40 components"
match = ["dark mode", "theme toggle", "/useTheme\\(/", "ui/**/toggle*"]
revisit-after = "2.0.0"
fingerprint = { ids = ["useTheme", "ThemeToggle"], minhash = "b3:9a1c..." }
+++
```

Re-introduction under a new ID (`text`):
```text
H-GRAVE-MATCH  src/ui/NightSwitch.tsx  similar to grv.dark-toggle-v1 (shingles 0.71, ids 2/3)
  fix: add revisits=grv.dark-toggle-v1 with a reason to the covering change doc, or remove
```

## Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Dev-only crate in a prod crate's `[dependencies]` | Error via manifest check; no AST needed | R-ENV-032 |
| 2 | Barrel/re-export hides a dev-only symbol in a prod path | Provable only if the barrel carries a `use`; otherwise `watch` text matching; AST-free limit documented | R-ENV-022 |
| 3 | cfg/feature expression absent from the bindings table | `H-ENV-UNBOUND` info; extend the table | R-ENV-033 |
| 4 | Feature unification: a dependent enables `dev-probe`, putting dev code in prod | Not detectable without resolving features; documented unprovable limit | R-ENV-030 |
| 5 | Runtime flag service gates a feature | `bind=flag:<name>` info; existence in registry only | R-ENV-034 |
| 6 | Rename env word `qa` to `staging` | Old word stays alias; optional `sync --migrate-vocab` | R-ENV-003 |
| 7 | Child scope wants env `canary` | Refused; escalation note appears in root `context` | R-ENV-042 |
| 8 | Stable item regressed to beta | Change doc, `H-MAT-REGRESS` warn, ledger entry | R-ENV-012 |
| 9 | `in=prod` item with no `mat` | Scope default, else `H-MAT-MISSING` | R-ENV-005 |
| 10 | Item more mature in prod than in dev | `H-MAT-ORDER` | R-ENV-011 |
| 11 | Deprecated item with `to=` pointing at a removed ID | Error at the flag | R-LC-007 |
| 12 | Replacement cycle A to B to A | Error, cycle printed | R-LC-007 |
| 13 | Removal due, 3 references remain | Seal blocked; options: finish, exception with `until`, or `retarget`; list by ID with files | R-LC-006 |
| 14 | Target typo, version never reached | `H-LC-STALE-PLAN` after K seals; fix by `retarget` | R-LC-006 |
| 15 | Bump skips (1.9 to 2.0) | Due compares `>=`, so nothing is missed | R-LC-004 |
| 16 | Reusing a removed ID | Refused by `new` and `check` (`HN-D003`) | R-LC-021 |
| 17 | `git revert` of a removal: files return with no declaration | Orphan and coverage findings plus `H-GRAVE-MATCH`/`HN-D003` on fingerprint or ID | R-LC-021, R-LC-033 |
| 18 | Rejected feature re-added under a new ID and new paths | Caught only by match terms or fingerprint; rewritten code evades | R-LC-033, R-LC-039 |
| 19 | Graveyard false positive on a common word | Use multi-word, glob or regex terms; `revisits=` silences with a logged reason | R-LC-036 |
| 20 | Agent marks a proposal `rejected` without the owner | In-repo unverifiable; CODEOWNERS and `approve` | R-LC-038 |
| 21 | `#[deprecated]` and a `flag` disagree on target | Flag is authoritative; check reports the attribute mismatch | R-LC-010 |
| 22 | Two version axes (fairlead vs package semver) disagree on "major" | Targets are fairlead versions; optional mirror check | R-VER-004 |
| 23 | Someone wants a tombstoned ID back | New ID with a version suffix via an approved `revive` doc; the old ID stays reserved | R-LC-024 |
| 24 | Rename an ID | `flag replace` then `flag remove`; old references stay deprecated warnings until cleared | R-LC-011 |
| 25 | New reference to a deprecated ID before its due version | Warn (counted as new); error from the due version | R-LC-008 |
| 26 | `#[cfg(test)]` module in a prod file | Code recognised as env `test` when declared; otherwise `H-ENV-UNDECLARED` warn | R-ENV-036 |
| 27 | Light and dark theme variants | Axis values, not envs; a variant word in `in=` is `H-VOC-UNKNOWN` | R-ENV-037 |
| 28 | Legacy scope with default maturity at the lowest rank | docs-first starts at `observe` | R-ENV-006 |
| 29 | Signed-record verification enabled, unsigned approval | `H-AUTH-UNSIGNED` | R-LC-040 |
| 30 | ID deleted by hand outside the lifecycle flow | `sync` appends a `[[tomb]]` to complete; the ID is reserved | R-LC-020 |

## Resolved questions
- Q21-1 (resolved 2026-10-09): the scope default maturity feeds the docs-first ramp; no separate `ramp` key; see R-ENV-006
- Q21-2 (resolved 2026-10-09): revive only through an approved `revive` doc and a new suffixed ID, never the old ID; see R-LC-024
- Q21-3 (resolved 2026-10-09): fingerprints are computed once at seal from the removed files only; see R-LC-031
- Q21-4 (resolved 2026-10-09): `rule.maturity-gap` severity depends on the rank gap by default, and a root may set one level instead; see R-ENV-020
- Q21-5 (resolved 2026-10-09): CODEOWNERS now, signed-commit verification as an optional root setting; see R-LC-038, R-LC-040
- Q-LIFE-1 (resolved 2026-10-09): owner-approved revive doc recorded by `fairlead approve`, new suffixed ID; see R-LC-024
- Q-LIFE-2 (resolved 2026-10-09): rename, merge and split use `flag replace` plus deprecated-reference warnings until cleared; no `move --id` in 0.1 to 0.3, D-50: `replaced-by` plus a permanent tombstone alias; see R-LC-011
- Q-LIFE-3 (resolved 2026-10-09): both content fingerprint and match terms, plus tombstone ID and paths; see R-LC-020, R-LC-031, R-LC-033
- Q-LIFE-4 (resolved 2026-10-09): warn until due, error from the due version; removed IDs always error; see R-LC-008, R-ENV-024
- Q-LIFE-5 (resolved 2026-10-09): state is fixed and plan flags are separate; env and maturity are their own axes; see R-LC-001, R-ENV-004
- Q-LIFE-6 (resolved 2026-10-09): fairlead flags are authoritative and native deprecation attributes are cross-checked; see R-LC-010
- Q-LIFE-7 (resolved 2026-10-09): test code defaults to env `test` when detectable and declared, otherwise warn; the CI matrix selects real builds; see R-ENV-036
- Q-LIFE-8 (resolved 2026-10-09): theme variants are a separate variation axis, not envs; see R-ENV-037
- Q-LIFE-9 (resolved 2026-10-09): feature unification is an accepted limit; declared edges only; see R-ENV-030
- Q-LIFE-10 (resolved 2026-10-09): no migration tool for old ID schemes; see R-LC-012
