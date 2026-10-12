# P1: the object model, the content cache and the Rust parser

> Outline goals 1 (fast first) and the parsing section; [50-architecture.md](../../50-architecture.md) R-ARCH-034, R-ARCH-154; [30-hierarchy-and-engines.md](../../30-hierarchy-and-engines.md) R-HIER-035; R-REL-006. Decisions D-26, D-40, D-63.

## Goal

Every supported file can be read as a list of objects (functions, types, constants, imports, exports, blocks), cached by content hash, parsed only when something needs it, and in parallel. The first parser is Rust. Benchmarks come before any rule uses objects.

## Design

- **Object.** `Object { id: ObjectId, kind: ObjectKind, name: String, path: RepoPath, span: Span, parent: Option<ObjectId>, visibility: Vis, signature_hash: u64, body_hash: u64, value: Option<String> }`. `ObjectKind`: `Fn`, `Type`, `Const`, `Static`, `Import`, `Export`, `Module`, `Block`, `Heading`, `Key`, `CssProp`. `ObjectId` is `path#qualified.name` (for example `src/sync.rs#engine::merge`), stable across line moves.
- **Parser trait.** `trait Parser { fn language(&self) -> &'static str; fn version(&self) -> &'static str; fn parse(&self, text: &str) -> Result<Vec<Object>, ParseError>; }`. One implementation per language, registered in a table; the version string enters the cache key, so a grammar upgrade re-parses.
- **Demand.** `ContentPlan { languages: BTreeSet<&str>, globs: GlobSet }` is derived from the control files: a `[content]` table, `const` declarations, `query`/`require` rules and anchored directives. A file is parsed only if its language is in the plan and a glob matches. With no demand, nothing is parsed and 0.3 behaviour is byte-identical.
- **Cache.** Objects are stored next to the existing extras (`.fairlead/cache/<key>/objects.bin`, serde), keyed by content hash plus parser version plus plan fingerprint. A warm run reads the cache and parses only changed files.
- **Failure.** A file that fails to parse keeps every line-based fact and gets `HN-O001` (info, observe): "parsed by lines only". Never an error: a parser bug must not block a check.

## Tasks

**T1. Milestone.** Register the 0.4 codes (master table) at `M04`, set `ACTIVE_MILESTONE = M04`, add `PENDING_0_4` to `tests/codes.rs`. Test: the 0.3 test pattern (`r_arch_151__*`) with the new list.

**T2. Object model.** The types above in `content/mod.rs`, with serde and a stable `ObjectId` format. Tests: `content__object_id_round_trips`, `content__object_id_survives_line_moves`.

**T3. `[content]` and the demand plan.** Root table `[content] languages = [...] globs = [...] max-file = N max-objects = N`; the plan merges it with what rules and directives need. Tests: `r_content_001__nothing_is_parsed_without_demand` (0.3 fixtures byte-identical), `r_content_001__a_rule_turns_its_language_on`.

**T4. Tree-sitter adoption.** After the owner allows it (Q4-4): add the pinned `tree-sitter` core and grammar crates to `fairlead-core` behind a `content` feature that is on by default, raise the MSRV to 1.90, and update the MSRV workflow and R-REL-006 text. Delete the weekly gate's throwaway crate once its job is folded into the CI. Done when the MSRV job passes on 1.90.

**T5. The Rust parser.** Functions (free, associated, trait items), structs, enums, traits, type aliases, `const`, `static`, `use` items, `pub` visibility, modules (inline and file), with qualified names from the module path. Tests: `content__rust_items_and_qualified_names`, `content__rust_visibility`, `content__rust_parse_error_falls_back` (`HN-O001`).

**T6. The object cache.** Read, write and invalidate as above; an object cache miss never re-reads a file that the extras already read in this run. Tests: `content__warm_run_parses_only_changed_files` (counts parses), `content__parser_version_invalidates`.

**T7. Parallel parsing.** Parse in the existing worker pool (`--threads`, `FAIRLEAD_THREADS`); results sorted, so output never depends on thread count. Test: `content__output_independent_of_threads`.

**T8. `explain <path>` lists objects.** A "objects" section with kind, qualified name and line for each object of the file (from the cache). Test: trycmd `explain-objects`.

**T9. Benchmarks.** Extend the bench harness with object parsing on the 100k corpus (cold, warm, one changed file) and record the 0.3 baseline first. The budgets of exit criterion 1 become CI assertions in the bench workflow. Done when the numbers are recorded in `docs/plans/0.4/errata.md`.
