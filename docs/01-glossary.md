# Glossary

> Status: draft · Milestone: mixed · Decisions: D-02, D-03, D-10, D-11, D-12, D-13, D-14, D-16, D-17, D-18, D-19, D-20, D-21, D-22, D-23, D-24, D-25, D-27, D-28, D-29, D-30, D-31, D-32, D-33, D-34, D-35, D-36, D-37, D-38, D-39, D-40, D-41, D-42, D-43, D-44, D-45, D-46, D-47, D-48, D-49 · Related: decisions.md, 10-scopes-and-inheritance.md, 11-files-and-classes.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 14-context.md, 15-cli.md, 20-versions-and-seal.md, 21-environments-maturity-lifecycle.md, 22-editions-and-upgrade.md, 23-git-integration.md, 30-hierarchy-and-engines.md, 31-variation-protocol.md, 32-portability-and-restructure.md, 33-retention-and-compaction.md, 40-agent-integration-and-trust.md, 50-architecture.md, 70-open-questions.md

## Purpose

One definition per term, used identically by every chapter. Where the source analyses used several names for one thing, this glossary picks one and lists the aliases; Section 1 lists every such reconciliation. Milestone column: the release in which the concept first has behaviour (D-06, D-38). "Defined in" names the chapter that owns the normative rules; every entry has one, and the definition here MUST match that chapter (R-GLOS-001). Where a decision amends an earlier entry, the decision wins (decisions.md, D-31 to D-49 last). Exit codes and diagnostic codes are not defined here: exit codes only in [15-cli](15-cli.md) (R-CLI-003), diagnostic codes only in [13-checks-and-diagnostics](13-checks-and-diagnostics.md) (R-CHK-042).

## 1. Name reconciliations

