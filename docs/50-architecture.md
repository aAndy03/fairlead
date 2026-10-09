# Architecture

> Status: draft · Milestone: mixed · Decisions: D-05, D-10, D-11, D-14, D-19, D-23, D-26, D-27, D-28, D-31, D-32, D-33, D-34, D-37, D-38, D-40, D-41, D-43, D-44, D-46, D-48, D-49 · Related: 01-glossary.md, 10-scopes-and-inheritance.md, 11-files-and-classes.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 14-context.md, 15-cli.md, 20-versions-and-seal.md, 23-git-integration.md, 60-roadmap.md, 70-open-questions.md

## 1. Purpose

This chapter describes fairlead's internal structure: how the workspace is split, how a command turns a directory tree into findings and context, what the in-memory registry contains, how incremental work and parallelism are organised without sacrificing determinism, how errors are modelled, and how the tool is tested and dogfooded. It contains no product code. User-visible behaviour is specified in the chapters it links to; this chapter constrains how that behaviour may be implemented. It cites exit codes ([15-cli.md](15-cli.md), R-CLI-003) and diagnostic codes ([13-checks-and-diagnostics.md](13-checks-and-diagnostics.md), R-CHK-042) and defines neither.

## 2. Concepts

| Term | Meaning here |
|---|---|
| **Pipeline** | The fixed sequence walk, extract, resolve, check, render. Every command runs a prefix of it. |
| **Extraction** | Per-file, context-free conversion of bytes into directive records. Cacheable by content hash. |
| **Resolution** | Global, single-threaded construction of scopes, the item registry and effective views from extraction results. |
| **Registry** | The in-memory model produced by resolution (Section 5). |
| **Write-set** | The set of files a command is allowed to create or replace. |
| **Cold / warm run** | No usable cache / a valid cache and an unchanged tree. |
| **Module** | A unit of `fairlead-core` with one responsibility and a narrow interface (Section 3). |

## 3. Workspace layout (proposal)

Three crates in one Cargo workspace. The split follows what must be testable and fuzzable without a filesystem.

| Crate | Kind | Contents | Depends on |
|---|---|---|---|
| `fairlead-syntax` | lib | Comment-delimiter table, lexer, directive grammar, frontmatter splitter, ID grammar, diagnostics for syntax errors. Pure functions over bytes and strings: no filesystem, no git, no threads. | `winnow`, `toml` |
| `fairlead-core` | lib | The modules listed below. | `fairlead-syntax`, `ignore`, `globset`, `blake3`, `rayon`, `git2`, `toml`, `toml_edit`, `serde`, `serde_json`, `jiff`, `similar`, `camino`, `dunce`, `tempfile`, `semver` |
| `fairlead` | bin | `clap` derive definitions for the fixed verb set, mapping of core errors to the exit codes defined in [15-cli.md](15-cli.md), terminal output. Contains no logic that is not a thin call into `fairlead-core`. | `fairlead-core`, `clap` |

Modules of `fairlead-core`:

| Module | Responsibility |
|---|---|
| `walk` | Parallel, sorted file listing with ignore rules (Section 4.2). |
| `extract` | Per-file extraction using `fairlead-syntax`; classifies the file (Section 4.3). |
| `manifest` | Read-only parse of Cargo TOML manifests (R-ARCH-006). |
| `git` | The only module that touches git: `git2` reads and the `git` executable calls (R-ARCH-005). |
| `registry` | Scopes, items, overrides, grants, effective views, provenance (Sections 4.4, 5). |
| `check` | Built-in checks and declared rules, baseline and stage application, fingerprints, floor rules. |
| `context` | Tiers, priority packing, token estimates, digests. |
| `render` | `human`, `terse`, `json`, `sarif` and `context` output; sanitising of repository text. |
| `cache` | The disposable `.fairlead/` cache (Section 6). |
| `write` | Write-set transactions, the writer lock, journal and quarantine (Section 13). |
| `net` | `upgrade --fetch` only, 0.2, isolated (R-ARCH-124). |

Rationale: `cargo install fairlead` installs the binary under the crate name users expect; the library lets tests, benchmarks and a later MCP server reuse the engine; `fairlead-syntax` is the smallest unit worth fuzzing and is the grammar stability boundary (D-17).

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-001 | The CLI crate MUST contain no check logic, path logic or file-format logic. | [0.1] |
| R-ARCH-002 | `fairlead-syntax` MUST NOT perform I/O and MUST NOT depend on `fairlead-core`. | [0.1] |
| R-ARCH-003 | The public API of `fairlead-core` is NOT a stability promise before 1.0. The stability surface is the CLI, the directive grammar per edition, and the output formats (see [60-roadmap.md](60-roadmap.md), R-REL-004). | [0.1] |
| R-ARCH-004 | A future MCP server (tools and resources only) MUST be a separate crate or binary that depends on `fairlead-core`, so the 0.1 binary carries no network stack. It is adopted after 0.1 (in the 0.3 line), with `rmcp` pinned to an exact minor and re-tested on every release. | [0.3] |
| R-ARCH-005 | All git access MUST sit in one module so it can be replaced ([23-git-integration.md](23-git-integration.md) R-GIT-070, R-GIT-076, D-48). Reading repository state (status, diff, history, rename detection, sparse and shallow detection) uses `git2`. The user's `git` executable is run only for `seal --commit`, `seal --tag` and the signature check of `check --ci` (0.2), as explicit foreground subprocesses. No other process is ever spawned: no shell, no `cargo`, no `rustc`. | [0.1] |
| R-ARCH-006 | Cargo TOML manifests are the only manifests parsed in 0.1, read-only, with the `toml` crate: workspace members for eligibility hints and default excludes. `package.json`, `pnpm-workspace.yaml`, capability JSON and `build.rs` are not parsed (Q-TOOL-2); from 0.2 `[features]` are read for binding existence checks ([21-environments-maturity-lifecycle.md](21-environments-maturity-lifecycle.md) R-ENV-032), from 0.3 workspace dependency edges ([30-hierarchy-and-engines.md](30-hierarchy-and-engines.md) R-HIER-027). A manifest that cannot be parsed yields no hints and never fails or blocks a run. | [0.1] |
| R-ARCH-007 | The three-crate layout is the 0.1 layout (Q-ARCH-5). `fairlead-syntax` may be merged into `fairlead-core` only before the first publish, with the reason recorded in decisions.md, and the fuzz boundary of R-ARCH-150 MUST survive as a module boundary. | [0.1] |

