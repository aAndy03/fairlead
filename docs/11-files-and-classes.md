# Files and Classes
> Status: draft · Milestone: mixed · Decisions: D-04, D-05, D-10, D-13, D-18, D-19, D-23, D-32, D-33, D-34, D-35, D-36, D-38, D-46 · Related: 10-scopes-and-inheritance.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 15-cli.md, 20-versions-and-seal.md, 33-retention-and-compaction.md, examples/ts-react.md

## Purpose
Every file fairlead reads or writes has a class that says who may write it and what lines it may contain. This chapter defines the five file classes, which line classes each may hold, the layout of a managerial folder, the styles/tokens design (authored CSS holds values, fairlead holds the contract and an index), vendor-owned files such as shadcn output, and the general mechanism by which one file extends another. Diagnostics named here in mnemonic form (for example `TOK-MISSING`) are listed by name, with their HN codes, in [checks](13-checks-and-diagnostics.md).

## Concepts
- **File class.** Says who may write a file: `authored`, `generated`, `reference`, `vendor-owned`, `ledger` (append-only, hash-chained). Class is assigned by location and declaration, never by content.
- **Line class.** Says what a line means. Source lines use the six verbs of D-18: `def`, `use`, `ovr`, `flag`, `env`, `end`. Control files hold TOML tables instead (`[[allow]]`, `[[req]]`, `[kinds]`, `[rules]`, `[policies]`, ...); they are never directive carriers. Identity and version live in TOML keys and generated headers, not in a line class; free text is the `-- text` tail of a directive. Line class gives meaning; file class gives authority: a `def` in an authored file is intent, in a generated file an emitted fact.
- **File-item.** An ID'd item (`file.styles`) declared in a scope's control file: kind, scope-relative path, optional `extends`. The anchor of the extends mechanism.
- **File kind (of a file-item).** Declared in the root control file as `[file-kinds.K]`, a namespace separate from the ID kinds of `[kinds.K]` ([syntax](12-directive-syntax.md)). Supplies an **extractor** (which names the file defines), a **native binding** (how extension is physically realised) and an optional import template.
- **Token contract.** The authored list of token items (ID, role, policy, doc) held by fairlead. **Token index.** The generated list of names and values extracted from the CSS.
- **Foreign region.** Text inside an authored file written by another tool; indexed, not name-policed.
- **Managerial folder.** `fairlead/` inside a scope; layout below. Its name and sub-names are config, not constants.

## Rules

### The five classes
- **R-FILE-001 [0.1]** Every file under a managerial folder MUST belong to exactly one class. Files outside managerial folders are ordinary code (authored by definition) unless declared `vendor-owned`, `generated` (external-generated) or `external`.
- **R-FILE-002 [0.1]** **Authored**: written by humans and agents. Fairlead MUST NOT modify authored files except by an explicit `new`, `move` or `upgrade` command. Authored intent is never copied into another file (D-10).
- **R-FILE-003 [0.1]** **Generated**: written only by `sync` (and `seal` for aggregates [0.2]). Content MUST be deterministic: sorted bytewise by ID, LF-only, no timestamps, no build-environment input, no usage counts. Generated files use the terse `ID k=v` line format (D-32), are committed, sharded per scope, and open with a `# scope <id> edition=<e> body=<20 base32>` header (the baseline opens with `# baseline edition=<e> body=<hash>`). The binary version is never stamped into a generated file, so a binary upgrade within an edition never makes a file stale (D-55, D-17); reference files (R-FILE-004) may carry a version.
- **R-FILE-004 [0.1]** **Reference**: shipped by fairlead for the pinned edition (syntax, kinds, diagnostics). Rewritten only by `upgrade`. A reference file carries only a machine-readable header (class, fairlead version, edition, content hash) and prose. It MUST contain no directives and MUST NEVER be scanned for them; fenced blocks in docs are ignored by the directive scanner (D-36).
- **R-FILE-005 [0.1]** **Vendor-owned**: written by a third-party tool (shadcn, heroui CLI, framework generators). Fairlead MUST NOT write them. It records a content hash of each in `vendored.lock` and reports hand edits or silent regeneration as drift. Directives inside a vendor-owned file MUST NOT be honoured; metadata about them lives in the lock and in fairlead-owned wrapper units.
- **R-FILE-006 [0.2]** **Ledger**: append-only, hash-chained (each year file's header holds the previous file's digest). `sync` MUST NOT rewrite it; `check` verifies the chain (a broken chain is never auto-fixed); only an explicit purge may rewrite. See [retention](33-retention-and-compaction.md).
- **R-FILE-007 [0.1]** Hand-editing a generated file MUST be reported as `HN-G002` (the header hash no longer matches the body) with the hint "edit the source". The correctness check is the full regeneration compare (`HN-G001`); a header alone is never proof of correctness (R-CHK-023, R-CHK-024). `sync` MUST refuse to overwrite a modified generated file unless told to, and report what it would lose.
- **R-FILE-008 [0.1]** Generated files MUST NOT contain "kept regions". An authored line in a generated file is wiped on regeneration and flagged first.
- **R-FILE-009 [0.1]** Files in folders declared `external-generated` (protobuf, OpenAPI, Prisma output) or `external` (vendored trees) MUST have their `// fairlead:` text ignored.
- **R-FILE-010 [0.1]** There MUST be no per-file version header (D-13). File history is derived from git; new docs get an immutable once-only stamp (`created`: in 0.1 the as-of date, from 0.2 a version, D-42).

