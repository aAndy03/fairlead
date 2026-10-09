# Implementation contract, release 0.1

> Status: draft · Milestone: 0.1 · Decisions: all of D-01..D-53 apply; this file adds none, it only fixes names · Related: 50-architecture.md, 60-roadmap.md, 10 to 15, 40

> **Amendments:** `02-contract-amendments.md` (decision D-57) takes precedence over this file wherever the two differ; read it before any phase plan.

This file is law for the phase plans P1 to P9. It fixes crate and module names, public types, function signatures, seams, phase ownership and test conventions. It contains signatures only (`todo!()` bodies are implied). If a spec chapter and this file disagree about a NAME, this file wins; if they disagree about BEHAVIOUR, the chapter and [decisions](../../decisions.md) win and the disagreement is a bug in this file to be reported. Everything the spec leaves open is closed in Section 9 as `C-n`. Rule IDs below (`R-xxx-nnn`) refer to the chapters in `docs/`.

Reading order for a phase engineer: Section 4 for your module, Section 5 for your seams, Section 6 for your rule list, Section 7 for test placement.

---

## 1. Workspace layout

The repository root is the Cargo workspace root (the directory name does not matter). Edition `2024`, MSRV `1.88` (R-ARCH-121), resolver `3`.

```text
Cargo.toml                    workspace: members = ["crates/*", "xtask"], exclude = ["fuzz", "tests/golden", "fixtures"]
Cargo.lock                    committed (R-ARCH-122)
rust-toolchain.toml           channel = "stable", components = ["rustfmt", "clippy"]  (MSRV is tested separately, R-ARCH-113)
rustfmt.toml                  edition = "2024", max_width = 100, newline_style = "Unix", use_field_init_shorthand = true
clippy.toml                   disallowed-types/methods (see below), msrv = "1.88"
deny.toml                     cargo-deny: bans YAML crates, bincode, any HTTP/TLS client (R-ARCH-123), advisories, licences
LICENSE                       PLACEHOLDER text "All rights reserved until the owner selects a licence"; workspace.package.license-file = "LICENSE"; publish = false
.editorconfig  .gitattributes `* text=auto eol=lf`, `fixtures/** -text`, `tests/golden/**/crlf/** -text`
.gitignore                    target/  .fairlead/
.cargo/config.toml            [alias] xtask = "run -p xtask --"
.github/workflows/            ci.yml  msrv.yml  audit.yml  fuzz.yml  bench.yml  dogfood.yml  release.yml
fairlead/                        this repo's OWN root managerial folder (R-ARCH-160): control.toml, approvals.toml, docs/, gen/
crates/
  fairlead-syntax/               lib, forbid(unsafe_code); own scope: crates/fairlead-syntax/fairlead/control.toml (R-ARCH-162)
  fairlead-core/                 lib, forbid(unsafe_code)
  fairlead/                      bin `fairlead` (package name `fairlead`), deny(unsafe_code)
tests/                        end-to-end tests: test targets OF THE `fairlead` PACKAGE declared with explicit paths (Section 7)
  testkit/mod.rs              RepoBuilder, TempRepo, golden runner
  golden/<name>/              golden repositories (Section 7)
  cli/*.trycmd                trycmd cases
  golden.rs determinism.rs readonly.rs sarif.rs secrets.rs replay.rs platform.rs cli.rs
fixtures/                     syntax/<host>/, secrets/, sarif/, corpus/ (never golden repos)
fuzz/                         cargo-fuzz project (own Cargo.toml, not a workspace member): fuzz_targets/{directive,control_file,frontmatter,lexer}.rs, corpus/
xtask/                        publish = false: `check-deps` (forbidden-crate scan), `sarif-validate`, `bench-corpus`, `golden-bless`
```

**Lints (workspace `[workspace.lints]`, inherited by every crate).**
- `unsafe_code = "forbid"` in `fairlead-syntax` and `fairlead-core` (crate attribute `#![forbid(unsafe_code)]`); `deny` in `fairlead`.
- CI: `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo fmt --all --check`. Denied in library code outside `#[cfg(test)]`: `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `print_stdout`, `print_stderr` (only `fairlead` prints), and `todo` once the owning phase has landed.
- `clippy.toml`: `disallowed-types` `HashMap`/`HashSet` in syntax and core (R-ARCH-060); `disallowed-methods` `std::process::exit` (R-ARCH-100), `std::env::var*` outside `discover.rs` and `fairlead`, `SystemTime::now`/`Instant::now` outside `time.rs` and `check/options.rs` (R-ARCH-090). A local `#[allow]` carries a reason comment.

**Cargo features.** Only `fairlead-core` has one: `vendored-libgit2` (default on, R-GIT-077). No `net` feature in 0.1 (R-ARCH-123).

---

## 2. Dependencies

Versions are those of [50-architecture.md](../../50-architecture.md) Section 11 (checked 2026-10-08). `UNVERIFIED` means the spec gives no version or no assessment: the executing engineer verifies with `cargo` (maturity, MSRV 1.88 compatibility, licence) and records the result in the Section 11 table before adding it (R-ARCH-120). Feature lists marked `UNVERIFIED` are the intent; the exact feature names must be confirmed.

| Crate | Dependency | Version | Features / note |
|---|---|---|---|
| fairlead-syntax | `winnow` | 1.0.4 | default |
| | `toml` | 1.1.7 | `default-features = false`, `std`, `parse`, `serde` (UNVERIFIED names) |
| | `serde` | 1.0.229 | `derive` (needed to cache records, C-2) |
| | `unicode-normalization` | UNVERIFIED | NFC (R-SYN-007, R-SYN-040), C-2 |
| | `memchr` | UNVERIFIED | marker prefilter (spec: transitive of `regex`, not assessed) |
| fairlead-core | `fairlead-syntax` | path | |
| | `ignore` | 0.4.33 | default (`require_git(false)`, `git_global(false)` at use) |
| | `globset` | 0.4.20 | default |
| | `regex` | UNVERIFIED | `pattern` rules, CSS and literal scanners (C-2); transitive of the two above |
| | `blake3` | 1.8.7 | default; no `rayon` feature |
| | `rayon` | 1.12.0 | |
| | `git2` | 0.21.0 | `default-features = false` (no https/ssh, no network stack) + `vendored-libgit2` (UNVERIFIED name) |
| | `toml` / `toml_edit` | 1.1.7 / 0.25.16 | `toml_edit` only for format-preserving appends to `approvals.toml` and scaffolds |
| | `serde`, `serde_json` | 1.0.229, 1.0.151 | `derive`; JSON cache, `json` output, hand-built SARIF structs |
| | `jiff` | 0.2.38 | `default-features = false`, `std` (UNVERIFIED); civil dates, UTC today |
| | `similar` | 3.2.0 | plan diffs, first three differing lines (R-CHK-023) |
| | `camino`, `dunce` | 1.2.6, 1.0.5 | absolute paths only; `RepoPath` is our own type |
| | `tempfile` | 3.27.0 | atomic replace |
| | `semver` | 1.0.28 | `Pin` (binary version) |
| | `thiserror` | UNVERIFIED | typed errors (R-ARCH-100); conventional, not assessed |
| | pid-liveness probe | UNVERIFIED | behind trait `PidLiveness` (Section 4.12); candidate `sysinfo`, not assessed; no `unsafe` in our code |
| fairlead | `fairlead-core` | path | |
| | `clap` | 4.6.7 | `derive`, `std`, `help`, `usage`, `error-context` (UNVERIFIED names) |
| dev | `insta` 1.49.0 (`json`, `glob`), `trycmd` 1.2.1 (bin only), `tempfile` 3.27.0 | | |
| dev, UNVERIFIED | `proptest`, `criterion` (core benches), `libfuzzer-sys` + `cargo-fuzz` (fuzz), a JSON-schema validator (xtask, SARIF 2.1.0, Q-TOOL-7) | | |

Forbidden everywhere (enforced by `deny.toml` and `xtask check-deps`): any YAML crate (`serde_yaml`, `serde_yml`, `serde_yaml_ng`), `bincode`, `sled`, `jwalk`, `serde-sarif`, and every HTTP/TLS client (`reqwest`, `ureq`, `hyper`, `openssl`, `rustls`, `native-tls`). `git2` without `https`/`ssh` features keeps the binary free of a network stack (R-ARCH-123, R-CLI-009).

---

## 3. Module trees

Module = one file unless shown as a directory. Owner = phase (Section 6). Paths relative to the crate.

### fairlead-syntax (`crates/fairlead-syntax/src/`, P2; no I/O, no `fairlead-core`, R-ARCH-002)

| Module | Path | Responsibility |
|---|---|---|
| `lib` | `lib.rs` | re-exports, `SYNTAX_VERSION: u32`, `GRAMMAR_EDITION: u32 = 1` |
| `ids` | `ids.rs` | `ScopeSlug`, `Kind`, `Name`, `Id`, slug and reserved-name validation (R-SYN-024..027) |
| `normalize` | `normalize.rs` | BOM strip, CRLF/CR to LF with offset map, NFC, tab handling (R-SYN-007, R-ARCH-033) |
| `delimiters` | `delimiters.rs` | per-extension comment-leader table `CommentTable` (R-SYN-006) |
| `lexer` | `lexer.rs` | comment-token spans honouring string and raw-string literals; marker prefilter (R-FILE-013, R-ARCH-030) |
| `directive` | `directive.rs` | grammar (winnow), `Directive`, `Arg`, known-key tables per verb (R-SYN-001..015, 028, 029, 048) |
| `frontmatter` | `frontmatter.rs` | `+++` splitter, `directives` array (R-SYN-036, 037) |
| `attach` | `attach.rs` | file/trailing/standalone/explicit-range attachment, regions (R-SYN-016..023, 045, 046) |
| `diag` | `diag.rs` | `DiagCode`, `Severity`, `Span`, `SyntaxDiag`, the `HN-S*`/`HN-I*` constants |
| `extract` | `extract.rs` | the single entry `extract()` tying the above together (R-ARCH-030..033) |

### fairlead-core (`crates/fairlead-core/src/`)

| Module | Path | Phase | Responsibility |
|---|---|---|---|
| `error` | `error.rs` | P3 | `Error`, `ErrorClass`, sub-errors (R-ARCH-100) |
| `outcome` | `outcome.rs` | P3 | `ExitCode`, `VerbOutcome` (R-CLI-003) |
| `diag` | `diag.rs` | P3 | `Diagnostic`, `Location`, `DiagCode` constants for every HN code |
| `time` | `time.rs` | P3 | `AsOf`, the only clock access |
| `hash` | `hash.rs` | P3 | `ContentHash`, `Digest`, `Fingerprint`, base32, normalised hashing |
| `path` | `path.rs` | P3 | `RepoPath`, case-fold key, reserved names |
| `discover` | `discover.rs` | P3 | root lookup, pin check (R-ARCH-010..013) |
| `walk` | `walk.rs` | P3 | sorted parallel listing (R-ARCH-020..026) |
| `classify` | `classify.rs` | P3 | `FileClass` from location and declarations (R-FILE-001..014) |
| `extract` | `extract/mod.rs`, `extract/styles.rs` (P3, CSS token sites), `extract/literals.rs` (P5-owned, called by P3, Section 4.5) | P3 | per-file `FileRecord` from `fairlead-syntax` |
| `manifest` | `manifest.rs` | P3 | Cargo TOML hints (R-ARCH-006) |
| `git` | `git.rs` | P3 | read-only git2 access (C-3) |
| `config` | `config/{mod,header,schema,rules,approvals}.rs` | P3 | `ControlFile` typed schema, lenient header pass, delegation tables |
| `cache` | `cache/{mod,files,key}.rs` | P3 | disposable JSON cache in `.fairlead/` (Section 6 of 50) |
| `registry` | `registry/{mod,scope,item,ovr,grant,view,ledger,resolve}.rs` | P4 | scopes, items, overrides, grants, effective views, ledger |
| `secrets` | `secrets.rs` | P5 | floor secret scan (R-AGT-022..024) |
| `check` | `check/{mod,codes,options,finding,fingerprint,floor,rules,baseline,summary}.rs`, `check/builtin/{syntax,refs,tokens,leak,generated,exceptions,controls,scopes,agents,vendor,paths,notices}.rs` | P5 | |
| `generate` | `generate/{mod,shard,members,header}.rs` | P5 | pure renderers of generated files (R-CHK-023, R-FILE-003) |
| `render` | `render/{mod,human,terse,json,sarif,sanitize,line}.rs` | P5 | output formats; `line.rs` = terse `ID k=v` writer shared with `generate` and `context` |
| `context` | `context/{mod,request,tiers,priority,budget,estimate,digest,brief}.rs` | P6 | |
| `explain` | `explain.rs`, `explain/topics.rs` | P7 | |
| `write` | `write/{mod,writeset,lock,journal,tx,init,new,sync,approve}.rs` | P7 | write layer and verb planners |
| `pipeline` | `pipeline.rs` | P8 | composes the seams; the only module that calls several stages (R-ARCH-001) |
| `facts` | `facts.rs` | P3 | `Facts`, collected once per run so checks stay pure |

### fairlead (`crates/fairlead/src/`, P8)

| Module | Path | Responsibility |
|---|---|---|
| `main` | `main.rs` | `fn main() -> std::process::ExitCode`; panic boundary (R-ARCH-101) |
| `cli` | `cli.rs` | clap derive: `Cli`, `GlobalArgs`, one `*Args` struct per verb (R-CLI-001, 006) |
| `commands` | `commands/{init,check,sync,new,context,explain,approve,reserved}.rs` | each: `run(&GlobalArgs, &XArgs) -> CommandResult`, a thin call into core |
| `exit` | `exit.rs` | `ExitCode` to `std::process::ExitCode`, `Error` to `ExitCode` (R-CLI-003) |
| `output` | `output.rs` | writes `CommandResult`; TTY and `NO_COLOR` detection; LF-only |

---

## 4. Public API skeleton

Auxiliary small types that are named but not expanded here (`PathError`, `IoFailure`, `UnknownKey`, `StyleKinds`, `WaiverCount`, `FieldChange`, `KeyDecl`, `BaselineIssue`, `AcceptRefusal`, `BriefHeader`, `InhLine`, `Omission`, `InitDetection`, `Preset`, `LivenessAnswer`, `ColorArg`, `ReservedArgs` and the like) are defined by the owning phase with the obvious fields; their NAMES and modules are fixed. Conventions: all collections that reach output are `BTreeMap`/`BTreeSet`/sorted `Vec` (R-ARCH-060). Lines and columns are 1-based; columns count Unicode scalar values of the ORIGINAL text (C-16); byte offsets refer to the original bytes (R-ARCH-033). `0` line means "whole file". Every type derives `Debug, Clone, PartialEq, Eq` unless stated, and `Serialize, Deserialize` where it is cached (marked `cached`).

### 4.1 fairlead-syntax: common types (`diag.rs`, `ids.rs`)

```rust
// diag.rs
/// Opaque stable code "HN-S001" (R-CHK-042, R-ARCH-055). Compare by string.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DiagCode(pub &'static str);
pub mod codes { use super::DiagCode;
    pub const HN_S001: DiagCode = DiagCode("HN-S001"); // unknown verb
    // ... one const per HN-S*/HN-I* row of 13; S003 is unallocated and stays unused.
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Severity { Info, Warn, Error }           // order: Info < Warn < Error

