# Directive syntax and the control file
> Status: draft · Milestone: mixed · Decisions: D-02, D-04, D-05, D-11, D-12, D-17, D-18, D-19, D-27, D-30, D-31, D-32, D-33, D-34, D-35, D-36, D-37, D-40, D-46 · Related: 10-scopes-and-inheritance.md, 11-files-and-classes.md, 13-checks-and-diagnostics.md, 14-context.md, 15-cli.md, 22-editions-and-upgrade.md

## Purpose
Defines how intent is written down so a machine can read it without understanding the host language: the directive grammar, the six verbs, ID syntax, how a directive attaches to code, the TOML control file, and TOML frontmatter for docs. One grammar serves code, docs and TOML. Everything is AST-free in 0.1 (D-27): extraction is comment scanning plus the file tree.

## Concepts
- **Directive**: one instruction written as a comment (or as a string in TOML frontmatter). Directive text is DATA, never an instruction to an agent (D-18; see [trust](40-agent-integration-and-trust.md)).
- **Marker**: the token after the comment leader that makes a comment a directive. Default `fairlead`, set by the `marker` key (D-30).
- **Verb**: the first word of a directive. Exactly six: `def use ovr flag env end`.
- **ID**: a path-independent name `[scope:]kind.name`. **Kind**: a user-defined category declared in a control file.
- **Region**: the lines a directive attaches to (its *extent*). **Carrier**: where a directive physically lives (source comment, doc frontmatter or body comment, TOML comment).
- **Control file**: the TOML file of a managerial folder holding its frozen header, policy and identity. It is configuration, not a directive carrier. `meta`, `note`, `kind`, `scope`, `set` and `retain` are NOT source-line verbs.
- **Delegable key**: a control-file key a non-root scope may set. All others are root-only.

## Rules

### Carriers and the marker
- **R-SYN-001 [0.1]** A directive MUST be a comment whose first token after the comment leader is `<marker>:` (or `<marker>|` for a continuation). Anything else is not a directive.
- **R-SYN-002 [0.1]** Directives MUST NOT be attributes, macros, decorators or doc comments. The leaders `///`, `//!`, `/**` and `/*!` are never carriers. A `marker:` found in one is ignored and reported `HN-S013` (error), because the text would leak into rustdoc/TSDoc.
- **R-SYN-003 [0.1]** The marker is case-sensitive. Near-misses (`Fairlead:`, `fairlead;`, `fairlead -`, `fairlead :`) MUST report `HN-S005` (warn) and are not directives.
- **R-SYN-004 [0.1]** `<marker>\:` (backslash before the colon) is a literal and MUST be ignored silently. Test fixtures that need the marker SHOULD use `skip` globs (`[scan] skip`) instead.
- **R-SYN-005 [0.1]** Markdown honours only (a) the TOML frontmatter and (b) `<!-- fairlead: ... -->` outside fenced code blocks. Fenced and indented code is never scanned.
- **R-SYN-006 [0.1]** Built-in leaders by extension are in the table below; `[comments]` in the root control file adds or overrides entries. A file with an unknown extension yields one `HN-I002` (info) with the hint line to add. Files that cannot hold comments (JSON) carry no directives in 0.1; from 0.2 they are annotated through `[[attach]]` (R-SYN-047).

| Extension | Leader(s) | Notes |
|---|---|---|
| `.rs .ts .tsx .js .jsx .mjs .go .java .kt .swift .c .h .cc .cpp .cs .scala .zig` | `//`, `/* */` | JSX children: `{/* fairlead: ... */}` only; a bare `//` there renders as text |
| `.css .scss .less` | `/* */` | |
| `.py .sh .rb .toml .yaml .yml .ps1 .gitignore` | `#` | |
| `.sql .lua .hs` | `--` | the directive separator `--` needs spaces on both sides, so no clash |
| `.html .md .xml .svg` | `<!-- -->` | Markdown also has frontmatter |
| `.json` | none | `[[attach]]` from 0.2 (R-SYN-047) |

Per-host placement (R-SYN-044), shipped in the reference file `ref/syntax.md`:

| Host | Honoured form | Not honoured |
|---|---|---|
| TS/JS statements, Rust, Go, Java and C-family | `//` and `/* */` line or block comments, before, trailing or inside a body | `///`, `//!`, `/**`, `/*!` (R-SYN-002) |
| JSX children | `{/* fairlead: ... */}` | bare `//` (renders as text; not detectable without an AST) |
| Markdown | TOML frontmatter `directives`; `<!-- fairlead: ... -->` outside fences | fenced and indented code |
| TOML, Python, shell, YAML | `#` comment; file form needs the blank line (R-SYN-016) | a `#` inside a string |
| CSS and SCSS | `/* */` between declarations or rules | `//` in plain CSS |
| HTML, XML, SVG | `<!-- fairlead: ... -->` | comment inside an attribute or script string |
| JSON and other comment-free files | none | use `[[attach]]` from 0.2 |

### Grammar (normative)
```text
line        = leader *SP marker ":" 1*SP body            ; first line of a directive
cont        = leader *SP marker "|" 1*SP tail            ; continuation, appended with one SP
body        = ["!"] verb *(1*SP arg) [1*SP "--" 1*SP note]
verb        = "def" / "use" / "ovr" / "flag" / "env" / "end"
arg         = ID / pair / extent / bare
pair        = key ("=" / "+=") value                     ; "+=" is the explicit list-append marker
key         = lower *(lower / DIGIT / "-")
value       = word / quoted / list
list        = word *("," word)                           ; no spaces inside a list
quoted      = DQUOTE *(qchar / "\\" / "\" DQUOTE) DQUOTE
word        = 1*(ALPHA / DIGIT / "-" / "_" / "." / "#" / "/" / "@" / "%" / ":" / "*")
bare        = word                                       ; verb-specific: flag word, env name
extent      = "+" (1*DIGIT / "end")
ID          = [scope] ":" kind "." name  /  kind "." name   ; ":kind.x" = root scope
scope       = slug                                       ; a stored scope id, not a path
kind        = 2*8lower                                   ; must be declared in a control file
name        = slug *("." slug)                           ; <= 64 chars total
slug        = 1*(lower / DIGIT) *("-" 1*(lower / DIGIT)) ; <= 32 chars
note        = *VCHAR-or-text-to-EOL                      ; free text, never parsed, never a rule
```
- **R-SYN-007 [0.1]** The grammar above is normative. IDs are ASCII lowercase; non-ASCII text is allowed only inside `quoted` and `note`. Input is normalised before parsing: BOM stripped, CRLF and lone CR to LF, tabs to one space outside quotes, NFC (see R-SYN-040). A list item is a `word`, so it may be a `name:type` word (`params=id:string,limit:u32` on a bridge); the colon inside a list item is never a scope separator, and the bridge `params` value is such a list ([hierarchy](30-hierarchy-and-engines.md) R-HIER-060).
- **R-SYN-008 [0.1]** `note` is the `-- text` tail of any directive; there is no `note` verb. It MUST NOT influence any check, ID, rule or output other than being shown (fenced as data) by `explain`.
- **R-SYN-009 [0.1]** A directive starting `!` is *must-understand*: every key in it MUST be known to the running edition, otherwise it is an error. Without `!`, an unknown key is a warning (`HN-S011`) and is ignored; an unknown verb is always an error (`HN-S001`) (D-17). The `!` always sits directly before the verb (`!def`), never on a key. Keys beginning `x-` are user extension keys: always known, never warned, ignored by fairlead, preserved, and allowed inside a `!` directive. `x-pin=<12 base32>` is the optional pin of an exception on an `ovr` line or `[[ovr]]` table ([scopes](10-scopes-and-inheritance.md) R-SCOPE-047): `sync` never writes it, `explain` prints `pin=<12>` for every ID.