### Which class may contain which line class
| Line class | authored | generated | reference | vendor-owned | ledger |
|---|---|---|---|---|---|
| `def` | yes (home scope) | emitted facts only | no | no (ignored) | frozen history records |
| `use` | yes | emitted edges only | no | no | no |
| `ovr` | yes | emitted ledger entries only | no | no | frozen history records |
| `flag` | yes | emitted rollups only | no | no | frozen records |
| `env` | yes | no | no | no | no |
| TOML tables `[[allow]]`, `[[req]]` (not directive verbs) | control files only | no | no | no | no |
| header keys (not a directive) | frontmatter and control files | generated headers and stamps | header only (R-FILE-004) | no | chain header |
| `-- text` tail (any directive) | yes | no | no | no | summaries |
- **R-FILE-011 [0.1]** `check` MUST report a line class in a class that may not hold it (`CLASS-LINE`, HN-C011).
- **R-FILE-012 [0.1]** Directive text, wherever it appears, is DATA and never an instruction to a reader or agent.
- **R-FILE-013 [0.1]** Directives are recognised only in comment tokens (D-18). A directive-shaped string inside a string literal, raw string or Markdown fence MUST NOT register; the false-positive limit of the AST-free core is documented in [directive syntax](12-directive-syntax.md).
- **R-FILE-014 [0.1]** The class of a generated-looking file follows its writer. A file fairlead itself writes is `generated`. A file produced by another tool (protobuf or OpenAPI output, `*.gen.ts`, Prisma client) is `external-generated`: fairlead never writes it and its `// fairlead:` text is ignored (R-FILE-009). A file MUST NOT have both writers (R-FILE-054).
- **R-FILE-015 [0.1]** Text that is not valid UTF-8 (legacy cp1250, ISO-8859-2) is skipped by the scanner with a warning (`ENCODING-SKIP`, HN-I003); it carries no directives and is never decoded by guess. No per-kind encoding setting exists. A skipped file is still hashed as bytes where a lock needs it.