| Concept | Name used in this spec | Aliases seen in drafts (do not use) | Reason |
|---|---|---|---|
| Override policy values | `open`, `needs-exception`, `sealed` | `exception` (for needs-exception) | D-02 names them. |
| Rule that a child may override with an `ovr` | `waivable` rule | "exception policy" | D-02: `waivable` is `needs-exception`. |
| Child request to an ancestor | **escalation** (a `[[req]]` table in the child control file, scaffolded by `fairlead new escalation`) | `req` as a source verb; "escalation proposal"; "escalation note" | D-18 allows six source verbs (12 R-SYN-041). |
| Ancestor grant on a sealed item | **allow grant** (an `[[allow]]` table in the granting ancestor's control file) | `allow` as a verb; "seventh verb"; "allow line" | D-34: `allow` is not a verb. |
| Override expiry key | `exp=YYYY-MM-DD` (0.1; dates stay allowed, checked against `--as-of`); `until=<version>` (default from 0.2) | `expires=`, `ttl=`, a version written into `exp=` | D-37 (amends D-14): one key per form (12 R-SYN-012). |
| Lifecycle target key on `flag` | `target=<MAJOR.MINOR.PATCH>` (`by-date=` opt-in, `since=` recorded by seal) | `until=` on a `flag` | D-49: `until` belongs to `ovr` only (12 R-SYN-048). |
| Override reason key | `why=` | `reason=`, `why`-less `ovr` | 12 R-SYN-011 and R-SYN-050: the key is `why` on `ovr`, `[[allow]]` and `[[req]]`. |
| Override owner key | `owner=`, naming a handle in the `owners` list | free-text owner; `approver=` | 12 R-SYN-011; `approver` is the party recorded by `fairlead approve`. |
| Declared deviation record | **exception** (general), **waiver** (an exception applied to a rule or check) | "variance", "debt entry" | Both live in one ledger. |
| Rolled-up list of deviations | **exception ledger** | "exceptions.led", "variance report" | Computed on read in 0.1; snapshotted at seal (0.2). |
| Hash-chained history file class | **append-only ledger** | "hist ledger", "history shard" | D-19. Not the exception ledger. |
| Per-scope per-version facts | **ledger shard** (0.2): the seal-written lines of one version, appended to `<scope>/fairlead/ledger/hist/<year>.hl` with a checkpoint line | `ledger/<version>.txt`, `ledger/<version>.json` | D-32; 20 Concepts; 33 R-RET-030. Distinct from the exception ledger. |
| Folder that holds fairlead files | **managerial folder** (`fairlead/` by default) | "scope folder", "fairlead dir" | |
| The file in it that holds policy | **control file** (`fairlead/control.toml`; the name is fixed) | `fairlead.ctl`, `fairlead.toml`, `control.yaml` | D-33; 12 R-SYN-038; D-05 forces TOML. |
| Header of the control file | **`[fairlead]` header** (keys `edition`, `root`, `scope`, `marker`) | "pin file", `fairlead-version` key, "header block" | D-33: frozen schema; no separate pin file. |
| Compatible binary range | `[settings] fairlead-range` | `[settings] requires` | 12 R-SYN-051: `requires` is the scope dependency field (D-22). |
| Document header block | **frontmatter**, TOML delimited by `+++`; doc-level directives in its `directives` array | YAML `---` fences | D-05; D-32; 12 R-SYN-036. |
| Global version | `MAJOR.MINOR.PATCH` (for example `1.0.2`); sealed = highest `versions/<version>.md`; open = derived from impacts | bare integer version, `versions/N.md`, "sealed + 1" | D-42 supersedes D-14's wording (D-45). |
| Edition | integer (`edition = 1`) | year editions such as `2026`, "edition year" | D-45: a year implies a cadence. |
| Draft change-doc identifier | **draft ID** `d-` plus 8 base32 characters, a doc word admitted by the ID grammar; `<scope>:d-<draft>` when qualified | `c-7f3a`, `chg-91cc`, `rq-7f3a`, per-kind aliases | One generator for all drafts (20 R-VER-049; 12 R-SYN-052). |
| Final doc number | **ordinal** | "doc number", "serial" | Assigned at seal, per scope and kind (20). |
| Cache directory | `.fairlead/` (gitignored) | "scan cache" | D-27/D-10. |
| Lifecycle states | `proposed`, `active`, `deprecated`, `removed`, `rejected` | user-defined states | Fixed states; labels may be aliased (21; D-16). |
| Version tool verbs | `seal` (absorbs compact), `context` (absorbs status) | `compact`, `status` | D-25. |
| Removed or relocated ID record | **tombstone**, a TOML `[[tomb]]` table in `fairlead/tombstones/<year>.toml` (not a directive, not a ledger line) | "alias tombstone", "moved-to line", `replaced-by` key, a terse `tomb` line | 21 R-LC-020; keys are `by=`, `why`, `paths`, `moved-to`. |
| Rejected-feature record | **graveyard entry** | "grave", "grv" (ID prefix only) | 21. |
| Reusable component, engine, pipeline, port, bridge | **unit** | "component" (UI only) | Item with a layer and a container (30). |
| Named ranked layer set | **ladder** | "layer set", "stack" | D-20. |
| Position in a ladder | **rank** (integer); a named position is a **layer** | "level" | `level` is reserved for the adoption ramp (L0-L3). |
| Stage of coverage adoption | **adoption ramp**, levels L0-L3 | "hier.level" | D-20. |
| Context size tiers | **tiers** L0 index, L1 scope card, L2 detail, L3 archive | "levels" | D-23. |
| Rollout state of a rule | **stage** (`observe`, `warn`, `enforce`) | "mode", "level" | Distinct from **severity** (`error`, `warn`, `info`), 13. |
| Generated item declaration | **`gen=` key on `def`** | a separate `gen` line; `gen` as a verb; a file-level `generated-from` marker | D-35. |
| Foreign region opener | **`def foreign.<name>`**, closed by `end` | `flag foreign`; a seventh verb | D-35; D-18 allows six verbs. |
| Once-only doc stamp | **created stamp** (the `created` key) | `sealed = "<version>"` as a stamp; "per-file version header" | D-13 forbids version headers. |
| Block written into agent instruction files | **managed block** (pointer block between `BEGIN`/`END` comments) | "rules copy", symlinked rules | 11 R-FILE-027; 15 R-CLI-016. |

## 2. Disambiguation: four meanings of "sealed"

The word is used in four independent senses. Always qualify it.

| Phrase | Meaning | Defined in |
|---|---|---|
| sealed **policy** (`"<key>" = "sealed"` under `[policies]`) | A key a child cannot override without an ancestor `allow`. | 10-scopes-and-inheritance.md |
| sealed **version** | The highest `versions/<version>.md`, created by `fairlead seal`. | 20-versions-and-seal.md |
| sealed **sub-root** | A mounted root with its own edition and no inheritance across the boundary. | 32-portability-and-restructure.md |
| sealed **document** | A change doc or version file whose body hash is frozen at seal (`sealed` and `body-hash` keys, written once). | 20-versions-and-seal.md |

## 3. Terms

### 3.1 Structure and identity

| Term | Definition | Milestone | Defined in |
|---|---|---|---|
| **Codebase root** | The managerial folder whose control file has `[fairlead] root = true`. Found by `--root`, then `FAIRLEAD_ROOT`, then an upward search from the working directory (or `-C <dir>`) that accepts only such folders; the nearest one wins. The search never invents a root, and no root is a usage error (D-31). Also "root of all roots". | 0.1 | 10, 15 |
| **Root marker** | `root = true` in the `[fairlead]` header of a root control file. A second marker below the root is the error `ROOT-DUP`. | 0.1 | 10, 12 |
| **Scope** | A directory with a managerial folder whose control file carries a stored scope ID. Identified by the ID, not its path. A file belongs to exactly one scope: the nearest ancestor-or-self scope (an `own` convention may decide otherwise from 0.2). | 0.1 | 10 |
| **Scope ID** | Stable ASCII slug (at most 32 characters) stored as the `scope` key in the `[fairlead]` header (D-33). Path-independent; moving a folder keeps it. Duplicates are an error (`DUP-SCOPE-ID`). | 0.1 | 10, 12 |
| **Managerial folder** | The visible, committed folder (`fairlead/` by default) holding a scope's control file, docs, generated files and reference files. Eligible for any directory; mandatory for none. | 0.1 | 10, 11 |
| **Implicit folder** | A folder with no managerial folder. It resolves to its nearest ancestor scope. Lines in its files are file-local: they apply to that file only. | 0.1 | 10 |
| **Eligibility** | A hint that a folder would benefit from a scope (thresholds in the root control file; manifest members). Never creates a scope by itself (`SCOPE-ELIGIBLE`, info). | 0.1 | 10 |
| **Promotion / demotion** | Folder to scope and scope to folder. Promotion (`fairlead new scope`, `--hoist`) is lossless: a scope is equivalent to its declarations kept in the ancestor with an `applies` glob. Demotion leaves a tombstone in the ancestor; the `move --demote` verb form is 0.2. | 0.1 (rules, promotion); 0.2 (`move --demote`) | 10, 32 |
| **Chain** | The ordered list of scopes from the root to the scope that owns a path. | 0.1 | 10 |
| **Cone** | The subtree plus the chain that a writing command may touch, selected by its invocation directory. | 0.1 | 10, 15 |
| **Effective view** | The result of folding declarations and overrides along a chain, each item with provenance. Computed at read time; never copied (D-10). | 0.1 | 10 |
| **Home scope** | The scope whose registry declares an item or unit. Overrides roll up to it and stop there. A `def` carrying `home=` must name an ancestor-or-self of every implementation site (`HOME-MISMATCH`). | 0.1 (items); 0.3 (units) | 10, 31 |
| **Convention** | A `[[convention]]` table (root or any scope) declaring what path rules cannot guess: `transparent` (a path segment is invisible to globs, for example Next.js route groups), `class` (for example `external-generated`) or, from 0.2, `own = "<scope id>"` (membership override). | 0.1 (`own` 0.2) | 10, 12 |
| **Delegate** | The root `[delegate]` table listing control-key patterns a child scope may set monotonically (`"<pattern>" = "tighten"`). Vocabulary keys can never be listed. Until 0.2 a child tightens through `ovr op=tighten`. | 0.2 | 10, 12 |
| **Mounted root / sealed sub-root** | A root placed inside a bigger codebase. It keeps its own edition, is opened only by explicit `adopt`, and is seen by the parent only through a rollup. An unregistered second root is `ROOT-DUP`. | 0.3 (edition handling 0.2, 22 R-EDN-040) | 32, 22 |
| **Adopt** | The explicit act (`move --adopt`) that opens a sealed sub-root and reconciles its declarations with the parent's effective view; needs an `approve` record. | 0.3 | 32 |
| **Attach / cut / copy** | `move --attach` places a cut or copied scope in a destination and diffs its `requires` against the new ancestors; `--cut` keeps the scope ID and logs `moved-from`; `--copy` mints a new ID with `derived-from`. | 0.3 | 32 |
| **Detach** | `move --detach SRC` (no destination) unlinks a scope from its ancestor chain without moving it: refuses with exit 4 unless the scope is `portable` or inherits nothing; snapshots the inherited view into `gen/scope.lock`; old ancestors' grants lapse. | 0.2 | 32, 15 |
| **DETACHED** | The info diagnostic and the state of a scope after detach (marked in its parent's membership shard) until it is attached or adopted. | 0.2 | 32 |
| **Lock** | A generated record of the resolved inputs a scope depends on (`gen/scope.lock`). Digest-only by default; a full snapshot only for `portable` scopes. Not the writer lock. | 0.2 (`reconciled`, `ancestor-digest`); 0.3 (requires digests) | 20, 32 |
| **Portable** | Scope flag requiring the lock to carry full snapshots of required items. | 0.3 | 32 |
| **requires / provides** | `provides` lists a scope's exports and is the visibility gate for qualified sideways references; `requires` is its dependencies (generated from `use` and `extends`, plus authored additions). `extends` and `requires` are separate fields. The `[scope]` table holds `portable`, `provides`, `requires`, never an `id`. | `provides`: qualifier 0.1, enforced 0.2 (`NOT-PROVIDED`); `requires` 0.3 | 10, 32 |
| **Tombstone** | A permanent record that an ID or scope existed and was removed, demoted, moved or replaced. Scope demotion writes one in the ancestor (10). A removed ID is reserved forever and is recorded as a TOML `[[tomb]]` table in `fairlead/tombstones/<year>.toml` (21). Never in default `context`. | 0.1 (scope demotion); 0.2 (items) | 10, 21, 33 |
| **Hoist** | Lifting repeated file-local directives into one scope-level declaration with an `applies` glob (`fairlead new scope --hoist`). | 0.1 | 10 |
| **Writer lock** | The advisory exclusive lock `.fairlead/lock` taken by writer verbs; a pid lock holding PID, host and start time. A dead PID is broken automatically; a live holder refuses a second writer with a refusal exit code. Per worktree; readers never take it. Network drives are unsupported. | 0.1 | 15, 11, 23 |
| **Allocation lock** | The single lock in the common git directory (`fairlead-alloc.lock`) that `seal`, `move` and `upgrade` take so two worktrees cannot allocate versions or move declarations at once. | 0.2 | 23 |

### 3.2 Files and lines

| Term | Definition | Milestone | Defined in |
|---|---|---|---|
| **File class** | One of: **authored** (humans and agents edit), **generated** (fairlead writes; deterministic, sorted, sharded, terse lines), **reference** (shipped fairlead docs for the pinned edition), **vendor-owned** (written by a third-party tool; fairlead indexes it and never rewrites it), **append-only ledger** (hash-chained history; rewritten only by an explicit purge). Class follows the writer. | 0.1 (authored, generated, reference, vendor-owned); 0.2 (ledger, 11 R-FILE-006); 0.3 (retention and checkpoints) | 11, 33 |
| **External-generated file** | A file written by another tool at build time (protobuf or OpenAPI output, `*.gen.ts`, Prisma client). Fairlead never writes it and its `// fairlead:` text is ignored; items inside it are declared at the generator input with `gen=`. A file never has two writers. | 0.1 | 11 |
| **Control file** | `fairlead/control.toml`: the authored TOML file of a managerial folder, holding vocabulary, policy and identity. It begins with the `[fairlead]` header. The root control file has full control; in 0.1 a child control file adds facts only (identity header, local tables, `[[ovr]]`, `[[allow]]` for descendants, `[[req]]`) and never sets an inherited key. It is not a directive carrier. The full table list is the schema index in 12. | 0.1 | 10, 12 |
| **Control-file header (`[fairlead]`)** | The first table of every control file, with keys `edition`, `root`, `scope` and `marker` (a non-root file carries only `scope`). Its schema is frozen across all editions and parsed in a lenient first pass. Other root keys sit in `[settings]`. There is no separate pin file (D-33). | 0.1 (keys); 0.2 (edition semantics) | 12 |
| **Vocabulary keys** | Root-only sealed keys: layer ranks, environment and maturity slots, ID namespace, policy kinds, kind definitions, file-class set. A child attempt is inert (`CTL-DELEG`) and a `[[req]]` is scaffolded. | 0.1 | 10 |
| **Line class** | What a line means: the verb it carries (`def`, `use`, `ovr`, `flag`, `env`, `end`) or a control-file key or table. Direction (up or down) is a fixed property of the verb. | 0.1 | 11, 12 |
| **Frontmatter** | The TOML block delimited by `+++` at the top of a doc. Holds doc-level metadata keys and the `directives` array (doc-level directives, D-32). | 0.1 | 12 |
| **Carrier** | The place a directive lives (D-32): a source comment (`//`, `#`, `/* */`, `--`, `<!-- -->`, including `#` comments in non-control TOML files), a Markdown body comment in the marker form, or the TOML frontmatter `directives` array. Control files are not carriers. Comment-free files (JSON) get an `[[attach]]` table from 0.2. | 0.1 | 12 |
| **`[[attach]]` table** | A control-file table (`path`, `directives`) that annotates files which cannot hold comments. Not the `move --attach` operation. | 0.2 | 12 |
| **Marker** | The first token after a comment leader that makes a comment a directive. Default `fairlead`; config key `marker` in the `[fairlead]` header (D-30, D-33). The diagnostic-code prefix `HN-` does not follow it. | 0.1 | 12 |
| **Foreign region** | A span of an authored file written by another tool. Declared with `def foreign.<name>` and closed by `end` (D-35); the opener takes the `+end` extent. Indexed, not name-policed. There is no dedicated verb. | 0.1 | 11, 12 |
| **Vendor-owned file** | A file produced by a third-party tool (shadcn or heroui output). fairlead never writes it; variants live in fairlead-owned wrapper units, never in the file. Its metadata lives in `vendored.lock` and the wrapper units. | 0.1 | 11 |
| **Generate-from-source kind** | A kind whose file fairlead writes from authored sources (`source = "generate"` in the control file), for formats nobody else edits. Not the default for stylesheets (D-04). | 0.1 | 11 |
| **Token contract** | The declared token IDs, roles, policy and docs that fairlead holds in `contract/`; values stay in authored CSS (D-04). A token `def` opts in to the literal-leak check with `applies=`. | 0.1 | 11 |
| **Derived index** | A generated, sorted registry shard derived from authored files, for example token names and values extracted from CSS (`gen/reg/tok.reg`). Terse lines (D-32). | 0.1 | 11 |
| **Shard** | A generated file holding one scope's slice of one registry kind (`gen/reg/<kind>.reg`), sorted bytewise by ID, terse lines. Split by slug bucket above 500 lines. Membership shards (`gen/members`) are separate. | 0.1 | 11 |
| **Generated header** | The first line of a generated file or baseline: `# scope <id> edition=<e> body=<hash>` (baseline: `# baseline edition=<e> body=<hash>`). It carries the edition and a body hash only; the binary version is never stamped (D-55), so a binary upgrade within an edition never makes a file stale. Reference files may keep a version (D-36). | 0.1 | 11, 13 |
| **Baseline** | A generated file (`gen/baseline.txt`, terse lines; header `# baseline edition=<e> body=<hash>`) recording accepted pre-existing findings by fingerprint, so a new rule can land in a legacy repo. Written only by `fairlead sync --baseline` (and by `init` through the same path); `check` is read-only (D-38). It only shrinks, except through `--accept-new --why`, which is counted. | 0.1 | 13, 15 |
| **Reference file** | A file shipped by fairlead for the pinned edition (syntax card, kinds, diagnostics). It carries only a machine-readable header (class, fairlead version, edition, content hash) and prose. It contains no directives and is never scanned for them. Rewritten only by `upgrade` (D-36). | 0.1 (class); refreshed by `upgrade` from 0.2 | 11 |
| **Managed block** | The fairlead-owned pointer block in `AGENTS.md`, `CLAUDE.md` and other `[agents] targets` files, between `BEGIN`/`END` comments. Identical in every target and scope; points at `fairlead context`; an edited or missing block is reported. | 0.1 | 11, 15, 40 |

### 3.3 Directives and IDs

| Term | Definition | Milestone | Defined in |
|---|---|---|---|
| **Directive** | A fairlead statement written in a carrier, never an attribute and never a doc comment. Its text is data, not an instruction to an agent (D-18). | 0.1 | 12 |
| **`def`** | In a doc: declares an item and its shape. In code or CSS: marks the region as an implementation site of an ID, or opens a foreign region (`def foreign.<name>`). Takes the `gen=` key for generated items (D-35). | 0.1 | 12 |
| **`use`** | The region consumes the listed IDs. Exists in 0.1 (D-06). Keys: `sym`; `dyn` and `pin` from 0.3. | 0.1 | 12 |
| **`ovr`** | A declared exception to a rule, value or key that applies to this region. Carries `why`, `owner` (a handle in `owners`) and `exp=` (date, 0.1) or `until=` (version, from 0.2). Loosening a sealed key needs an `allow`; loosening a `waivable` rule needs a complete `ovr` (D-34). | 0.1 | 10, 12 |
| **`flag`** | A lifecycle word on a region or ID (deprecate, replace, edit, remove, or a user alias; `awaiting` is reserved by the variation protocol). Keys `to=`, `target=<version>`, `by-date=`, `since=`; never `until=` (D-49). | syntax 0.1; behaviour 0.2 | 12, 21 |
| **`env`** | The region or item exists only in the listed environments. | syntax 0.1; behaviour 0.2 | 12, 21 |
| **`end`** | Closes the innermost open `+end` region. | 0.1 | 12 |
| **Must-understand (`!`)** | A `!` directly before the verb (`!def`): every key in the directive must be known to the running edition, otherwise it is an error. Keys beginning `x-` are user extension keys, always known. | 0.1 (grammar); 0.2 (edition behaviour) | 12, 22 |
| **Control-file tables and keys (not verbs)** | Written only in control files: `[fairlead]`, `[settings]`, `[kinds.K]`, `[file-kinds.K]`, `[policies]`, `[rules.R]`, `[[allow]]`, `[[req]]`, `[[ovr]]`, `[[convention]]`, `[context]`, `[scan]`, `[comments]`, `[agents]`, `[exceptions]`, `[authority]`, `[env]`, `[maturity]`, `[layers]`, `[bind]`, and the others of the schema index in 12. The keys `kind`, `scope`, `set` and `retain` are not source-line verbs. | 0.1 (later tables per index) | 12 |
| **Generated item (`gen=`)** | A key on `def` naming the generator that writes an item, for example `gen=build.rs`. The declaration sits at the generator input, never at the output. There is no separate `gen` line (D-35). | 0.1 | 11, 12 |
| **ID** | `[scope:]kind.name`: path-independent and ASCII; the name is one or more dotted slugs (at most 64 characters). Bare IDs are unique along a chain; siblings may reuse names; a sideways reference needs a scope qualifier and a `provides`. The draft doc word `d-<8 base32>` is admitted in doc-reference keys but is not a `kind.name` ID. | 0.1 | 10, 12 |
| **Kind** | A user-defined category (2 to 8 lowercase letters, for example `rule`, `tok`, `cmp`, `eng`) declared in the control file under `[kinds.K]`; the first segment of an ID. `exc` is a reserved built-in kind; `handoff` is a built-in doc kind. Readable labels live in `title` or `proto`, not in the prefix. | 0.1 | 12 |
| **Item** | Anything with an ID: rule, token, constant, component, doc, file-item, kind definition. Has a home scope and an override policy. | 0.1 | 10 |
| **Region / extent** | The lines a directive governs: file form (first comment block plus blank line), trailing, standalone (next code line and its block), explicit (`+N`), or `+end` closed by `end`. | 0.1 | 12 |
| **Alias (ID)** | A recorded old-to-new ID mapping, kept in the tombstone (`by`; `moved-to` is the container-move target) as long as the tombstone, which is permanent; no `alias` line in source. Label aliases for environment, maturity and lifecycle words are a separate concept (21). | 0.2 | 12, 21 |

### 3.4 Inheritance, policy and exceptions

| Term | Definition | Milestone | Defined in |
|---|---|---|---|
| **Override policy** | Per key, declared at the root under `[policies]`: `open` (nearest scope wins), `needs-exception` (a child `ovr` with `why`, `owner` and expiry is effective and recorded), `sealed` (inert until an ancestor `allow`). | 0.1 | 10, 12 |
| **Sealed by default** | Rules are `sealed` unless marked `waivable`; children may only tighten. | 0.1 | 10 |
| **Waivable** | Marks a rule as `needs-exception` (`waivable = true`). | 0.1 | 10 |
| **Inert** | A directive that parses but has no effect (for example an ungranted `ovr`). The ancestor value stays effective and a diagnostic is emitted. | 0.1 | 10 |
| **Shadowing** | A child redefining an inherited ID with `def` instead of `ovr`. An error (D-12). | 0.1 | 10 |
| **Replace / append** | Per-field override semantics: replace by default; a list is extended only with an explicit append marker (`k+=a,b`, 12). | 0.1 | 10, 12 |
| **Monotonic key** | A key where tightening is free (`op=tighten`) and loosening needs an exception (severity, stage, retention length, required slots). | 0.1 | 10 |
| **Allow grant (`[[allow]]`)** | A table in the granting ancestor's control file that lets named scopes (`for`) override a sealed item (`target`, optional `field`), with `why` and `exp` (`until` from 0.2). Not a verb. Only the home scope authors one, and it is not re-delegable (D-34). | 0.1 | 10, 12 |
| **Escalation** | A `[[req]]` table in a child control file that asks an ancestor to decide what the child cannot: loosen a key, add vocabulary, or grant an override. Scaffolded by `fairlead new escalation`; inert until the ancestor acts; may be `blocking`. Unresolved escalations never fail `check` in 0.1; from 0.2 one that is old enough errors at seal. | 0.1 | 10, 12, 15, 20 |
| **Floor rules** | Rules compiled into the binary. They cannot be waived, baselined, lowered or staged below enforce, and no policy, `allow` or `ovr` can override them (D-03). | 0.1 | 13, 40 |
| **Approval** | An `[[approval]]` record in `fairlead/approvals.toml`, written by `fairlead approve` when a human accepts a named fingerprint (an `ovr`, a change doc, a revive or upgrade doc). Terminal-gated: it refuses without a TTY and under CI, so an unattended agent cannot approve its own work. It is not authentication; the review gate is CODEOWNERS on the managerial folder (D-03, D-34). | 0.1 | 15, 40 |
| **Owner / `owners`** | `owners` is the list of handles responsible for a scope, mapped to CODEOWNERS. The `owner=` key of an `ovr` must name a handle in the `owners` of the declaring scope or an ancestor; free text is an error. `approve` records the verifying party. | 0.1 | 12, 40 |
| **Owner kind** | An `owners` entry may be an inline table with `kind = "human"` (default) or `"agent"`. An agent handle cannot approve, grant or accept a baseline on floor-adjacent keys (`APPROVER-AGENT`). | schema 0.1; checks 0.2 | 12, 13, 40 |
| **Severity and stage** | Severity is what a finding is (`error`, `warn`, `info`). Stage is how hard it bites: the **severity ramp** `observe`, then `warn`, then `enforce`, per rule per scope (D-28); raising a stage is free, lowering it is a relaxation. | 0.1 | 13 |
| **Relaxation** | A rule, stage or policy set below its default, or any `[[allow]]`. Listed in the `relaxations:` summary of every `check` run. | 0.1 | 13, 40 |
| **Finding** | One result of a check: stable code, location, message, intent excerpt, fix hint. | 0.1 | 13 |
| **Drift** | Divergence between declared intent and the codebase. Reported as findings. | 0.1 | 13 |
| **Literal leak** | A literal equal to a declared token value on a property listed in the token's `applies=` (`HN-T001`); exact after normalisation. The 0.1 proof of the wedge (D-43). Richer rules (near colours, `rem` against `px`, arbitrary values) are 0.3 (31). | 0.1 | 13, 31 |
| **Fingerprint** | A line-number-independent identity of a finding (blake3 over code, rule, scope, path, collapsed text and occurrence, 12 base32 characters), used by the baseline and by approvals. | 0.1 | 13 |
| **Exception** | A declared, owned, expiring deviation recorded in the ledger. Relative: it is the effective principle inside its scope and a ledger entry outside it. | 0.1 | 10 |
| **Waiver** | An exception applied to a waivable rule or a finding. | 0.1 | 13 |
| **Exception ledger** | The roll-up of exceptions visible to each ancestor up to the item's home scope. Computed on read in 0.1; snapshotted at seal (0.2). | 0.1 | 10, 20 |
| **Expiry (`exp=`, `until=`)** | When an exception stops being valid. In 0.1, `exp=YYYY-MM-DD` is evaluated against `--as-of`. From 0.2, `until=<version>` is the default; dates remain allowed (D-37). An expired exception is ineffective: the underlying finding returns and a floor check (`EXC-EXPIRED`) fails the run; its value is not kept. `never` is not a valid expiry. | 0.1 (date); 0.2 (version) | 10, 12, 20 |
| **Exception cap** | The bounded lifetime of an exception: `exceptions.max-days` (default 90) in 0.1, `exceptions.max-minors` (default 3) from 0.2. A child may lower it, never raise it. | 0.1 (date cap); 0.2 (version cap) | 20 |
| **Stale exception** | An exception whose target changed or vanished upstream. Reported as `EXC-STALE`; never auto-deleted. | 0.1 | 10 |
| **`x-pin`** | The optional `x-pin=<12 base32>` key of an exception, pinning the target's content hash so a parent change lists its dependents. `sync` does not write it; `explain` prints `pin=`. | 0.1 | 10, 12 |
| **Signed records** | The root setting `authority.signed-records = true` requiring a signed commit for `rejected`, `revisited` and `do-not-reintroduce` records and `[[approval]]` tables; `check --ci` runs `git verify-commit`. General signature verification of authority records is 0.3. | 0.2 (opt-in); 0.3 (general) | 21, 40 |

### 3.5 Versions, editions and lifecycle

| Term | Definition | Milestone | Defined in |
|---|---|---|---|
| **Seal** | Verb and noun. `fairlead seal` closes the open version all-or-nothing and writes `versions/<version>.md` last, the commit point. There is no partial seal. | 0.2 | 20 |
| **Sealed version** | The highest `versions/<MAJOR.MINOR.PATCH>.md` (D-42). Derived, never stored. | 0.2 | 20 |
| **Open version** | The sealed version bumped by the largest `impact` among pending change docs: `break` -> major, `add` -> minor, `fix` -> patch; one level down while MAJOR is 0 (D-42). Derived, never stored. | 0.2 | 20 |
| **Global version** | Version of the declared intent, `MAJOR.MINOR.PATCH` (D-42), independent of Cargo or npm versions; optional mirror check. | 0.2 | 20 |
| **Virtual file version** | What `fairlead explain <file>` reports, computed from the ledger and git. No per-file version headers (D-13). | 0.2 | 20 |
| **Created stamp** | The once-only, immutable `created` key written into a new doc at creation. It is not a version header and is never rewritten (D-13; 11 R-FILE-010). It is the only stamp (20 R-VER-013). `sealed` and `body-hash` are seal facts written once when a doc is sealed (20 R-VER-012), not stamps. | 0.1 | 11, 20 |
| **Change doc** | An authored doc (`kind = "change"`; also `revert`, `mechanical`, `bulk`, `early-removal`, `revive`, `upgrade`) that declares and covers a set of changes through a `covers` list. Status runs `draft`, `ready`, `sealed`, or `abandoned` (20). | 0.2 | 20 |
| **Draft ID / ordinal** | Draft ID: `d-` plus 8 random base32 characters, drawn at creation and never derived from content; admitted by the ID grammar as a doc word (`<scope>:d-<draft>` when qualified), no per-kind aliases. Ordinal: a per-scope, per-kind display number assigned at seal; references always use IDs (20). | 0.2 | 20, 12 |
| **Alias map** | The generated, derived file in a scope's `gen/` mapping each draft ID to its ordinal. Holds no counter. | 0.2 | 20 |
| **Docs-first coverage** | The check that each changed file is covered by a change doc. Proves docs present, not docs first (D-15). The unit is the file; IDs in `covers` count from 0.3. | 0.2 | 13, 20 |
| **Exempt globs / rolling chore doc** | Path patterns excluded from coverage; one rolling `chore` doc per seal for trivial work (20). | 0.2 | 20 |
| **Restructure doc** | The generated change doc that `move` writes; its `covers` lists the moved paths, so move-only changes are covered. | 0.2 | 13, 20 |
| **Hotfix** | An opt-in path (`docs-first.hotfix = true`; `fairlead new change --hotfix`) that opens a backfill debt: a draft change doc with `hotfix = true` that must be `ready` within `docs-first.hotfix-window` whole days (default 3, UTC, against `--as-of`). `context` lists open debts; `check` warns while open and errors after the deadline. There is no bypass variable. | 0.2 | 20 |
| **Revive doc** | A change doc `kind = "revive"` with `revives = "<tombstoned id>"`, valid only with an `[[approval]]`; it creates a NEW version-suffixed ID and leaves the old ID reserved. | 0.2 | 21, 15 |
| **Upgrade doc** | The generated change doc (`kind = "upgrade"`, `impact = "break"`) carrying the mechanical edits of a one-edition upgrade and a plan hash. `upgrade --yes` writes the draft; applying needs the approved doc with a matching hash. | 0.2 | 22 |
| **Edition** | An integer (`edition = 1`) in the `[fairlead]` header of the root control file: the byte-stable contract for grammar and generated output. New rules ship off within an edition. One per codebase root; a sealed sub-root may carry its own. An upgrade moves exactly one edition (D-17, D-33, D-45). | 0.1 (key); 0.2 (semantics) | 22, 12 |
| **Pin / `fairlead-version`** | The `[settings]` key naming the binary that last wrote the codebase. A binary older than the pin, or on another edition, is refused with the exact install line on every verb; lowering a pin is a relaxation needing an approved change doc. | 0.1 (check); 0.2 (relaxation) | 15, 22 |
| **`fairlead-range`** | The `[settings]` key giving the compatible binary range, honoured by CI. Renamed from `requires` so it cannot collide with the scope `requires` (12 R-SYN-051). | 0.1 | 12, 22 |
| **Environment** | Where an item exists or runs: a user word for a position on an ordered promotion path declared at the root, ordered by exposure toward production. | 0.2 | 21 |
| **Maturity** | How settled an item is, per environment: user words map to integer ranks; only rank is compared. | 0.2 | 21 |
| **Lifecycle** | Planned and actual state: `proposed`, `active`, `deprecated`, `removed`, plus `rejected`. States are fixed; their labels may be aliased (21). Plans are `flag` directives; a flag never changes state until seal applies it. | 0.2 | 21 |
| **Binding** | The declared name of the real gate for an environment, in the `[bind]` table: a Cargo feature, a `cfg` expression or a TS flag token. Checked for existence, never evaluated (21). | 0.2 | 21 |
| **Graveyard** | Permanent entries for rejected proposals, each with match terms and a content fingerprint, to catch re-introduction under a new ID (21, 33). Lexical and fingerprint matching only. | 0.2 | 21, 33 |
| **Ledger shard** | The seal-written facts of one scope for one version: terse-line records appended to `<scope>/fairlead/ledger/hist/<year>.hl`, with a checkpoint line per touched version; written only for scopes that the seal touched (20). | 0.2 | 20 |
| **Mirror** | Optional check that the global version matches a named Cargo or npm version. Warns, never fails (20). | 0.2 | 20 |

### 3.6 Hierarchy and variation

| Term | Definition | Milestone | Defined in |
|---|---|---|---|
| **Unit** | An item with a layer and a container: component, engine, pipeline, port, bridge, util. Declared with `def` (D-40). | 0.3 | 30 |
| **Container** | A logical, declared grouping of units; nests; may span folders. Not a scope. Declared with `def` and `parent=` (D-40). | 0.3 | 30 |
| **Containment** | Tree relation: each unit has exactly one container, named by `container=` on its `def`. Separate from `uses`. A unit binds by a `path=` glob or a code `def`, never both. | 0.3 | 30 |
| **`uses`** | DAG relation: a unit depends on another. Written as `// fairlead: use <id>` in code and aggregated upward. Use evidence is best-effort and every report says it is a lower bound (D-40). | 0.3 | 30 |
| **Ladder** | A user-defined named ordered set of layers (for example `ui`, `core`), declared in the root `[layers.<ladder>]` table. Presets scaffolded by `fairlead new ladder`. Declared only in the root control file. | 0.3 | 30 |
| **Layer / rank** | A named position in a ladder with an integer rank. Ranks compare only within one ladder. Free layers (`chrome`, `platform`) are unranked; a ranked layer such as the backend `shell` may be entry-only through `[layer-policy]`. | 0.3 | 30 |
| **Flow** | An explicit sanctioned cross-ladder edge (`[[flow]]`). | 0.3 | 30 |
| **Port** | A lower-layer unit used to invert an upward runtime need; an `implements` bridge connects it (D-20). | 0.3 | 30 |
| **Bridge** | A unit with two ends in different languages (TS and Rust), declaring parameters, not structs. Checks: presence on both sides, wire literal, version pin (D-29); a consumer `use ... pin=<N>` is compared with the bridge `ver`. | 0.3 | 30, 32 |
| **`spell=`** | A key on a bridge `def` naming the wire spelling convention of that bridge (for example snake_case function names, camelCase keys); declared, never inferred. | 0.3 | 12, 30 |
| **Exposure** | A bridge key `exposure=<word>` stating intent (for example `all-windows`). Fairlead does not parse capability files; reports say `exposure unverified`. | 0.3 | 30 |
| **External input** | An artifact a build configuration consumes from outside the scope (Tauri `frontendDist`), declared `def ext.<name> role=input`. Vendor-owned for scanning. | 0.3 | 30, 31 |
| **Entry** | A root of reachability, with an `etype`: `user-action`, `cli`, `export`, `test`, `registered`. Declared with `def` (D-40). | 0.3 | 30 |
| **Orphan / stale / dormant / forgotten** | Orphan: no container. Stale: unreachable from any entry. Dormant: unchanged for N seals, read from seal-ledger digests, never from git. Forgotten: stale and dormant. Verdicts are lower bounds until an import extractor exists (D-40). | 0.3 | 30 |
| **`sym` / evidence** | A distinctive identifier declared on a unit (at least 6 characters, unique); fairlead greps for it, as a whole identifier, in the consumer as evidence for a `uses` edge. | 0.3 | 30 |
| **Adoption ramp** | Coverage levels L0 (containers) to L3 (entries and traces), set by `[hier] level`. A ramp, not a totality target. | 0.3 | 30 |
| **Visibility** | `subtree` (default) or `public`, giving a home scope teeth. Independent of rank: an edge must pass both. | 0.3 | 31, 30 |
| **Axis** | A declared dimension of a unit with a closed value set (`density = sm, md, lg`). Using a declared value needs no protocol. | 0.3 | 31 |
| **Theme-variant axis** | Theme variants (light, dark, brand) are values of a variation axis, never environments; a theme word used as an environment is `H-VOC-UNKNOWN`. | 0.3 (rule 0.2 in 21) | 31, 21 |
| **Variant** | A unit declared in the home scope as `variant-of` another, or a new value on an axis; `native=` aliases a native spelling. | 0.3 | 31 |
| **Variation request** | A consumer-scope record asking the home scope for a variation. Four outcomes: use an existing axis, request a variant, wrap locally (expiring interim wrapper), reject to a permanent adapter. | 0.3 | 31 |
| **Interim wrapper** | A consumer-side composition flagged `flag awaiting to=<request> target=<version>`; `awaiting` is a reserved flag word. Rolled into the exception ledger at seal. | 0.3 | 31 |
| **Wrapper / adapter** | A fairlead-owned unit that composes or adapts a primitive, including vendor-owned ones. A permanent adapter owns the unprefixed name and declares `import-as=`. | 0.3 | 31 |
| **Fork** | A copy-and-edit of a unit: declared (`ovr rule.var-fork`, an exception) or undeclared (a suspect, detection ships off). | 0.3 | 31 |

### 3.7 Context and retention

| Term | Definition | Milestone | Defined in |
|---|---|---|---|
| **Context** | The budgeted brief returned by `fairlead context`. Format: terse `ID k=v` lines (D-23, D-32). Path and ID queries only until 0.3; free text exits 2 with "no match". | 0.1 | 14 |
| **Tiers** | L0 index (one line per scope), L1 scope card (hard cap 400 tokens), L2 detail (`explain`), L3 archive (tombstones, graveyard, compacted history). | 0.1 (L0-L2); 0.2 (L3; rollups 0.3) | 14 |
| **Budget** | Estimated tokens (`ceil(bytes / bytes-per-token)`, default 4, configurable in `[context]`) for `--budget N`; priority packing; defaults 500 (no argument) and 700 (path); truncation always prints `+N more` and mandatory content that does not fit exits 2. `--exact-tokens` is opt-in. | 0.1 | 14 |
| **Digest** | The first 40 bits of blake3 over canonical bytes, 8 base32 characters; Merkle chain over the scope chain. | 0.1 | 14 |
| **Delta (`--since`)** | Output limited to what changed since a version or git revision (primary) or a digest (accelerator). Recomputed on the current tree. | 0.2 | 14 |
| **Task match** | BM25 matching of free text against IDs, titles and tags. | 0.3 | 14 |
| **Retention / TTL** | Rules for how long records stay detailed. Version-based, with a calendar floor (`min-age`, 90 days by default). Hot = open plus last 3 versions (age 0 to 3); warm = age 4 to 8; cold above 8 (D-39, D-24). A child may lengthen but not shorten. | 0.3 | 33 |
| **Compaction** | Replacing due records by authored summaries (`sum:` at creation, `out:` at close, D-39), inside `seal`, using the seal's recorded timestamp and never the wall clock (D-24). | 0.3 | 33 |
| **Decision-grade record** | A record kept forever as at least one line: decisions, graveyard entries, tombstones (D-24). | 0.3 | 33 |
| **Ledger checkpoint** | A periodic checkpoint line in the hash-chained append-only ledger, so that purging old detail never forces re-chaining of finalised years (D-39). Continuity is checkable from the first ledger that writes one. | 0.2 (continuity check); 0.3 (compaction chain) | 33, 23 |
| **Handoff** | An authored end-of-session note scaffolded by `fairlead new handoff` (built-in doc kind `handoff`, mandatory `sum`); the session-start brief prints the newest one as a quoted `handoff:` line. | 0.1 | 14 |

### 3.8 Operation

| Term | Definition | Milestone | Defined in |
|---|---|---|---|
| **Verb set** | `init`, `check`, `sync`, `new`, `move`, `context`, `explain`, `seal`, `upgrade`, `approve` (D-25). `move` is 0.2 (D-38). `approve` stays its own verb. | per verb | 15 |
| **Exit codes** | Defined only in 15-cli.md (R-CLI-003, D-41, D-44); this glossary and every other chapter cite them and define none. | 0.1 | 15 |
| **Plan** | The list of files a writer verb would create or change, with a diff. `--dry-run` prints it and exits 0; `init`, `upgrade`, `seal` and `move --adopt` print it and refuse without `--yes`; `sync` and `new` apply directly. | 0.1 | 15 |
| **Obligations diff** | What `move` prints before applying: which rules start or stop applying, and the code sites (`code-site <path>:<line>`) it reports but does not rewrite. | 0.2 | 15 |
| **Cache** | `.fairlead/`, gitignored, disposable, never changes output. JSON in 0.1 until profiling justifies another format; never committed. Its key covers fairlead version, grammar, config hash and `--as-of`. | 0.1 | 50 |
| **Walk** | Parallel filesystem traversal respecting ignore files, without requiring a git repository. | 0.1 | 50 |
| **`.fairleadignore`** | Fairlead-specific ignore file, added to the walk. | 0.1 | 50 |
| **As-of** | The explicit date used for time-dependent checks (`--as-of`, D-37). Defaults to UTC today and is printed in the output header. | 0.1 | 13, 50 |
| **Write-set** | The files a command may write. Read commands have an empty write-set apart from the cache. | 0.1 | 50 |
| **Commit point** | In a multi-file write, the single last file renamed into place; its presence defines success (for seal, `versions/<version>.md`). | 0.2 | 20, 23, 50 |
| **Dogfooding** | Fairlead's own repository declares and checks itself with fairlead. | 0.1 | 50 |
| **docs-check** | `cargo xtask docs-check`: the repository task that checks `docs/` for dangling IDs and rule references until the docs carry `def` directives (D-57); not a fairlead verb. | 0.1 | 50 |

## Rules

| ID | Rule | Milestone |
|---|---|---|
| R-GLOS-001 | A term defined here MUST be used in all other chapters with this meaning. A chapter needing a new term MUST add it here in the same change. | [0.1] |
| R-GLOS-002 | The word "sealed" MUST be qualified (policy, version, sub-root, document) in the first use within a section. | [0.1] |
| R-GLOS-003 | Aliases listed in Section 1 MUST NOT appear in normative text, except when quoting a source. | [0.1] |
| R-GLOS-004 | `owner=` MUST be glossed as an identity: a handle in the `owners` list of the declaring scope or an ancestor, claimed at authoring and confirmed by the `approve` record (12 R-SYN-011, 40 R-AGT-028). | [0.1] |
| R-GLOS-005 | Ladders, layers and flows are vocabulary declared only in the root control file; units, containers, entries and bridges are `def` directives in code comments or Markdown registry docs (30 R-HIER-001, R-HIER-013). | [0.3] |

## Resolved questions

- Q-GLOS-1 (resolved 2026-10-09): the control file is `fairlead/control.toml`; see 12 R-SYN-038 and D-33
- Q-GLOS-2 (resolved 2026-10-09): the edition is an integer, `edition = 1`; see D-45 and Section 1
- Q-GLOS-3 (resolved 2026-10-09): `owner=` is a claim at authoring restricted to a handle in `owners`, and `approve` records the verifying party; see R-GLOS-004
- Q-GLOS-4 (resolved 2026-10-09): the leaning (docs allowed for ladders) is superseded by D-02 and 30 R-HIER-001: ladder vocabulary is root control-file only, while units and containers are `def` directives in docs (D-40); the allow-placement part is D-34; see R-GLOS-005
- Q-GLOS-5 (resolved 2026-10-09): foreign regions are `def foreign.<name>` closed by `end`; see D-35 and Section 3.2