## 4. Pipeline

```text
walk  ->  extract  ->  resolve  ->  check  ->  render
(files)   (records)    (registry)   (findings)  (bytes)
parallel  parallel     single       parallel    single
          by file      thread       by scope    thread
```

| Stage | Input | Output | Cached by |
|---|---|---|---|
| **Discover** | cwd, flags | root directory, edition, pin check, config hash | none (cheap) |
| **Walk** | root, ignore rules | sorted list of `(repo-relative path, size, mtime)` | none |
| **Extract** | one file | `FileRecord`: class, content hash, directive records, syntax diagnostics | content hash plus extractor version |
| **Resolve** | all `FileRecord`s plus control files | `Registry`: scopes, items, edges, overrides, effective views | hash of the chain's authored inputs |
| **Check** | `Registry`, rule set, baseline | findings (unsorted) | none |
| **Render** | findings, registry slices | text, JSON, SARIF, `context` lines, or file writes | none |

Per-verb prefixes:

| Verb | Stages run | Writes |
|---|---|---|
| `check`, `explain` | discover to render | none (cache only) |
| `check --changed` | discover to render; extraction limited to the changed set; global checks still use the whole registry | none (cache only) |
| `context` | discover to resolve, then pack | none (cache only) |
| `sync` | discover to check, then regenerate | generated files; the baseline only with `--baseline` (D-38) |
| `new`, `init`, `approve` | discover, targeted resolve | the scaffolded or approved files |
| `move`, `seal`, `upgrade` (0.2) | discover to check, then staged apply | declared write-sets (Section 13) |

### 4.1 Discover

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-010 | The codebase root MUST be found by `--root`, else `FAIRLEAD_ROOT`, else an upward search from the working directory that accepts only a managerial folder whose control file has `[fairlead] root = true`. The nearest such folder wins, and the search MUST NOT invent a root (D-31). A managerial folder without the root marker MUST NOT be taken as the root. | [0.1] |
| R-ARCH-011 | If no root control file can be located, or an unparseable control file is found on the path, every verb except `init` MUST fail closed with the usage or configuration error code defined in [15-cli.md](15-cli.md), never a clean pass. | [0.1] |
| R-ARCH-012 | A repository with zero declared rules MUST print the notice of [13-checks-and-diagnostics.md](13-checks-and-diagnostics.md) R-CHK-003 ("0 rules declared, nothing verified") and MUST NOT print a bare success. | [0.1] |
| R-ARCH-013 | Discover MUST perform the pin check of [15-cli.md](15-cli.md) R-CLI-005 before any other stage: a binary older than the `fairlead-version` pin, or on another edition, is refused with the exact install line and no partial work. | [0.1] |

### 4.2 Walk

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-020 | The walk MUST use the `ignore` crate's parallel builder with `require_git(false)` and `git_global(false)`, and MUST add `.fairleadignore` as a custom ignore file name. Output MUST NOT depend on the user's global git excludes. | [0.1] |
| R-ARCH-021 | Default excludes for generated and vendored trees (`target/`, `node_modules/`, `dist/`, build output) MUST live in the control file, not in code, and MUST be overridable. | [0.1] |
| R-ARCH-022 | Symbolic links and Windows junctions MUST NOT be followed by default (inside a managerial folder they are an error, [10-scopes-and-inheritance.md](10-scopes-and-inheritance.md) R-SCOPE-006). A git submodule boundary (gitlink) and a linked worktree (a directory whose `.git` is a file) MUST stop the walk. | [0.1] |
| R-ARCH-023 | Files larger than a configurable limit (default 2 MB) and files containing a NUL byte in the first 8 KB MUST be skipped with an info diagnostic, never read fully. A file that is not valid UTF-8 MUST be skipped with a warning, never decoded by guess. | [0.1] |
| R-ARCH-024 | The walk result MUST be sorted bytewise by repo-relative path before any later stage consumes it. | [0.1] |
| R-ARCH-025 | A root that is not inside a git repository MUST work for `check`, `context` and `explain`; git-dependent features MUST report "unavailable" explicitly. | [0.1] |
| R-ARCH-026 | In a sparse checkout or partial clone a scope whose managerial folder or files are absent MUST be reported unknown, never passing, and `check` MUST exit 5, cannot determine, for a verdict that depends on it (D-56; an unresolved scope with an unparseable control file stays exit 2) ([23-git-integration.md](23-git-integration.md) R-GIT-007, [13-checks-and-diagnostics.md](13-checks-and-diagnostics.md) R-CHK-057). | [0.1] |

### 4.3 Extract

Extraction is deliberately AST-free in 0.1 (D-27).

1. A cheap byte prefilter looks for the configured marker. Files without it skip parsing entirely. (The prefilter is a byte-search primitive; `memchr`, a transitive dependency of `regex`, is the likely choice and was not assessed separately.)
2. A hand-written lexer, driven by a per-extension comment-delimiter table, identifies comment spans while respecting string and raw-string literals. A directive counts only when the marker is the first token after the comment leader (R-SYN rules in [12-directive-syntax.md](12-directive-syntax.md)). Markdown bodies are scanned only for marker-form HTML comments outside fenced code; TOML frontmatter carries doc-level keys (D-32). Reference, vendor-owned and external-generated files are never scanned for directives.
3. The directive body is parsed with `winnow` (grammar) and `toml` (frontmatter).
4. Attachment (file form, trailing, standalone, explicit) is computed by line heuristics, capped at 500 lines, confirmed by golden fixtures per host language (R-SYN-046).

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-030 | Extraction MUST be a pure function of file bytes, file extension, and configuration. It MUST NOT read other files. | [0.1] |
| R-ARCH-031 | Every marker candidate MUST yield either a directive record or a syntax diagnostic. A malformed directive MUST NOT be silently dropped. | [0.1] |
| R-ARCH-032 | Extraction MUST NOT panic on any byte input, including invalid UTF-8; non-UTF-8 files are reported and skipped. | [0.1] |
| R-ARCH-033 | Line endings MUST be normalised (CRLF and lone CR to LF) before parsing and hashing. Byte offsets in diagnostics refer to the original bytes. | [0.1] |
| R-ARCH-034 | A tree-sitter or `syn` import extractor, when added, MUST feed the same `uses` edge structure and MUST NOT change the directive grammar. | [0.3] |