### Managerial folder layout
- **R-FILE-020 [0.1]** A managerial folder MAY contain: `control.toml` (authored; the `[fairlead]` header comes first, D-33), `approvals.toml` (authored; `[[approval]]` records written only by `fairlead approve`, reviewed via CODEOWNERS, D-34), `docs/` (authored, free layout inside), `contract/` (authored declarations), `gen/` (generated; includes `gen/baseline.txt`, written by `sync --baseline`, D-38, and from 0.2 the draft-ID alias map of R-VER-046), `ledger/` (ledger, 0.2+), `ref/` (reference; root only unless a sealed sub-root pins its own), `vendored.lock` (generated). From 0.2 it may also contain `versions/` (root only: version files, [versions](20-versions-and-seal.md)), `tombstones/<year>.toml` (live `[[tomb]]` records with `id`, `removed`, `by`, `why` and optional `paths`, [lifecycle](21-environments-maturity-lifecycle.md) R-LC-020), `gen/scope.lock` (per-scope lock, R-VER-015; also written for a detached scope, R-PORT-013) and `docs/graveyard/` (graveyard entries, R-LC-030). The folder name is the `folder` key and sub-folder names are set in `[layout]` ([syntax](12-directive-syntax.md)).
- **R-FILE-021 [0.1]** A scope's `gen/` MUST be written only from that scope's own facts and effective view. Ancestor aggregates are computed on read and written only by `seal` [0.2].
- **R-FILE-022 [0.1]** Registry shards MUST be per scope and kind (`gen/reg/<kind>.reg`) and split by slug bucket above 500 lines. Empty scopes MUST NOT get shards. Membership shards (`gen/members`) MUST be separate so folder moves touch only those. Format of `gen/members`, under the standard header: one `self.<id> dir=<repo path, or . for the root> up=<parent id, or -> chain=<id>,<id>,...` line and one `scope.<child-id> dir=<repo path>` line per registered child scope, sorted bytewise; an absent shard means nothing is registered (`SCOPE-UNREGISTERED`, HN-C008, for every non-root scope).
- **R-FILE-023 [0.1]** The cache (`.fairlead/`) MUST be disposable, gitignored, never the only home of a fact, and rebuildable. `init` MUST include the cache ignore line in its plan (R-FILE-026) and MUST NOT add an ignore rule covering any managerial folder; `check` MUST fail when a declared or managerial path is matched by an ignore rule.
- **R-FILE-024 [0.1]** Managerial folders SHOULD be marked `eol=lf` and `linguist-generated` (for `gen/`) via the `.gitattributes` lines that `init` shows in its plan and applies only with `--yes` (R-CLI-015). Content hashes MUST be computed on normalised content (LF, no BOM).
- **R-FILE-025 [0.1]** Conflicts in generated files MUST be resolved by regenerating them with `sync` (no custom merge driver). `ledger/` SHOULD use union merge.
- **R-FILE-026 [0.1]** `init` writes outside the managerial folder and `.fairlead/` only with `--yes`, only the edits listed in R-CLI-015 (marked, appended lines in `.gitignore`, `.gitattributes`, agent instruction files and CODEOWNERS), each shown in the plan first. It MUST NEVER write `.git/config`, a manifest or a source file (R-CLI-014); hooks are installed only with an explicit `--hooks` (D-38). Without `--yes` the lines are printed, not written.
- **R-FILE-027 [0.1]** The block in `AGENTS.md`, `CLAUDE.md` and any other `[agents] targets` file is a fairlead-owned pointer block delimited by `<!-- BEGIN <marker> -->` / `<!-- END <marker> -->` (R-CLI-016). Its text is identical in every target file and for every scope (it points at `fairlead context <path>`, so nested scopes need no copy of their own); bytes outside the delimiters are never touched; an edited or missing block is `HN-A001` (R-AGT-002). Full copies of rules and symlinks to the rules are not used.
- **R-FILE-028 [0.2]** No custom merge driver or keyed merge exists for authored `control.toml`. A textual conflict is resolved by hand in git; a merge that is textually clean but semantically wrong (duplicate ids, a `[[allow]]` made inert, overlapping `ovr`s) is caught by `check` (`DUP-ID` HN-R005, `DUP-OVR` HN-X014, `OVR-INCOMPLETE` HN-S016), which runs on the merge result in CI (R-AGT-005).
- **R-FILE-029 [0.1]** `.fairlead/lock` is the per-worktree advisory writer lock inside the disposable cache, held only while a writing command runs (`init`, `sync`, `new`, `approve`, `upgrade`): a second writer in the same worktree finds it held and is refused with exit 4 ([CLI](15-cli.md) R-CLI-008, [portability](32-portability-and-restructure.md) R-PORT-015). `seal` and `move` additionally take a lock in the git common directory so that two worktrees cannot seal or move at the same time ([git](23-git-integration.md), D-52). The cache lock is never the only home of a fact and is not committed; readers never take it.