### The six verbs
Direction is a property of the verb, not chosen by the author.

| Verb | Form | Meaning | Flows | Milestone |
|---|---|---|---|---|
| `def` | `def ID [k=v..] [+N\|+end]` | In a doc: declares the item and its shape (control files use TOML tables). In code/CSS: marks the region as an implementation site of an ID declared in a doc; `def foreign.<name>` opens a foreign region (R-SYN-014) | down (effective view) | 0.1 |
| `use` | `use ID [ID..] [sym=..]` | The region consumes the IDs | up (usage facts) | 0.1 |
| `ovr` | `ovr ID why="" [owner=] [exp=\|until=] [op=] [base=] [field=val..]` | Declared exception to a rule or value that applies here | local effect; up to the ledger | 0.1 |
| `flag` | `flag WORD [ID] [to=ID] [target=VERSION] [by-date=DATE] [since=VERSION]` | Lifecycle word on the region or ID. Words are user-defined and map to canonical slots; keys: R-SYN-048 | up (rollup) | syntax 0.1, checks 0.2 |
| `env` | `env a,b` | Region exists only in these environments | down (gate) | syntax 0.1, checks 0.2 |
| `end` | `end` | Closes the innermost open region | - | 0.1 |

- **R-SYN-010 [0.1]** A `def` in code MUST name an ID that is declared in a doc (or in a control-file table). Declarations are never invented inline; a code `def` of an undeclared ID is dangling (`HN-R001`). Code `def`s are *sites*: several are legal unless the kind sets `impl = 1` (`HN-D002`).
- **R-SYN-011 [0.1]** `ovr` MUST carry `why=` unless the target key's policy is `open`, which needs no reason and is still counted as variance ([scopes](10-scopes-and-inheritance.md) R-SCOPE-031). A missing `why` is `HN-S016` only when the policy is `needs-exception` or `sealed`; it is decided at resolution, never by the extractor. When the target key's policy is `needs-exception` it MUST also carry `owner=` and `exp=` (`until=` from 0.2). When the policy is `sealed` the `ovr` is inert until an ancestor's `[[allow]]` grants it (D-02, [scopes](10-scopes-and-inheritance.md)). `owner=` MUST name a handle in the `owners` list of the declaring scope or an ancestor; free text is `HN-X012` and the `ovr` is ineffective ([checks](13-checks-and-diagnostics.md) R-CHK-053). A malformed `ovr` never grants anything (fail closed); an incomplete one is `HN-S016`.
- **R-SYN-012 [0.1]** `exp=` is an ISO date (`2026-12-31`), evaluated against `--as-of` (D-37). From 0.2 `until=<version>` is the version-based key. In 0.1 `until=` is rejected, and so is an `exp` value that holds a version instead of an ISO date; both raise `HN-S012` and the `ovr` is dropped. `[policies.waiver] max-exp` (default 90 days) caps the horizon.
- **R-SYN-013 [0.1]** Override semantics per field: `k=v` replaces; `k+=a,b` appends to a list (D-12); there is no `op=append` and no separate append line. `ovr` of a field the ID does not have is `HN-S009`. Keys known to `ovr` in 0.1: `why`, `owner`, `exp`, `op` (`loosen`, `tighten`, `set`, `replace`, `add`, `remove`; R-SCOPE-040), `base` (written by `new` and `sync`), and the target's field names. `until` is added in 0.2.
- **R-SYN-014 [0.1]** `def foreign.<name> tool=<tool> +end` opens a foreign region, text written by another tool (D-04, D-35). Its `+end` extent opens the region (R-SYN-019) and the matching `end` closes it, with the nesting of R-SYN-019. There is no dedicated verb and no `flag foreign`. A `def foreign` without `+end` is not a region, so its `end` would be unmatched (`HN-S008`). `check` skips literal and shadowing checks inside it. Other flag words are declared in `[flagwords]` (0.2 semantics). `awaiting` is a flag word reserved by the variation protocol ([variation](31-variation-protocol.md) R-VAR-060, 0.3). `def ext.<name> role=input` is a valid external-input declaration ([hierarchy](30-hierarchy-and-engines.md) R-HIER-068, 0.3).
- **R-SYN-015 [0.1]** Keys known to `def` in 0.1: `st` (status, default `active`, omitted when default), `sum` (<= 240 chars), `kw` (list), `sym`, `of` (variant-of), `policy`, `value`, `applies`, `min-len`, `home`, `gen` (names the generator that emits the item, D-35), plus keys declared by the kind (`[kinds.K.keys]`). A token `def` also takes `classes=<pattern,...>`, a safelist of class-name patterns for computed class references ([checks](13-checks-and-diagnostics.md) R-CHK-048, 0.2). The hierarchy and variation chapters add 0.3 `def` keys: `exposure`, `role`, `native`, `import-as`, `adapter`, `wraps`, `rejected-by`, `variant-of`, `axis`, `values`, `default`, `request`, `extends`, `imports`, `vendor`, and for tokens `css`, `use-css`, `use-tw`, `match` ([hierarchy](30-hierarchy-and-engines.md), [variation](31-variation-protocol.md)); `ovr` gains `of`, `dismiss` and `pair` (`ovr rule.var-fork`, R-VAR-080, R-VAR-082). Known to `use`: `sym`, `dyn` (`dyn=1` marks a dynamic-dispatch edge, R-HIER-025, 0.3) and `pin` (R-SYN-049, 0.3). Anything else follows R-SYN-009.