### 4.4 Resolve

Resolution is single-threaded over compact tables on purpose: it is the part where ordering bugs would break determinism, and it is cheap relative to I/O.

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-040 | Resolution MUST fold declarations and overrides along each chain from root to scope in a fixed order: control keys (including `[[allow]]` grants), declarations, overrides; within each, bytewise by ID then by source path. | [0.1] |
| R-ARCH-041 | An invalid line (ungranted override, delegation violation) MUST be inert and MUST emit a diagnostic. It MUST NOT abort resolution of other scopes. | [0.1] |
| R-ARCH-042 | The effective view MUST record provenance (home scope, applied overrides) for every item, because `explain` and the exception ledger are built from it. | [0.1] |
| R-ARCH-043 | Resolution MUST be independent of the invocation directory; the directory only selects the cone to report on or write into. | [0.1] |

### 4.5 Check and render

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-050 | Checks MUST be pure functions of the registry, the rule set and flags such as `--as-of`. They MAY run in parallel per scope. | [0.1] |
| R-ARCH-051 | Findings MUST be totally ordered before rendering by the keys of [13-checks-and-diagnostics.md](13-checks-and-diagnostics.md) R-CHK-011 (path, line, column, code), with the message as the final tie-break. Display grouping and truncation (floor first, 20 findings) follow R-CHK-038 and are a render concern. | [0.1] |
| R-ARCH-052 | Every renderer MUST emit LF line endings, forward-slash repo-relative paths and no locale-dependent formatting. | [0.1] |
| R-ARCH-053 | Findings MUST carry the line-number-independent fingerprint of [13-checks-and-diagnostics.md](13-checks-and-diagnostics.md) R-CHK-031 (blake3 over code, rule, scope, path, collapsed text and occurrence index), so that baselines survive unrelated edits. This chapter does not define it. | [0.1] |
| R-ARCH-054 | Text (`human`, `terse`), JSON and SARIF 2.1.0 output MUST be available ([13-checks-and-diagnostics.md](13-checks-and-diagnostics.md) R-CHK-036). SARIF is written with hand-built serde structs for the subset used and MUST be validated against the official schema in CI (Q-TOOL-7). | [0.1] |
| R-ARCH-055 | Diagnostic codes are opaque, stable identifiers allocated only in [13-checks-and-diagnostics.md](13-checks-and-diagnostics.md) (R-CHK-042). `fairlead-core` holds one registry entry per code (default severity and stage, milestone); a code tagged for a later milestone is inactive in earlier builds. | [0.1] |
| R-ARCH-056 | Token counts MUST be `ceil(bytes / bytes-per-token)` with the ratio from `[context]` (default 4), labelled an estimate in every output; per-object estimates are cached so packing never re-tokenises ([14-context.md](14-context.md) R-CTX-043). An exact tokenizer is opt-in (`--exact-tokens`) and MUST NOT bring a network stack (R-ARCH-123). | [0.1] |

## 5. Data model of the registry

No code; entities and their relations.

| Entity | Key | Main fields | Notes |
|---|---|---|---|
| **Repo** | root path | edition, pin, config hash, marker, excludes | One per run. |
| **Scope** | scope ID | directory (repo-relative), parent scope, control keys, `owners` (handle, kind) | Identity is the ID; the path is an attribute. |
| **FileRecord** | repo-relative path (case-folded key) | class (authored, generated, reference, vendor-owned, ledger, external-generated), size, content hash, scope ID, directive records, syntax diagnostics | Class from [11-files-and-classes.md](11-files-and-classes.md). |
| **Directive** | (file, ordinal) | verb, IDs, key=value args, carrier, span, extent | Stored with original byte offsets. |
| **Item** | `[scope:]kind.name` | kind, home scope, policy, fields, lifecycle (0.2), env (0.2) | Declared by `def` (D-40): in a doc, in a control file, in a code comment next to the symbol, or in a marked HTML-comment block of a Markdown registry doc. |
| **Override** | (file or scope, target) | target item and field, value, reason (`why`), owner, exp, base, state (`effective`, `inert`, `expired`, `stale`) | State is computed during resolution. An expired override is ineffective. |
| **Grant** | (granting scope, target) | target item, optional field, grantee scopes, `why`, exp | A `[[allow]]` table in the granting ancestor's control file (D-34). |
| **Approval** | fingerprint | target ID, scope, approver, date, renewal count | An `[[approval]]` table in `fairlead/approvals.toml` (D-34). |
| **Edge** | (source site, target ID) | kind (`use`), evidence flag | `use` sites only in 0.1. |
| **RuleDef** | rule ID | severity and stage per scope, policy, glob, intent text, fix hint | Floor rules are built in; others come from `[rules.R]` tables in a control file ([12-directive-syntax.md](12-directive-syntax.md)). |
| **Finding** | fingerprint | code, severity, stage, location, message, excerpt, fix hint, state (`new`, `baselined`, `waived`, `waiver-ineffective`) | Output of checks. |
| **BaselineEntry** | fingerprint | code, path, `acc` marker for accepted additions | Generated file; sorted. |
| **LedgerView** | (scope, item) | overrides visible to that scope up to the item's home | Computed on read in 0.1. |
| **Tombstone** | ID | removed version, by, why, paths | `[[tomb]]` tables, 0.2 ([21-environments-maturity-lifecycle.md](21-environments-maturity-lifecycle.md)). |

Relations: Scope 1..n FileRecord; Scope tree is a forest rooted at the root scope; Item belongs to exactly one home scope; Override targets an Item visible in its chain; Finding references a Directive or a FileRecord.

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-060 | All keys that determine output order MUST be orderable bytewise. Output MUST be produced from ordered maps or explicitly sorted vectors, never from hash-map iteration. | [0.1] |
| R-ARCH-061 | The registry MUST be immutable after resolution. | [0.1] |
| R-ARCH-062 | Item lookup by ID MUST resolve bare IDs along the chain, nearest first, and MUST report an ambiguity error rather than choosing among siblings. | [0.1] |