### Styles and tokens (D-04)
- **R-FILE-030 [0.1]** Default: **authored CSS is the source of truth for token values**. Fairlead holds (a) the token contract, authored in `contract/`, and (b) a derived token index, generated into `gen/reg/tok.reg` by running the stylesheet kind's extractor. Fairlead MUST NOT merge CSS contents.
- **R-FILE-031 [0.1]** Per kind, the control file MAY set `source = "generate"`. Fairlead then writes the file (class generated) and the stylesheet is no longer edited by hand or by third-party CLIs. This option is meant for formats nobody else edits (for example TS constants generated from Rust constants); it MUST be rejected for a file also listed as `vendor-owned`.
- **R-FILE-032 [0.1]** `fairlead new token` MUST scaffold the contract line and, optionally, a stub insertion in the authored CSS at a configured marker.
- **R-FILE-033 [0.1]** `check` MUST report: a contract token missing from the CSS (`TOK-MISSING`, HN-T003), a CSS token declared outside a foreign region and absent from the contract (`TOK-UNDECLARED`, HN-T004), and a value differing from the base without an `ovr` (`OVR-MISSING`, HN-T002).
- **R-FILE-034 [0.1]** The extractor MUST be per-kind and regex-level (for CSS: `--([\w-]+)\s*:`), recording the enclosing selector or at-rule so the same token in `:root`, `.dark` and `@theme` is one item with several value sites. CSS-in-JS, JS theme objects and other formats need a custom kind; precision limits MUST be documented in the kind.
- **R-FILE-035 [0.2]** For Tailwind v4, the stylesheet kind MUST encode these caveats as checks: (a) a token whose value contains `var()` and is bridged by `@theme` MUST use `@theme inline`, else it resolves at the declaring element and subtree overrides do not propagate; (b) a derived token (`--sidebar: var(--background)`) MUST be re-declared in every theme block that overrides its base (`TOK-DERIVED`, HN-T005 [0.1] for derivations declared in the token contract; detection from the derived-token graph is `HN-T010` [0.2], [checks](13-checks-and-diagnostics.md) R-CHK-046); (c) an override in unlayered CSS beats `@layer utilities` whatever the file order, so `check` MUST validate the declared `@layer` order statement (`HN-T008`) and forbid a bare override (`HN-T009`) instead of assuming "later file wins" ([checks](13-checks-and-diagnostics.md) R-CHK-047); (d) a `:root`-only literal override without the matching `.dark` value keeps the light value in dark mode; (e) Tailwind's own palette namespace (`--color-*`) MUST be declared inside a `def foreign` region or as external so references to it do not dangle; (f) `@source` lists decide which files emit utilities and are checked only for existence of declared generated folders.
- **R-FILE-036 [0.1]** Which element carries `.dark` is not knowable statically. Fairlead MUST NOT infer it from the DOM; the rule is static (every theme block that declares a base must be matched).
- **R-FILE-037 [0.1]** A leaf folder with only local CSS (for example `src/organisms/sidebar/sidebar.css`) needs no managerial folder; its token defs belong to the nearest ancestor scope.
- **R-FILE-038 [0.1]** The committed token index stores extracted values (`light=`, `dark=`) because `context` needs them. If diff noise from CSS churn becomes a problem, a root setting `[settings] index-values = "hash"` stores value hashes instead; the default is `"value"`.
- **R-FILE-039 [0.1]** The literal-leak scan (D-43; [checks](13-checks-and-diagnostics.md) R-CHK-019, R-CHK-020) is opt-in per declaration: a token item is scanned for only when its `def` carries `applies=` (the property context), and a value shorter than its `min-len` (default 4 characters) is never matched. There is no per-file-type globbing beyond the rule's own `glob` and `exclude`, so SVG assets, fixtures and brand colours are scanned only where a declaration and a rule both opt in.