/// Original-byte span plus 1-based line and char column of the start (C-16).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Span { pub start: u32, pub end: u32, pub line: u32, pub col: u32 }

/// R-SYN-032, R-ARCH-103: code, location, expected form, did-you-mean, one hint.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SyntaxDiag {
    pub code: DiagCode, pub severity: Severity, pub span: Span,
    pub message: String, pub expected: Option<String>,
    pub did_you_mean: Option<String>, pub hint: String,
}
```

```rust
// ids.rs  (R-SYN-006..027, R-SCOPE-001)
pub struct ScopeSlug(String);   // [a-z0-9]+(-[a-z0-9]+)*, <= 32, not Windows-reserved (R-SYN-027)
pub struct Kind(String);        // 2..=8 lowercase letters
pub struct Name(String);        // slug *("." slug), <= 64
pub enum IdQualifier { Bare, Root, Scope(ScopeSlug) }   // "kind.n", ":kind.n", "web:kind.n"
pub struct Id { pub qualifier: IdQualifier, pub kind: Kind, pub name: Name }
impl Id {
    pub fn parse(s: &str) -> Result<Id, IdError>;          // grammar of R-SYN-007; never panics
    pub fn kind_name(&self) -> String;                      // "kind.name"
}
impl core::fmt::Display for Id {}                           // canonical text; Ord = bytewise on Display
impl ScopeSlug { pub fn parse(s: &str) -> Result<Self, IdError>; pub fn as_str(&self) -> &str; }
pub enum IdError { Empty, BadChar { at: usize }, TooLong, ReservedName, BadKind, BadShape }
/// R-SYN-027: con prn aux nul com1..9 lpt1..9, trailing dot or space.
pub fn is_reserved_windows_name(s: &str) -> bool;
pub const PLACEHOLDER: &str = "TODO(fairlead)";   // R-CLI-031 scaffold slot value that `check` rejects (C-31)
```

### 4.2 fairlead-syntax: directives, extraction (`directive.rs`, `delimiters.rs`, `frontmatter.rs`, `extract.rs`)

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Verb { Def, Use, Ovr, Flag, Env, End }          // exactly six (D-18, R-SYN-009 table)
pub enum ArgOp { Set, Append }                            // "=" / "+=" (R-SYN-013)
pub enum ArgValue { Word(String), Quoted(String), List(Vec<String>) }
pub struct Arg { pub key: String, pub op: ArgOp, pub value: ArgValue, pub span: Span }
pub enum Carrier { LineComment, BlockComment, HtmlComment, Frontmatter }   // R-SYN-005, D-32
/// R-SYN-016..019, 022. Lines are 1-based inclusive. `Doc` = frontmatter or heading binding.
pub enum Extent { File, Doc, Lines { first: u32, last: u32 } }

/// One attached directive. Dropped/malformed directives never appear here (R-SYN-032, R-ARCH-031).
pub struct Directive {                                    // cached
    pub ordinal: u32,                  // position in the file, stable
    pub verb: Verb,
    pub must_understand: bool,         // leading "!" (R-SYN-009)
    pub ids: Vec<Id>,                  // def/ovr: exactly one; use: 1..n; flag: 0..1; env/end: none
    pub bare: Vec<String>,             // flag word; env names
    pub args: Vec<Arg>,                // key=value pairs in source order
    pub note: Option<String>,          // "-- text" tail, data only (R-SYN-008)
    pub carrier: Carrier,
    pub span: Span,                    // first line of the directive
    pub extent: Extent,
    pub end_span: Option<Span>,        // matching `end` of a "+end" region (R-SYN-019)
}
impl Directive { pub fn arg(&self, key: &str) -> Option<&Arg>; }

/// R-SYN-015: built-in keys per verb in edition 1, for HN-S011 (checked by resolve, C-5).
pub fn known_keys(verb: Verb) -> &'static [&'static str];

pub struct Leader { /* Line(String) | Block{open,close} | Html */ }
pub struct CommentTable(BTreeMap<String, Vec<Leader>>);   // key: extension without dot
impl CommentTable {
    pub fn builtin() -> Self;                              // table of R-SYN-006
    pub fn with_overrides(self, ext: &str, leaders: Vec<Leader>) -> Self;   // [comments]
    pub fn leaders_for(&self, ext: &str) -> Option<&[Leader]>;
}

pub struct Frontmatter { pub span: Span, pub table: toml::Table, pub directive_bodies: Vec<String> }  // cached
pub fn split_frontmatter(text: &str) -> Option<(core::ops::Range<usize>, core::ops::Range<usize>)>;

pub struct ExtractConfig {
    pub marker: String,                    // [fairlead] marker, default "fairlead"
    pub comments: CommentTable,
    pub header_skip: Vec<String>,          // [scan] header-skip patterns (R-SYN-016)
    pub max_file: u64,                     // default 2_000_000 (R-SYN-030)
    pub max_directives: u32,               // default 500 (R-SYN-031)
}
impl ExtractConfig { pub fn config_fingerprint(&self) -> [u8; 32]; }   // cache key part

pub enum ScanStatus { Scanned, NoMarker, SkippedTooLarge, SkippedBinary, SkippedNotUtf8 }
pub struct Extraction {                               // cached, keyed by content hash + config (R-ARCH-073)
    pub status: ScanStatus,
    pub directives: Vec<Directive>,
    pub frontmatter: Option<Frontmatter>,
    pub diags: Vec<SyntaxDiag>,
    pub candidates: u32,                   // marker candidates seen
    pub dropped: u32,                      // invariant: candidates == directives.len() + dropped (R-ARCH-031)
}

/// THE entry point. Pure function of bytes, extension and config (R-ARCH-030). Total: never panics,
/// never loops, any byte input (R-ARCH-032). Offsets in results are original-byte offsets.
pub fn extract(bytes: &[u8], ext: &str, cfg: &ExtractConfig) -> Extraction;
/// Stable re-rendering used by the parse-render-parse fuzz invariant (R-ARCH-150 table).
pub fn render_directive_body(d: &Directive) -> String;
```

`normalize.rs` also exports `pub fn lf_bytes(bytes: &[u8]) -> Cow<'_, [u8]>` (BOM stripped, CRLF and lone CR to LF) which `fairlead-core::hash` uses so hashes ignore `core.autocrlf` (R-ARCH-076, R-FILE-024).

### 4.3 fairlead-core: primitives (`path.rs`, `hash.rs`, `time.rs`, `outcome.rs`, `error.rs`)

```rust
// path.rs: repo-relative, UTF-8, forward slashes, no "..", no leading "/" (Section 10 of 50)
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RepoPath(String);
impl RepoPath {
    pub fn new(s: &str) -> Result<Self, PathError>;
    pub fn as_str(&self) -> &str;
    pub fn fold_key(&self) -> String;        // NFC + case-fold; lookup key (R-SCOPE-007, 50 Section 10)
    pub fn parent(&self) -> Option<RepoPath>;
    pub fn join(&self, seg: &str) -> Result<RepoPath, PathError>;
    pub fn extension(&self) -> &str;
    pub fn is_within(&self, dir: &RepoPath) -> bool;
}

// hash.rs: blake3 everywhere (D-50). Base32 = lowercase Crockford "0123456789abcdefghjkmnpqrstvwxyz" (C-7).
pub struct ContentHash(pub [u8; 32]);                    // cached
pub fn hash_normalised(bytes: &[u8]) -> ContentHash;     // lf_bytes first (R-ARCH-076)
pub fn hash_raw(bytes: &[u8]) -> ContentHash;            // non-UTF-8 files (R-PORT-016)
pub struct Fingerprint(String);   // 12 chars = first 60 bits (R-CHK-031); also used for approvals
pub struct Digest(String);        // 8 chars = first 40 bits (14 Concepts)
pub struct BodyHash(String);      // 20 chars = first 100 bits; generated-file header (C-17)
pub fn base32(bytes: &[u8], chars: usize) -> String;

// time.rs
pub struct AsOf(pub jiff::civil::Date);                  // R-CHK-014
impl AsOf { pub fn parse(s: &str) -> Result<Self, Error>; pub fn today_utc() -> Self; }  // sole clock read

// outcome.rs: exit codes are defined ONLY in 15 R-CLI-003
#[repr(u8)] #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ExitCode { Ok = 0, Findings = 1, Usage = 2, StaleGenerated = 3, Refused = 4, Internal = 5 }
impl ExitCode {
    /// Precedence 5, 2, 4, 1, 3, 0 (R-CLI-003).
    pub fn resolve(codes: &[ExitCode]) -> ExitCode;
}
/// What a verb reports besides text; the bin turns it into the process exit code.
#[derive(Default)] pub struct VerbOutcome { signals: Vec<ExitCode> }
impl VerbOutcome {
    pub fn ok() -> Self; pub fn signal(&mut self, c: ExitCode); pub fn merge(&mut self, o: VerbOutcome);
    pub fn exit_code(&self) -> ExitCode;     // = ExitCode::resolve(signals), Ok if empty
}

// error.rs: typed, never process::exit (R-ARCH-100). `class()` is data; the bin owns the mapping.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    NoRoot { searched_from: String },                         // R-ARCH-011, R-CLI-004: Usage
    PathOutsideRoot { path: String },                         // R-CTX-003: Usage
    Usage(String),                                            // Usage
    Reserved { verb: &'static str, milestone: &'static str }, // R-CLI-001: Usage ("<verb> arrives in <milestone>")
    Config(Box<ConfigFailure>),                               // R-SYN-033, R-SCOPE-054: Usage
    Pin(PinRefusal),                                          // R-CLI-005: Refused
    Refused(Refusal),                                         // lock, no-clobber, hand-edited gen, unconfirmed plan, TTY: Refused
    Io(IoFailure),                                            // C-10: NotFound on user path = Usage, else Internal
    CannotDetermine(String),                                  // R-ARCH-102: Internal (5)
    Internal(String),                                         // R-ARCH-101: Internal (5)
}
pub enum ErrorClass { Usage, Refused, Internal }
impl Error {
    pub fn class(&self) -> ErrorClass;
    pub fn diagnostics(&self) -> &[Diagnostic];               // printed like findings (R-ARCH-103)
}
pub enum RefusalKind { PinMismatch, LockHeld, NoClobber, HandEditedGenerated, PlanNotConfirmed, NotATty, CiSet, UnbalancedBlock, Floor }
pub struct Refusal { pub kind: RefusalKind, pub message: String, pub hint: String }
```