## 6. Incremental hashing and cache

The cache is a disposable accelerator and never a source of truth. It is neither authored nor generated, so the TOML and terse-line rules of D-32 do not apply to it, and it is never committed.

**File table.** One row per file: repo-relative path, size, mtime in nanoseconds, content hash (blake3 over LF-normalised bytes), and a reference to the cached extraction result. A file is rehashed only when (size, mtime) differ from the row; after a checkout that rewrites mtimes, a mismatch falls back to hashing that file only. Extraction results are keyed by content hash, so identical files across worktrees and branches share one entry.

**Resolution cache.** Keyed by the hash of the authored inputs of the chain (control files, declarations, overrides) plus the cache key below.

**Cache key (Q-TOOL-1).** Every entry is valid only for the tuple (fairlead version, grammar version, config hash, `--as-of` where a result depends on the date). The grammar version covers the edition and the extractor; the config hash covers the control files, including the marker. Any change invalidates; an upgrade can never serve stale results.

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-070 | The cache MUST live in `.fairlead/`, which `init` adds to ignore rules and which is never committed. | [0.1] |
| R-ARCH-071 | Output MUST be byte-identical with a cold cache, a warm cache, and a deleted cache. This MUST be tested. | [0.1] |
| R-ARCH-072 | Corruption, version skew or an unreadable cache MUST be handled by discarding and rebuilding; it MUST NOT be an error. A read-only cache location MUST degrade to uncached operation. | [0.1] |
| R-ARCH-073 | In 0.1 the cache format MUST be JSON, readable and diffable by agents and humans. A binary format (for example `postcard`) MAY replace it only after profiling shows JSON to be the bottleneck on a real repository (Q-TOOL-1, R-ARCH-131). `bincode` MUST NOT be used (discontinued). | [0.1] |
| R-ARCH-074 | Cache writes MUST be atomic (temporary file in the same directory, then rename). Readers (`check`, `context`, `explain`) never take the writer lock ([15-cli.md](15-cli.md) R-CLI-008); they MUST tolerate a concurrent writer by reading only complete files, and concurrent cache writers are harmless because entries are keyed by content hash. | [0.1] |
| R-ARCH-075 | `check --changed` MUST be supported to hit the incremental performance target (D-43). In 0.1 "changed" means the working tree and index against HEAD with no base argument ([15-cli.md](15-cli.md) R-CLI-022), read through the git module; unreadable git state exits with the cannot-determine code of R-CLI-003, never a pass. Extraction is limited to the changed files and the files that reference IDs declared in them; global checks still use the whole registry (R-CHK-002). | [0.1] |
| R-ARCH-076 | Content hashes MUST be computed after line-ending normalisation so that `core.autocrlf` settings do not change them. | [0.1] |
| R-ARCH-077 | The cache key of the tuple above MUST include `--as-of` for every entry whose result depends on the date (R-ARCH-090) and no other time input. | [0.1] |

## 7. Parallelism

| Stage | Strategy |
|---|---|
| Walk | `ignore::WalkBuilder::build_parallel`. Results gathered then sorted (R-ARCH-024). |
| Extract | `rayon` over the file list; each task pure (R-ARCH-030). |
| Resolve | Single thread. |
| Check | `rayon` per scope; each scope's findings collected then merged and sorted. |
| Render | Single thread. |

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-080 | Output MUST be identical for any thread count, including one. A test MUST run with `RAYON_NUM_THREADS=1` and with the default and compare bytes. | [0.1] |
| R-ARCH-081 | A `git2::Repository` MUST NOT be shared across threads (Q-TOOL-5, [23-git-integration.md](23-git-integration.md) R-GIT-076): each worker that needs git access opens its own handle, from the codebase root and never discovered from the working directory (R-GIT-071). | [0.1] |
| R-ARCH-082 | Thread count MUST be settable through a flag and an environment variable, with a conservative default (physical cores) so that agents running several instances do not starve one another. | [0.1] |

## 8. Determinism guarantees

| # | Guarantee | Mechanism |
|---|---|---|
| 1 | Same tree, same configuration, same binary: same bytes out. | Sorted maps, sorted findings, LF output. |
| 2 | Independent of the user's machine configuration. | `git_global(false)`; no locale APIs; no environment reads other than documented variables. |
| 3 | Independent of wall-clock time unless asked. | Time enters only through `--as-of`; the default (UTC today) is recorded in the output header. |
| 4 | Independent of working directory and invocation scope. | R-ARCH-043. |
| 5 | Independent of thread count and cache state. | R-ARCH-071, R-ARCH-080. |
| 6 | Independent of path casing on case-insensitive filesystems. | Canonical-casing rule in Section 10. |
| 7 | Randomness only where unavoidable and never in checks. | Draft IDs of `new` are random (see [20-versions-and-seal.md](20-versions-and-seal.md)); no check output depends on them. |

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-090 | Any check result that depends on the date MUST print the `as-of` value used in the output header and MUST include it in the cache key. | [0.1] |
| R-ARCH-091 | Generated files MUST be terse `ID k=v` lines (D-32), sorted bytewise, LF-only, end with a single newline, and contain no timestamps, binary version or machine-specific data; the header carries the edition and body hash only (D-55), so a binary upgrade within an edition never makes a file stale. | [0.1] |

## 9. Error model

Four kinds of outcome are kept strictly apart.

| Kind | Example | Where reported | Exit effect (code defined in [15-cli.md](15-cli.md)) |
|---|---|---|---|
| **Finding** | Hex literal that equals a token value | Output, with rule ID | Findings at enforce severity fail (R-CLI-003) |
| **Diagnostic** | Malformed directive, unparseable control file, inert override | Output, with stable code (13) | Syntax and configuration errors fail the run; an inert override is a finding-class diagnostic |
| **Tool error** | Unreadable file, held writer lock, write failure | stderr | Usage, refusal or environment code of R-CLI-003 |
| **Internal error** | Violated invariant, panic; or a result that cannot be determined | stderr with a request to file an issue | Exit 5 of R-CLI-003 (D-44) |