### Vendor-owned files and foreign regions
- **R-FILE-040 [0.1]** Vendor-owned files MUST be declared in the owning scope's control file (path or glob, tool name). The lock stores a blake3 hash per file (D-50). Format of `<folder>/vendored.lock`, one file per scope under the standard header, sorted by id: `vendor.<10 base32 of blake3(path)> path=<scope-relative path> tool=<tool> b3=<64 lowercase hex>`. A CLI regeneration changes the hash and is reported as `VENDOR-CHANGED` (HN-G004); accepting it is `fairlead sync --accept-vendor` ([CLI](15-cli.md)).
- **R-FILE-041 [0.1]** Variants and wrappers around a vendor-owned primitive MUST live in a fairlead-owned wrapper unit (an `atoms/button` wrapping `ui/button`). Editing the vendor file to add a variant is `VENDOR-EDIT` (HN-G005) because a re-add will erase it.
- **R-FILE-042 [0.1]** Where a third-party tool appends text to an authored file (shadcn adding variables to the global CSS), the region MUST be bracketed as a foreign region: `def foreign.<tool> tool=<name> +end` ... `end` (D-35; [syntax](12-directive-syntax.md) R-SYN-014). Contents are indexed for context, not name-policed. Outside any foreign region, undeclared names are drift.
- **R-FILE-043 [0.1]** Relationship declarations (`extends`, file-items, foreign regions' owners) MUST live in the managerial folder. In-file comment anchors are optional and a mismatch with the managerial declaration is drift, so CLI tools stripping comments (for example a re-run `shadcn init`) lose nothing but the optional anchors; `check` reports `OVR-MISSING` (HN-T002) for a value that differs from its base. From 0.2 it also reports `FOREIGN-LOST` (HN-G006) and reconstructs from the ledger what it can [0.2]. Vendor CLI edits are not forbidden; they are detected.
- **R-FILE-045 [0.1]** Fairlead promises nothing about the survival of optional in-file anchors after a third-party tool rewrites a file; only the declarations in the managerial folder are guaranteed (R-FILE-043).
- **R-FILE-044 [0.1]** Metadata for vendor-owned files lives only in `vendored.lock` and the wrapper units; vendor-owned files are exempt from docs-first coverage and from line-class checks.

### File extends file
- **R-FILE-050 [0.1]** A file-item MUST be declared in a scope's control file as a `[[file]]` table: `id`, `kind` (a file kind), scope-relative `path`, optional `extends = "<file id>"`. Fairlead MUST NOT merge content; it checks the extends contract and the native mechanism does the cascade.
- **R-FILE-051 [0.1]** The extends contract checked per kind is: (1) the chain is acyclic (`EXT-CYCLE`, HN-C012) and the native import resolves to the declared base; (2) a name redefined in the extension needs an `ovr` unless the base name is `open`; (3) a new name is declared as an item or sits in a foreign region; (4) nearest-wins matches native cascade order.
- **R-FILE-052 [0.1]** When a scope declares no file-item of a kind, the extends target is the nearest file-item of that kind up the chain. Declaring one later rebinds the target; the rebind MUST be reported as an effective-view change by `sync`.
- **R-FILE-053 [0.1]** Native bindings and extracted names per kind:

| Kind | Native binding checked | Names extracted |
|---|---|---|
| stylesheet | `@import`, cascade order, `@layer` order | custom properties |
| tsconfig / eslint | `extends` or config spread | option keys |
| cargo manifest | `[workspace.package]`, `x.workspace = true`, `[lints] workspace = true` | dependency and lint names |
| doc template | `refines` IDs | claim IDs |

- **R-FILE-054 [0.1]** Where a tool writes a second file into the tree during build (`build.rs`, bundlers), a file with two writers is not allowed: the generated output MUST be declared `external-generated`, and declarations about the items it contains made at their generator input, as a `def` carrying the `gen=` key (D-35). Build-output directories (`target/`, `OUT_DIR`) are excluded from scanning by default; no hash fairlead computes includes the build environment or `OUT_DIR` content.
- **R-FILE-055 [0.2]** A declared `external-generated` file with a named generator is hashed over its normalised content (R-FILE-024) only; a change is reported as drift of the generator's input, never as a hand edit.
- **R-FILE-056 [0.2]** Extractors for CSS-in-JS, Tailwind theme objects and JS theme objects are custom file kinds (`[file-kinds.K]`) with their own extractor and a documented precision limit (R-FILE-034). There is no declared safelist and these formats are not deferred: until a kind exists, the names are simply not indexed.

## Examples
Managerial folder of the root and of one scope (illustrative; names are config):
```text
acme/
├─ .fairlead/                         cache, gitignored, disposable
├─ AGENTS.md                       marker block -> "run: fairlead context"
├─ fairlead/                          root scope, committed
│  ├─ control.toml                  authored: [fairlead] header, kinds, policies, ranks, file-items
│  ├─ approvals.toml                authored: [[approval]] records (fairlead approve; CODEOWNERS)
│  ├─ contract/tokens.md            authored: token defs (contract)
│  ├─ docs/                         authored: ADRs, principles, change docs; docs/graveyard/ entries [0.2]
│  ├─ gen/
│  │  ├─ reg/tok.reg  reg/rule.reg  generated: sorted terse lines, sharded
│  │  ├─ members                    generated: path-derived membership
│  │  ├─ scope.lock                 generated: reconciled version, ancestor digest [0.2]
│  │  ├─ baseline.txt                 generated: sync --baseline (fp k=v lines)
│  │  └─ exceptions.led             generated: written at seal only [0.2]
│  ├─ versions/                     authored: version files, root only [0.2]
│  ├─ tombstones/2026.toml          live: [[tomb]] records, one file per year [0.2]
│  ├─ ledger/hist/2026.hl           ledger: hash-chained [0.2]
│  └─ ref/syntax.md                 reference: pinned edition
└─ apps/console/
   ├─ fairlead/                        scope "apps-console"
   │  ├─ control.toml               authored: [fairlead] scope, [[file]], [[ovr]], [[req]]
   │  ├─ gen/reg/tok.reg            generated: this scope's tokens only
   │  └─ vendored.lock              generated: blake3 of vendor-owned files
   ├─ app/globals.css               authored (with a foreign region for shadcn)
   ├─ app/overrides.css             authored; extends root styles
   ├─ components.json               vendor-owned
   └─ src/organisms/sidebar/sidebar.css   leaf, no managerial folder
```
Token contract and the extends declaration:
```fairlead
<!-- fairlead: def tok.color-primary role=brand policy=sealed css=--primary -->
<!-- fairlead: def tok.radius-base policy=needs-exception css=--radius -->
```
```toml
# apps/console/fairlead/control.toml
[[file]]
id      = "file.console-styles"
kind    = "stylesheet"
path    = "app/overrides.css"
extends = "root:file.styles"

[[vendor]]
path = "components.json"
tool = "shadcn"
```
```fairlead
/* app/globals.css */
/* fairlead: def foreign.shadcn tool=shadcn +end */
  --chart-1: var(--color-blue-300);
/* fairlead: end */
/* app/overrides.css */
/* fairlead: ovr tok.radius-base op=loosen value=0.25rem why="dense admin tables" owner=andy exp=2027-03-31 */
```
Generated index lines (terse, sorted, derived from the CSS):
```text
# scope apps-console edition=1
tok.radius-base css=--radius light=0.25rem src=app/overrides.css ovr=exc.x1
```

## Edge cases
| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Agent edits a generated shard and forges its stamp | `HN-G002` (hand-edited): check diffs the full regenerated output; `sync` refuses to overwrite silently | R-FILE-007 |
| 2 | Hand-written line inside a generated file | Flagged then wiped on regen; hint "move to authored file"; no kept regions | R-FILE-008 |
| 3 | `shadcn add` appends variables to `globals.css` | Inside a foreign region: indexed, no failure. Outside: `TOK-UNDECLARED` with hint "declare or move into foreign" | R-FILE-033, 042 |
| 4 | `shadcn init` rewrites the whole CSS and strips comments | Declarations live in the managerial folder, so only anchors and `ovr` comments are lost; `OVR-MISSING` + `FOREIGN-LOST` reconstructible from ledger | R-FILE-043 |
| 5 | `shadcn add --overwrite` erases a local edit in a vendor file | Hash change reported (`VENDOR-CHANGED`); variants belong in the wrapper unit; editing the vendor file is `VENDOR-EDIT` | R-FILE-040, 041 |
| 6 | Directive-looking text in vendor file or generated code (protobuf, Prisma) | Ignored by class; no ID flood, no duplicate-ID errors on regeneration | R-FILE-005, 009 |
| 7 | Reference docs show `// fairlead: def` sample syntax | Reference files never scanned; fenced blocks in docs ignored | R-FILE-004 |
| 8 | `// fairlead:` inside a Python string, Rust raw string or Markdown fence | Only comment tokens register; residual false positives documented | R-FILE-013 |
| 9 | `@theme` token holds `var()` without `inline` | `check` error: subtree and dark overrides would not propagate | R-FILE-035 |
| 10 | Derived `--sidebar: var(--background)` and a child overrides `--background` | `TOK-DERIVED`: derived token must be re-declared in the overriding block | R-FILE-035 |
| 11 | Child unlayered CSS vs Tailwind `@layer utilities` | Order statement is validated; "later wins" is not assumed | R-FILE-035 |
| 12 | `extends` cycle (style-a extends styles extends style-a) | `EXT-CYCLE` error | R-FILE-051 |
| 13 | Scope has no stylesheet, then gains one | Target rebinds from ancestor file to the new one; reported as view change | R-FILE-052 |
| 14 | `.gitignore` matches `fairlead/` or a declared generated path | `check` fails: authored rules would be lost on clone | R-FILE-023 |
| 15 | `core.autocrlf=true` on Windows | Hashes over normalised content; no false "stale"; CR stripped before parsing | R-FILE-024 |
| 16 | Tokens defined via CSS-in-JS or Tailwind theme objects | Regex extractor cannot see them; need a custom kind; limits documented | R-FILE-034 |
| 17 | `build.rs` writes into the source tree and hashes differ per environment | Declared `external-generated`; hashes never include build env; items declared at the generator | R-FILE-054 |
| 18 | Hand edit to the sealed ledger | Hash chain fails; never auto-fixed | R-FILE-006 |
| 19 | `init` run without `--yes` | Plan printed, nothing applied outside `fairlead/`; `.git/config` never touched | R-FILE-026 |
| 20 | Same marker block in `AGENTS.md` of a nested scope | Identical pointer text; edited block is `HN-A001` | R-FILE-027 |
| 21 | Windows-1250 source file inside a scope | Skipped with `ENCODING-SKIP` (HN-I003); not guessed | R-FILE-015 |
| 22 | Two merged branches each add an `[[allow]]` for one target | git merges text; `check` reports the semantic duplicate | R-FILE-028 |
| 23 | Token without `applies=` holds a value used literally in code | Not leak-scanned (opt-in) | R-FILE-039 |
| 24 | protobuf output in the tree | `external-generated`, not `generated` | R-FILE-014 |

## Resolved questions
- Q11-1 (resolved 2026-10-09, D-35): `def foreign.<name>` closed by `end`; see R-FILE-042
- Q11-2 (resolved 2026-10-09): the index holds values for 0.1, with a hash option if diff noise matters; see R-FILE-038
- Q11-3 (resolved 2026-10-09): no promise that in-file anchors survive; only managerial declarations are guaranteed; see R-FILE-045, R-FILE-043
- Q11-4 (resolved 2026-10-09): `source = "generate"` is per kind; see R-FILE-031
- Q11-5 (resolved 2026-10-09): `init` shows `.gitattributes` lines and applies them only with `--yes`; see R-FILE-024, R-FILE-026
- Q-FILE-1 (resolved 2026-10-09): `init` writes outside `fairlead/` only with `--yes`, listed lines only, never `.git/config`; see R-FILE-026
- Q-FILE-2 (resolved 2026-10-09): fairlead-owned pointer block with drift check, identical in every target; see R-FILE-027
- Q-FILE-3 (resolved 2026-10-09): open schema, unknown keys warn (D-17), minimal 0.1 schema; see R-SYN-039 in 12 and R-FILE-020
- Q-FILE-4 (resolved 2026-10-09): class follows the writer: fairlead-written is generated, other tools' output is external-generated; see R-FILE-014, R-FILE-009
- Q-FILE-5 (resolved 2026-10-09): foreign regions are indexed; `OVR-MISSING` in 0.1, `FOREIGN-LOST` reconstruction 0.2; vendor CLI edits detected, not forbidden; see R-FILE-043
- Q-FILE-6 (resolved 2026-10-09): custom file kinds per extractor with documented precision, Tailwind v4 caveat checks move to 0.2; see R-FILE-056, R-FILE-035
- Q-FILE-7 (resolved 2026-10-09): leak scan is opt-in per declaration with a minimum length; see R-FILE-039
- Q-FILE-8 (resolved 2026-10-09): build outputs are external-generated with a declared generator, hashes exclude the build environment; see R-FILE-054, R-FILE-055
- Q-FILE-9 (resolved 2026-10-09): non-UTF-8 text is skipped with a warning; see R-FILE-015
- Q-FILE-11 (resolved 2026-10-09): control-file merge conflicts are left to git and `check`; see R-FILE-028