### 4.4 fairlead-core: discover, walk, classify, facts (`discover.rs`, `walk.rs`, `classify.rs`, `facts.rs`, `git.rs`, `manifest.rs`)

```rust
// discover.rs  (R-CLI-004, R-ARCH-010..013, D-31, D-33)
pub struct Edition(pub u32);                               // integer (D-45); this build supports Edition(1)
pub const SUPPORTED_EDITION: Edition = Edition(1);
pub struct Pin { pub version: Option<semver::Version>, pub edition: Edition }   // "0.1" is padded to 0.1.0 (C-12)
pub enum RootSource { Flag, Env, Search }
pub struct Root {
    pub dir: camino::Utf8PathBuf,       // absolute, dunce-cleaned
    pub folder: String,                 // [settings] folder, default "fairlead"
    pub source: RootSource,
    pub edition: Edition, pub pin: Pin,
    pub config_hash: ContentHash,       // hash of all control files incl. marker (cache key)
}
pub struct DiscoverRequest {
    pub root_flag: Option<camino::Utf8PathBuf>, pub env_root: Option<camino::Utf8PathBuf>,
    pub cwd: camino::Utf8PathBuf, pub binary_version: semver::Version,
    pub require_root: bool,                                // false only for `init`
}
pub struct Discovered { pub root: Root, pub root_control: ControlFile, pub cwd_rel: RepoPath }
pub fn discover(req: &DiscoverRequest) -> Result<Discovered, Error>;       // I/O
/// Pure core of the upward search: `probe(dir)` says whether dir/<folder>/control.toml has root = true.
pub enum RootProbe { NoControl, NotRoot, Root }
pub fn search_upward<F: FnMut(&camino::Utf8Path) -> RootProbe>(start: &camino::Utf8Path, probe: F) -> Option<camino::Utf8PathBuf>;
pub enum PinVerdict { Ok, OkNewerBinary { info: String }, Refused(PinRefusal) }
pub struct PinRefusal { pub install_line: String, pub reason: String }
/// Pure (R-CLI-005, D-50(4)): older binary or other edition refuses; newer binary on same edition proceeds.
pub fn pin_check(binary: &semver::Version, supported: Edition, pin: &Pin) -> PinVerdict;

// walk.rs  (R-ARCH-020..026, R-SCOPE-006/007)
pub struct WalkOptions { pub excludes: Vec<String>, pub max_file: u64, pub threads: Option<usize> }
pub struct WalkEntry { pub path: RepoPath, pub size: u64, pub mtime_ns: i128 }
pub enum SkipReason { TooLarge, Binary, NotUtf8, SymlinkOrJunction, Excluded }
pub struct WalkResult {
    pub files: Vec<WalkEntry>,                 // sorted bytewise by path (R-ARCH-024)
    pub skipped: Vec<(RepoPath, SkipReason)>,
    pub ignored_managerial: Vec<RepoPath>,     // managerial paths matched by an ignore rule (R-FILE-023, C-6)
    pub diags: Vec<Diagnostic>,                // HN-P001 case collisions, HN-C... symlink in managerial folder
}
pub fn walk(root: &Root, opts: &WalkOptions) -> Result<WalkResult, Error>;  // I/O, parallel, sorted

// classify.rs  (R-FILE-001..014, R-SCOPE-018)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum FileClass { Authored, Generated, Reference, VendorOwned, Ledger, ExternalGenerated }
pub struct ClassifyRules { /* [[vendor]], [[convention]] class=..., managerial folder name, layout */ }
impl ClassifyRules { pub fn from_controls(root: &Root, controls: &ControlSet) -> Self; }
/// Pure. Class by location and declaration, never content (R-FILE-001).
pub fn classify(path: &RepoPath, rules: &ClassifyRules) -> FileClass;
/// Pure. Reference, vendor-owned, external-generated files are never scanned for directives (R-FILE-004/005/009).
pub fn scans_directives(class: FileClass) -> bool;

// facts.rs: everything a pure check/context/sync-plan needs from disk or git, gathered once (P3)
pub struct ChangedSet(pub BTreeSet<RepoPath>);
pub struct GitFacts {
    pub available: bool, pub changed: Option<ChangedSet>,    // working tree + index vs HEAD, untracked included (C-36)
    pub sparse_absent: Vec<RepoPath>,                         // R-ARCH-026, R-CHK-057
    pub unreadable: Option<String>,                           // set => `--changed` exits 5 (R-ARCH-075)
}
pub struct Facts {
    pub generated_on_disk: BTreeMap<RepoPath, Arc<[u8]>>,     // every file of class Generated incl. baseline
    pub agent_files: BTreeMap<RepoPath, Arc<str>>,            // [agents] targets (R-AGT-002)
    pub codeowners: Option<Arc<str>>,                          // textual, offline (R-CHK-053)
    pub vendor_hashes: BTreeMap<RepoPath, ContentHash>,       // actual hashes of vendor-owned files
    pub vendor_lock: BTreeMap<RepoPath, ContentHash>,         // recorded in vendored.lock
    pub approvals: ApprovalsFile,
    pub git: GitFacts,
    pub manifest_hints: ManifestHints,
}
pub fn collect_facts(root: &Root, walk: &WalkResult, controls: &ControlSet, want_changed: bool) -> Result<Facts, Error>;

// git.rs: the ONLY module touching git (R-ARCH-005). Read-only in 0.1: no commit/stage/push API exists (R-CLI-010).
pub struct GitRepo { /* opened from Root.dir, never discovered from cwd (R-GIT-071); one handle per worker */ }
impl GitRepo {
    pub fn open(root: &Root) -> Option<GitRepo>;
    pub fn changed_files(&self) -> Result<ChangedSet, Error>;       // Err => CannotDetermine
    pub fn sparse_absent(&self) -> Result<Vec<RepoPath>, Error>;
}

// manifest.rs (R-ARCH-006, R-SCOPE-016): Cargo TOML only, read-only, failure => empty hints
pub struct ManifestHints { pub workspace_members: BTreeSet<RepoPath>, pub default_excludes: Vec<String> }
pub fn read_manifest_hints(root: &Root, walk: &WalkResult) -> ManifestHints;
```

### 4.5 fairlead-core: extraction records (`extract/mod.rs`, `extract/styles.rs`, `extract/literals.rs`, `secrets.rs`)

```rust
/// R-ARCH-030: pure of (bytes, path, config). Everything checks need from file text is computed here,
/// once, cached by content hash (this is what keeps `check` a pure function of the registry, R-ARCH-050).
pub struct FileRecord {                                   // cached (record part)
    pub path: RepoPath,
    pub class: FileClass,
    pub size: u64,
    pub hash: ContentHash,                                 // normalised (R-ARCH-076); raw bytes if not UTF-8
    pub extraction: Option<Arc<Extraction>>,               // None when class forbids scanning or file skipped
    pub css_tokens: Vec<CssTokenSite>,                     // P3, extract/styles.rs (R-FILE-034)
    pub literals: Vec<LiteralSite>,                        // P5-owned scanner, extract/literals.rs (R-CHK-019)
    pub secrets: Vec<SecretHit>,                           // only for files under a managerial folder (R-AGT-032)
    pub skip: Option<SkipReason>,
}
pub struct CssTokenSite { pub name: String, pub value: String, pub context: String /* selector or at-rule */, pub span: Span, pub in_foreign: bool }
pub struct LiteralSite {
    pub property: String,          // canonical CSS property ("background-color") (R-CHK-019)
    pub value_norm: String,        // normalised (R-CHK-020)
    pub raw: String, pub span: Span, pub projection_hint: Option<String>,
}
pub struct SecretHit { pub class: String, pub span: Span, pub fp_prefix: String }   // never carries the value (R-AGT-023)

pub struct ExtractPlan { pub syntax: ExtractConfig, pub classify: ClassifyRules, pub styles: StyleKinds, pub leak_properties: Vec<String> }
/// Pure per file. `bytes` already read.
pub fn extract_file(entry: &WalkEntry, bytes: &[u8], plan: &ExtractPlan) -> FileRecord;
/// I/O + rayon, result sorted by path; consults and fills the cache (R-ARCH-071).
pub fn extract_files(walk: &WalkResult, root: &Root, plan: &ExtractPlan, cache: &Cache, only: Option<&BTreeSet<RepoPath>>)
    -> Result<Corpus, Error>;
pub struct Corpus { pub records: BTreeMap<RepoPath, FileRecord> }

// extract/literals.rs (P5 implements; signature fixed here, P3 calls it from extract_file)
pub fn scan_literals(text: &str, ext: &str, properties: &[String]) -> Vec<LiteralSite>;
// secrets.rs (P5; also called by write guard (P7) and context (P6))
pub fn scan_secrets(text: &str) -> Vec<SecretHit>;
pub fn redact(text: &str) -> String;                       // R-AGT-023: matches never printed
```

### 4.6 fairlead-core: control files (`config/`)

The 0.1 keyset is the "0.1" rows of the schema index in [12](../../12-directive-syntax.md). Unknown keys and tables are kept in `unknown` and reported `HN-C002` (R-SYN-039). Duplicate keys and wrong types are `HN-C001` (parse failure).