Exit codes are defined only in [15-cli.md](15-cli.md) (D-41, D-44). This chapter names outcome kinds and cites the code a kind maps to; it defines no code. When several outcomes apply, the precedence is that of R-CLI-003.

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-100 | The core library MUST return typed errors and MUST NOT call `process::exit`. Only the CLI maps errors to exit codes. | [0.1] |
| R-ARCH-101 | A panic inside a worker MUST be caught at the command boundary and reported as an internal error, with exit 5 ([15-cli.md](15-cli.md) R-CLI-003). The tool MUST NOT report success after a panic. | [0.1] |
| R-ARCH-102 | A run that cannot determine a result (for example unreadable git state under `--changed`) MUST say so, exit with the cannot-determine code of [15-cli.md](15-cli.md) R-CLI-003 (exit 5), and MUST NOT silently pass. | [0.1] |
| R-ARCH-103 | Diagnostics MUST be rendered as: stable code, severity, `path:line:col`, an excerpt with a caret, the expected form, and a fix hint; machine output MUST carry the same fields. | [0.1] |
| R-ARCH-104 | Errors in one scope's declarations MUST NOT hide findings in unrelated scopes. | [0.1] |

## 10. Cross-platform concerns

| Concern | Rule |
|---|---|
| **Path form** | Every path stored in an artifact, cache, finding or ID context is repo-relative, UTF-8, forward-slash. Paths shown to users on Windows are converted back and stripped of `\\?\` prefixes with `dunce`. |
| **Canonical casing** | NTFS and APFS are case-insensitive by default. Scope membership and path lookups use a case-folded key; the on-disk spelling is preserved for display. Two paths that fold equal but differ in case are reported as a collision. A case-only rename is a move that changes no scope ID. |
| **Non-UTF-8 paths** | Reported and skipped (R-ARCH-032 style); `camino` is used for UTF-8 paths. |
| **Reserved Windows names** | `fairlead new` validates names against `CON`, `NUL`, `AUX`, `PRN`, `COM1`-`COM9`, `LPT1`-`LPT9`, trailing dots or spaces, `..`, and paths longer than the platform limit. |
| **Line endings** | LF for all output and generated files; CRLF and CR normalised on input. `init` shows the `eol=lf` `.gitattributes` lines in its plan and applies them with `--yes`. |
| **Atomic replace** | Temporary file in the same directory, then rename. On Windows the rename can fail while an antivirus scanner or editor holds the target (UNVERIFIED): bounded retry with backoff. |
| **File locking** | The writer lock is a pid lock file `.fairlead/lock` (R-ARCH-146), not an OS advisory lock. Network drives and shared mounts are unsupported and documented as such (Q-TOOL-6). |
| **Symlinks and junctions** | Not followed (R-ARCH-022). |
| **Subprocesses** | No shelling out for core features. The only process fairlead may spawn is the user's `git` executable for `seal --commit`, `seal --tag` and signature verification (R-ARCH-005, D-48); reads use `git2` (D-27). |
| **Long paths** | Paths beyond 260 characters on Windows are handled via the extended-length form internally and reported if the operation fails. |

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-110 | CI MUST run the full test suite on Linux, Windows and macOS from the first commit. The Windows job MUST include paths over 260 characters, locked files and an antivirus-latency measurement ([32-portability-and-restructure.md](32-portability-and-restructure.md) R-PORT-014). | [0.1] |
| R-ARCH-111 | `check` output MUST be identical for the same repository state checked out with LF and with CRLF line endings. | [0.1] |
| R-ARCH-112 | Writers MUST never produce a file whose bytes depend on the platform. | [0.1] |
| R-ARCH-113 | The MSRV MUST be tested in CI on all three platforms. | [0.1] |

## 11. Dependency choices (D-27)

Risk: **Low** = active or org-backed; **Med** = last release 12-24 months ago or single maintainer; **High** = archived, deprecated or advisory. Versions as checked on 2026-10-08.

| Need | Choice | Version, last release | Risk | Alternative |
|---|---|---|---|---|
| Walk | `ignore` | 0.4.33, 2026-08-04 | Low | `walkdir` (no ignore rules) |
| Globs | `globset` | 0.4.20 | Low | `regex` |
| Git reads (D-48) | `git2` | 0.21.0, 2026-05-18 | Low | `gix` 0.89.0 (pre-1.0, pure Rust); revisit at 1.0 |
| Git actions (D-48) | the user's `git` executable (no crate) | n/a | n/a | none; `seal --commit`, `--tag`, `verify-commit` only |
| Hash | `blake3` | 1.8.7 | Low | `sha2` |
| Parallelism | `rayon` | 1.12.0 | Low | `std::thread::scope` |
| CLI | `clap` (derive) | 4.6.7 | Low | `bpaf` |
| Serialisation | `serde`, `serde_json` | 1.0.229 / 1.0.151 | Low | none |
| TOML read | `toml` | 1.1.7 | Low | none |
| TOML edit (format-preserving) | `toml_edit` | 0.25.16 | Low | `toml` alone loses formatting |
| Directive grammar | `winnow` | 1.0.4 | Low | `pest` |
| Diff | `similar` | 3.2.0 | Low | `imara-diff` |
| Atomic writes | `tempfile` | 3.27.0 | Low | `atomic-write-file` |
| Writer lock (Q-TOOL-6) | pid lock file created exclusively; liveness check of the recorded pid on the same host | not assessed: the liveness primitive MUST be chosen and recorded under R-ARCH-120 before adoption | n/a | `fd-lock` 4.0.4 (2025-03-10, Low-Med), the advisory OS lock, only if the pid lock proves unreliable |
| Semver | `semver` | 1.0.28 | Low | `versions` |
| Paths | `camino`, `dunce` | 1.2.6 / 1.0.5 | Low | `normpath` |
| Dates (0.1; D-37) | `jiff` | 0.2.38 | Low | `time` |
| Snapshots | `insta`, `trycmd` | 1.49.0 / 1.2.1 | Low | `expect-test` |
| Diagnostics | own formatter in 0.1 | n/a | n/a | `ariadne` 0.6.0 later |
| SARIF (0.1) | hand-written serde structs for the subset used, validated against the official schema in CI | n/a | n/a | `serde-sarif` 0.8.0 is Med-High (stale) |
| Network client (0.2) | for `upgrade --fetch` only; not assessed | n/a | n/a | none; see R-ARCH-124 |
| Import graph (later) | `tree-sitter` and per-language grammars | 0.27.1; TS grammar 0.23.2 (2024-11) is Med; compatibility with 0.27 UNVERIFIED and gated by R-ARCH-154 | Med | `syn` 3.0.6 for Rust only |
| MCP (later) | `rmcp` | 3.5.1, high churn: pin an exact minor | Low, churn | `rust-mcp-sdk` |
| Token counting | documented chars-per-token estimate; no offline Claude tokenizer exists | n/a | n/a | exact counts through a built-in counter or vendor API, opt-in only |

**Avoid:** `serde_yaml` (archived), `serde_yml` (RUSTSEC-2025-0068), `bincode` (RUSTSEC-2025-0141, discontinued; a 3.0.0 release exists, status UNVERIFIED), `sled` (pre-1.0), `jwalk` (crate description points to another crate), `serde_yaml_ng` (stale). YAML is not needed at all (D-05), which is also why `pnpm-workspace.yaml` is not parsed (R-ARCH-006).

[11-files-and-classes.md](11-files-and-classes.md) R-FILE-040 uses blake3 for `vendored.lock` (D-50). One hash function (`blake3`) avoids a second dependency.

Not assessed in the survey and therefore not endorsed here: fuzzing and property-test tooling (`cargo-fuzz`, `proptest`), benchmark harnesses (`criterion`), error-helper crates (`thiserror`, `anyhow`), the byte-search crate `memchr`, and a process-liveness primitive for the pid lock. Each is a conventional choice; maturity MUST be checked before adoption.

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-120 | New dependencies MUST be recorded in this table with version, date, licence and risk before being added. | [0.1] |
| R-ARCH-121 | The MSRV floor MUST be Rust 1.88 for 0.1, raised to 1.90 only when `tree-sitter` is adopted, and every raise MUST be announced in the changelog (see R-REL-006). | [0.1] |
| R-ARCH-122 | `Cargo.lock` MUST be committed for the binary; releases MUST be built with `--locked`. `cargo audit` or equivalent MUST run in CI. | [0.1] |
| R-ARCH-123 | The 0.1 binary MUST NOT link a network stack. | [0.1] |
| R-ARCH-124 | The only network use in the product is `upgrade --fetch` ([15-cli.md](15-cli.md) R-CLI-009, [22-editions-and-upgrade.md](22-editions-and-upgrade.md) R-EDN-025). Its client MUST sit in the isolated `net` module behind a Cargo feature, MUST retrieve edition migration data only and never a binary, and MUST NOT be called by any other verb. There is no background or automatic update check (D-26, Q-TOOL-9). | [0.2] |

## 12. Performance targets

Measured on a 100,000-file tree, Windows 11, Defender enabled, as set for 0.1.

| Metric | Target |
|---|---|
| Cold `check` | at most 60 s |
| Warm `check`, nothing changed | at most 5 s |
| `check --changed` | at most 1 s |
| Peak RSS | at most 500 MB |
| `context` on a leaf path | at most 800 tokens at p90; at most 1,500 at depth 3 (estimator: bytes divided by 4); byte-identical across two runs |

Design aspirations, not targets until benchmarked: warm no-change `check` under 100 ms from a stat sweep alone, cold scan 1 to 3 s on a fast Linux disk, `context` under 30 ms warm. They are assumptions from the design notes and have not been measured ([14-context.md](14-context.md) R-CTX-072).

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-130 | A benchmark corpus (synthetic 100k-file tree plus at least three real repositories, one TypeScript/React) MUST exist before 0.1 is declared complete, and CI MUST track the targets above, failing on a regression greater than a stated threshold. The same corpus MUST measure the bytes-per-token ratio against the target model's tokenizer and the cost of Merkle digests at 100,000 files ([14-context.md](14-context.md) R-CTX-072, [60-roadmap.md](60-roadmap.md) R-ROAD-017). | [0.1] |
| R-ARCH-131 | Profiling MUST precede any change of cache format, parallel strategy or data structure justified by performance. | [0.1] |

## 13. Writes, locking and the read-only guarantee

| Verb | Write-set | Milestone |
|---|---|---|
| `check`, `context`, `explain` | `.fairlead/` cache only | 0.1 |
| `init` | at most 3 created files: `<folder>/control.toml`, `<folder>/gen/baseline.txt` (only with findings) and `AGENTS.md` (only when no agent file exists); every other change is a marker-delimited append, shown as a diff first and applied only with `--yes` ([15-cli.md](15-cli.md) R-CLI-013, R-CLI-015) | 0.1 |
| `new` | the scaffolded file(s) | 0.1 |
| `sync` | scope index shards and membership shards; the baseline only with `--baseline` (D-38); the cache; exactly the set of [15-cli.md](15-cli.md) R-CLI-046 | 0.1 |
| `approve` | `fairlead/approvals.toml` (`[[approval]]`, D-34) | 0.1 |
| `move`, `seal`, `upgrade` | declared write-sets, staged in `.fairlead/tx/` with a single commit-point file renamed last (`move` is 0.2, D-38) | 0.2 |

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-140 | `check`, `context` and `explain` MUST NOT create, modify or delete any file outside `.fairlead/`. A test MUST assert a clean `git status` after each. | [0.1] |
| R-ARCH-141 | `.git/config` MUST never be written. Git hooks are written only by `init --hooks` (D-38); `.gitignore`, `.gitattributes`, agent instruction files and CODEOWNERS are edited only by `init --yes`, as marked appends shown in the plan ([15-cli.md](15-cli.md) R-CLI-015, [11-files-and-classes.md](11-files-and-classes.md) R-FILE-026). | [0.1] |
| R-ARCH-142 | A writer MUST hold the exclusive writer lock of R-ARCH-146 for the whole command; readers MUST NOT take it or block on it. A stale lock (dead pid, same host) MUST be taken over with a notice; a live holder, or a lock from another host, refuses the second writer with the refusal code of [15-cli.md](15-cli.md) R-CLI-003 and names the holder. `seal`, `move` and `upgrade` additionally take the allocation lock in the common git directory ([23-git-integration.md](23-git-integration.md) R-GIT-052). | [0.1] |
| R-ARCH-143 | Multi-file writes MUST be idempotent: re-running the same command after an interruption MUST complete or reproduce the same result. | [0.1] |
| R-ARCH-144 | Before writing any file in `fairlead/`, a writer MUST scan the new content for secret-like values and refuse. Secret-like values MUST be declared by environment-variable name only. | [0.1] |
| R-ARCH-145 | Writers MUST stage all outputs to temporary files and rename atomically, with a journal in `.fairlead/` so an interrupted run is resumed or rolled back by the next one ([15-cli.md](15-cli.md) R-CLI-007). A hand-edited generated file is saved to `.fairlead/quarantine/` before `sync` refuses (R-CLI-026). | [0.1] |
| R-ARCH-146 | The writer lock is a pid lock file `.fairlead/lock` created exclusively and holding pid, host and start time ([15-cli.md](15-cli.md) R-CLI-008, [23-git-integration.md](23-git-integration.md) R-GIT-052), per worktree and held only while a command runs (D-26). It is not an OS advisory lock; network drives are unsupported (Q-TOOL-6). | [0.1] |

## 14. Testing strategy

| Layer | Method | Detail |
|---|---|---|
| **Unit** | Standard tests in each crate | ID grammar, scope resolution, policy decisions, budget packing. |
| **Snapshot** | `insta` | Diagnostics text, `context` output, `explain` output, generated files. Snapshots are committed and reviewed. |
| **CLI contract** | `trycmd` | Each verb with flags, expected stdout, stderr and exit code. |
| **Golden repositories** | Fixture trees under `tests/golden/` with expected outputs | `ts-react` (the mandatory TypeScript/React tree, R-ARCH-153), `rust-workspace`, `fullstack-tauri`, `legacy-baseline`, `crlf-and-windows-names`, `nested-scopes-ovr`, `unparseable-control`, `zero-rules`, `sparse-checkout`, `legacy-encoding`, `two-writers`. Each runs on three platforms. |
| **Directive parser fuzzing** | Coverage-guided fuzzing of `fairlead-syntax` | Invariants: never panics; never loops; every marker candidate yields a directive or a diagnostic (R-ARCH-031); parse then render then parse is stable for valid directives; resource caps hold (8 continuation lines, 1 KB). Seed corpus from real repositories. |
| **Syntax conformance** | Golden fixtures per host language | Attachment and extent rules of [12-directive-syntax.md](12-directive-syntax.md) R-SYN-016 to R-SYN-019, R-SYN-045, R-SYN-046; a fixture that disagrees with the text is a spec bug. |
| **Property tests** | Generated trees | Resolution is order-independent in input file order; thread count and cache state do not change output; the baseline is a fixed point. |
| **Determinism** | Differential runs | Cold vs warm cache, 1 vs N threads, LF vs CRLF checkout, two working directories. |
| **Read-only** | Repository state assertion | `git status` clean after `check`, `context`, `explain` on three real repositories. |
| **Replay** | History replay | Fifty commits that introduced a hardcoded declared value, and twenty unrelated commits, to measure recall and false positives ([60-roadmap.md](60-roadmap.md)). |
| **SARIF** | Schema validation | Output validated against the official 2.1.0 schema in CI (0.1). |
| **Secrets** | Seeded values | Fifty secret-like values must all be rejected. |
| **Benchmarks** | Corpus from Section 12 | Tracked in CI. |
| **Agent-in-the-loop** | Ablation experiments | Not part of CI; see [60-roadmap.md](60-roadmap.md). |

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-150 | A fuzz target for the directive parser and one for the control-file loader MUST exist and run for a bounded time in CI. | [0.1] |
| R-ARCH-151 | Every diagnostic code MUST have at least one snapshot test. | [0.1] |
| R-ARCH-152 | A bug fix MUST add a regression fixture before the fix lands. | [0.1] |
| R-ARCH-153 | A golden repository MUST include a TypeScript/React tree, because dogfooding on a Rust-only repository exercises none of that path ([60-roadmap.md](60-roadmap.md) R-ROAD-015). | [0.1] |
| R-ARCH-154 | Before any TypeScript parsing through tree-sitter is adopted, a CI compatibility test MUST show that the chosen `tree-sitter-typescript` grammar loads and parses a fixture corpus under the pinned `tree-sitter` core version ([30-hierarchy-and-engines.md](30-hierarchy-and-engines.md) R-HIER-035). Until then the evidence source is the regex pass. | [0.3] |

## 15. Dogfooding

Fairlead's own repository carries a root managerial folder from the first commit that has a directive grammar. It declares the project's rules (no `unwrap` in library code outside tests, no network crates in the core, all paths repo-relative) as `[rules.*]` tables in its root control file ([12-directive-syntax.md](12-directive-syntax.md)) with `waivable` where appropriate, and `fairlead check` runs in CI against the in-tree build and against the last released binary.

| ID | Rule | Milestone |
|---|---|---|
| R-ARCH-160 | The fairlead repository MUST pass its own `fairlead check` in CI, using the previously released binary as the pinned reference to avoid bootstrap circularity; until a release exists, the in-tree build is the only reference. | [0.1] |
| R-ARCH-161 | Docs in `docs/` MUST be checked for dangling IDs and rule references by `cargo xtask docs-check` until the docs carry `def` directives (D-40, D-57); fairlead itself checks them after that. | [0.1] |
| R-ARCH-162 | The repository MUST contain at least one nested scope (for example `crates/fairlead-syntax/`) with a sealed key and a real `ovr`, so the wedge features run against real data. | [0.1] |

## 16. Examples

A scope tree and what is cached or written.

```text
repo/
  fairlead/                       authored + generated, committed
    control.toml               [fairlead] header (root = true, edition); kinds, excludes, severities
    approvals.toml             [[approval]] records written by fairlead approve (D-34)
    docs/010-principles.md     +++ TOML frontmatter +++
    gen/baseline.txt           generated, terse lines, sorted fingerprints
  apps/web/fairlead/control.toml  [fairlead] scope header and local facts only in 0.1
  apps/web/fairlead/gen/reg/tok.reg   generated registry shard for scope web, terse lines
  .fairlead/                      gitignored cache; JSON in 0.1
    files.json  extract/<hash>.json   disposable cache
    lock                       writer lock: pid, host, start time
    sync.journal               journal of an interrupted sync
    quarantine/                hand-edited generated files saved before sync refuses
    tx/                        staged multi-file writes (0.2)