### Attachment (AST-free, deterministic)
- **R-SYN-016 [0.1]** *File form.* A directive binds to the whole file iff it is in the first comment block of the file (after skipping a BOM, shebang, `<?php`, `"use client"`, `'use strict'`, `@ts-check` and any `[scan] header-skip` pattern) AND that block is followed by a blank line or EOF. A license banner counts as the first block; a directive in a *later* block binds by R-SYN-018. The file form has no dedicated extent word; `+file` is not part of the grammar of edition 1 (a later edition may add it, D-17).
- **R-SYN-017 [0.1]** *Trailing form.* A directive in a comment on a line that also holds code binds to that line.
- **R-SYN-018 [0.1]** *Standalone form (symbol/block).* A directive on its own line binds to the next code line plus its extent: if the line opens unbalanced `{ ( [`, the extent runs to the matching close, capped at 500 lines; if it ends in `:` or the next line is more indented, the extent is the indented block; otherwise one line. Consecutive directive lines form a stack that binds to the same next code line.
- **R-SYN-019 [0.1]** *Explicit range.* `+N` sets the extent to N lines. `+end` opens a nestable region closed by a later `end` directive. An `end` with no open region, or a region unclosed at EOF, is `HN-S008`. A region is opened only by an explicit `+end`; a directive never becomes a region by lookahead for a later `end`.
- **R-SYN-020 [0.1]** `sym=name` on `def`/`use` names the symbol the region is expected to contain. In 0.1 it is a plain text-presence check within the extent (warn `HN-S014` if absent). It never changes the extent.
- **R-SYN-021 [0.1]** A directive that binds to nothing (EOF, or only blank lines follow) is dropped with `HN-S010` (warn).
- **R-SYN-022 [0.1]** *Doc form.* A frontmatter directive binds to the doc, which is itself an item. A body comment `<!-- fairlead: ... -->` binds to the next block (paragraph, list, table or code fence); before a heading it binds to that heading's section.
- **R-SYN-023 [0.1]** Heuristic extents MUST NOT change the meaning of IDs: attachment decides *where* a use/def is counted, never *what* it refers to. A tree-sitter/syn layer may later refine extents without changing syntax.

### IDs, kinds, resolution
- **R-SYN-024 [0.1]** Kinds are declared in a control file under `[kinds.<k>]`. A directive whose ID has an undeclared kind is `HN-S002` and lists the declared kinds. A child scope MUST NOT redefine an inherited kind (sealed); it MAY add a new kind namespaced to its scope. The kind is the 2 to 8 letter prefix of the ID; a readable or hyphenated label (for example `tauri-command`) is the kind's `title` or a `proto` word in the control file, and the grammar is not relaxed. `handoff` is a built-in doc kind (shape `doc`, `dir` `docs/handoff/`, mandatory `sum`; [context](14-context.md) R-CTX-008, 0.1). `exc` is a reserved built-in kind and cannot be declared ([scopes](10-scopes-and-inheritance.md) R-SCOPE-065).
- **R-SYN-025 [0.1]** Scope identity in an ID is a stored scope id (`web:`), never a folder path. `:cmp.x` names the root scope. An unqualified ID resolves lexically upward through the scope chain (nearest declaring scope wins). A reference from an ancestor into a descendant MUST be qualified.
- **R-SYN-026 [0.1]** Two doc `def`s of one ID in one scope are `HN-D001`. A child redefining an inherited ID without `ovr` is `HN-R005` (D-12). IDs are never recycled once removed [0.2].
- **R-SYN-027 [0.1]** Scope ids and IDs MUST NOT be Windows-reserved names (`con prn aux nul com1..9 lpt1..9`), end in `.`, or differ only by case from another (`HN-P001`).

### Continuation, quoting, limits
- **R-SYN-028 [0.1]** Long directives continue on following comment lines starting `marker|`; at most 8 continuation lines and 1 KB per directive, else `HN-S006`.
- **R-SYN-029 [0.1]** Quoted values escape only `\"` and `\\`. No newline inside quotes; use continuation. An unterminated quote or unknown escape is `HN-S007`.
- **R-SYN-030 [0.1]** Files over `[scan] max-file` (default 2 MB) and files with a NUL byte in the first 8 KB are skipped with `HN-I001`; a file that is not valid UTF-8 is skipped with a warning (`HN-I003`, [files](11-files-and-classes.md) R-FILE-015). Block comments are read line by line with a leading `*` stripped.
- **R-SYN-031 [0.1]** More than `[scan] max-directives` (default 500) directives in one file is a warning (`HN-S015`); counts aggregate per item.

### Malformed input: fail closed vs warn
- **R-SYN-032 [0.1]** A malformed directive MUST be removed from the registry and MUST emit a diagnostic with file:line, a caret, the expected form and a did-you-mean. It never aborts the run and is never silently dropped (silent loss of intent is drift). Default severity `error`.
- **R-SYN-033 [0.1]** An unparseable control file fails closed (D-11): the scope and its descendants are *unresolved*, `check` reports `HN-C001` and exits 2, and `context`/`explain` for paths below it refuse with the same error. Fairlead MUST NOT fall back to the ancestor's rules.
- **R-SYN-034 [0.1]** Fails closed (error, directive dropped): unknown verb, unknown kind, bad ID, unterminated quote, bad `ovr`/`exp`, unmatched `end`, oversize continuation, doc-comment carrier. `HN-S002` also covers a lexically invalid ID or kind (message "invalid ID"), and `HN-S016` also covers any other malformed directive body (wrong arity, stray characters, an extent in frontmatter); no code is renumbered. Warns only: near-miss marker, unknown key without `!`, bind-to-nothing, `sym` absent, directive-count explosion.
- **R-SYN-035 [0.1]** Parsing is read-only. No directive can cause a write; only verbs in [cli](15-cli.md) write.