```rust
pub enum Policy { Open, NeedsException, Sealed }          // R-SCOPE-030; "stricter" order Open < NeedsException < Sealed
pub enum Stage { Observe, Warn, Enforce }                  // ordered ramp (R-CHK-033)
pub enum OwnerKind { Human, Agent }

/// Frozen header, parsed in a first lenient pass (D-33, R-SYN-038).
pub struct FairleadHeader { pub scope: ScopeSlug, pub edition: Option<Edition>, pub root: bool, pub marker: Option<String> }
pub fn parse_header_lenient(bytes: &[u8]) -> Result<FairleadHeader, ConfigFailure>;

pub struct Owner { pub handle: String, pub kind: OwnerKind }
pub struct Settings {          // [settings]; non-root files may set only `purpose` (C-4)
    pub folder: Option<String>, pub fairlead_version: Option<String>, pub fairlead_range: Option<String>,
    pub index_values: IndexValues /* Value | Hash */, pub purpose: Option<String>, pub owners: Vec<Owner>,
}
pub struct ScanConfig { pub exclude: Vec<String>, pub skip: Vec<String>, pub header_skip: Vec<String>, pub max_file: u64, pub max_directives: u32 }
pub struct KindDecl { pub title: Option<String>, pub shape: KindShape /* Doc|Code|Either */, pub impl_limit: ImplLimit /* Any|None|N(u32) */,
                      pub unused: Option<Severity>, pub dir: Option<String>, pub source_generate: bool, pub keys: BTreeMap<String, KeyDecl> }
pub struct FileKindDecl { pub extractor: String, pub binding: String, pub import_template: Option<String> }
pub struct Layout { pub docs: String, pub contract: String, pub gen: String, pub ledger: String, pub reference: String }
pub struct FileItemDecl { pub id: String, pub kind: String, pub path: String, pub extends: Option<String> }
pub struct VendorDecl { pub path: String, pub tool: String }
pub struct OvrDecl { pub target: String, pub op: OvrOp, pub applies: Option<String>, pub set: BTreeMap<String, ArgValue>,
                     pub why: Option<String>, pub owner: Option<String>, pub exp: Option<String>, pub base: Option<String> }  // [[ovr]]
pub struct RuleDecl {                                      // [rules.<name>] (R-CHK-005)
    pub name: String, pub check: RuleCheckDecl, pub sev: Option<Severity>, pub stage: Option<Stage>,
    pub glob: Vec<String>, pub exclude: Vec<String>, pub waivable: bool, pub why: Option<String>, pub since: Option<jiff::civil::Date>,
}
pub enum RuleCheckDecl { Leak { tokens: Vec<String> }, Pattern { forbid: String, msg: String }, Builtin }
pub struct PolicyTable {                                    // [policies] (R-SYN-053, R-SCOPE-030)
    pub by_pattern: BTreeMap<String, Policy>, pub waiver: WaiverPolicy, pub fields: BTreeMap<String, FieldPolicy>,
}
pub struct WaiverPolicy { pub max_exp_days: u32 /*90*/, pub budget: u32 /*25, C-26*/, pub max_renewals: u32 /*2*/, pub approval_required: bool /*true*/ }
pub struct FieldPolicy { pub target: Option<String>, pub fields: Vec<String>, pub policy: Policy }
pub struct AllowDecl { pub target: String, pub field: Option<String>, pub for_scopes: Vec<String>, pub why: String, pub exp: String }  // [[allow]]
pub struct ReqDecl { pub to: String, pub target: String, pub set: Option<String>, pub why: String, pub blocking: bool }               // [[req]]
pub struct ConventionDecl { pub glob: String, pub action: ConventionAction /* Transparent | Class(FileClass) | Own(String) [0.2] */ }
pub struct ContextConfig { pub budget_start: u32 /*500*/, pub budget_path: u32 /*700*/, pub bytes_per_token: u32 /*4*/,
                           pub tokenizer: Option<String>, pub sev_weight: SevWeight /*error 4 warn 2 info 1, C-27*/ }
pub struct BaselineConfig { pub file: String /* default "<folder>/gen/baseline.txt" */ }
pub struct SecretsConfig { pub allow: Vec<String> }
pub struct ExceptionsConfig { pub max_days: Option<u32>, pub max_live_per_rule: u32 /* C-25 */ }
pub struct EligibilityConfig { pub min_lines: u32, pub min_files: u32 }    // C-24

pub struct ControlFile {
    pub path: RepoPath, pub dir: RepoPath,                  // dir = directory that OWNS the managerial folder
    pub content_hash: ContentHash,
    pub fairlead: FairleadHeader, pub settings: Settings,
    pub scan: ScanConfig, pub comments: BTreeMap<String, Vec<String>>, pub agents: Vec<String>,
    pub kinds: BTreeMap<String, KindDecl>, pub file_kinds: BTreeMap<String, FileKindDecl>, pub layout: Layout,
    pub files: Vec<FileItemDecl>, pub vendors: Vec<VendorDecl>, pub ovrs: Vec<OvrDecl>,
    pub rules: BTreeMap<String, RuleDecl>, pub policies: PolicyTable,
    pub allows: Vec<AllowDecl>, pub reqs: Vec<ReqDecl>, pub conventions: Vec<ConventionDecl>,
    pub context: ContextConfig, pub baseline: BaselineConfig, pub secrets: SecretsConfig,
    pub exceptions: ExceptionsConfig, pub eligibility: EligibilityConfig,
    pub unknown: Vec<UnknownKey>,                           // HN-C002 (R-SYN-039)
    pub violations: Vec<Diagnostic>,                        // HN-C003 non-delegable keys in a child (R-SYN-041, R-SCOPE-051/052/056)
}
/// Pure. Whole-file parse; on failure the scope is unresolved (R-SYN-033).
pub fn parse_control(path: &RepoPath, bytes: &[u8], is_root_dir: bool) -> Result<ControlFile, ConfigFailure>;
pub struct ConfigFailure { pub path: RepoPath, pub diags: Vec<Diagnostic> }

pub struct ControlSet {                                     // every control.toml found by the walk
    pub files: BTreeMap<RepoPath, Result<ControlFile, ConfigFailure>>,
}
pub fn load_controls(root: &Root, walk: &WalkResult) -> Result<ControlSet, Error>;      // I/O wrapper

// config/approvals.rs: fairlead/approvals.toml (D-34, R-CLI-037)
pub struct Approval { pub fp: Fingerprint, pub target: Id, pub scope: ScopeSlug, pub approver: String, pub date: jiff::civil::Date, pub renewal: u32 }
pub struct ApprovalsFile { pub approvals: Vec<Approval> }
pub fn parse_approvals(bytes: &[u8]) -> Result<ApprovalsFile, ConfigFailure>;
```

### 4.7 fairlead-core: cache (`cache/`)

```rust
/// Valid only for this tuple (Q-TOOL-1, R-ARCH-077). `as_of` is Some only for date-dependent entries.
pub struct CacheKey { pub fairlead_version: String, pub grammar: u32, pub config_hash: ContentHash, pub as_of: Option<AsOf> }
pub struct Cache { /* dir .fairlead/, enabled flag; every failure degrades to uncached (R-ARCH-072) */ }
impl Cache {
    pub fn open(root: &Root, enabled: bool, key: CacheKey) -> Cache;                 // infallible
    pub fn get_extraction(&self, h: &ContentHash) -> Option<Arc<Extraction>>;
    pub fn put_extraction(&self, h: &ContentHash, e: &Extraction);                    // atomic (R-ARCH-074)
    pub fn file_row(&self, p: &RepoPath) -> Option<FileRow>;                          // (size, mtime, hash)
    pub fn put_file_rows(&self, rows: &[(RepoPath, FileRow)]);
    pub fn token_estimate(&self, h: &ContentHash) -> Option<u32>;                      // R-CTX-043
    pub fn put_token_estimate(&self, h: &ContentHash, tokens: u32);
}
pub struct FileRow { pub size: u64, pub mtime_ns: i128, pub hash: ContentHash }
```

### 4.8 fairlead-core: registry (`registry/`)

Resolution is `registry::resolve`, pure, single-threaded, immutable result (R-ARCH-040, 061).

```rust
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ScopeId(pub ScopeSlug);                                     // identity is the id, never the path (R-SCOPE-002)
pub enum ScopeState { Resolved, Unresolved { reason: Vec<Diagnostic> }, AbsentFromTree }   // R-SCOPE-054, R-ARCH-026
pub struct Scope {
    pub id: ScopeId, pub dir: RepoPath, pub parent: Option<ScopeId>, pub depth: u32,
    pub control: Option<Arc<ControlFile>>, pub state: ScopeState, pub owners: Vec<Owner>,
}

/// Fully qualified item identity: home scope + kind + name. `Id` (syntax) is what was WRITTEN.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ItemId { pub scope: ScopeId, pub kind: Kind, pub name: Name }
impl ItemId { pub fn to_id(&self) -> Id; }                             // qualified form
pub enum Origin { ControlTable { control: RepoPath }, DocDef { path: RepoPath, span: Span } }   // D-40
pub struct Site { pub path: RepoPath, pub span: Span, pub extent: Extent, pub scope: ScopeId }   // code `def` site (R-SYN-010)
pub enum FieldValue { Scalar(String), List(Vec<String>) }
pub struct Item {
    pub id: ItemId, pub origin: Origin, pub policy: Policy, pub sum: Option<String>,
    pub fields: BTreeMap<String, FieldValue>,                          // declared (pre-override) values
}
pub struct Edge { pub from: Site, pub to: Id, pub sym: Option<String>, pub resolved: Option<ItemId> }  // `use` edges (R-SCOPE-060)

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct OvrId(pub Fingerprint);                                      // fingerprint of (target, scope, why, owner, exp) (R-CHK-026)
pub enum OvrOp { Loosen, Tighten, Set, Replace, Add, Remove }           // R-SCOPE-040
pub enum InertReason { SealedNoAllow, Incomplete, FloorRule, NonDelegable, UnknownField, DuplicateOvr, VersionExpiry, OutsideSubtree, UnknownOwner }
pub enum OvrState { Effective, Inert(InertReason), Expired, Stale }     // R-SCOPE-046/047
pub enum ApprovalStatus { NotRequired, Approved { renewal: u32 }, Missing }
pub struct Override {
    pub id: OvrId, pub target: ItemId, pub field: Option<String>, pub op: OvrOp,
    pub set: BTreeMap<String, FieldChange>,                             // FieldChange = Replace(FieldValue) | Append(Vec<String>) (R-SYN-013)
    pub why: Option<String>, pub owner: Option<String>, pub exp: Option<jiff::civil::Date>, pub base: Option<OvrId>,
    pub declared_in: ScopeId, pub applies: Option<String>, pub at: Location, pub state: OvrState, pub approval: ApprovalStatus,
    pub target_hash: Option<ContentHash>,                               // pin for EXC-STALE (R-SCOPE-047)
}
pub struct Allow { pub id: String, pub granting: ScopeId, pub target: ItemId, pub field: Option<String>, pub for_scopes: Vec<ScopeId>,
                   pub why: String, pub exp: jiff::civil::Date, pub valid: bool /* authored in home scope (R-SCOPE-036) */ }
pub struct Req { pub from: ScopeId, pub to: ScopeId, pub target: String, pub set: Option<String>, pub why: String, pub blocking: bool, pub at: Location }

pub struct Provenance { pub home: ScopeId, pub declared_at: Location, pub applied: Vec<OvrId>, pub grants: Vec<String> }   // R-ARCH-042
pub struct EffectiveField { pub value: FieldValue, pub provenance: Provenance }
pub struct EffectiveItem { pub id: ItemId, pub policy: Policy, pub fields: BTreeMap<String, EffectiveField> }
pub struct EffectiveView { pub scope: ScopeId, pub chain: Vec<ScopeId>, pub items: BTreeMap<ItemId, EffectiveItem>, pub authored_digest: ContentHash }

/// One exception ledger line (R-SCOPE-043/044), computed on read in 0.1.
pub struct LedgerEntry { pub ovr: OvrId, pub target: ItemId, pub home: ScopeId, pub varied_in: ScopeId, pub depth: u32,
                         pub why: Option<String>, pub owner: Option<String>, pub exp: Option<jiff::civil::Date>, pub state: OvrState }
pub struct LedgerView { pub for_scope: ScopeId, pub entries: Vec<LedgerEntry> }

pub enum Lookup { Found(ItemId), Ambiguous(Vec<ItemId>), Missing { nearest: Vec<Id> } }    // R-ARCH-062, R-CHK table HN-R001

pub struct ResolveInput<'a> {
    pub root: &'a Root, pub controls: &'a ControlSet, pub corpus: &'a Corpus,
    pub facts: &'a Facts,                 // approvals and gen/members shards on disk (HN-C007, HN-C008)
    pub as_of: AsOf,
}
/// Pure. Invalid lines are inert plus a Diagnostic, never an abort (R-ARCH-041, R-SCOPE-038).
pub fn resolve(input: &ResolveInput<'_>) -> Registry;

pub struct Registry { /* private; immutable (R-ARCH-061) */ }
impl Registry {
    pub fn root_scope(&self) -> &ScopeId;
    pub fn scopes(&self) -> impl Iterator<Item = &Scope>;               // sorted by id
    pub fn scope(&self, id: &ScopeId) -> Option<&Scope>;
    pub fn scope_of(&self, path: &RepoPath) -> Option<&ScopeId>;        // nearest ancestor-or-self scope (R-SCOPE-003)
    pub fn chain(&self, id: &ScopeId) -> Vec<&ScopeId>;                 // root ..= id
    pub fn items(&self) -> impl Iterator<Item = &Item>;
    pub fn item(&self, id: &ItemId) -> Option<&Item>;
    pub fn lookup(&self, from: &ScopeId, r: &Id) -> Lookup;
    pub fn view(&self, scope: &ScopeId) -> &EffectiveView;
    pub fn overrides(&self) -> &[Override];                             // sorted by (target, scope, at)
    pub fn grants(&self) -> &[Allow];
    pub fn requests(&self) -> &[Req];
    pub fn edges(&self) -> &[Edge];
    pub fn sites_of(&self, id: &ItemId) -> &[Site];
    pub fn ledger(&self, scope: &ScopeId) -> LedgerView;
    pub fn diagnostics(&self) -> &[Diagnostic];                         // structural diagnostics (Section 4.9 emitter table)
    pub fn as_of(&self) -> AsOf;
}
/// R-CHK-026: the approval/ovr fingerprint, pure.
pub fn ovr_fingerprint(target: &ItemId, scope: &ScopeId, why: &str, owner: &str, exp: Option<&jiff::civil::Date>) -> Fingerprint;
```

### 4.9 fairlead-core: check (`check/`)