```

Control file fragment relevant to this chapter.

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"

[scan]
exclude  = ["target/**", "node_modules/**", "dist/**"]
max-file = 2000000

[context]
bytes-per-token = 4
```

## 17. Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | `.gitignore` present but directory is not a git repo (copied subtree) | Ignore rules still honoured; git features report unavailable. | R-ARCH-020, R-ARCH-025 |
| 2 | User's global `core.excludesFile` ignores `*.local` | Not honoured; output identical on every machine. | R-ARCH-020 |
| 3 | `git checkout` rewrites mtimes on thousands of files | Rehash by content; extraction reused via hash. | Section 6 |
| 4 | Cache deleted between two runs | Same output, slower. | R-ARCH-071 |
| 5 | Two agents run `sync` at once | The second fails at once with the refusal code of [15-cli.md](15-cli.md), naming the lock holder. | R-ARCH-142; 15 R-CLI-008 |
| 6 | Antivirus holds the target during rename | Bounded retry, then tool error, never partial content. | Section 10 |
| 7 | `Button.tsx` and `button.tsx` in one directory on Linux, then cloned on Windows | Collision reported on every platform from the case-folded key. | Section 10 |
| 8 | File with CRLF in a repo with `core.autocrlf=true` | Same hash and same findings as the LF checkout. | R-ARCH-033, R-ARCH-111 |
| 9 | Marker inside a string literal in a test fixture | Parses as a directive unless a `skip` glob or the `fairlead\:` escape is used; accepted limitation until an AST layer exists. | R-ARCH-031 |
| 10 | Control file has a TOML syntax error | Fail closed with the configuration error code ([15-cli.md](15-cli.md)); no partial check. | R-ARCH-011 |
| 11 | `check --changed` where git state is unreadable | Exit with the cannot-determine code (exit 5 of R-CLI-003), never a pass. A shallow clone is handled in 0.2 ([23-git-integration.md](23-git-integration.md) R-GIT-040). | R-ARCH-102, R-ARCH-075 |
| 12 | 10,000 `use` directives in one generated file | Aggregated per item with a warning above the configured limit. | 12 R-SYN-031 |
| 13 | Binary older than the pinned edition or `fairlead-version` | Refused in Discover with the refusal code that [15-cli.md](15-cli.md) assigns to a pin mismatch, with the install hint; no partial work. | R-ARCH-013 |
| 14 | Panic in a worker thread | Internal error, exit 5; no partial writes committed. | R-ARCH-101 |
| 15 | Output redirected to a file on Windows | LF endings regardless of shell. | R-ARCH-052 |
| 16 | Sparse checkout hides a scope | Scope reported unknown; `check` exits 5 (cannot determine, D-56) for a verdict that depends on it. | R-ARCH-026 |
| 17 | Windows-1250 source file | Skipped with a warning, listed, never decoded by guess. | R-ARCH-023, R-ARCH-032 |
| 18 | Workspace with `pnpm-workspace.yaml` and `package.json` | Not parsed in 0.1; scopes come from declarations, hints from Cargo manifests only. | R-ARCH-006 |
| 19 | Writer lock left by a crashed process on the same host | Dead pid detected; lock taken over with a notice. | R-ARCH-142 |
| 20 | Writer lock found on a shared mount from another host | Not broken automatically; the second writer is refused and the holder named; the mount is unsupported. | R-ARCH-142, R-ARCH-146 |
| 21 | Hand-edited generated shard | Saved to `.fairlead/quarantine/`, `sync` refuses without `--force`. | R-ARCH-145 |