### Rename, bridges, hosts, attachment extras
- **R-SYN-042 [0.2]** A rename or merge leaves no `alias` line in source and uses no `move --id` (not part of 0.1 to 0.3, D-50). It is written as `flag replace <old> to=<new>` ([lifecycle](21-environments-maturity-lifecycle.md) R-LC-011); the old ID is then recorded as a permanent `replaced-by` alias in the tombstone (`by`, R-LC-020). A `use` of the old ID resolves through the tombstone and raises the alias-reference diagnostic (HN-R004) until `sync` or the author rewrites it. The alias lives as long as the tombstone, which is permanent (D-16); the old ID stays reserved.
- **R-SYN-043 [0.3]** Events, `Channel` arguments and error shapes of a bridge are ordinary user-defined kinds whose `proto` word names the transport ([hierarchy](30-hierarchy-and-engines.md) R-HIER-060), for example `evt.*` with `proto=tauri-event`. The wire spelling convention (snake_case function, camelCase keys) is a key `spell=` on the bridge `def`, declared per bridge; fairlead does not infer it.
- **R-SYN-044 [0.1]** Per-host placement and whitespace tolerance are normative as the table following R-SYN-006, which also ships in the reference file `ref/syntax.md`. Any run of spaces or tabs may follow the leader and indent the line; trailing whitespace is ignored; the marker must be the first token after the leader. Comment-only carriers are the only forms; there is no non-comment alternative.
- **R-SYN-045 [0.1]** Attribute and decorator lines (`#[..]`, `@Name(..)`, a C# `[Attr]` line) and `///` doc-comment lines that directly follow a standalone directive are skipped when choosing the binding line; the binding line is the first following code line that is none of these and the extent includes the skipped lines. This is what lets `// fairlead: def cmd.x` sit above `#[tauri::command]` and bind to the function.
- **R-SYN-046 [0.1]** A multi-line signature followed by a body binds as one item: when the balanced close of the first line's opener is followed (on the same or a following line, before any `;` at depth 0) by a line that opens `{`, the extent continues to that block's matching `}`; capped at 500 lines. These rules (R-SYN-016 to R-SYN-019, R-SYN-045, R-SYN-046) are confirmed by a conformance suite of golden fixtures per host language; a fixture that disagrees with the text is a spec bug.
- **R-SYN-047 [0.2]** Files that cannot hold comments (JSON, lockfiles) are annotated by an `[[attach]]` table in the owning scope's control file: `path` (scope-relative glob) and `directives` (an array of directive bodies, as in frontmatter, R-SYN-036). Each directive binds as file form to every matching file. Only `def use ovr flag env` are allowed (no `end`, no regions). No new file class and no sidecar file exists.
- **R-SYN-048 [0.1]** `flag` keys: `to=ID` (replacement or target unit), `target=<MAJOR.MINOR.PATCH>` (authored target version, D-42; symbolic targets per [lifecycle](21-environments-maturity-lifecycle.md) R-LC-003), `by-date=YYYY-MM-DD` (opt-in date target, [0.2] semantics) and `since=<version>` (recorded by `seal`; authoring it is allowed only for back-fill). A `flag` never takes `until=` (D-49): it is an unknown key (`HN-S011`, error under `!`). `ovr` takes `exp=<date>` (0.1) or `until=<version>` (0.2+) (D-37). Syntax is 0.1, checks 0.2.
- **R-SYN-049 [0.3]** `use ID pin=<N>` records the bridge version the consumer was written against; it is compared with the `ver` of the bridge declaration ([hierarchy](30-hierarchy-and-engines.md) R-HIER-063, D-29). The `@N` suffix form does not exist.
- **R-SYN-050 [0.1]** One carrier per family. Units, bridges and tokens are `def` directives (D-40, R-FILE-030); there are no `[cmd.<name>]` or `[tok.<name>]` tables. Kinds are `[kinds.K]` tables, never an inline `[kinds]` map. A grant reason is the key `why`, never `reason`.
- **R-SYN-051 [0.1]** Control-file key names are unique across tables: the compatible binary range is `[settings] fairlead-range` (formerly `requires`), and the portable-scope table `[scope]` carries `portable`, `provides` and `requires` (dependencies, [scopes](10-scopes-and-inheritance.md) R-SCOPE-066) and no `id`; scope identity is only `[fairlead] scope` (D-33).

- **R-SYN-053 [0.1]** Field-level policies use the form `[policies."<policy-id>"]` in the root control file, with a `fields = [...]` key (field paths or patterns inside the governed item or file-item), a `policy` key (`open`, `needs-exception` or `sealed`) and optionally `target` (an item pattern or file kind). The policy-id is a free label and MUST NOT be `waiver`. Example, a Cargo dependency-version policy:
```toml
[policies."cargo.dep-version"]
target = "cargo"
fields = ["dependencies.*.version", "workspace.dependencies.*.version"]
policy = "sealed"
```
- **R-SYN-052 [0.2]** The draft doc word `d-<8 base32>` and its scope-qualified form `<scope>:d-<draft>` are admitted in the doc-reference keys `to=`, `answers=`, `revisits=`, `covers`, `supersedes`, `of` and `revives`. It is not a `kind.name` ID, never appears as the ID of a `def`, and has no per-kind aliases ([versions](20-versions-and-seal.md) R-VER-049).

### Doc frontmatter (TOML, D-05)
- **R-SYN-036 [0.1]** A doc starts with `+++`, TOML, `+++`. The key `directives` is an array of strings, each a full directive body (no leader, no marker). The key name is fixed, so a marker rename never rewrites docs. Single-quoted TOML literal strings are RECOMMENDED so `"` needs no escape.
- **R-SYN-037 [0.1]** Frontmatter that is not valid TOML, or `directives` that is not an array of strings, is `HN-S004`. Other frontmatter keys are the author's and ignored.

### Control file (TOML)
- **R-SYN-038 [0.1]** Every managerial folder holds exactly one control file, `control.toml`, which begins with the frozen `[fairlead]` header (D-33): `scope` in every file; `edition`, `root` and `marker` in the root, where `root = true`. A non-root file carries only `scope`. Other root keys sit in the `[settings]` table, because TOML places top-level keys before the first table. The `[fairlead]` header holds only the four frozen keys (D-33); a key written after it belongs to `[fairlead]`, so every other root key goes under `[settings]`. There is no separate pin file. The folder name is the `folder` key (default `fairlead`); a mismatch with the actual folder name is `HN-C005`. Renaming marker or folder is done by `fairlead upgrade` (D-30).
- **R-SYN-039 [0.1]** The control-file schema is open. Unknown control keys are warnings (`HN-C002`); unknown tables likewise, and tools that rewrite the file preserve them. A 0.1 build requires only the keys and tables marked 0.1 below; a key or table of a later milestone warns as unknown in a build that predates it. Duplicate keys and wrong types are parse errors (`HN-C001`).
- **R-SYN-040 [0.1]** Path hygiene at parse: NFC for all names and IDs; separators normalised to `/`; case-only collisions and reserved names rejected (`HN-P001`).
- **R-SYN-041 [0.1]** A non-root control file that sets a non-delegable key is `HN-C003`; the fix hint is `fairlead new escalation` (writes a `[[req]]` for the ancestor). Delegable keys apply locally. An escalation is only ever a `[[req]]` table (written by `fairlead new escalation`); there is no eighth line class, no source-line request and no `def` of a request kind.

Control-file schema (root shown; child files carry only delegable keys):

| Key / table | Type | Purpose | Delegable? | Milestone |
|---|---|---|---|---|
| `[fairlead]` | table | frozen header, parsed first (D-33): `edition` (integer, e.g. `1`, D-17, D-45), `root` (bool, `true` only in the root), `scope` (slug, stored scope id, immutable; D-11), `marker` (directive marker, D-30) | root keys only; schema frozen across editions | 0.1 (`edition` semantics 0.2) |
| `[settings]` | table | root body keys: `folder` (managerial folder name; mismatch is HN-C005), `fairlead-version` (binary pin; mismatch refuses, exit 4, [CLI](15-cli.md)), `fairlead-range` (compatible binary range, [editions](22-editions-and-upgrade.md) R-EDN-001; renamed from `requires` so it cannot collide with the scope `requires` of [portability](32-portability-and-restructure.md), R-SYN-051), `index-values` (`value` or `hash`, R-FILE-038), `purpose`, `owners` | root only | 0.1 |
| `purpose` | string <= 160 | scope card text (shown fenced as data) | yes | 0.1 |
| `owners` | list of handles | scope owners; mapped to CODEOWNERS (advisory in-tree). Root-only list in 0.1; from 0.2 a declaring scope may add owners for its subtree (R-SCOPE-056, D-51). An entry is a handle or an inline table `{ handle = "@x", kind = "human" \| "agent" }` (default `human`) | root only in 0.1; subtree additions from 0.2 | 0.1 |
| `[scan]` | table | `exclude`, `skip`, `header-skip`, `max-file`, `max-directives` | additive `exclude`/`skip` only | 0.1 |
| `[comments]` | table | extension to leader overrides | no | 0.1 |
| `[agents]` | table | `targets` (instruction files that get the context block) | no | 0.1 |
| `[kinds.K]` | tables | kind declarations: `title`, `shape` (`doc`/`code`/`either`), `impl` (`any`/`none`/N), `unused`, `dir` (where `fairlead new` puts docs of this kind), `source` (`generate`, R-FILE-031), `[kinds.K.keys]` (a key table may set `required = true` under `[kinds.<K>.keys.<key>]`; `fairlead new` scaffolds required keys) | new namespaced kinds only | 0.1 |
| `[file-kinds.K]` | tables | file kinds: extractor, native binding, import template ([files](11-files-and-classes.md)) | root only; children may add a namespaced kind | 0.1 |
| `[layout]` | table | sub-folder names: `docs`, `contract`, `gen`, `ledger`, `ref` | root only | 0.1 |
| `[[file]]` | array | file-items: `id`, `kind` (a file kind), `path`, `extends` (R-FILE-050) | yes, local | 0.1 |
| `[[vendor]]` | array | vendor-owned declarations: `path` or glob, `tool` (R-FILE-040) | yes, local | 0.1 |
| `[[ovr]]` | array | scope-level exception with `applies`; same keys as the `ovr` directive (R-SCOPE-041); a key with a trailing `+` appends (`"tags+" = ["a","b"]`, the TOML spelling of `k+=a,b`, R-SYN-013); `x-pin` | subject to the item's policy | 0.1 |
| `[rules.R]` | tables | rule declarations: `check`, `sev`, `stage`, `glob`, `exclude`, `waivable`, `why` (see [checks](13-checks-and-diagnostics.md)) | new rules in 0.1; key tightening via `ovr` (0.1) or `[delegate]` (0.2) | 0.1 |
| `[policies]` | table | override policy per key pattern (`"rule.*" = "sealed"`): `open` / `needs-exception` / `sealed`; `[policies.waiver]` (`max-exp`, `budget`, `max-renewals`, `approval`); `[policies."<policy-id>"]` field-level form (`fields`, `policy`, R-SYN-053) | no (items refine with `policy=` on `def`) | 0.1 |
| `[[allow]]` | array | ancestor grants: `target` (item, optionally `field`), `for` (scope ids), `why`, `exp` (0.1) or `until` (0.2) | yes, for descendants only | 0.1 |
| `[[req]]` | array | escalation proposal to an ancestor: `to` (ancestor scope id), `target`, `set` (optional proposed value), `why` | yes | 0.1 |
| `[context]` | table | `budget-start` (default 500), `budget-path` (700), `bytes-per-token` (4), `tokenizer`, `sev-weight` ([context](14-context.md) R-CTX-043 to R-CTX-045) | no | 0.1 |
| `[delegate]` | table | root list of delegable G2 key patterns, `"<pattern>" = "tighten"` (R-SCOPE-057) | no | 0.2 |
| `[[convention]]` | array | membership and glob conventions: `glob` plus `transparent`, `class` or `own` (R-SCOPE-018, R-SCOPE-019) | yes, local | 0.1 (`own` 0.2) |
| `[[attach]]` | array | directives for comment-free files: `path`, `directives` (R-SYN-047) | yes, local | 0.2 |
| `[baseline]` | table | `file`, ramp defaults | no | 0.1 |
| `[secrets]` | table | `allow` fingerprints for the floor secret scan | no (root only) | 0.1 |
| `[flagwords]`, `[env]`, `[maturity]`, `[layers.<ladder>]` | tables | vocabularies; root-only sealed (D-02) | no | 0.2 / 0.3 |

### Control-file schema index
Every TOML table and key family used anywhere in the drafts, with the chapter that owns its meaning. Owners cite rule IDs; the schema table above stays the detailed reference for root and child keys. Examples use only the carriers fixed by R-SYN-050.

| Table or key family | File | Keys | Owner | Milestone |
|---|---|---|---|---|
| `[fairlead]` | every `control.toml` | `edition`, `root`, `scope`, `marker` only (frozen, D-33) | 12 R-SYN-038; 22 R-EDN-001 | 0.1 |
| `[settings]` | root `control.toml` | `folder`, `fairlead-version`, `fairlead-range`, `index-values`, `purpose`, `owners` | 12 R-SYN-038, R-SYN-051; 22 R-EDN-001; 11 R-FILE-038 | 0.1 |
| `[scan]` | `control.toml` | `exclude`, `skip`, `header-skip`, `max-file`, `max-directives` (child: `exclude`, `skip` only) | 12 R-SYN-030, R-SYN-031 | 0.1 |
| `[comments]`, `[agents]`, `[baseline]`, `[secrets]` | root `control.toml` | leader overrides; `targets`; `file` and ramp defaults; fingerprint `allow` | 12 R-SYN-006, R-AGT-001, R-CHK-030; 40 R-AGT-024 | 0.1 |
| `[kinds.K]`, `[kinds.K.keys]` | root `control.toml` | `title`, `shape`, `impl`, `unused`, `dir`, `source`; `layers`, `role = "container"` (D-65); `required` under `[kinds.<K>.keys.<key>]` | 12 R-SYN-024; 11 R-FILE-031; 30 R-HIER-001 | 0.1 (`layers`, `role` 0.3) |
| `[file-kinds.K]`, `[layout]` | root `control.toml` | extractor and native binding per file kind; `docs`, `contract`, `gen`, `ledger`, `ref` | 11 R-FILE-020, R-FILE-053 | 0.1 |
| `[[file]]`, `[[vendor]]` | scope `control.toml` | `id`, `kind`, `path`, `extends`; `path`, `tool` | 11 R-FILE-040, R-FILE-050 | 0.1 |
| `[[ovr]]` | scope `control.toml` | scope-level exception; the keys of the `ovr` directive; `"key+"` append spelling, `x-pin` | 12 table (R-SCOPE-041 in [scopes](10-scopes-and-inheritance.md)) | 0.1 |
| `[delegate]` | root `control.toml` | `"<G2 key pattern>" = "tighten"` | 10 R-SCOPE-057 | 0.2 |
| `[[convention]]` | any `control.toml` | `glob`; `transparent`, `class`, `own` | 10 R-SCOPE-018, R-SCOPE-019 | 0.1 (`own` 0.2) |
| `[[attach]]` | scope `control.toml` | `path`, `directives` | 12 R-SYN-047 | 0.2 |
| `[rules.<name>]` | any `control.toml` | `check`, `sev`, `stage`, `glob`, `exclude`, `waivable`, `why`, type keys | 13 R-CHK-005 | 0.1 |
| `[policies]`, `[policies.waiver]` | root `control.toml` | `"<key pattern>" = "open"` / `"needs-exception"` / `"sealed"`; `max-exp`, `budget`, `max-renewals`, `approval`; field-level `[policies."<policy-id>"]` with `fields` and `policy` | 12 R-SYN-011, R-SYN-053; 13 R-CHK-025 to R-CHK-027 | 0.1 |
| `[[allow]]` | ancestor `control.toml` | `target`, `field`, `for`, `why`, `exp` (0.1) or `until` (0.2) | 12 table; 10; 21 R-ENV-041; 20 R-VER-071 | 0.1 |
| `[[req]]` | child `control.toml` | `to`, `target`, `set`, `why` (escalation) | 12 table; 15 R-CLI-030 | 0.1 |
| `[flagwords]` | root `control.toml` | flag word to canonical slot | 12 R-SYN-014 | 0.2 |
| `[env]`, `[env.alias]`, `[maturity]`, `[lifecycle.alias]`, `[bind.<env>]` | root `control.toml` | `order`; env aliases; word to rank; lifecycle display names; `rust` and `ts` gate tokens | 21 R-ENV-001 to R-ENV-035 | 0.2 |
| `[version]`, `[docs-first]`, `[changelog]` | root `control.toml` | `start`, `mirror`, `seal-where`, `integration-branch`, `ancestor-grace`, `escalation-grace`, `sweep-after`; `level`, `exempt`, `hotfix`, `hotfix-window`, `strict-order`; `file` | 20 R-VER-003 to R-VER-058, R-VER-045, R-VER-047, R-VER-048 | 0.2 |
| `[context]` | root `control.toml` | `budget-start`, `budget-path`, `bytes-per-token`, `tokenizer`, `sev-weight` | 14 R-CTX-043 to R-CTX-045 | 0.1 |
| `[exceptions]` | root `control.toml` | `max-live-per-rule` (default 10; drives `HN-X011`), `max-minors` (0.2), `max-days` (0.1) | 10 R-SCOPE-048; 20 R-VER-075 | 0.1 / 0.2 |
| `[eligibility]` | root `control.toml` | `min-lines` (default 5), `min-files` (default 3); feed `SCOPE-ELIGIBLE` hints | 10 R-SCOPE-011 | 0.1 |
| `[authority]` | root `control.toml` | `signed-records` | 21 R-LC-040 | 0.2 |
| `[retain]`, `[retain.<kind>]` | root `control.toml` | `min-age`, `roll-after`; `hot`, `ttl`, `keep-rejected`, `bind`, `max` | 33 R-RET-010 to R-RET-026 | 0.3 |
| `[layers.<ladder>]`, `[free.<name>]`, `[[flow]]` | root `control.toml` | layer to rank; `of`, `may-use`, `usable-by`; `from`, `to`, `via` | 30 R-HIER-001, R-HIER-002, R-HIER-005 | 0.3 |
| `[hier]` | root `control.toml` | `level`, `same-rank`, `unbound-ok`, `promote`; `governed` globs; `etypes` (entry types beyond the shipped five, R-HIER-040); `dormant-seals` (R-HIER-043, default 4) | 30 R-HIER-080, R-HIER-051 | 0.3 |
| `[variation]`, `[tokens]` | root `control.toml` | `request-layers`, `stale-seals` (default 4), `axis-after` (default 3), `fork-detect` (`off`, `observe`, `warn`, `enforce`), `similarity` (reserved, `off`); `root-px` (default 16), `near` (default 2), `[tokens.tailwind]` prefix-to-property table | 31 R-VAR-020, R-VAR-032, R-VAR-081, R-VAR-084, R-VAR-093, R-VAR-095 | 0.3 |
| `[layer-policy."<ladder>.<layer>"]` | root `control.toml` | `used-by` | 30 R-HIER-006 | 0.3 |
| `[scope]` | any scope `control.toml` | `portable`, `provides`, `requires`; `accepts`, `layer-alias`, `strict-coverage` (30 R-HIER-004, R-HIER-081, D-65) (no `id`: identity is `[fairlead] scope`, R-SYN-051) | 32 R-PORT-020, R-PORT-022 | 0.3 |
| `[[mount]]` | root `control.toml` | `scope`, `path`, `inherit`, `as` | 32 R-PORT-030, R-PORT-035 | 0.3 |
| `[[approval]]` | `fairlead/approvals.toml` | `fp`, `target`, `scope`, `approver`, `date`, `renewal` | 15 R-CLI-037; 13 R-CHK-026; 22 R-EDN-006 | 0.1 |
| `[[tomb]]` | `fairlead/tombstones/<year>.toml` | `id`, `removed`, `by`, `why`; `paths`, `repl`, `moved-to`, `deleted-by`, `at`, `d` | 21 R-LC-020; 33 R-RET-040 to R-RET-043; 30 R-HIER-101 | 0.2; user deletion and `moved-to` 0.3 (`paths`: R-LC-020) |
| `[[op]]` | change doc frontmatter (`type = "refactor"`) | `action`, `from`, `into`, `unit`, `container`, `by`, `removal`, `moved-to` | 30 (refactor example) | 0.3 |
| Version file frontmatter | `fairlead/versions/` | `version`, `prev`, `bump`, `sealed-at`, `scopes`, `docs`, `carried`, `dirty` | 20 R-VER-050 | 0.2 |
| Doc frontmatter (`+++`) | any doc | `directives` (array of strings, R-SYN-036); per-kind keys: `kind`, `id`, `status`, `impact`, `covers`, `created`, `sealed`, `body-hash`, `sum`, `out`, `why`, `touches`, `answers` | 12 R-SYN-036; the chapter that defines each kind (20, 21, 22, 31, 33) | 0.1 for `directives`; 0.2 to 0.3 per kind |
| `[package]`, `[dependencies]`, `[workspace]` | `Cargo.toml` (read only) | Cargo keys, never written (R-CLI-014) | 21 R-ENV-032; 12 examples | 0.1 |

## Examples
Root control file:
```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1"
purpose       = "Sync app: desktop shell, web UI, core engines."
owners        = ["@platform"]

[scan]
exclude = ["target/", "node_modules/", "dist/", "**/*.gen.ts"]
skip = ["tests/fixtures/**"]

[agents]
targets = ["AGENTS.md", "CLAUDE.md"]

[kinds.tok]
title = "design token"
shape = "doc"
[kinds.cmp]
title = "component"
impl = 1
[kinds.cmp.keys]
layer = { type = "enum", values = ["atom", "molecule", "organism"] }
[kinds.adr]
title = "decision"
shape = "doc"

[rules.no-raw-hex]
check = "leak"
tokens = ["tok.*"]
sev = "error"
stage = "observe"
glob = ["**/*.css", "**/*.tsx"]
waivable = true
why = "colours come from tokens"

[policies]
"rule.*" = "sealed"
"rule.no-raw-hex" = "needs-exception"
[policies.waiver]
max-exp = "90d"
budget = 25

[[allow]]
for = ["web"]
target = "rule.no-raw-hex.stage"
why = "web is migrating"
exp = "2027-03-31"
```
A token doc (frontmatter carrier):
```fairlead
+++
title = "Design tokens"
directives = [
  'def tok.primary applies=color,background-color,border-color min-len=4',
  'def adr.0003 st=active sum="CSS variables + shadcn tokens; Tailwind rejected (build cost)"',
  'use rule.no-raw-hex',
]
+++

# Colour
<!-- fairlead: use tok.primary -- example only -->
```
Rust (`//`, never `///`):
```fairlead
// fairlead: def cmd.sync-now sym=sync_now
// fairlead: use eng.sync-verify eng.sync-purify
#[tauri::command]
pub async fn sync_now() { /* ... */ }

// fairlead: ovr rule.no-unwrap why="startup invariant" owner=@platform exp=2026-12-31
let cfg = load().unwrap();
```
TypeScript/TSX (JSX comment form inside children):
```fairlead
// fairlead: def cmp.sync-button sym=SyncButton layer=molecule
// fairlead: use tok.primary cmp.button
// fairlead: flag dep to=cmp.sync-menu
export function SyncButton() {
  return (
    <Button>
      {/* fairlead: use tok.surface */}
      <Icon />
    </Button>
  );
}
```
CSS (value stays authored here; D-04):
```fairlead
/* fairlead: def tok.primary sym=--primary */
:root { --primary: #2563eb; }
.btn { color: var(--primary); } /* fairlead: use tok.primary */
/* fairlead: env dev +end */
.debug-grid { outline: 1px solid red; }
/* fairlead: end */
/* fairlead: def foreign.shadcn tool=shadcn +end */
.shadcn-block { color: #111; }
/* fairlead: end */
```
TOML (`Cargo.toml`; file form needs the blank line):
```fairlead
# fairlead: def crate.sync-core

[package]
name = "sync-core"

[dependencies]
# fairlead: use eng.sync-verify
sync-verify = { path = "../sync-verify" }
```
Multiline via continuation:
```fairlead
// fairlead: ovr rule.no-raw-hex owner=@design exp=2026-12-31
// fairlead| why="legacy email template; brand colour owned by marketing"
// fairlead| value=#ff0000
```
Diagnostic for a malformed directive:
```text
HN-S002 error src/ui/Sync.tsx:3:17 unknown kind "comp" in "comp.sync-button"
  declared kinds: tok cmp adr rule crate cmd eng
  did you mean: cmp.sync-button | directive dropped
```

## Edge cases
| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Marker inside a string literal or fixture | Parses as a directive (no AST). Mitigate with `skip` globs or `marker\:` | R-SYN-004 |
| 2 | `// fairlead:` in a Rust `///` or `/**` comment | Ignored for binding; `HN-S013` error (rustdoc leak) | R-SYN-002 |
| 3 | Bare `// fairlead:` between JSX children | Cannot be detected without an AST; renders as text. Use `{/* */}` | R-SYN-006 |
| 4 | Directive above a blank line then EOF | Binds to nothing: `HN-S010`, dropped | R-SYN-021 |
| 5 | Directive directly above code at top of file (no blank line) | Standalone form, not file form | R-SYN-016 |
| 6 | License banner, shebang, BOM, `"use client"` at top | Skipped or counted as first block per list; later blocks bind to next line | R-SYN-016 |
| 7 | Typo in kind (`comp.`) or wrong kind for a slug | `HN-S002`/`HN-R001` with nearest IDs and kind suggestion | R-SYN-024 |
| 8 | Agent invents an ID in code `def` | Dangling `HN-R001` with the 3 nearest real IDs | R-SYN-010 |
| 9 | `ovr` without `why` on a `needs-exception` or `sealed` key; or on a sealed key | `HN-S016`, dropped; or inert until `[[allow]]`. On an `open` key `why` is optional | R-SYN-011 |
| 10 | `ovr until=1.9.0`, or an `exp` holding a version, in 0.1 | `HN-S012`: version expiry needs seals [0.2]; use `exp=YYYY-MM-DD` | R-SYN-012 |
| 11 | Unclosed `+end` region or stray `end` | `HN-S008`; the region is dropped | R-SYN-019 |
| 12 | CRLF, BOM (PowerShell `Out-File`), NFD names | Normalised before parsing | R-SYN-007 |
| 13 | Control file with a TOML typo in a child scope | That subtree unresolved, exit 2, no silent ancestor fallback | R-SYN-033 |
| 14 | Child sets `marker` or `[comments]` | `HN-C003`; hint `fairlead new escalation` | R-SYN-041 |
| 15 | Minified or generated source | `exclude`/`skip`; extents capped at 500 lines; file-form `use` still works | R-SYN-018 |
| 16 | `tests/*.rs` or `#[cfg(test)]` modules | Directives parse; env is unknown unless declared; no cfg evaluation | R-SYN-014 |
| 17 | Rust `#[path]`, re-exports, macro output | Scope is folder-based; generated items are declared at the generator source with `def` | R-SYN-010 |
| 18 | Directive-like text in a vendored README | Vendored and excluded paths are never scanned | R-SYN-030 |
| 19 | `// fairlead: def cmd.x` above `#[tauri::command]` and a multi-line `fn` | Binds to the function: attribute skipped, signature and body form one extent | R-SYN-045, R-SYN-046 |
| 26 | `owner=` is free text, or an agent handle not in `owners` | `HN-X012`; the `ovr` is ineffective | R-SYN-011 |
| 27 | Reference key holds `d-k3m9qxa7` or `web:d-k3m9qxa7` | Admitted doc word, not a `kind.name` ID | R-SYN-052 |
| 20 | Rename `cmp.old` to `cmp.new` | No alias line; alias in the tombstone; `use cmp.old` warns via HN-R004 until rewritten | R-SYN-042 |
| 21 | `flag deprecate cmp.x target=2.0.0`, versus `until=2.0.0` on a `flag` | `target=` is the only flag key; `until=` on a `flag` is an unknown key (`HN-S011`) | R-SYN-048 |
| 22 | Annotating `package.json` | `[[attach]]` in the scope control file from 0.2; nothing in 0.1 | R-SYN-047 |
| 23 | A directive carries the key `x-owner=team` | Known user extension key, no warning | R-SYN-009 |
| 24 | `[tok.color]` table or `[cmd.x]` table in `control.toml` | Unknown table warning; declare with `def` | R-SYN-050 |
| 25 | Kind label `tauri-command` used as an ID prefix | Invalid kind; use a 2 to 8 letter kind and put the label in `title` or `proto` | R-SYN-024 |

## Resolved questions
- Q12-1 (resolved 2026-10-09): region-open form is the explicit `+end`; see R-SYN-019
- Q12-2 (resolved 2026-10-09): file form stays first block plus blank line, no `+file` in edition 1; see R-SYN-016
- Q12-3 (resolved 2026-10-09): comment-free files use an `[[attach]]` table in the control file, 0.2; see R-SYN-047
- Q12-4 (resolved 2026-10-09, D-31): root lookup is `--root`, `FAIRLEAD_ROOT`, then the nearest `root = true`; see R-SCOPE-004
- Q12-5 (resolved 2026-10-09, D-37): `exp` dates are compared with `--as-of`; see R-SYN-012
- Q12-6 (resolved 2026-10-09, D-34): `[[allow]]` is a control-file table, not a verb; see R-SCOPE-036
- Q12-7 (resolved 2026-10-09, D-35): `def foreign.<name>` closed by `end`; see R-SYN-014
- Q12-8 (resolved 2026-10-09): kind names stay 2 to 8 lowercase letters (admits `variant`, `contract`); see R-SYN-024
- Q12-9 (resolved 2026-10-09): the binary range key is renamed `[settings] fairlead-range`, `[scope]` loses `id`; chosen by the maintainer because `requires` is the dependency field of D-22 and `[fairlead] scope` is frozen by D-33; see R-SYN-051
- Q12-10 (resolved 2026-10-09): one carrier per family, bridges and tokens as `def` (D-40), `[kinds.K]`, `why`; see R-SYN-050
- Q-SYN-3 (resolved 2026-10-09): authored `target=<version>` on `flag`, `since` recorded by seal, `by-date` opt-in; `flag` never takes `until=` (D-49); see R-SYN-048
- Q-SYN-4 (resolved 2026-10-09): rename alias is recorded in the tombstone, not as a source line; see R-SYN-042
- Q-SYN-5 (resolved 2026-10-09): `pin=<N>` key on `use`; see R-SYN-049
- Q-SYN-6 (resolved 2026-10-09): events and shapes are user kinds with a `proto` word, spelling declared per bridge with `spell=`; see R-SYN-043
- Q-SYN-9 (resolved 2026-10-09): escalation is a `[[req]]` table written by `fairlead new escalation`, no eighth verb; see R-SYN-041
- Q-SYN-10 (resolved 2026-10-09): per-host placement table in the reference docs; see R-SYN-044
- Q-SYN-11 (resolved 2026-10-09): S4 attachment rules kept and confirmed by golden fixtures, with attribute skipping and signature-plus-body extents; see R-SYN-045, R-SYN-046
- Q-SYN-12 (resolved 2026-10-09): `!` sits before the verb, `x-` keys are user extension keys; see R-SYN-009
- Q-SYN-15 (resolved 2026-10-09): the prefix is the kind (2 to 8 letters), labels are text in control; see R-SYN-024
- Q-SYN-17 (resolved 2026-10-09): list append marker is `k+=a,b`, not `op=append`; see R-SYN-013