```rust
pub enum Milestone { M01, M02, M03 }
pub const ACTIVE_MILESTONE: Milestone = Milestone::M01;                 // R-ARCH-055: later codes inactive
pub enum Emitter { Syntax, Config, Registry, Check, Generate }

/// One row per code of 13 (all milestones; numbering frozen, R-CHK-042). Static table in check/codes.rs.
pub struct DiagnosticSpec {
    pub code: DiagCode, pub mnemonic: &'static str, pub title: &'static str,
    pub default_severity: Severity, pub default_stage: Stage,
    pub floor: bool, pub waivable: bool, pub milestone: Milestone, pub emitter: Emitter,
}
pub fn spec(code: DiagCode) -> Option<&'static DiagnosticSpec>;
pub fn all_specs() -> &'static [DiagnosticSpec];                        // sorted by code
pub fn is_active(code: DiagCode) -> bool;

// diag.rs (P3-owned, re-exported by `check`): constants for every HN code + the pre-finding type
/// A pre-finding: what every stage before check emits (syntax, config, walk, registry).
pub struct Diagnostic {
    pub code: DiagCode, pub severity: Severity, pub location: Location, pub scope: Option<ScopeSlug>,
    pub rule: Option<String> /* "rule.<name>" */, pub related: Option<String>, pub message: String, pub expected: Option<String>,
    pub did_you_mean: Option<String>, pub hint: String, pub text: String /* offending text, for the fingerprint */,
}
impl From<SyntaxDiag> for Diagnostic {}
pub struct Location { pub path: RepoPath, pub line: u32, pub col: u32 }

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)] pub struct RuleId(pub Name);      // kind is always `rule`
pub enum FindingState { New, Baselined, Waived, WaiverIneffective }     // R-CHK-035
pub struct Finding {                                                    // R-CHK-035
    pub code: DiagCode, pub severity: Severity, pub stage: Stage, pub location: Location, pub scope: ScopeId,
    pub rule: Option<RuleId>, pub related: Option<String>, pub message: String,
    pub intent: Option<String>,        // the rule's `why`, <= 80 chars, data
    pub fix: String,                   // exactly one hint, <= 100 chars
    pub occurrence: u32, pub fingerprint: Fingerprint, pub state: FindingState, pub waived_by: Option<OvrId>,
}
/// Total order: (path, line, col, code), then message (R-CHK-011, R-ARCH-051).
impl Ord for Finding {}

// fingerprint.rs (R-CHK-031, R-ARCH-053)
pub struct FingerprintInput<'a> { pub code: DiagCode, pub rule: Option<&'a RuleId>, pub scope: &'a ScopeId,
                                  pub path: &'a RepoPath, pub text: &'a str, pub occurrence: u32 }
pub fn fingerprint(i: &FingerprintInput<'_>) -> Fingerprint;            // blake3, whitespace collapsed, 12 base32 chars
pub fn collapse_whitespace(s: &str) -> String;

// floor.rs: compiled in, not configurable (R-AGT-009, R-CHK-008, R-SCOPE-039)
pub fn is_floor(code: DiagCode) -> bool;

// rules.rs
pub struct DeclaredRule { pub id: RuleId, pub home: ScopeId, pub check: RuleCheck, pub sev: Severity, pub stage: Stage,
                          pub glob: Vec<String>, pub exclude: Vec<String>, pub waivable: bool, pub why: Option<String>, pub since: Option<jiff::civil::Date> }
pub enum RuleCheck { Leak { tokens: Vec<Id> }, Pattern { forbid: regex::Regex, msg: String } }
pub struct RuleSet { /* effective rules per scope, from Registry views */ }
impl RuleSet {
    pub fn collect(reg: &Registry) -> RuleSet;
    pub fn for_scope(&self, s: &ScopeId) -> &[DeclaredRule];
    pub fn applies(&self, r: &DeclaredRule, path: &RepoPath, reg: &Registry) -> bool;   // R-CHK-007, [[convention]] transparent
}
/// Built-in structural checks (one impl per file in check/builtin/). Pure.
pub trait Check: Sync {
    fn codes(&self) -> &'static [DiagCode];
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>);   // parallel per scope (R-ARCH-050)
}
pub fn builtin_checks() -> &'static [&'static dyn Check];

// baseline.rs  (R-CHK-030..032, R-CHK-052)
pub struct BaselineEntry { pub code: DiagCode, pub path: RepoPath, pub accepted: Option<jiff::civil::Date> }
pub struct Baseline { pub entries: BTreeMap<Fingerprint, BaselineEntry> }
impl Baseline {
    pub fn parse(bytes: &[u8]) -> Result<Baseline, BaselineIssue>;     // header-hash mismatch / floor entry => HN-B002
    pub fn render(&self, edition: Edition, fairlead_version: &str) -> String;
    pub fn contains(&self, fp: &Fingerprint) -> bool;
    /// `sync --baseline`: drop fixed entries, never add (R-CHK-032).
    pub fn prune(&self, current: &[Finding]) -> Baseline;
    /// `--accept-new --why`: refuses floor findings; authority-family findings need an approval (R-CHK-052).
    pub fn accept_new(&self, new: &[Finding], why: &str, as_of: AsOf, approvals: &ApprovalsFile) -> Result<Baseline, AcceptRefusal>;
}

// options.rs  (R-CHK-001, R-CLI-021..023)
pub struct CheckOptions {
    pub as_of: AsOf,
    pub changed: bool,                          // --changed (working tree + index vs HEAD, R-ARCH-075)
    pub files_from: Option<BTreeSet<RepoPath>>, // --files-from
    pub paths: Vec<RepoPath>,                   // [PATH...] report filter; global checks still whole-registry (R-CHK-002)
    pub rule_filter: Option<DiagCode>,          // --rule
    pub max: MaxFindings,                       // --max N; Default = 20, All = 0 (R-CHK-038)
    pub ramp: Ramp,
    pub for_agent: bool, pub no_baseline: bool, pub require_declarations: bool,
    pub budget: Option<core::time::Duration>,   // --budget DURATION (C-14)
}
pub struct Ramp { pub show_observe: bool, pub deny_warnings: bool, pub nudge_days: [u32; 2] /* [60, 120] (R-CHK table HN-X009) */ }
pub enum MaxFindings { Default, Limit(u32), All }

// mod.rs
pub struct CheckInput<'a> {
    pub registry: &'a Registry, pub corpus: &'a Corpus, pub facts: &'a Facts, pub load_diags: &'a [Diagnostic],
    pub baseline: Option<&'a Baseline>, pub options: &'a CheckOptions,
}
pub struct ReportHeader { pub fairlead_version: String, pub edition: Edition, pub as_of: AsOf, pub scopes: u32, pub files: u32, pub rules: u32 }
pub struct Summary {                           // R-CHK-028/029, R-AGT-033/034
    pub errors: u32, pub warns: u32, pub infos: u32, pub baselined: u32, pub waived: u32, pub observe: u32, pub accepted: u32,
    pub waivers: BTreeMap<ScopeId, WaiverCount>, pub relaxations: Vec<String>, pub exceptions_delta: i32,
    pub codeowners_uncovered: u32, pub requests_open: u32, pub oldest_request_days: Option<u32>,
}
pub struct CheckReport {
    pub header: ReportHeader,
    pub findings: Vec<Finding>,                // ALL findings, totally ordered; truncation is a render concern (R-ARCH-051)
    pub summary: Summary,
    pub partial: bool,                         // --budget deadline hit (R-CLI-023)
    pub unknown_scopes: Vec<ScopeId>,          // sparse/unresolved: verdict depends on them (R-CHK-057)
    pub zero_declarations: bool,               // HN-N001 (R-CHK-003)
}
/// Pure (R-ARCH-050). Stage x severity table of 13, baseline, waivers and ramp applied.
pub fn run(input: &CheckInput<'_>) -> CheckReport;
impl CheckReport {
    /// R-CHK-040 mapping, exactly as C-22.
    pub fn outcome(&self, opts: &CheckOptions) -> VerbOutcome;
}
/// Read-only bundle handed to each `Check` (everything pure).
pub struct CheckCx<'a> { pub registry: &'a Registry, pub corpus: &'a Corpus, pub facts: &'a Facts,
                         pub generated: &'a GeneratedSet, pub rules: &'a RuleSet, pub options: &'a CheckOptions }
```

Emitters before `check::run` (so P2 to P5 do not collide): P2 `HN-S001 S004 S005 S006 S007 S008 S010 S012 S013 S014 S015, HN-I001 I002 I003, HN-P001` (IDs). P3 `HN-C001 C002 C003 C004 C005 C006, HN-P001` (paths). P4 `HN-S002 S009 S011 S016, HN-C007 C008, HN-R005 R007, HN-D001, HN-X012 X014` (in `Registry::diagnostics`). P5 everything else, including `HN-C009 C011 C012 C013` and the override-state findings `X001 X002 X004 X005 X006 X010` derived from `OvrState`. The `DiagCode` constants for ALL codes live in `fairlead-core/src/diag.rs` (P3, mechanical transcription of 13); the `DiagnosticSpec` table in `check/codes.rs` (P5) references them.

### 4.10 fairlead-core: generate and render (`generate/`, `render/`)

```rust
// generate/ : pure, byte-exact (R-FILE-003, R-ARCH-091, R-CHK-023/024, C-17). check and sync call the same function.
pub enum GenKind { Shard(Kind), Exceptions, Members, Baseline, VendorLock }     // gen/reg/<kind>.reg, gen/reg/exc.reg, gen/members, gen/baseline.txt, vendored.lock (C-8)
pub struct GeneratedFile { pub path: RepoPath, pub scope: ScopeId, pub kind: GenKind, pub body: String }
impl GeneratedFile { pub fn render(&self, edition: Edition, fairlead_version: &str) -> Vec<u8>; }   // header `# scope <id> edition=<e> fairlead=<v> body=<BodyHash>` + sorted lines
pub struct GeneratedSet { pub files: BTreeMap<RepoPath, GeneratedFile> }
pub fn generate(reg: &Registry, corpus: &Corpus, facts: &Facts, baseline: Option<&Baseline>, edition: Edition, fairlead_version: &str) -> GeneratedSet;
pub enum HeaderVerdict { Ok, HandEdited, HasConflictMarkers }              // HN-G002 / regenerable (R-CHK-024)
pub fn verify_header(bytes: &[u8]) -> HeaderVerdict;
pub fn first_differing_lines(want: &[u8], have: &[u8], n: usize) -> Vec<String>;   // R-CHK-023: first three

// render/line.rs: shared terse `ID k=v` writer (R-CTX-030/031, D-23)
pub struct TerseLine { pub id: String, pub pairs: Vec<(String, TerseValue)> }   // TerseValue = Word(String) | Quoted(String)
impl TerseLine { pub fn render(&self) -> String; }                         // st=active omitted by the caller; quotes escaped
// render/sanitize.rs (R-CHK-037, R-AGT-017)
pub fn sanitize_data(text: &str, cap_chars: usize) -> String;               // strip control, ANSI, bidi; NFC; cap

// render/mod.rs
pub enum OutputFormat { Human, Terse, Json, Sarif }
pub struct RenderOptions { pub format: OutputFormat, pub for_agent: bool, pub max: MaxFindings, pub show_observe: bool, pub color: bool, pub quiet: bool }
pub trait Renderer {
    fn format(&self) -> OutputFormat;
    /// LF-only, repo-relative `/` paths, no timestamps (R-ARCH-052). Truncation/grouping per R-CHK-038/039 happens here.
    fn check(&self, report: &CheckReport, opts: &RenderOptions) -> String;
    fn context(&self, out: &ContextOutput) -> Option<String>;             // Terse and Json only; None otherwise
    fn explain(&self, out: &ExplainOutput) -> Option<String>;
}
pub fn renderer(format: OutputFormat) -> &'static dyn Renderer;
// json.rs: schema = { fairlead, edition, as_of, findings[], summary } (R-CHK-036). sarif.rs: hand-built serde subset, SARIF 2.1.0.
```

### 4.11 fairlead-core: context (`context/`)

```rust
pub enum Tier { L0Index, L1ScopeCard, L2Detail, L3Archive /* inactive in 0.1 */ }
pub enum Priority { P0Header, P1ErrorRules, P2Drift, P3Items, P4WarnRules, P5Recent /* 0.2 */, P6SiblingsChildren, P7AncestorL1 }   // table in 14
pub enum Budget { Default, Tokens(u32), Unlimited }                         // --budget N; 0 = Unlimited (R-CTX-044)
pub struct TokenEstimate { pub tokens: u32, pub exact: bool }               // ceil(bytes / bpt) (R-ARCH-056, R-CTX-043)
pub fn estimate_bytes(bytes: usize, bytes_per_token: u32) -> TokenEstimate;
pub enum ContextTarget { SessionStart, Path(RepoPath), IdLookup(Id) }       // IdLookup is forwarded to explain (R-CTX-004)
pub enum ContextArgError { NoMatchTaskQuery, OutsideRoot }                   // both exit 2 (R-CTX-003/004)
/// Pure classification given a "path exists" predicate (R-CLI-032, C-13).
pub fn parse_target(arg: Option<&str>, exists: &dyn Fn(&RepoPath) -> bool, root_rel: &dyn Fn(&str) -> Result<RepoPath, ContextArgError>)
    -> Result<ContextTarget, ContextArgError>;