## 18. Resolved questions

All questions were accepted by their stated leaning on 2026-10-09 (D-46).

- Q-ARCH-1 (resolved 2026-10-09, D-31): root lookup is `--root`, `FAIRLEAD_ROOT`, then the nearest `root = true`; see R-ARCH-010
- Q-ARCH-2 (resolved 2026-10-09, D-38): the baseline is written only by `sync --baseline`; see Section 13
- Q-ARCH-3 (resolved 2026-10-09, D-37): `exp=` is date-based in 0.1 with `--as-of`; see R-ARCH-090
- Q-ARCH-4 (resolved 2026-10-09): the cache is JSON in 0.1 and changes format only after profiling; see R-ARCH-073, R-ARCH-131
- Q-ARCH-5 (resolved 2026-10-09): three crates for the fuzz boundary; merging into two is allowed only before the first publish; see R-ARCH-007
- Q-ARCH-6 (resolved 2026-10-09, D-38): `move` is 0.2 as a whole; see Section 13
- Q-ARCH-7 (resolved 2026-10-09): token counts use a configurable bytes-per-token ratio, default 4, labelled an estimate; see R-ARCH-056
- Q-ARCH-8 (resolved 2026-10-09, D-44): an internal error and a result that cannot be determined map to exit 5, a held lock to the refusal code (4), precedence as in R-CLI-003; see R-ARCH-101, R-ARCH-102, R-ARCH-142
- Q-ARCH-9 (resolved 2026-10-09, D-43): `check --changed` is 0.1; in 0.1 it is the working tree and index against HEAD with no base; see R-ARCH-075
- Q-TOOL-1 (resolved 2026-10-09): the cache is JSON in 0.1 until profiled, keyed by fairlead version, grammar, config hash and `--as-of`; see Section 6, R-ARCH-073, R-ARCH-077
- Q-TOOL-2 (resolved 2026-10-09): only Cargo TOML manifests are parsed in 0.1; see R-ARCH-006
- Q-TOOL-5 (resolved 2026-10-09): one `git2` repository handle per worker; see R-ARCH-081
- Q-TOOL-6 (resolved 2026-10-09): a pid lock file; network drives are documented as unsupported; see R-ARCH-146
- Q-TOOL-7 (resolved 2026-10-09): CI validates SARIF output against the official 2.1.0 schema; see R-ARCH-054
- Q-TOOL-8 (resolved 2026-10-09): the MCP server comes after 0.1, with `rmcp` pinned and re-tested on each release; see R-ARCH-004
- Q-TOOL-9 (resolved 2026-10-09): updates are checked only through the explicit `upgrade --fetch`, never in the background; see R-ARCH-124