pub struct ContextRequest {
    pub target: ContextTarget, pub budget: Budget, pub scope_pin: Option<ScopeId>, pub full: bool,
    pub depth: Option<u32>, pub digest_only: bool, pub exact_tokens: bool, pub as_of: AsOf,
}
pub struct ContextInput<'a> { pub registry: &'a Registry, pub corpus: &'a Corpus, pub report: &'a CheckReport,
                              pub config: &'a ContextConfig, pub request: &'a ContextRequest, pub cache: &'a Cache }
pub struct ContextBrief {                       // one structure => identical text and JSON ordering (R-CTX-033, C-15)
    pub header: BriefHeader,                    // scope, effective digest, edition, ancestor chain (P0)
    pub purpose: Option<String>,
    pub inh: Vec<InhLine>,                      // `^root dq3f2xbr 6err` (R-CTX-020)
    pub rules: Vec<BriefRule>,                  // introduced/changed/overridden at or below (R-CTX-020/021)
    pub uses: Vec<String>, pub drift: Vec<DriftLine>, pub waivers: u32,
    pub children: Vec<L0Line>, pub handoff: Option<String>,                   // R-CTX-008
    pub omitted: Vec<Omission>,                 // every drop prints `+N more -> fairlead explain <x>` (R-CTX-040)
    pub footer: BriefFooter,                    // `=d <digest>` and `next:` (R-CTX-050)
}
pub struct ContextOutput { pub brief: ContextBrief, pub tokens: TokenEstimate, pub status: ContextStatus }
pub enum ContextStatus { Ok, TruncatedMandatory { min_budget: u32 } }       // exit 2 (R-CTX-041)
/// Pure.
pub fn build(input: &ContextInput<'_>) -> ContextOutput;
/// Merkle chain: H(parent effective digest, own authored digest, generator version) (14, Digest chain).
pub fn effective_digest(parent: Option<&Digest>, own: &ContentHash, generator_version: &str) -> Digest;
```

### 4.12 fairlead-core: write layer and verbs (`write/`, `explain.rs`)

```rust
pub enum WriteKind { Create, Replace, AppendBlock { marker: String } }       // AppendBlock = marker-delimited append (R-CLI-015/016)
pub struct PlannedWrite { pub path: RepoPath, pub kind: WriteKind, pub bytes: Vec<u8>, pub class: FileClass, pub expect_prior: Option<ContentHash> }
pub enum WriteSetKind { Init, Sync, New, Approve }                           // allowed paths per verb (R-ARCH-140..141, R-CLI-046)
pub struct WriteSet { pub kind: WriteSetKind, pub writes: BTreeMap<RepoPath, PlannedWrite> }
impl WriteSet { pub fn add(&mut self, w: PlannedWrite) -> Result<(), Error>; pub fn validate(&self, root: &Root) -> Result<(), Error>; }  // secret scan (R-ARCH-144), path allow-list, never .git/config

pub struct Plan { pub verb: WriteSetKind, pub set: WriteSet, pub lines: Vec<PlanLine>, pub plan_hash: ContentHash, pub notes: Vec<String> }
pub struct PlanLine { pub action: PlanAction /* Create|Edit|Skip|Quarantine */, pub path: RepoPath, pub detail: String, pub diff: Option<String> }

pub trait PidLiveness { fn is_alive(&self, pid: u32, host: &str) -> LivenessAnswer; }   // Dead | Alive | OtherHost | Unknown
pub struct WriterLock<'a> { /* .fairlead/lock holding pid, host, start time; released on drop (R-ARCH-146) */ }
impl<'a> WriterLock<'a> {
    pub fn acquire(root: &Root, probe: &'a dyn PidLiveness) -> Result<WriterLock<'a>, Error>;  // Refused(LockHeld) names holder; stale same-host lock taken over with a notice
}
pub struct Journal { /* .fairlead/sync.journal */ }
pub enum Recovery { Clean, Resumed, RolledBack }
pub struct Transaction<'l> { /* staged temp files, then atomic renames with bounded Windows retry (R-ARCH-145) */ }
impl<'l> Transaction<'l> {
    pub fn begin(root: &Root, lock: &'l WriterLock<'_>, set: WriteSet) -> Result<Self, Error>;
    pub fn recover(root: &Root, lock: &WriterLock<'_>) -> Result<Recovery, Error>;     // called first by every writer
    pub fn commit(self) -> Result<CommitReport, Error>;                                  // idempotent on re-run (R-ARCH-143)
}
pub struct CommitReport { pub written: Vec<RepoPath>, pub unchanged: Vec<RepoPath> }

// Verb planners: pure given their inputs; they never touch disk.
pub struct InitRequest { pub preset: Preset /* None|Rust|Ts */, pub no_baseline: bool, pub hooks: bool, pub codeowners: bool, pub agents: Vec<String>, pub yes: bool }
pub fn init_plan(req: &InitRequest, detected: &InitDetection, first_scan: &CheckReport) -> Result<Plan, Error>;   // <= 3 created files (R-CLI-013)
pub struct NewRequest { pub kind: String, pub name: String, pub scope: Option<ScopeId>, pub fields: BTreeMap<String, String>, pub print: bool, pub at: Option<RepoPath>, pub hoist: bool }
pub fn new_plan(req: &NewRequest, reg: &Registry, facts: &Facts, as_of: AsOf) -> Result<Plan, Error>;              // never overwrites (R-CLI-028)
pub struct SyncRequest { pub baseline: BaselineMode, pub accept_vendor: bool, pub force: bool, pub scope: Option<ScopeId> }
pub enum BaselineMode { Off, Prune, AcceptNew { why: String } }
pub fn sync_plan(req: &SyncRequest, input: &CheckInput<'_>, report: &CheckReport, edition: Edition, fairlead_version: &str) -> Result<Plan, Error>;
pub struct ApproveRequest { pub target: String, pub as_handle: Option<String>, pub list: bool, pub revoke: Option<Fingerprint> }
pub struct ApprovePreview { pub fp: Fingerprint, pub ovr_line: String, pub scope: ScopeId, pub exp: Option<jiff::civil::Date>, pub confirm_prefix: String /* first 4 chars */ }
pub fn approve_preview(req: &ApproveRequest, reg: &Registry, facts: &Facts) -> Result<ApprovePreview, Error>;
pub fn approve_plan(p: &ApprovePreview, approver: &str, as_of: AsOf, existing: &ApprovalsFile) -> Result<Plan, Error>;  // TTY/CI gate is the bin's (R-CLI-037), checked again via Refusal kinds
pub fn plan_demote(reg: &Registry, scope: &ScopeId) -> Result<Plan, Error>;   // C-9: library only in 0.1 (R-SCOPE-014)

// explain.rs (R-CLI-035). Topic text is embedded in the binary.
pub enum ExplainTarget { File(RepoPath), Id(Id), Code(DiagCode), Topic(String) }          // dispatch order C-21
pub fn parse_explain_target(arg: &str, exists: &dyn Fn(&RepoPath) -> bool) -> Result<ExplainTarget, Error>;
pub struct ExplainOutput { pub target: ExplainTarget, pub sections: Vec<ExplainSection> }   // prose + terse lines, all sanitised data
pub fn explain(target: &ExplainTarget, reg: &Registry, report: Option<&CheckReport>) -> Result<ExplainOutput, Error>;
```

### 4.13 fairlead-core: pipeline; fairlead bin

```rust
// pipeline.rs (P8): the ONLY place stages are chained. No logic of its own beyond sequencing (R-ARCH-001).
pub struct LoadRequest { pub discover: DiscoverRequest, pub use_cache: bool, pub threads: Option<usize>, pub want_changed: bool, pub only: Option<BTreeSet<RepoPath>> }
pub struct Loaded { pub root: Root, pub controls: ControlSet, pub walk: WalkResult, pub corpus: Corpus, pub facts: Facts, pub load_diags: Vec<Diagnostic>, pub cache: Cache }
pub fn load(req: &LoadRequest) -> Result<Loaded, Error>;                              // discover -> walk -> controls -> extract -> facts
pub fn resolve_loaded(l: &Loaded, as_of: AsOf) -> Registry;                           // registry::resolve
pub fn check_loaded(l: &Loaded, reg: &Registry, opts: &CheckOptions) -> Result<CheckReport, Error>;   // loads baseline, check::run
pub fn context_loaded(l: &Loaded, reg: &Registry, rep: &CheckReport, req: &ContextRequest) -> ContextOutput;
```

```rust
// fairlead/src/cli.rs  (R-CLI-001, R-CLI-006). Reserved verbs parse but exit 2 "<verb> arrives in 0.2" via commands/reserved.rs.
pub struct GlobalArgs { pub root: Option<Utf8PathBuf>, pub cwd: Option<Utf8PathBuf> /* -C */, pub format: Option<OutputFormat>, pub quiet: bool,
                        pub color: ColorArg, pub as_of: Option<AsOf>, pub no_cache: bool, pub threads: Option<usize> /* C-11 */ }
pub enum Command { Init(InitArgs), Check(CheckArgs), Sync(SyncArgs), New(NewArgs), Context(ContextArgs), Explain(ExplainArgs),
                   Approve(ApproveArgs), Move(ReservedArgs), Seal(ReservedArgs), Upgrade(ReservedArgs) }
// fairlead/src/commands/*.rs
pub struct CommandResult { pub stdout: String, pub stderr: String, pub outcome: VerbOutcome }
// fairlead/src/exit.rs
pub fn to_process_exit(c: ExitCode) -> std::process::ExitCode;
pub fn exit_for_error(e: &Error) -> ExitCode;      // Usage->2, Refused->4, Internal->5; panics caught in main -> 5
```

---

## 5. Data flow

```text
discover -> walk -> load_controls -> classify+extract -> collect_facts -> resolve -> check -> render
  (I/O)     (I/O)      (I/O+pure)       (I/O+rayon)         (I/O)         (pure)    (pure)   (pure)
                                                                              \-> context::build -> render::context   (pure)
                                                                              \-> generate -> sync_plan -> Transaction   (pure -> I/O)
```

| # | Seam (producer -> consumer) | Function | Passes | Pure? |
|---|---|---|---|---|
| S1 | cli -> discover | `discover::discover(&DiscoverRequest) -> Result<Discovered, Error>` | `Root`, root `ControlFile` | I/O; `pin_check`, `search_upward` pure |
| S2 | discover -> walk | `walk::walk(&Root, &WalkOptions) -> Result<WalkResult, Error>` | sorted `WalkEntry` list | I/O |
| S3 | walk -> config | `config::load_controls(&Root, &WalkResult) -> Result<ControlSet, Error>` over `parse_control(&RepoPath, &[u8], bool)` | `ControlSet` | parse pure, load I/O |
| S4 | controls+walk -> extract | `extract::extract_files(&WalkResult, &Root, &ExtractPlan, &Cache, Option<&BTreeSet<RepoPath>>) -> Result<Corpus, Error>` over `extract_file` and `fairlead_syntax::extract` | `Corpus` of `FileRecord` | file read I/O, `extract_file` pure |
| S5 | walk+controls -> facts | `facts::collect_facts(&Root, &WalkResult, &ControlSet, bool) -> Result<Facts, Error>` | `Facts` | I/O (disk, git) |
| S6 | all -> registry | `registry::resolve(&ResolveInput) -> Registry` | `Registry` (+ `Diagnostic`s) | pure |
| S7 | registry -> check | `check::run(&CheckInput) -> CheckReport` | `CheckReport` (all findings sorted) | pure |
| S8 | check -> render | `Renderer::check(&CheckReport, &RenderOptions) -> String` | bytes (LF) | pure |
| S9 | registry+check -> context | `context::build(&ContextInput) -> ContextOutput`, then `Renderer::context` | `ContextOutput` | pure |
| S10 | registry -> generate | `generate::generate(...) -> GeneratedSet`; used by `check` (HN-G001/G002) and `sync_plan` | `GeneratedSet` | pure |
| S11 | check+generate -> write | `write::sync_plan / init_plan / new_plan / approve_plan -> Plan`; `Transaction::begin(...).commit()` | `WriteSet` -> disk | plan pure, commit I/O |
| S12 | everything -> exit | `CheckReport::outcome`, `VerbOutcome`, `exit_for_error` | `ExitCode` | pure |

Rules of the flow:
- Only S1 to S5 and S11 (commit) and the cache touch the disk or git. `registry`, `check`, `generate`, `context`, `render`, the planners and `explain` take plain values and are testable without a filesystem (Section 7).
- Order inside S4: the root `ControlFile` supplies `marker`, `[comments]`, `[scan]` for the `ExtractConfig`; child `[[vendor]]` and `[[convention]]` come from S3, so S3 completes before S4. Control files and approvals are read by S3/S5, never by the extractor.
- `check` never reads a file: HN-G001/G002 compare `generate(...)` against `Facts.generated_on_disk`; HN-T001 uses `FileRecord.literals`; HN-F001 uses `FileRecord.secrets`; HN-A001/A002 use `Facts.agent_files` and `Facts.codeowners`; HN-V013 uses `Facts.git.sparse_absent`.
- `context` takes a `CheckReport` built with `CheckOptions.paths = [queried path]` so only that path's drift is rechecked while global checks stay whole-registry (R-CTX-013, R-CHK-002).
- `sync_plan` is the only consumer of `Baseline::prune/accept_new`; `check` only calls `Baseline::parse/contains`.
- Parallelism: S2 and S4 (rayon / ignore parallel), `check` per scope; results are gathered and sorted before crossing the seam (R-ARCH-024, R-ARCH-080). `GitRepo` is opened per worker (R-ARCH-081).
- Cache: S4 and `context` consult it; no other stage may.

---

## 6. Phase ownership

Phases: **P1** bootstrap and CI; **P2** `fairlead-syntax`; **P3** core foundation; **P4** registry and resolve; **P5** check, baseline, generate, render, secrets; **P6** context; **P7** write layer, `init`/`new`/`sync`/`approve`/`explain`; **P8** CLI integration and end-to-end; **P9** quality. Build order: P1, then P2 and P3 in parallel (shared types are fixed here), P4, then P5/P6/P7 in parallel, P8, P9 (P9 harnesses start in P1 and are completed last). A rule listed under a phase is implemented there; where a test belongs to another phase it is noted in Section 7.

**Modules by phase.** P1: workspace files, `.github/workflows/*`, `xtask`, `deny.toml`, lints. P2: Section 3 fairlead-syntax. P3: `error outcome diag time hash path discover walk classify extract(mod,styles) manifest git facts config cache`. P4: `registry/*`. P5: `secrets check/* generate/* render/* extract/literals`. P6: `context/*`. P7: `explain write/*`. P8: `pipeline`, whole `fairlead` bin. P9: `tests/*`, `fixtures/*`, `fuzz/*`, benches, `bench.yml`, `fuzz.yml`, `dogfood.yml`, `fairlead/` dogfood declarations.

**Verbs by phase.** `check` P5 (library) + P8 (bin); `context` P6 + P8; `explain`, `init`, `new`, `sync`, `approve` P7 + P8.

**Rule assignment** (every [0.1] rule of chapters 10, 11, 12, 13, 14, 15, 40, 50 exactly once; counts: 59, 43, 48, 51, 29, 41, 27, 77 = 375). Numbers are the suffix of the rule ID in that chapter's family: SCOPE (10), FILE (11), SYN (12), CHK (13), CTX (14), CLI (15), AGT (40), ARCH (50).

| Phase | Rules (family number list) | n |
|---|---|---|
| **P1** | ARCH 002 003 007 060 110 113 120 121 122 123 152; CLI 009; AGT 007 | 13 |
| **P2** | SYN 001-009 012 014-023 027-032 034-037 044 045 046 048; FILE 013 015 042; ARCH 031 032 033 | 40 |
| **P3** | SCOPE 004 006 007 016 018 051 052 056 065; FILE 001 004 005 009 014 020 023 024 031 034 044 050 054; SYN 038-041 050 051 053; CHK 005 013 014; CLI 004 005 010; AGT 013 018; ARCH 005 006 010-013 020-026 030 070 072-074 076 077 081 100 102 | 60 |
| **P4** | SCOPE 001-003 005 012 015 020-027 030-038 040 041 043 045-047 049 053-055 058 060 064; FILE 037 043 052; SYN 011 013 024-026 033; CHK 006 025 026 053; AGT 012; ARCH 040-043 061 062 104 | 57 |
| **P5** | SCOPE 011 014 017 028 039 048; FILE 003 007 008 011 012 022 030 033 036 038-041 045 051 053; SYN 010; CHK 001-004 007-011 017-024 028-033 035-039 042 046 049 050 052 055 057 060; CLI 022-024; AGT 002 009 010 011 015 017 022 023 024 032 033 034; ARCH 050-055 075 090 091 103 | 84 |
| **P6** | CTX 001-004 006-008 010-013 020-022 030-033 040-046 050 070 071; CLI 033 034; AGT 016; ARCH 056 | 32 |
| **P7** | SCOPE 010 013 042 044 050 061-063; FILE 002 010 021 025-027 029 032; CHK 027 041 056; CLI 007 008 012-020 025-031 035 037-039 046; AGT 001 003 004 008 019 025 026 028; ARCH 112 141-146 | 57 |
| **P8** | CHK 040; CLI 001 002 003 006 011 021 032 043 045; ARCH 001 082 101 | 13 |
| **P9** | CHK 012 015 016 051; CTX 072; AGT 005 006; ARCH 071 080 111 130 131 140 150 151 153 160 161 162 | 19 |

Sanity: 13+40+60+57+84+32+57+13+19 = 375.

Notes on split ownership (the owner implements; the others only consume):
- Behaviour that several phases must satisfy but one phase proves: ARCH-071, 080, 111, 140 and CHK-012, 015 are proved by P9 tests; the implementing phases (P3 cache, P5 sort, P7 write-set) must not violate them.
- ARCH-075 (`--changed`) is P5; it consumes `Facts.git.changed` from P3. FILE-040/041 (vendor drift) are detected in P5; the lock write (`--accept-vendor`) is P7's `sync_plan`.
- SCOPE-014: P5 raises `HN-C009`; the tombstone writer is `plan_demote` (P7 code, no CLI verb in 0.1, C-9). SCOPE-004: P3 also raises `HN-C004` (ROOT-DUP) from the `ControlSet`.
- CHK-032 and CHK-052 are the pure `Baseline` methods (P5); CLI-025 (`sync`) calls them (P7).
- CHK-053: P4 raises `HN-X012`; `HN-A004` is raised in P5 under AGT-010.

**Adjacent [0.1] rules outside the audited chapters** (assigned so nothing is orphaned): R-EDN-001 P7; R-GIT-001 P9; R-GIT-007 P5 (`HN-V013`, facts from P3); R-VAR-091 P5; R-PORT-001 P7; R-PORT-002/003/008 P5 (`HN-M001`-`M003`, C-35); R-PORT-007 P4; R-PORT-009 none (AST-free); R-PORT-014 P9; R-PORT-015 P7; R-PORT-016 P3.

**Unassigned:** none. Rules with no executable content (ARCH-003, FILE-045, CLI-039, AGT-007, AGT-015) are tracked under the phase shown and satisfied by documentation or by the absence of an API.

---

## 7. Test conventions

**Where tests live.**

| Kind | Location | Naming | Needs disk? |
|---|---|---|---|
| Unit | `#[cfg(test)] mod tests` at the bottom of the module file | `fn r_syn_016__file_form_after_license()` (rule id in snake case, `__`, case) | no |
| Crate integration, pure | `crates/<crate>/tests/<topic>.rs` using the public API with in-memory inputs (`ControlFile`s built by `parse_control`, `FileRecord`s by `extract_file`) | `<topic>.rs` | no |
| Syntax conformance | `crates/fairlead-syntax/tests/conformance.rs` iterating `fixtures/syntax/<host>/<case>.<ext>` | case file + sibling `<case>.expect` (terse list of `verb ids extent` lines and diag codes) | reads fixtures |
| Snapshot | `insta`, snapshots committed under `crates/<crate>/tests/snapshots/` | `<CODE>__<case>` for diagnostics (one per code, R-ARCH-151); `context__<case>`, `explain__<case>`, `gen__<file>` | no |
| Property | `crates/fairlead-core/tests/prop_<invariant>.rs` (`proptest`) | `prop_resolution_order_independent`, `prop_baseline_fixed_point`, `prop_promote_demote_roundtrip` (R-SCOPE-012) | no |
| CLI contract | `tests/cli/*.trycmd` run by `tests/cli.rs` (`trycmd`) | one file per verb and flag family | temp dir |
| Golden repository | `tests/golden/<name>/` run by `tests/golden.rs` | directory name = fixture name from 50 Section 14 | temp copy |
| Determinism, read-only, SARIF, secrets, replay, platform | `tests/{determinism,readonly,sarif,secrets,replay,platform}.rs` | | temp copy |
| Fuzz | `fuzz/fuzz_targets/*.rs`, 60 s bound in `fuzz.yml` (R-ARCH-150); seeds copied from `fixtures/corpus/` | | |
| Benchmarks | `crates/fairlead-core/benches/*.rs`, corpus from `xtask bench-corpus` | | generated |

**Test targets of the bin package.** `crates/fairlead/Cargo.toml` declares `[[test]] name = "golden" path = "../../tests/golden.rs"` and likewise for `determinism`, `readonly`, `sarif`, `secrets`, `replay`, `platform`, `cli`. This makes `env!("CARGO_BIN_EXE_fairlead")` and `trycmd`'s binary lookup available without any test ever invoking `cargo` or `rustc` (rule: tests spawn only the built `fairlead` binary and `git` for the read-only fixtures that need a repository). No Cargo workspace is built inside a fixture; fixture `Cargo.toml` files are data and the workspace `exclude` list keeps cargo from seeing them.

**Golden repository declaration.** `tests/golden/<name>/` contains `repo/` (the tree, copied to a temp directory per run; `repo/dot-git/` is renamed to `.git` and `git init`-ed if `needs_git = true`), `expected/` (stdout and generated files) and `golden.toml`:

```toml
[golden]
name = "nested-scopes-ovr"
platforms = ["linux", "windows", "macos"]
needs_git = false
line_endings = "lf"            # "lf" | "crlf": the harness converts text files before the run

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
cwd = "."                      # relative to repo/
exit = 1
stdout = "expected/check.terse.txt"      # byte-compared; re-blessed only by `cargo xtask golden-bless <name>`
git_status_clean = true        # R-ARCH-140 for reader verbs
```

A `[[case.edit]]` table (`path`, `append`) before a case mutates the temp copy. Mandatory golden set (R-ARCH-153): `ts-react`, `rust-workspace`, `fullstack-tauri`, `legacy-baseline`, `crlf-and-windows-names`, `nested-scopes-ovr`, `unparseable-control`, `zero-rules`, `sparse-checkout`, `legacy-encoding`, `two-writers`.

**Helper (module, not a crate).** `tests/testkit/mod.rs`: `RepoBuilder::new().file(path, text).crlf(bool).control_root(|c| ...).scope("web", dir, text).git_init(bool).build() -> TempRepo`; `TempRepo::run(&[&str]) -> Run { code, stdout, stderr }` (spawns `CARGO_BIN_EXE_fairlead` with `NO_COLOR=1`, `FAIRLEAD_ROOT` cleared, `RAYON_NUM_THREADS` settable), `git_status()`, `tree_hash()`. Pure tests in `fairlead-core` do NOT use it: they build inputs in memory (`parse_control`, `extract_file`) and call `resolve`, `check::run`, `context::build`, `generate` and the planners directly.

**Rules for every phase.** Each rule in its Section 6 row has at least one test whose name starts with the snake-case rule id. A bug fix adds the regression fixture first (R-ARCH-152). Snapshots are reviewed; CI runs with `INSTA_UPDATE=no`. Time is never read in tests: pass `AsOf`.

---

## 8. Cross-phase invariants

1. **Determinism.** Same tree, config, binary, `--as-of` give the same bytes: cold/warm/deleted cache (R-ARCH-071), `RAYON_NUM_THREADS=1` and default (R-ARCH-080), LF and CRLF checkouts (R-ARCH-111), any working directory (R-ARCH-043). Every collection that reaches output is ordered; `HashMap`/`HashSet` are lint-forbidden in `fairlead-syntax` and `fairlead-core`.
2. **LF and paths.** All output and all written files are LF-only, end with one newline, carry repo-relative forward-slash UTF-8 paths, no timestamps, no absolute paths, no machine data (R-ARCH-052, R-ARCH-091, R-ARCH-112). Hashes are over `lf_bytes`.
3. **Sorted order.** Walk result bytewise by path; items bytewise by `Id` text; findings by (path, line, col, code, message); generated lines bytewise by ID; fingerprints bytewise in the baseline.
4. **No panics, no loops.** `fairlead_syntax::extract` and `parse_control` accept any bytes. A worker panic is caught in `fairlead::main` and reported as exit 5; success is never reported after a panic (R-ARCH-101). Library code never calls `process::exit`.
5. **Exit-code rule.** Only `ExitCode` (0 to 5) from `fairlead_core::outcome`; precedence 5, 2, 4, 1, 3, 0 via `ExitCode::resolve`; core returns `Error`/`VerbOutcome`, the bin maps (R-ARCH-100). A result that cannot be determined (unreadable git under `--changed`, sparse absent scope) is never 0.
6. **Read-only readers.** `check`, `context`, `explain` write only under `.fairlead/` and never take the writer lock (R-ARCH-140, R-ARCH-074). The only write path in the product is `write::Transaction`, which is only reachable from a `Plan` built for `Init|Sync|New|Approve`.
7. **Fail closed.** Unparseable control file: scope subtree `Unresolved`, `check` exits 2 (via `Error::Config` or `unknown_scopes`), no fallback to ancestor rules (R-SYN-033). Malformed directive: removed from the registry with a diagnostic, never silently dropped (R-ARCH-031).
8. **Data not instructions.** Every repository string that reaches output passes `render::sanitize::sanitize_data`; secrets are redacted (`secrets::redact`) before printing (R-AGT-015/017/023).
9. **Time.** The only clock reads are `AsOf::today_utc` and the `--budget` deadline.
10. **Diagnostic codes** are `DiagCode` constants (`fairlead-syntax::diag::codes` for `HN-S*`/`HN-I*`, `fairlead-core::diag::codes` re-exporting those and adding the rest); the `DiagnosticSpec` table in `check/codes.rs` has one row per code; no string literal `"HN-..."` anywhere else.

**Handoff seams (producer phase -> type -> consumer phases).**

| From -> to | Type(s) crossing |
|---|---|
| P2 -> P3 | `Extraction`, `Directive`, `ExtractConfig`, `CommentTable`, `Frontmatter`, `SyntaxDiag`, `DiagCode`, `Severity`, `Span`, `Id`, `ScopeSlug`, `Kind`, `Name`, `PLACEHOLDER` |
| P3 -> P4 | `Root`, `ControlSet`, `ControlFile`, `ConfigFailure`, `Corpus`, `FileRecord`, `FileClass`, `ApprovalsFile`, `RepoPath`, `AsOf`, `ContentHash`, `Diagnostic` (as structs only) |
| P3 -> P5/P6/P7 | `Facts`, `GitFacts`, `WalkResult`, `Cache`, `Error`, `ExitCode`, `VerbOutcome` |
| P4 -> P5/P6/P7 | `Registry` (+ `ScopeId`, `ItemId`, `Item`, `Override`, `OvrState`, `Allow`, `Req`, `EffectiveView`, `LedgerView`, `Lookup`, `Edge`, `Site`) |
| P5 -> P6 | `CheckReport`, `Finding`, `Summary`, `sanitize_data`, `TerseLine`, `secrets::redact` |
| P5 -> P7 | `GeneratedSet`, `Baseline`, `CheckInput`, `CheckReport`, `verify_header`, `scan_secrets` |
| P6, P7, P5 -> P8 | `ContextOutput`, `Plan`, `Transaction`, `WriterLock`, `ExplainOutput`, `Renderer`, `CheckOptions`, `ContextRequest`, `OutputFormat` |
| P8 -> P9 | the `fairlead` binary, `pipeline::*`, all golden/trycmd harnesses |

---

## 9. Open gaps and contract decisions

Each item closes an ambiguity by the most consistent choice. Rule IDs name the source of the gap.

- **C-1 (R-ARCH-007).** Product crates stay exactly three; `xtask` and `fuzz/` are tooling, the test helper is a module.
- **C-2 (R-ARCH table 3).** `fairlead-syntax` also needs `serde` (cached records) and `unicode-normalization`; `fairlead-core` also `regex` and `thiserror`. UNVERIFIED until the executing engineer checks them (R-ARCH-120).
- **C-3 (R-ARCH-005, P3 list).** The `git` module and `facts` module belong to P3 although the phase list names neither; P5 only consumes `Facts.git`.
- **C-4 (R-SYN-038, schema index).** Every control file has a `[settings]` table. The root may set all keys; a non-root file may set only `purpose` (anything else there is `HN-C003`). `owners` is read from the root only in 0.1.
- **C-5 (R-SYN-024, R-SYN-009, R-SYN-013).** Extraction is context-free (R-ARCH-030), so it checks only lexical validity. Unknown kind (`HN-S002`), unknown key on a directive without `!` (`HN-S011`; built-ins from `known_keys`, plus the kind's `[kinds.K.keys]`), `ovr` of a field the item lacks (`HN-S009`) and incomplete `ovr` (`HN-S016`) are raised by `resolve` in P4.
- **C-6 (R-SCOPE-065, R-FILE-023, R-FILE-031).** No code exists for: a control file declaring `[kinds.exc]`; a managerial path matched by an ignore rule; `source = "generate"` with no generator (none ships in 0.1) or on a vendor-owned file. All three are `HN-C001` (error, floor) with a distinguishing message.
- **C-7 (R-CHK-031, R-CTX).** Base32 is the lowercase Crockford alphabet, hand-implemented (no crate). Fingerprint = first 60 bits of blake3 as 12 characters; context digest = first 40 bits as 8 characters.
- **C-8 (R-CLI-046, R-FILE-020/022, R-SCOPE-044).** The 0.1 `sync` write-set, concretely: `<folder>/gen/reg/<kind>.reg` per declared kind with items (sharded `<kind>-<bucket>.reg` above 500 lines), `<folder>/gen/reg/exc.reg` (the scope's own ledger facts), `<folder>/gen/members`, `<folder>/vendored.lock` (only when `[[vendor]]` entries exist), `<folder>/gen/baseline.txt` (`--baseline`), and the cache. No ref-graph file: `use` edges are computed on read.
- **C-9 (R-SCOPE-014).** No 0.1 verb demotes a scope. `HN-C009` detects deletion; the tombstone-writing plan exists as `write::plan_demote`, exercised by tests only; the verb is `move --demote` (0.2).
- **C-10 (R-ARCH section 9).** Unreadable required file or failed write: `Error::Io`; NotFound on a user-supplied path is class Usage (2), everything else is Internal (5). A held lock is `Refused` (4).
- **C-11 (R-ARCH-082).** Global flag `--threads N` and env `FAIRLEAD_THREADS`, default = physical cores. Not listed in 15's flag table: spec amendment required.
- **C-12 (R-CLI-005, D-50(4)).** `fairlead-version = "0.1"` is padded to `0.1.0`; the binary must be >= the pin and have the same edition.
- **C-13 (R-CLI-032, R-CTX-004).** Context argument: an existing path (relative to the cwd, then to the root) wins; else a string matching the ID grammar is an `IdLookup`; else exit 2 `no match: task queries need 0.3; pass a path or an ID`.
- **C-14 (R-CLI-023).** `--budget DURATION` grammar: integer plus `ms|s|m` (`90s`, `2m`). A hit deadline sets `partial = true` and the outcome rule of R-CLI-023.
- **C-15 (R-CTX-033).** Text and JSON context output are two renderings of one `ContextBrief`, so order is identical by construction.
- **C-16 (R-ARCH-033, R-ARCH-103).** Diagnostic columns count Unicode scalar values in the original line; offsets are original bytes.
- **C-17 (R-FILE-003, R-CHK-024, R-CHK-030).** Generated header: `# scope <id> edition=<e> fairlead=<v> body=<20 base32 chars>`; baseline header `# baseline edition=<e> fairlead=<v> body=<hash>`; entries `<fp> code=<code> path=<path> [acc=<date>]`, sorted by fingerprint. A file with conflict markers is regenerable (`HeaderVerdict::HasConflictMarkers`).
- **C-18 (R-CHK-005, schema `[exceptions]`).** Two caps exist: `[policies.waiver] max-exp` (HN-X006) and `[exceptions] max-days`; the effective horizon is the smaller of those set, default 90 days.
- **C-19 (R-CHK-025).** `[policies.waiver] approval` defaults to required for `needs-exception`; `open` needs none; a `sealed` item with an `[[allow]]` behaves as `needs-exception` (R-SCOPE-033).
- **C-20 (R-CLI-037).** In 0.1 `approve` accepts a 12-character fingerprint or the ID of an item with a pending `ovr`; `ready` revive/upgrade docs are 0.2 and refused.
- **C-21 (R-CLI-035).** `explain` dispatch order: topic (`syntax`, `kinds`, `approve`), code (`HN-` prefix), existing path, ID grammar, else exit 2. `pr-template` is 0.2.
- **C-22 (R-CHK-040, R-CLI-003).** `CheckReport::outcome`: Usage (2) for an unresolved scope, for any non-baselined, non-waived error diagnostic of the syntax or control family (`HN-S*`, `HN-C*`), and for `partial` with nothing failing; Findings (1) for other enforce-stage errors, for warns under `deny_warnings`, and for zero declarations under `--require-declarations`; StaleGenerated (3) for `HN-G001` at warn/enforce. Precedence 2 over 1 over 3.
- **C-23 (R-CHK-003).** `HN-N001` is emitted when the effective view of the root has zero rules and zero IDs; exit 0 unless `--require-declarations`.
- **C-24 (R-SCOPE-011).** Eligibility hints use an `[eligibility]` table (`min-lines` 5, `min-files` 3) plus Cargo workspace members; both are added to the schema index by amendment.
- **C-25 (R-SCOPE-048).** `[exceptions] max-live-per-rule`, default 10, drives `HN-X011`.
- **C-26 (R-CHK-028).** Default waiver `budget` is 25 per scope (the example value).
- **C-27 (R-CTX-045).** `[context] sev-weight` is `{ error = 4, warn = 2, info = 1 }`; relevance constants 1.0/0.7/0.4 are compiled defaults tied to the edition.
- **C-28 (R-CLI-032, R-CHK-036).** `context --format human` behaves as `terse`. `--format` defaults to `human` on a TTY, else `terse`; the bin decides, core receives an explicit `OutputFormat` (C-29 is merged here).
- **C-30 (R-CHK-005).** `forbid` is Rust `regex` syntax compiled at load; an invalid pattern is `HN-C001`.
- **C-31 (R-CLI-031).** The scaffold placeholder is the constant `PLACEHOLDER = "TODO(fairlead)"`; `check` reports any authored mandatory slot equal to it as a missing key (`HN-S004` for frontmatter, `HN-C001` for control files).
- **C-32 (D-40, R-CHK-006).** `[rules.<name>]` produces an `Item` of kind `rule` (ID `rule.<name>`, home = declaring scope, default policy Sealed, or NeedsException when `waivable`); `ovr` of `sev`, `stage`, `glob`, `exclude` changes its fields; `waivable` is not overridable.
- **C-33 (R-CTX-008, D-13).** The newest handoff is the item of kind `handoff` with the greatest `created` frontmatter stamp, ties broken by ID bytewise.
- **C-34 (R-SYN-038).** A root control file without `edition` is `HN-C001`; an `edition` other than 1 is refused as a pin mismatch (exit 4).
- **C-35 (R-PORT-003/008).** "Last committed membership shards" means the on-disk `gen/members` as last written by `sync`, not git history; no git is needed for `HN-M001`-`M003`.
- **C-36 (R-ARCH-075).** `--changed` includes untracked, non-ignored files; `files_from` and `changed` combine as set intersection.
- **C-37 (R-ARCH-023).** A file skipped for size, NUL or encoding gets one info/warn diagnostic and a `FileRecord` with `skip = Some(..)` and its hash over raw bytes.
- **C-38 (R-CHK-036).** The JSON schema has exactly the top-level keys `fairlead`, `edition`, `as_of`, `findings`, `summary`; SARIF is the subset of 2.1.0 listed in R-CHK-036.
- **C-39 (R-CLI-016).** The pointer block text is the one in R-CLI-016 with the configured marker substituted; the EOL style of the target file is preserved.
- **C-40 (R-CHK-035, R-AGT-017).** Caps: `intent` 80, `fix` 100 chars, emitted repository lines 240; `--for-agent` 2 KB total (R-CHK-039).
