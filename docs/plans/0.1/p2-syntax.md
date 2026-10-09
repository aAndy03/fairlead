# P2 fairlead-syntax Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `crates/fairlead-syntax`, the pure crate that turns file bytes plus an extension plus an `ExtractConfig` into an `Extraction` (attached directives, frontmatter, syntax diagnostics), exactly as fixed by `plans/0.1/01-contract.md` Sections 3, 4.1, 4.2 and 6 (row P2).

**Architecture:** One total, pure function `extract(bytes, ext, cfg) -> Extraction` (R-ARCH-030..033) composed of small modules: `normalize` (BOM and EOL stripping with an original-byte offset map) -> `lexer` (comment tokens that honour string and raw-string literals, marker-line classification, continuation assembly) -> `directive` (a winnow tokenizer plus per-verb construction) -> `frontmatter` (`+++` splitter and `toml` parse) -> `attach` (file, trailing, standalone, explicit-range and Markdown doc attachment, `+end` regions) -> `diag` (codes `HN-S*`/`HN-I*`, `SyntaxDiag`, `Span`). `extract` ties them together and keeps the invariant `candidates == directives.len() + dropped`. Everything is verified by unit tests per rule, golden conformance fixtures under `fixtures/syntax/<host>/`, and property tests (never panics, CRLF/LF equivalence, offsets map to original bytes).

**Tech Stack:** Rust edition 2024, MSRV 1.88; `winnow` 1.0.4, `toml` 1.1.7 (`default-features = false`, `std`, `parse`, `serde`), `serde` 1.0.229 (`derive`), `unicode-normalization`, `memchr` (both UNVERIFIED, C-2); dev: `proptest` (UNVERIFIED). No other crates.

**Spec:**
- [01-contract.md](01-contract.md) (LAW: sections 1, 2, 3, 4.1, 4.2, 6 row P2, 7, 8, C-1..C-40)
- [decisions.md](../../decisions.md) (D-17, D-18, D-27, D-32, D-35, D-46; wins over any chapter)
- [conventions.md](../../conventions.md)
- [12-directive-syntax.md](../../12-directive-syntax.md) (R-SYN-001..009, 012, 014..023, 027..032, 034..037, 044..046, 048)
- [11-files-and-classes.md](../../11-files-and-classes.md) (R-FILE-013, R-FILE-015, R-FILE-042)
- [50-architecture.md](../../50-architecture.md) section 4.3 (R-ARCH-031, R-ARCH-032, R-ARCH-033) and R-ARCH-060
- [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) (code table, R-CHK-042)

**Global Constraints (verbatim, apply to every task):**
- MSRV 1.88 (edition 2024, resolver 3); nothing newer than Rust 1.88 may be used.
- `#![forbid(unsafe_code)]` in the crate root.
- No I/O: no `std::fs`, `std::env`, `std::process`, no clock, no network. `fairlead-syntax` does not depend on `fairlead-core` (R-ARCH-002). The only file reads happen in the integration test `tests/conformance.rs`.
- No `HashMap` / `HashSet` anywhere (R-ARCH-060): use `BTreeMap` / `BTreeSet` / sorted `Vec`.
- Deterministic: same bytes, extension and config give the same `Extraction`, independent of thread, locale, time.
- LF: every string this crate produces (messages, hints, `render_directive_body`) uses `\n` only and never embeds a CR.
- Library code outside `#[cfg(test)]` must not use `unwrap`, `expect`, `panic!`, indexing (`a[i]`, `&s[a..b]`), `print!`, `eprintln!`, `todo!`; use `.get(..)`, `?`, `let ... else`, pattern matching. `extract` accepts any bytes and never panics, never loops.

**Conventions used in the code below.**
- Unit tests sit in `#[cfg(test)] mod tests` at the bottom of the module and are named `r_syn_016__file_form_after_license` (rule id in snake case, `__`, case), contract Section 7.
- `crate::normalize::slice(s, a, b)` is the single panic-free sub-slice helper (`s.get(a..b).unwrap_or("")`).
- All commits end with the trailer `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (second `-m`).
- "Verify:" notes mark API details I could not check offline (winnow 1.0, toml 1.1, unicode-normalization, memchr, proptest). When the compiler disagrees with the code, adapt the call, never the tested behaviour.

**File map (all paths relative to the repository root):**

| File | Task | Responsibility |
|---|---|---|
| `crates/fairlead-syntax/Cargo.toml` | 1 | manifest and dependencies |
| `crates/fairlead-syntax/src/lib.rs` | 1 (grows) | re-exports, `SYNTAX_VERSION`, `GRAMMAR_EDITION` |
| `crates/fairlead-syntax/src/diag.rs` | 1 | `DiagCode`, `codes`, `Severity`, `Span`, `SyntaxDiag`, code registry, `did_you_mean` |
| `crates/fairlead-syntax/src/ids.rs` | 2 | `ScopeSlug`, `Kind`, `Name`, `IdQualifier`, `Id`, `IdError`, reserved names |
| `crates/fairlead-syntax/src/normalize.rs` | 3 | `Normalized`, `lf_bytes`, `nfc`, tab handling, `slice` |
| `crates/fairlead-syntax/src/delimiters.rs` | 4 | `Leader`, `CommentTable` |
| `crates/fairlead-syntax/src/lexer.rs` | 5, 6, 7, 8 | comment tokens, markers, continuation assembly |
| `crates/fairlead-syntax/src/directive.rs` | 9..13 | winnow tokenizer, `Directive` types, per-verb construction, `known_keys`, rendering |
| `crates/fairlead-syntax/src/frontmatter.rs` | 14 | `split_frontmatter`, `Frontmatter`, `parse_frontmatter` |
| `crates/fairlead-syntax/src/attach.rs` | 15..18 | attachment and regions |
| `crates/fairlead-syntax/src/extract.rs` | 19, 20 | `ExtractConfig`, `Extraction`, `extract` |
| `crates/fairlead-syntax/tests/conformance.rs` | 21 | golden fixture runner |
| `fixtures/syntax/<host>/<case>.<ext>` + `<case>.expect` | 21 | golden fixtures |
| `crates/fairlead-syntax/tests/prop_extract.rs` | 22 | property tests |

---

### Task 1: Crate skeleton and diagnostics registry (`diag.rs`)

**Rules:** R-SYN-032 (diagnostic shape), R-ARCH-103 fields, R-CHK-042 (stable codes), C-16 (span columns).

**Files:**
- Create: `crates/fairlead-syntax/Cargo.toml`
- Create: `crates/fairlead-syntax/src/lib.rs`
- Create: `crates/fairlead-syntax/src/diag.rs`

**Interfaces:**
- Consumes: nothing (leaf module). Workspace `Cargo.toml` from P1 lists `crates/*` as members and provides `[workspace.package]`/`[workspace.lints]`.
- Produces (contract 4.1, unchanged): `DiagCode(pub &'static str)`, `codes::HN_S001 ..`, `Severity { Info, Warn, Error }`, `Span { start, end, line, col }`, `SyntaxDiag { code, severity, span, message, expected, did_you_mean, hint }`. Additive: `codes::HN_P001`, `CodeInfo`, `REGISTRY`, `info(DiagCode)`, `info_by_str(&str)`, `did_you_mean(word, candidates)`, `Span::whole_file()`, `SyntaxDiag::new(..)`.

- [ ] **Step 1: Write the failing test.** Create the manifest and `lib.rs` first (they are needed for every later task), then create `diag.rs` containing only the test module.

`crates/fairlead-syntax/Cargo.toml` (Verify: if P1 already created this file keep P1's `[package]` keys and only merge `[dependencies]`):

```toml
[package]
name = "fairlead-syntax"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
publish = false
license-file.workspace = true
description = "Pure directive extraction for fairlead: comments, grammar, frontmatter, attachment."

[lints]
workspace = true

[dependencies]
winnow = "1.0.4"
toml = { version = "1.1.7", default-features = false, features = ["std", "parse", "serde"] }
serde = { version = "1.0.229", features = ["derive"] }
unicode-normalization = "0.1"
memchr = "2"

[dev-dependencies]
proptest = "1"
```

`crates/fairlead-syntax/src/lib.rs`:

```rust
//! Pure directive extraction for fairlead (R-ARCH-002, R-ARCH-030).
//! No I/O, no clock, no `fairlead-core`. Every public function is total.
#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod diag;

/// Bumped when extraction behaviour changes in a way that must invalidate cached records.
pub const SYNTAX_VERSION: u32 = 1;
/// Directive grammar edition implemented by this crate (D-17, D-45).
pub const GRAMMAR_EDITION: u32 = 1;

pub use diag::{codes, DiagCode, Severity, Span, SyntaxDiag};
```

`crates/fairlead-syntax/src/diag.rs` (test only for now):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::IntoDeserializer;
    use serde::Deserialize;

    #[test]
    fn r_syn_032__registry_has_each_syntax_code_once_and_never_s003() {
        let mut seen = std::collections::BTreeSet::new();
        for c in REGISTRY {
            assert!(seen.insert(c.code.0), "duplicate {}", c.code.0);
            assert!(c.code.0.starts_with("HN-S") || c.code.0.starts_with("HN-I") || c.code.0 == "HN-P001");
        }
        assert!(!seen.contains("HN-S003"), "S003 is unallocated");
        for expected in [
            "HN-S001", "HN-S002", "HN-S004", "HN-S005", "HN-S006", "HN-S007", "HN-S008",
            "HN-S009", "HN-S010", "HN-S011", "HN-S012", "HN-S013", "HN-S014", "HN-S015",
            "HN-S016", "HN-I001", "HN-I002", "HN-I003", "HN-P001",
        ] {
            assert!(seen.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn r_syn_032__severities_match_table_13() {
        assert_eq!(info(codes::HN_S001).map(|i| i.severity), Some(Severity::Error));
        assert_eq!(info(codes::HN_S005).map(|i| i.severity), Some(Severity::Warn));
        assert_eq!(info(codes::HN_S010).map(|i| i.severity), Some(Severity::Warn));
        assert_eq!(info(codes::HN_S013).map(|i| i.severity), Some(Severity::Error));
        assert_eq!(info(codes::HN_I001).map(|i| i.severity), Some(Severity::Info));
        assert_eq!(info(codes::HN_I003).map(|i| i.severity), Some(Severity::Warn));
        assert!(Severity::Info < Severity::Warn && Severity::Warn < Severity::Error);
    }

    #[test]
    fn r_syn_032__did_you_mean_finds_close_verbs_only() {
        let verbs = ["def", "use", "ovr", "flag", "env", "end"];
        assert_eq!(did_you_mean("deff", &verbs), Some("def"));
        assert_eq!(did_you_mean("fag", &verbs), Some("flag"));
        assert_eq!(did_you_mean("frobnicate", &verbs), None);
        assert_eq!(did_you_mean("", &verbs), None);
    }

    #[test]
    fn r_syn_032__diag_code_deserializes_through_the_registry() {
        let d: serde::de::value::StrDeserializer<'_, serde::de::value::Error> =
            "HN-S007".into_deserializer();
        assert_eq!(DiagCode::deserialize(d).unwrap(), codes::HN_S007);
        let bad: serde::de::value::StrDeserializer<'_, serde::de::value::Error> =
            "HN-Z999".into_deserializer();
        assert!(DiagCode::deserialize(bad).is_err());
    }

    #[test]
    fn r_syn_032__new_diag_takes_severity_from_registry() {
        let d = SyntaxDiag::new(codes::HN_S010, Span::whole_file(), "binds to nothing", "put code after it");
        assert_eq!(d.severity, Severity::Warn);
        assert_eq!(d.span.line, 0, "0 line means whole file");
        assert!(d.expected.is_none() && d.did_you_mean.is_none());
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_032` — Expected: FAIL to compile: `cannot find value REGISTRY / function info / function did_you_mean in this scope` (the module has only tests).

- [ ] **Step 3: Write the implementation.** Insert this above the `#[cfg(test)]` block of `diag.rs`:

```rust
//! Diagnostics shared by every syntax stage (R-SYN-032, R-CHK-042).
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Opaque stable code "HN-S001" (R-CHK-042, R-ARCH-055). Compare by string.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DiagCode(pub &'static str);

impl Serialize for DiagCode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for DiagCode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        info_by_str(&text)
            .map(|i| i.code)
            .ok_or_else(|| serde::de::Error::custom("unknown diagnostic code"))
    }
}

pub mod codes {
    use super::DiagCode;
    pub const HN_S001: DiagCode = DiagCode("HN-S001"); // unknown verb
    pub const HN_S002: DiagCode = DiagCode("HN-S002"); // unknown kind (resolve) / lexically bad ID (extract)
    pub const HN_S004: DiagCode = DiagCode("HN-S004"); // frontmatter invalid
    pub const HN_S005: DiagCode = DiagCode("HN-S005"); // near-miss marker
    pub const HN_S006: DiagCode = DiagCode("HN-S006"); // continuation limit
    pub const HN_S007: DiagCode = DiagCode("HN-S007"); // quote / escape
    pub const HN_S008: DiagCode = DiagCode("HN-S008"); // unmatched end / unclosed region
    pub const HN_S009: DiagCode = DiagCode("HN-S009"); // ovr of a missing field (resolve)
    pub const HN_S010: DiagCode = DiagCode("HN-S010"); // binds to nothing
    pub const HN_S011: DiagCode = DiagCode("HN-S011"); // unknown key (resolve)
    pub const HN_S012: DiagCode = DiagCode("HN-S012"); // until / non-date exp in 0.1
    pub const HN_S013: DiagCode = DiagCode("HN-S013"); // doc-comment carrier
    pub const HN_S014: DiagCode = DiagCode("HN-S014"); // sym absent
    pub const HN_S015: DiagCode = DiagCode("HN-S015"); // directive-count explosion
    pub const HN_S016: DiagCode = DiagCode("HN-S016"); // incomplete directive / ovr
    pub const HN_I001: DiagCode = DiagCode("HN-I001"); // file skipped (size / NUL)
    pub const HN_I002: DiagCode = DiagCode("HN-I002"); // unknown comment syntax
    pub const HN_I003: DiagCode = DiagCode("HN-I003"); // non-UTF-8 skipped
    pub const HN_P001: DiagCode = DiagCode("HN-P001"); // reserved name in an ID (R-SYN-027)
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// Original-byte span plus 1-based line and char column of the start (C-16).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub line: u32,
    pub col: u32,
}

impl Span {
    /// Line 0 means "whole file" (contract 4).
    pub const fn whole_file() -> Span {
        Span { start: 0, end: 0, line: 0, col: 0 }
    }
}

/// R-SYN-032, R-ARCH-103: code, location, expected form, did-you-mean, one hint.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SyntaxDiag {
    pub code: DiagCode,
    pub severity: Severity,
    pub span: Span,
    pub message: String,
    pub expected: Option<String>,
    pub did_you_mean: Option<String>,
    pub hint: String,
}

impl SyntaxDiag {
    /// Severity comes from the registry; `expected` and `did_you_mean` start empty.
    pub fn new(code: DiagCode, span: Span, message: impl Into<String>, hint: impl Into<String>) -> SyntaxDiag {
        SyntaxDiag {
            code,
            severity: info(code).map_or(Severity::Error, |i| i.severity),
            span,
            message: message.into(),
            expected: None,
            did_you_mean: None,
            hint: hint.into(),
        }
    }
    pub fn expected(mut self, e: impl Into<String>) -> SyntaxDiag {
        self.expected = Some(e.into());
        self
    }
    pub fn suggest(mut self, s: Option<String>) -> SyntaxDiag {
        self.did_you_mean = s;
        self
    }
}

/// One registry row: default severity and display name (13, R-CHK-042).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CodeInfo {
    pub code: DiagCode,
    pub name: &'static str,
    pub severity: Severity,
    pub summary: &'static str,
}

const fn row(code: DiagCode, name: &'static str, severity: Severity, summary: &'static str) -> CodeInfo {
    CodeInfo { code, name, severity, summary }
}

/// Every code this crate or its consumers raise for syntax and file scanning. Sorted by code.
pub const REGISTRY: &[CodeInfo] = &[
    row(codes::HN_I001, "FILE-SKIPPED", Severity::Info, "file skipped: too large or binary"),
    row(codes::HN_I002, "UNKNOWN-COMMENT-SYNTAX", Severity::Info, "no comment leaders known for this extension"),
    row(codes::HN_I003, "ENCODING-SKIP", Severity::Warn, "file is not valid UTF-8 and was not scanned"),
    row(codes::HN_P001, "PATH-HYGIENE", Severity::Error, "reserved Windows name in an ID or scope id"),
    row(codes::HN_S001, "UNKNOWN-VERB", Severity::Error, "unknown directive verb"),
    row(codes::HN_S002, "UNKNOWN-KIND", Severity::Error, "ID with an undeclared or malformed kind"),
    row(codes::HN_S004, "FRONTMATTER", Severity::Error, "frontmatter is not valid TOML or directives is not an array of strings"),
    row(codes::HN_S005, "MARKER-NEAR-MISS", Severity::Warn, "comment looks like a directive but the marker is misspelled"),
    row(codes::HN_S006, "CONTINUATION-LIMIT", Severity::Error, "more than 8 continuation lines or 1 KB"),
    row(codes::HN_S007, "QUOTE", Severity::Error, "unterminated quote or unknown escape"),
    row(codes::HN_S008, "REGION", Severity::Error, "unmatched end or unclosed +end region"),
    row(codes::HN_S009, "OVR-FIELD", Severity::Error, "ovr of a field the item does not have"),
    row(codes::HN_S010, "BINDS-NOTHING", Severity::Warn, "directive binds to nothing"),
    row(codes::HN_S011, "UNKNOWN-KEY", Severity::Warn, "unknown key on a directive"),
    row(codes::HN_S012, "EXPIRY", Severity::Error, "until= or a non-ISO exp in edition 1"),
    row(codes::HN_S013, "DOC-COMMENT-CARRIER", Severity::Error, "marker inside a doc comment would leak into rustdoc/TSDoc"),
    row(codes::HN_S014, "SYM-ABSENT", Severity::Warn, "sym= name not found in the extent"),
    row(codes::HN_S015, "DIRECTIVE-COUNT", Severity::Warn, "more directives in one file than max-directives"),
    row(codes::HN_S016, "OVR-INCOMPLETE", Severity::Error, "incomplete directive"),
];

pub fn info(code: DiagCode) -> Option<&'static CodeInfo> {
    info_by_str(code.0)
}

pub fn info_by_str(code: &str) -> Option<&'static CodeInfo> {
    REGISTRY.iter().find(|i| i.code.0 == code)
}

/// Nearest candidate within edit distance 2 (and shorter than the word's length), ties by order.
pub fn did_you_mean<'a>(word: &str, candidates: &[&'a str]) -> Option<&'a str> {
    if word.is_empty() {
        return None;
    }
    let mut best: Option<(usize, &'a str)> = None;
    for c in candidates {
        let d = edit_distance(word, c);
        if d <= 2 && d < word.chars().count().max(c.chars().count()) && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = Vec::with_capacity(b.len() + 1);
        cur.push(i + 1);
        for (j, cb) in b.iter().enumerate() {
            let sub = prev.get(j).copied().unwrap_or(0) + usize::from(ca != cb);
            let del = prev.get(j + 1).copied().unwrap_or(0) + 1;
            let ins = cur.get(j).copied().unwrap_or(0) + 1;
            cur.push(sub.min(del).min(ins));
        }
        prev = cur;
    }
    prev.get(b.len()).copied().unwrap_or(usize::MAX)
}
```

Verify: `Option::is_none_or` is stable since Rust 1.82 (fine for MSRV 1.88). `"..".into_deserializer()` needs `serde::de::IntoDeserializer` in scope (the test imports it).

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax r_syn_032` — Expected: PASS (5 tests). Also run `cargo clippy -p fairlead-syntax --all-targets -- -D warnings` and `cargo fmt --all --check`.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): crate skeleton, diagnostic codes and registry" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: ID grammar (`ids.rs`)

**Rules:** R-SYN-007 (ID grammar, ASCII lowercase), R-SYN-027 (Windows-reserved names). Kind/qualifier lexical shape for R-SYN-024/025 (semantic resolution is P4).

**Files:**
- Create: `crates/fairlead-syntax/src/ids.rs`
- Modify: `crates/fairlead-syntax/src/lib.rs` (add `pub mod ids;` and re-exports)

**Interfaces:**
- Consumes: nothing.
- Produces (contract 4.1): `ScopeSlug`, `Kind`, `Name`, `IdQualifier { Bare, Root, Scope(ScopeSlug) }`, `Id { qualifier, kind, name }`, `Id::parse(&str) -> Result<Id, IdError>`, `Id::kind_name(&self) -> String`, `impl Display for Id`, `Ord` bytewise on `Display`, `ScopeSlug::parse`, `ScopeSlug::as_str`, `IdError { Empty, BadChar { at: usize }, TooLong, ReservedName, BadKind, BadShape }`, `is_reserved_windows_name(&str) -> bool`, `PLACEHOLDER: &str = "TODO(fairlead)"`. Additive: `Kind::parse`, `Kind::as_str`, `Name::parse`, `Name::as_str`.

- [ ] **Step 1: Write the failing test.** Create `ids.rs` with only this module and add `pub mod ids;` to `lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_syn_007__id_forms_bare_root_and_scoped() {
        let a = Id::parse("cmp.sync-button").unwrap();
        assert_eq!(a.qualifier, IdQualifier::Bare);
        assert_eq!(a.kind_name(), "cmp.sync-button");
        let r = Id::parse(":tok.primary").unwrap();
        assert_eq!(r.qualifier, IdQualifier::Root);
        assert_eq!(r.to_string(), ":tok.primary");
        let s = Id::parse("web:cmp.button").unwrap();
        assert_eq!(s.to_string(), "web:cmp.button");
        let dotted = Id::parse("adr.0003.sub-part").unwrap();
        assert_eq!(dotted.name.as_str(), "0003.sub-part");
    }

    #[test]
    fn r_syn_007__id_is_ascii_lowercase_only() {
        assert_eq!(Id::parse("Cmp.x"), Err(IdError::BadChar { at: 0 }));
        assert_eq!(Id::parse("cmp.Xy"), Err(IdError::BadChar { at: 4 }));
        assert_eq!(Id::parse("cmp.\u{e9}t\u{e9}"), Err(IdError::BadChar { at: 4 }));
    }

    #[test]
    fn r_syn_007__id_shape_errors() {
        assert_eq!(Id::parse(""), Err(IdError::Empty));
        assert_eq!(Id::parse("cmp"), Err(IdError::BadShape));
        assert_eq!(Id::parse("cmp."), Err(IdError::BadShape));
        assert_eq!(Id::parse("a:b:cmp.x"), Err(IdError::BadShape));
        assert_eq!(Id::parse("cmp.x..y"), Err(IdError::BadShape));
        assert_eq!(Id::parse("cmp.-x"), Err(IdError::BadShape));
        assert_eq!(Id::parse("c.x"), Err(IdError::BadKind), "kind needs 2..=8 letters");
        assert_eq!(Id::parse("abcdefghi.x"), Err(IdError::BadKind));
        assert!(matches!(Id::parse("tauri-command.x"), Err(IdError::BadChar { at: 5 })));
    }

    #[test]
    fn r_syn_007__id_length_limits() {
        let long_name = format!("cmp.{}", "a".repeat(65));
        assert_eq!(Id::parse(&long_name), Err(IdError::TooLong));
        let ok_name = format!("cmp.{}.{}", "a".repeat(32), "b".repeat(31));
        assert!(Id::parse(&ok_name).is_ok());
        let long_scope = format!("{}:cmp.x", "a".repeat(33));
        assert_eq!(Id::parse(&long_scope), Err(IdError::TooLong));
    }

    #[test]
    fn r_syn_007__ord_is_bytewise_on_display() {
        let mut v: Vec<Id> = ["web:cmp.b", ":cmp.a", "cmp.a", "cmp.b"]
            .iter()
            .map(|s| Id::parse(s).unwrap())
            .collect();
        v.sort();
        let shown: Vec<String> = v.iter().map(|i| i.to_string()).collect();
        let mut expect = shown.clone();
        expect.sort();
        assert_eq!(shown, expect);
        assert_eq!(shown.first().map(String::as_str), Some(":cmp.a"));
    }

    #[test]
    fn r_syn_027__windows_reserved_names() {
        for n in ["con", "PRN", "aux", "nul", "com1", "COM9", "lpt1", "lpt9", "web.", "x "] {
            assert!(is_reserved_windows_name(n), "{n}");
        }
        for n in ["com0", "com10", "console", "web", "lpt"] {
            assert!(!is_reserved_windows_name(n), "{n}");
        }
    }

    #[test]
    fn r_syn_027__reserved_scope_and_name_segments_are_rejected() {
        assert_eq!(ScopeSlug::parse("con"), Err(IdError::ReservedName));
        assert_eq!(Id::parse("nul:cmp.x"), Err(IdError::ReservedName));
        assert_eq!(Id::parse("cmp.aux"), Err(IdError::ReservedName));
        assert_eq!(Id::parse("cmp.ok.com3"), Err(IdError::ReservedName));
        assert!(ScopeSlug::parse("web").is_ok());
    }

    #[test]
    fn r_syn_007__parse_never_panics_on_junk() {
        for s in ["::", ":", ".", ":.", "..", "a:", ":a", "\u{0}", "\u{feff}cmp.x", "cmp.x\n", "é:é.é", "-:-.-", "a:b.c:d"] {
            let _ = Id::parse(s);
        }
    }

    #[test]
    fn r_cli_031__placeholder_constant() {
        assert_eq!(PLACEHOLDER, "TODO(fairlead)");
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_0 -- ids` (or `cargo test -p fairlead-syntax ids::`) — Expected: FAIL to compile: `cannot find type Id / ScopeSlug / IdError in this scope`.

- [ ] **Step 3: Implement.** Add above the tests in `ids.rs`:

```rust
//! Identifiers: `[scope:]kind.name` (R-SYN-007, R-SYN-024..027). Pure lexical validation.
use core::cmp::Ordering;
use core::fmt;
use serde::{Deserialize, Serialize};

/// R-CLI-031 scaffold slot value that `check` rejects (C-31).
pub const PLACEHOLDER: &str = "TODO(fairlead)";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum IdError {
    Empty,
    BadChar { at: usize },
    TooLong,
    ReservedName,
    BadKind,
    BadShape,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Empty => f.write_str("empty ID"),
            IdError::BadChar { at } => write!(f, "character not allowed at byte {at}"),
            IdError::TooLong => f.write_str("ID part too long"),
            IdError::ReservedName => f.write_str("Windows-reserved name"),
            IdError::BadKind => f.write_str("kind must be 2 to 8 lowercase letters"),
            IdError::BadShape => f.write_str("expected [scope:]kind.name"),
        }
    }
}

/// R-SYN-027: con prn aux nul com1..9 lpt1..9, trailing dot or space.
pub fn is_reserved_windows_name(s: &str) -> bool {
    if s.ends_with('.') || s.ends_with(' ') {
        return true;
    }
    let l = s.to_ascii_lowercase();
    if matches!(l.as_str(), "con" | "prn" | "aux" | "nul") {
        return true;
    }
    for p in ["com", "lpt"] {
        if let Some(d) = l.strip_prefix(p) {
            if d.len() == 1 && d.bytes().all(|b| (b'1'..=b'9').contains(&b)) {
                return true;
            }
        }
    }
    false
}

/// Validates `[a-z0-9]+(-[a-z0-9]+)*` of at most `max` bytes. `base` is the offset of `s` in the whole ID.
fn check_slug(s: &str, base: usize, max: usize) -> Result<(), IdError> {
    if s.is_empty() {
        return Err(IdError::BadShape);
    }
    for (i, c) in s.bytes().enumerate() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-') {
            return Err(IdError::BadChar { at: base + i });
        }
    }
    if s.len() > max {
        return Err(IdError::TooLong);
    }
    if s.starts_with('-') || s.ends_with('-') || s.contains("--") {
        return Err(IdError::BadShape);
    }
    Ok(())
}

/// Scope id: `[a-z0-9]+(-[a-z0-9]+)*`, at most 32, not Windows-reserved (R-SYN-027).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ScopeSlug(String);

impl ScopeSlug {
    pub fn parse(s: &str) -> Result<Self, IdError> {
        ScopeSlug::parse_at(s, 0)
    }
    fn parse_at(s: &str, base: usize) -> Result<Self, IdError> {
        if s.is_empty() {
            return Err(IdError::Empty);
        }
        check_slug(s, base, 32)?;
        if is_reserved_windows_name(s) {
            return Err(IdError::ReservedName);
        }
        Ok(ScopeSlug(s.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Kind: 2 to 8 lowercase ASCII letters (R-SYN-024).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Kind(String);

impl Kind {
    pub fn parse(s: &str) -> Result<Self, IdError> {
        Kind::parse_at(s, 0)
    }
    fn parse_at(s: &str, base: usize) -> Result<Self, IdError> {
        for (i, c) in s.bytes().enumerate() {
            if !c.is_ascii_lowercase() {
                return Err(IdError::BadChar { at: base + i });
            }
        }
        if !(2..=8).contains(&s.len()) {
            return Err(IdError::BadKind);
        }
        Ok(Kind(s.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Name: `slug *("." slug)`, at most 64 bytes in total, each segment at most 32.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Name(String);

impl Name {
    pub fn parse(s: &str) -> Result<Self, IdError> {
        Name::parse_at(s, 0)
    }
    fn parse_at(s: &str, base: usize) -> Result<Self, IdError> {
        if s.is_empty() {
            return Err(IdError::BadShape);
        }
        let mut off = base;
        for seg in s.split('.') {
            check_slug(seg, off, 32)?;
            if is_reserved_windows_name(seg) {
                return Err(IdError::ReservedName);
            }
            off += seg.len() + 1;
        }
        if s.len() > 64 {
            return Err(IdError::TooLong);
        }
        Ok(Name(s.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum IdQualifier {
    Bare,
    Root,
    Scope(ScopeSlug),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Id {
    pub qualifier: IdQualifier,
    pub kind: Kind,
    pub name: Name,
}

impl Id {
    /// Grammar of R-SYN-007. Never panics.
    pub fn parse(s: &str) -> Result<Id, IdError> {
        if s.is_empty() {
            return Err(IdError::Empty);
        }
        let (qualifier, rest, base) = match s.split_once(':') {
            None => (IdQualifier::Bare, s, 0usize),
            Some(("", r)) => (IdQualifier::Root, r, 1),
            Some((sc, r)) => (IdQualifier::Scope(ScopeSlug::parse(sc)?), r, sc.len() + 1),
        };
        if rest.contains(':') {
            return Err(IdError::BadShape);
        }
        let (kind_s, name_s) = rest.split_once('.').ok_or(IdError::BadShape)?;
        if kind_s.is_empty() {
            return Err(IdError::BadKind);
        }
        let kind = Kind::parse_at(kind_s, base)?;
        let name = Name::parse_at(name_s, base + kind_s.len() + 1)?;
        Ok(Id { qualifier, kind, name })
    }

    /// `"kind.name"` without the qualifier.
    pub fn kind_name(&self) -> String {
        format!("{}.{}", self.kind.0, self.name.0)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.qualifier {
            IdQualifier::Bare => {}
            IdQualifier::Root => f.write_str(":")?,
            IdQualifier::Scope(s) => write!(f, "{}:", s.0)?,
        }
        write!(f, "{}.{}", self.kind.0, self.name.0)
    }
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Id {
    /// Bytewise on the canonical text (contract 4.1, R-ARCH-060).
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_string().cmp(&other.to_string())
    }
}
```

Then in `lib.rs` add `pub use ids::{is_reserved_windows_name, Id, IdError, IdQualifier, Kind, Name, ScopeSlug, PLACEHOLDER};`.

Note for the executor: `Id` derives `Hash` and `PartialEq` from fields while `Ord` compares text; both agree because the text is a bijective rendering of the fields.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax ids::` — Expected: PASS (9 tests). If `r_syn_007__id_is_ascii_lowercase_only` fails on the `"cmp.\u{e9}t\u{e9}"` offset, the first non-ASCII byte is at byte 4 (`cmp.` is 4 bytes), which is what the test asserts.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): ID grammar, kinds, names and reserved Windows names" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

### Task 3: Normalisation with an offset map (`normalize.rs`)

**Rules:** R-ARCH-033 (CRLF and lone CR to LF; offsets refer to original bytes), R-SYN-007 (BOM stripped, tabs to one space outside quotes, NFC), C-16 (columns count scalar values), contract 4.2 note (`lf_bytes`).

**Files:**
- Create: `crates/fairlead-syntax/src/normalize.rs`
- Modify: `crates/fairlead-syntax/src/lib.rs` (`pub mod normalize;`)

**Interfaces:**
- Consumes: `diag::Span`.
- Produces: `pub fn lf_bytes(bytes: &[u8]) -> Cow<'_, [u8]>` (contract 4.2, used by `fairlead-core::hash`), `pub fn slice(s: &str, a: usize, b: usize) -> &str` (crate-wide panic-free slicing), `pub fn nfc(s: &str) -> String`, `pub fn tabs_to_spaces_outside_quotes(s: &str) -> String`, and `Normalized` with `new`, `line_of`, `line_start`, `line_end`, `line_text`, `line_count`, `orig_offset`, `span`.

- [ ] **Step 1: Write the failing test.** Create `normalize.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn r_arch_033__crlf_and_lone_cr_become_lf_with_original_offsets() {
        let n = Normalized::new("a\r\nb\rc");
        assert_eq!(n.text, "a\nb\nc");
        assert_eq!(n.orig_offset(0), 0);
        assert_eq!(n.orig_offset(1), 1, "the LF of a CRLF maps to the CR");
        assert_eq!(n.orig_offset(2), 3);
        assert_eq!(n.orig_offset(3), 4, "a lone CR maps to itself");
        assert_eq!(n.orig_offset(4), 5);
        assert_eq!(n.orig_offset(5), 6, "one past the end maps to the original length");
        assert_eq!(n.line_count(), 3);
    }

    #[test]
    fn r_syn_007__bom_is_stripped_and_offsets_stay_original() {
        let n = Normalized::new("\u{feff}x\n");
        assert_eq!(n.text, "x\n");
        assert_eq!(n.orig_offset(0), 3);
    }

    #[test]
    fn r_arch_033__span_columns_count_scalar_values() {
        let n = Normalized::new("\u{e9} = 1\r\n// x");
        let eq = n.text.find('=').unwrap();
        let sp = n.span(eq, eq + 1);
        assert_eq!((sp.line, sp.col, sp.start, sp.end), (1, 3, 3, 4));
        let slash = n.text.find("//").unwrap();
        let sp2 = n.span(slash, slash + 2);
        assert_eq!((sp2.line, sp2.col), (2, 1));
        assert_eq!(sp2.start, 8, "e-acute 2 + ' = 1' 4 + CRLF 2 original bytes");
    }

    #[test]
    fn r_arch_033__line_helpers() {
        let n = Normalized::new("one\ntwo\n");
        assert_eq!(n.line_count(), 2, "a trailing newline does not open a third line");
        assert_eq!(n.line_text(1), "one");
        assert_eq!(n.line_text(2), "two");
        assert_eq!(n.line_text(3), "");
        assert_eq!(n.line_of(4), 2);
        assert_eq!(Normalized::new("").line_count(), 1);
    }

    #[test]
    fn r_arch_033__lf_bytes_strips_bom_and_cr() {
        assert_eq!(&*lf_bytes(b"\xef\xbb\xbfa\r\nb\rc"), b"a\nb\nc");
        assert!(matches!(lf_bytes(b"plain\n"), Cow::Borrowed(_)));
        assert_eq!(&*lf_bytes(b"\xff\r\n"), b"\xff\n", "non-UTF-8 bytes pass through");
    }

    #[test]
    fn r_syn_007__nfc_composes_and_ascii_is_untouched() {
        assert_eq!(nfc("e\u{301}"), "\u{e9}");
        assert_eq!(nfc("plain"), "plain");
    }

    #[test]
    fn r_syn_007__tabs_outside_quotes_become_one_space() {
        assert_eq!(tabs_to_spaces_outside_quotes("a\t\"b\tc\"\td"), "a \"b\tc\" d");
        assert_eq!(tabs_to_spaces_outside_quotes("\"a\\\"\tb\"\tc"), "\"a\\\"\tb\" c");
    }

    #[test]
    fn r_arch_032__slice_never_panics() {
        assert_eq!(slice("h\u{e9}llo", 1, 2), "", "not a char boundary");
        assert_eq!(slice("abc", 2, 9), "");
        assert_eq!(slice("abc", 1, 3), "bc");
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax normalize::` — Expected: FAIL to compile: `cannot find type Normalized in this scope`.

- [ ] **Step 3: Implement.** Put above the tests:

```rust
//! Line-ending, BOM and tab normalisation with an offset map back to the original bytes
//! (R-ARCH-033, R-SYN-007).
use crate::diag::Span;
use std::borrow::Cow;
use unicode_normalization::UnicodeNormalization;

/// Panic-free `&s[a..b]`; empty when out of range or not on char boundaries.
pub fn slice(s: &str, a: usize, b: usize) -> &str {
    s.get(a..b).unwrap_or("")
}

pub(crate) fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// BOM stripped, CRLF and lone CR to LF (R-ARCH-076). Works on arbitrary bytes.
pub fn lf_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    let body = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    if body.len() == bytes.len() && !body.contains(&b'\r') {
        return Cow::Borrowed(bytes);
    }
    let mut out = Vec::with_capacity(body.len());
    let mut it = body.iter().peekable();
    while let Some(&b) = it.next() {
        if b == b'\r' {
            if it.peek() == Some(&&b'\n') {
                it.next();
            }
            out.push(b'\n');
        } else {
            out.push(b);
        }
    }
    Cow::Owned(out)
}

/// NFC for semantic strings (quoted values, notes). R-SYN-007.
pub fn nfc(s: &str) -> String {
    if s.is_ascii() {
        s.to_owned()
    } else {
        s.nfc().collect()
    }
}

/// Tabs to one space outside double quotes (R-SYN-007). Length-preserving, so offsets stay valid.
pub fn tabs_to_spaces_outside_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let (mut in_q, mut esc) = (false, false);
    for c in s.chars() {
        if in_q {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_q = false;
            }
            out.push(c);
        } else if c == '"' {
            in_q = true;
            out.push(c);
        } else if c == '\t' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// LF-only text plus the maps needed to report original-byte spans.
#[derive(Clone, Debug)]
pub struct Normalized {
    pub text: String,
    /// `orig[i]` = original byte offset of normalized byte `i`; one extra entry for the end.
    orig: Vec<u32>,
    /// Byte offset of the start of each line in `text`.
    starts: Vec<usize>,
}

impl Normalized {
    pub fn new(src: &str) -> Normalized {
        let body = src.strip_prefix('\u{feff}').unwrap_or(src);
        let bom = src.len() - body.len();
        let mut text = String::with_capacity(body.len());
        let mut orig: Vec<u32> = Vec::with_capacity(body.len() + 1);
        let mut it = body.char_indices().peekable();
        while let Some((i, c)) = it.next() {
            let o = to_u32(bom + i);
            if c == '\r' {
                if matches!(it.peek(), Some((_, '\n'))) {
                    it.next();
                }
                text.push('\n');
                orig.push(o);
            } else {
                text.push(c);
                for k in 0..c.len_utf8() {
                    orig.push(o.saturating_add(to_u32(k)));
                }
            }
        }
        orig.push(to_u32(src.len()));
        let mut starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Normalized { text, orig, starts }
    }

    pub fn orig_offset(&self, off: usize) -> u32 {
        self.orig
            .get(off)
            .copied()
            .or_else(|| self.orig.last().copied())
            .unwrap_or(0)
    }

    /// 1-based line holding byte `off` (offsets past the end map to the last line).
    pub fn line_of(&self, off: usize) -> u32 {
        to_u32(self.starts.partition_point(|&s| s <= off).max(1))
    }

    /// Number of lines; a trailing newline does not open another line.
    pub fn line_count(&self) -> u32 {
        let n = if self.text.ends_with('\n') { self.starts.len() - 1 } else { self.starts.len() };
        to_u32(n.max(1))
    }

    pub fn line_start(&self, line: u32) -> usize {
        let i = (line as usize).saturating_sub(1);
        self.starts.get(i).copied().unwrap_or(self.text.len())
    }

    /// End of the line, excluding the `\n`.
    pub fn line_end(&self, line: u32) -> usize {
        match self.starts.get(line as usize) {
            Some(&next) => next.saturating_sub(1),
            None => self.text.len(),
        }
    }

    pub fn line_text(&self, line: u32) -> &str {
        if line == 0 || line > self.line_count() {
            return "";
        }
        slice(&self.text, self.line_start(line), self.line_end(line))
    }

    /// Original-byte span of the normalized range `a..b`, with 1-based line and scalar column of `a`.
    pub fn span(&self, a: usize, b: usize) -> Span {
        let line = self.line_of(a);
        let col = slice(&self.text, self.line_start(line), a).chars().count() + 1;
        Span {
            start: self.orig_offset(a),
            end: self.orig_offset(b.max(a)),
            line,
            col: to_u32(col),
        }
    }
}
```

Verify: `UnicodeNormalization::nfc` returns an iterator of `char`; `.collect::<String>()` is correct for unicode-normalization 0.1.x. `src.len() - body.len()` cannot underflow because `body` is a suffix of `src`.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax normalize::` — Expected: PASS (8 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): LF/BOM normalisation with original-byte offset map" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Comment-leader table (`delimiters.rs`)

**Rules:** R-SYN-006 (built-in leaders; `[comments]` overrides; JSON has none), R-SYN-044 (per-host placement; `.gitignore` is the extension `gitignore`).

**Files:**
- Create: `crates/fairlead-syntax/src/delimiters.rs`
- Modify: `crates/fairlead-syntax/src/lib.rs` (`pub mod delimiters;`)

**Interfaces:**
- Consumes: nothing.
- Produces (contract 4.2): `Leader` (variants `Line(String)`, `Block { open, close }`, `Html`), `CommentTable`, `CommentTable::builtin()`, `with_overrides(self, ext, leaders)`, `leaders_for(&self, ext) -> Option<&[Leader]>`. Additive: `Leader::from_spec(&str) -> Option<Leader>` (turns a `[comments]` control-file string such as `"//"`, `"/* */"`, `"<!-- -->"` into a `Leader`), `CommentTable::iter()` (for the config fingerprint).

- [ ] **Step 1: Write the failing test.** Create `delimiters.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn line(s: &str) -> Leader {
        Leader::Line(s.to_owned())
    }
    fn block() -> Leader {
        Leader::Block { open: "/*".to_owned(), close: "*/".to_owned() }
    }

    #[test]
    fn r_syn_006__builtin_table_matches_the_spec() {
        let t = CommentTable::builtin();
        for ext in ["rs", "ts", "tsx", "js", "jsx", "mjs", "go", "java", "kt", "swift", "c", "h", "cc", "cpp", "cs", "scala", "zig"] {
            assert_eq!(t.leaders_for(ext), Some(&[line("//"), block()][..]), "{ext}");
        }
        for ext in ["css", "scss", "less"] {
            assert_eq!(t.leaders_for(ext), Some(&[block()][..]), "{ext}");
        }
        for ext in ["py", "sh", "rb", "toml", "yaml", "yml", "ps1", "gitignore"] {
            assert_eq!(t.leaders_for(ext), Some(&[line("#")][..]), "{ext}");
        }
        for ext in ["sql", "lua", "hs"] {
            assert_eq!(t.leaders_for(ext), Some(&[line("--")][..]), "{ext}");
        }
        for ext in ["html", "md", "xml", "svg"] {
            assert_eq!(t.leaders_for(ext), Some(&[Leader::Html][..]), "{ext}");
        }
    }

    #[test]
    fn r_syn_006__json_has_no_leaders_and_unknown_is_none() {
        let t = CommentTable::builtin();
        assert_eq!(t.leaders_for("json"), Some(&[][..]));
        assert_eq!(t.leaders_for("xyz"), None, "unknown extension yields HN-I002 upstream");
        assert_eq!(t.leaders_for(""), None);
    }

    #[test]
    fn r_syn_006__overrides_add_and_replace() {
        let t = CommentTable::builtin()
            .with_overrides("foo", vec![line(";")])
            .with_overrides("rs", vec![line("--")]);
        assert_eq!(t.leaders_for("foo"), Some(&[line(";")][..]));
        assert_eq!(t.leaders_for("rs"), Some(&[line("--")][..]));
        assert_eq!(t.leaders_for("ts"), Some(&[line("//"), block()][..]), "other entries keep their leaders");
    }

    #[test]
    fn r_syn_044__lookup_ignores_leading_dot_and_case() {
        let t = CommentTable::builtin();
        assert_eq!(t.leaders_for(".RS"), t.leaders_for("rs"));
        assert_eq!(t.leaders_for("Gitignore"), t.leaders_for("gitignore"));
    }

    #[test]
    fn r_syn_006__from_spec_reads_control_file_strings() {
        assert_eq!(Leader::from_spec("//"), Some(line("//")));
        assert_eq!(Leader::from_spec("# "), Some(line("#")));
        assert_eq!(Leader::from_spec("/* */"), Some(block()));
        assert_eq!(Leader::from_spec("<!-- -->"), Some(Leader::Html));
        assert_eq!(Leader::from_spec("{- -}"), Some(Leader::Block { open: "{-".into(), close: "-}".into() }));
        assert_eq!(Leader::from_spec(""), None);
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax delimiters::` — Expected: FAIL to compile: `cannot find type Leader in this scope`.

- [ ] **Step 3: Implement.** Above the tests:

```rust
//! Per-extension comment leaders (R-SYN-006, R-SYN-044).
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Leader {
    /// A leader that runs to the end of the line, e.g. `//`, `#`, `--`.
    Line(String),
    /// A delimited comment, e.g. `/* */`.
    Block { open: String, close: String },
    /// `<!-- -->`.
    Html,
}

impl Leader {
    /// Parses a `[comments]` value: `"//"`, `"/* */"`, `"<!-- -->"`, `"{- -}"`.
    pub fn from_spec(spec: &str) -> Option<Leader> {
        let spec = spec.trim();
        if spec.is_empty() {
            return None;
        }
        if spec == "<!-- -->" {
            return Some(Leader::Html);
        }
        match spec.split_once(' ') {
            Some((open, close)) if !open.is_empty() && !close.trim().is_empty() => {
                Some(Leader::Block { open: open.to_owned(), close: close.trim().to_owned() })
            }
            _ => Some(Leader::Line(spec.to_owned())),
        }
    }
}

/// Key: extension without the dot, lowercase.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommentTable(BTreeMap<String, Vec<Leader>>);

fn norm_ext(ext: &str) -> String {
    ext.trim_start_matches('.').to_ascii_lowercase()
}

impl CommentTable {
    /// The table of R-SYN-006.
    pub fn builtin() -> Self {
        let mut m: BTreeMap<String, Vec<Leader>> = BTreeMap::new();
        let slash = || {
            vec![
                Leader::Line("//".to_owned()),
                Leader::Block { open: "/*".to_owned(), close: "*/".to_owned() },
            ]
        };
        for e in ["rs", "ts", "tsx", "js", "jsx", "mjs", "go", "java", "kt", "swift", "c", "h", "cc", "cpp", "cs", "scala", "zig"] {
            m.insert(e.to_owned(), slash());
        }
        for e in ["css", "scss", "less"] {
            m.insert(e.to_owned(), vec![Leader::Block { open: "/*".to_owned(), close: "*/".to_owned() }]);
        }
        for e in ["py", "sh", "rb", "toml", "yaml", "yml", "ps1", "gitignore"] {
            m.insert(e.to_owned(), vec![Leader::Line("#".to_owned())]);
        }
        for e in ["sql", "lua", "hs"] {
            m.insert(e.to_owned(), vec![Leader::Line("--".to_owned())]);
        }
        for e in ["html", "md", "xml", "svg"] {
            m.insert(e.to_owned(), vec![Leader::Html]);
        }
        m.insert("json".to_owned(), Vec::new()); // comment-free: [[attach]] from 0.2 (R-SYN-047)
        CommentTable(m)
    }

    /// `[comments]` of the root control file adds or replaces an entry.
    pub fn with_overrides(mut self, ext: &str, leaders: Vec<Leader>) -> Self {
        self.0.insert(norm_ext(ext), leaders);
        self
    }

    /// `Some(&[])` = known but comment-free; `None` = unknown extension (HN-I002).
    pub fn leaders_for(&self, ext: &str) -> Option<&[Leader]> {
        self.0.get(&norm_ext(ext)).map(Vec::as_slice)
    }

    /// Sorted iteration, used for the config fingerprint.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<Leader>)> {
        self.0.iter()
    }
}
```

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax delimiters::` — Expected: PASS (5 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): built-in comment leader table with overrides" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

### Task 5: Lexer for code hosts (`lexer.rs`, part 1: comments and string literals)

**Rules:** R-FILE-013 (directives only in comment tokens; strings and raw strings never register), R-SYN-002 (doc-comment leaders `///`, `//!`, `/**`, `/*!` are flagged), R-SYN-030 (block comments are read line by line with a leading `*` stripped), R-SYN-017 input (inline flag), R-ARCH-032 (total).

**Files:**
- Create: `crates/fairlead-syntax/src/lexer.rs`
- Modify: `crates/fairlead-syntax/src/lib.rs` (`pub mod lexer;`)

**Interfaces:**
- Consumes: `normalize::{Normalized, slice}`, `delimiters::Leader`.
- Produces (crate-internal API, public for the integration tests): `CommentKind { Line, Block, Html }`, `CLine { line, start, end }` (`CLine::text(&Normalized) -> &str`), `Comment { kind, start, end, doc, inline, lines }`, `Lexed { comments, strings }`, `pub fn lex(n: &Normalized, ext: &str, leaders: &[Leader]) -> Lexed`. All offsets are byte offsets into `Normalized::text`.

Design notes. The lexer is a single left-to-right byte scan, O(n), no backtracking beyond `memmem` searches that always advance. Per-extension string rules: Rust has `"..."`, char literals `'x'` (lifetimes `'a` are NOT literals) and raw strings `r#"..."#`/`br"..."`; JS/TS family has `"`, `'` (to end of line), and backtick templates (multi-line); Go has char literals and backtick raw strings; Java/Kotlin/Swift/C#/Scala additionally have `"""`; Python/Ruby/TOML/PowerShell have `"`, `'`, and triple quotes; shell/YAML have `"`, `'` and treat `#` as a comment only at the start of a word; everything else gets `"` and `'` to end of line. Known false positives and negatives (JS regex literals, nested Rust block comments, C# `@"..."`) are the documented limit of the AST-free core (R-FILE-013).

- [ ] **Step 1: Write the failing test.** Create `lexer.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::delimiters::CommentTable;

    fn lex_str(src: &str, ext: &str) -> (Normalized, Lexed) {
        let n = Normalized::new(src);
        let table = CommentTable::builtin();
        let leaders = table.leaders_for(ext).unwrap().to_vec();
        let l = lex(&n, ext, &leaders);
        (n, l)
    }

    /// Each comment as its line texts joined with `|`.
    fn texts(n: &Normalized, l: &Lexed) -> Vec<String> {
        l.comments
            .iter()
            .map(|c| c.lines.iter().map(|x| x.text(n)).collect::<Vec<_>>().join("|"))
            .collect()
    }

    #[test]
    fn r_file_013__double_quoted_string_hides_comment_marker() {
        let (n, l) = lex_str("let s = \"// fairlead: use x\";\n// real\n", "rs");
        assert_eq!(texts(&n, &l), vec!["real"]);
        assert_eq!(l.strings.len(), 1);
    }

    #[test]
    fn r_file_013__rust_raw_strings_hide_comments() {
        let src = "let a = r#\"// fairlead: x \"quoted\" \"#;\nlet b = br\"// fairlead: y\";\nlet c = r##\"\"# // z\"##; // tail\n";
        let (n, l) = lex_str(src, "rs");
        assert_eq!(texts(&n, &l), vec!["tail"]);
    }

    #[test]
    fn r_file_013__rust_lifetimes_and_char_literals_are_not_strings() {
        let src = "fn f<'a>(x: &'a str) { let q = '\"'; let e = '\\''; } // tail\n";
        let (n, l) = lex_str(src, "rs");
        assert_eq!(texts(&n, &l), vec!["tail"]);
    }

    #[test]
    fn r_file_013__js_quotes_and_templates() {
        let src = "const a = '// no'; const b = `// no\n// still no`; // yes\n";
        let (n, l) = lex_str(src, "ts");
        assert_eq!(texts(&n, &l), vec!["yes"]);
    }

    #[test]
    fn r_file_013__unterminated_single_line_string_ends_at_newline() {
        let (n, l) = lex_str("x = 'abc\n// real\n", "ts");
        assert_eq!(texts(&n, &l), vec!["real"]);
    }

    #[test]
    fn r_file_013__hash_hosts_python_shell_toml() {
        let (n, l) = lex_str("x = \"# no\"  # yes\ny = '''\n# inside triple\n'''\n", "py");
        assert_eq!(texts(&n, &l), vec!["yes"]);
        let (n2, l2) = lex_str("echo ${#x} a#b # real\n", "sh");
        assert_eq!(texts(&n2, &l2), vec!["real"]);
        let (n3, l3) = lex_str("a = '#no'\n# yes\n", "toml");
        assert_eq!(texts(&n3, &l3), vec!["yes"]);
    }

    #[test]
    fn r_file_013__css_and_sql_hosts() {
        let (n, l) = lex_str("a { content: \"/* no */\"; } /* yes */\n", "css");
        assert_eq!(texts(&n, &l), vec!["yes"]);
        let (n2, l2) = lex_str("select '-- no'; -- yes\n", "sql");
        assert_eq!(texts(&n2, &l2), vec!["yes"]);
    }

    #[test]
    fn r_syn_030__block_comment_lines_strip_leading_star() {
        let (n, l) = lex_str("/*\n * fairlead: def tok.a\n * more\n */\ncode();\n", "rs");
        assert_eq!(l.comments.len(), 1);
        let c = &l.comments[0];
        assert_eq!(c.kind, CommentKind::Block);
        let lines: Vec<(u32, &str)> = c.lines.iter().map(|x| (x.line, x.text(&n))).collect();
        assert_eq!(lines, vec![(1, ""), (2, "fairlead: def tok.a"), (3, "more"), (4, "")]);
    }

    #[test]
    fn r_syn_030__unterminated_block_runs_to_eof() {
        let (n, l) = lex_str("/* fairlead: use x\nstill comment", "rs");
        assert_eq!(l.comments.len(), 1);
        assert_eq!(l.comments[0].end, n.text.len());
    }

    #[test]
    fn r_syn_002__doc_comment_leaders_are_flagged() {
        let src = "/// a\n//! b\n/** c */\n/*! d */\n//// e\n/**/\n/*** f */\n// g\n";
        let (_, l) = lex_str(src, "rs");
        let flags: Vec<bool> = l.comments.iter().map(|c| c.doc).collect();
        assert_eq!(flags, vec![true, true, true, true, false, false, false, false]);
    }

    #[test]
    fn r_syn_017__inline_flag_marks_comments_sharing_a_line_with_code() {
        let (_, l) = lex_str("let x = 1; // a\n// b\n/* c */ foo();\n  // d\n", "rs");
        let flags: Vec<bool> = l.comments.iter().map(|c| c.inline).collect();
        assert_eq!(flags, vec![true, false, true, false]);
    }

    #[test]
    fn r_syn_006__custom_leader_from_overrides() {
        let n = Normalized::new("x ; fairlead: use tt.b\n");
        let l = lex(&n, "foo", &[Leader::Line(";".to_owned())]);
        assert_eq!(texts(&n, &l), vec!["fairlead: use tt.b"]);
    }

    #[test]
    fn r_arch_032__lexer_is_total_on_hostile_input() {
        for src in ["'", "\"", "`", "/*", "r#\"", "br", "'\\", "<!--", "\u{0}\u{1}", "\u{e9}'\u{e9}", "'''", "\"\"\""] {
            let _ = lex_str(src, "rs");
            let _ = lex_str(src, "py");
            let _ = lex_str(src, "ts");
        }
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax lexer::` — Expected: FAIL to compile: `cannot find function lex / type Lexed in this scope`.

- [ ] **Step 3: Implement.** Above the tests in `lexer.rs`:

```rust
//! Comment-token lexer honouring string and raw-string literals (R-FILE-013, R-ARCH-030).
use crate::delimiters::Leader;
use crate::normalize::{slice, Normalized};
use core::ops::Range;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CommentKind {
    Line,
    Block,
    Html,
}

/// One logical line of a comment: leader, closing delimiter, leading `*` and outer blanks removed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CLine {
    pub line: u32,
    pub start: usize,
    pub end: usize,
}

impl CLine {
    pub fn text<'a>(&self, n: &'a Normalized) -> &'a str {
        slice(&n.text, self.start, self.end)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Comment {
    pub kind: CommentKind,
    /// Whole token including delimiters.
    pub start: usize,
    pub end: usize,
    /// `///`, `//!`, `/**`, `/*!` (R-SYN-002).
    pub doc: bool,
    /// Code shares a line with this comment (trailing form, R-SYN-017).
    pub inline: bool,
    pub lines: Vec<CLine>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Lexed {
    pub comments: Vec<Comment>,
    /// Byte ranges of string literals (used to mask text for bracket matching).
    pub strings: Vec<Range<usize>>,
}

// ---- byte helpers (all total) -------------------------------------------------------------

fn at(b: &[u8], i: usize) -> u8 {
    b.get(i).copied().unwrap_or(0)
}

fn starts(b: &[u8], i: usize, pat: &[u8]) -> bool {
    !pat.is_empty() && b.get(i..).is_some_and(|r| r.starts_with(pat))
}

fn find(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    let r = b.get(from..)?;
    memchr::memmem::find(r, pat).map(|p| p + from)
}

fn eol(b: &[u8], from: usize) -> usize {
    let r = b.get(from..).unwrap_or(&[]);
    memchr::memchr(b'\n', r).map_or(b.len(), |p| p + from)
}

fn is_blank(b: &[u8]) -> bool {
    b.iter().all(|c| matches!(c, b' ' | b'\t'))
}

fn trim_range(b: &[u8], mut s: usize, mut e: usize) -> (usize, usize) {
    while s < e && matches!(at(b, s), b' ' | b'\t') {
        s += 1;
    }
    while e > s && matches!(at(b, e - 1), b' ' | b'\t') {
        e -= 1;
    }
    (s, e)
}

fn utf8_len(lead: u8) -> usize {
    match lead {
        0..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

fn inline_flag(n: &Normalized, start: usize, end: usize) -> bool {
    let b = n.text.as_bytes();
    let l0 = n.line_of(start);
    let before = b.get(n.line_start(l0)..start).unwrap_or(&[]);
    let l1 = n.line_of(end.saturating_sub(1).max(start));
    let after = b.get(end.min(b.len())..n.line_end(l1).max(end.min(b.len()))).unwrap_or(&[]);
    !is_blank(before) || !is_blank(after)
}

// ---- per-extension string rules -----------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq)]
enum Sq {
    Off,
    Char,
    Str,
}

struct Rules {
    dq: bool,
    sq: Sq,
    backtick: bool,
    triple: bool,
    rust_raw: bool,
    hash_needs_space: bool,
}

fn rules_for(ext: &str) -> Rules {
    let r = |sq, backtick, triple, rust_raw, hash_needs_space| Rules {
        dq: true,
        sq,
        backtick,
        triple,
        rust_raw,
        hash_needs_space,
    };
    match ext {
        "rs" => r(Sq::Char, false, false, true, false),
        "ts" | "tsx" | "js" | "jsx" | "mjs" => r(Sq::Str, true, false, false, false),
        "go" => r(Sq::Char, true, false, false, false),
        "java" | "kt" | "swift" | "cs" | "scala" => r(Sq::Char, false, true, false, false),
        "c" | "h" | "cc" | "cpp" | "zig" => r(Sq::Char, false, false, false, false),
        "py" | "rb" | "toml" | "ps1" | "gitignore" => r(Sq::Str, false, true, false, false),
        "sh" | "yaml" | "yml" => r(Sq::Str, false, false, false, true),
        _ => r(Sq::Str, false, false, false, false),
    }
}

/// `"..."` / `'...'` ending at the closing quote or the newline; triple quotes span lines.
fn scan_quoted(b: &[u8], i: usize, q: u8, triple: bool) -> usize {
    if triple && at(b, i + 1) == q && at(b, i + 2) == q {
        return find(b, i + 3, &[q, q, q]).map_or(b.len(), |p| p + 3);
    }
    let mut j = i + 1;
    while j < b.len() {
        match at(b, j) {
            b'\\' => j += 2,
            c if c == q => return j + 1,
            b'\n' => return j,
            _ => j += 1,
        }
    }
    b.len()
}

/// Rust/C-family char literal at `i` (`'x'`, `'\n'`, `'\u{1f600}'`); a lifetime returns `i + 1`.
fn scan_char(b: &[u8], i: usize) -> usize {
    let n1 = at(b, i + 1);
    if n1 == b'\\' {
        let lim = (i + 14).min(b.len());
        let mut j = i + 3;
        while j < lim {
            match at(b, j) {
                b'\'' => return j + 1,
                b'\n' => break,
                _ => j += 1,
            }
        }
        return i + 1;
    }
    let l = utf8_len(n1);
    if n1 != 0 && n1 != b'\n' && n1 != b'\'' && at(b, i + 1 + l) == b'\'' {
        i + 2 + l
    } else {
        i + 1
    }
}

fn scan_backtick(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        match at(b, j) {
            b'\\' => j += 2,
            b'`' => return j + 1,
            _ => j += 1,
        }
    }
    b.len()
}

/// `r"..."`, `r#"..."#`, `br"..."` starting at `i`; `None` when `i` does not start one.
fn scan_rust_raw(b: &[u8], i: usize) -> Option<usize> {
    if i > 0 {
        let p = at(b, i - 1);
        if p.is_ascii_alphanumeric() || p == b'_' {
            return None;
        }
    }
    let mut j = i;
    if at(b, j) == b'b' {
        j += 1;
    }
    if at(b, j) != b'r' {
        return None;
    }
    j += 1;
    let mut hashes = 0usize;
    while at(b, j) == b'#' {
        hashes += 1;
        j += 1;
    }
    if at(b, j) != b'"' {
        return None;
    }
    let mut closer = vec![b'"'];
    closer.extend(std::iter::repeat_n(b'#', hashes));
    Some(find(b, j + 1, &closer).map_or(b.len(), |p| p + closer.len()))
}

// ---- comment tokens -----------------------------------------------------------------------

/// Splits the content `body_start..body_end` of a delimited comment into logical lines.
fn block_lines(n: &Normalized, body_start: usize, body_end: usize, strip_star: bool) -> Vec<CLine> {
    let b = n.text.as_bytes();
    let mut out = Vec::new();
    let mut a = body_start.min(body_end);
    let mut first = true;
    loop {
        let seg = b.get(a..body_end).unwrap_or(&[]);
        let nl = memchr::memchr(b'\n', seg);
        let seg_end = nl.map_or(body_end, |p| a + p);
        let mut s = a;
        if !first {
            while s < seg_end && matches!(at(b, s), b' ' | b'\t') {
                s += 1;
            }
            if strip_star && at(b, s) == b'*' && s < seg_end {
                s += 1;
            }
        }
        let (s, e) = trim_range(b, s, seg_end);
        out.push(CLine { line: n.line_of(a), start: s, end: e.max(s) });
        match nl {
            None => break,
            Some(_) => {
                a = seg_end + 1;
                first = false;
            }
        }
    }
    out
}

fn push_line_comment(n: &Normalized, out: &mut Lexed, i: usize, leader_len: usize, is_slash: bool) -> usize {
    let b = n.text.as_bytes();
    let end = eol(b, i);
    let doc = is_slash
        && leader_len == 2
        && ((at(b, i + 2) == b'/' && at(b, i + 3) != b'/') || at(b, i + 2) == b'!');
    let start = if doc { i + 3 } else { i + leader_len };
    let (s, e) = trim_range(b, start.min(end), end);
    out.comments.push(Comment {
        kind: CommentKind::Line,
        start: i,
        end,
        doc,
        inline: inline_flag(n, i, end),
        lines: vec![CLine { line: n.line_of(i), start: s, end: e }],
    });
    end.max(i + 1)
}

fn push_delimited(n: &Normalized, out: &mut Lexed, i: usize, open: &str, close: &str, kind: CommentKind) -> usize {
    let b = n.text.as_bytes();
    let body0 = i + open.len();
    let doc = kind == CommentKind::Block
        && open == "/*"
        && ((at(b, body0) == b'*' && at(b, body0 + 1) != b'*' && at(b, body0 + 1) != b'/') || at(b, body0) == b'!');
    let body_start = if doc { body0 + 1 } else { body0 };
    let (body_end, end) = match find(b, body_start.min(b.len()), close.as_bytes()) {
        Some(p) => (p, p + close.len()),
        None => (b.len(), b.len()),
    };
    out.comments.push(Comment {
        kind,
        start: i,
        end,
        doc,
        inline: inline_flag(n, i, end),
        lines: block_lines(n, body_start, body_end, kind == CommentKind::Block),
    });
    end.max(i + 1)
}

/// Lexes `n` for the leaders of `ext`. Total and linear.
pub fn lex(n: &Normalized, ext: &str, leaders: &[Leader]) -> Lexed {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    if leaders.iter().any(|l| matches!(l, Leader::Html)) {
        return lex_html(n, matches!(ext.as_str(), "md" | "markdown"));
    }
    lex_code(n, &ext, leaders)
}

fn lex_code(n: &Normalized, ext: &str, leaders: &[Leader]) -> Lexed {
    let b = n.text.as_bytes();
    let rules = rules_for(ext);
    let mut lines: Vec<&str> = leaders
        .iter()
        .filter_map(|l| match l {
            Leader::Line(s) if !s.is_empty() => Some(s.as_str()),
            _ => None,
        })
        .collect();
    lines.sort_by_key(|s| core::cmp::Reverse(s.len()));
    let blocks: Vec<(&str, &str)> = leaders
        .iter()
        .filter_map(|l| match l {
            Leader::Block { open, close } if !open.is_empty() && !close.is_empty() => Some((open.as_str(), close.as_str())),
            _ => None,
        })
        .collect();
    let mut out = Lexed::default();
    let mut i = 0usize;
    'scan: while i < b.len() {
        for l in &lines {
            let word_start = i == 0 || matches!(at(b, i - 1), b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(');
            if starts(b, i, l.as_bytes()) && (!rules.hash_needs_space || *l != "#" || word_start) {
                i = push_line_comment(n, &mut out, i, l.len(), *l == "//");
                continue 'scan;
            }
        }
        for (open, close) in &blocks {
            if starts(b, i, open.as_bytes()) {
                i = push_delimited(n, &mut out, i, open, close, CommentKind::Block);
                continue 'scan;
            }
        }
        match at(b, i) {
            b'"' if rules.dq => {
                let e = scan_quoted(b, i, b'"', rules.triple).max(i + 1);
                out.strings.push(i..e);
                i = e;
            }
            b'\'' => match rules.sq {
                Sq::Str => {
                    let e = scan_quoted(b, i, b'\'', rules.triple).max(i + 1);
                    out.strings.push(i..e);
                    i = e;
                }
                Sq::Char => {
                    let e = scan_char(b, i);
                    if e > i + 1 {
                        out.strings.push(i..e);
                    }
                    i = e.max(i + 1);
                }
                Sq::Off => i += 1,
            },
            b'`' if rules.backtick => {
                let e = scan_backtick(b, i).max(i + 1);
                out.strings.push(i..e);
                i = e;
            }
            b'r' | b'b' if rules.rust_raw => match scan_rust_raw(b, i) {
                Some(e) => {
                    out.strings.push(i..e.max(i + 1));
                    i = e.max(i + 1);
                }
                None => i += 1,
            },
            _ => i += 1,
        }
    }
    out
}

/// HTML-family lexing; Markdown fences and code are handled in Task 6.
fn lex_html(n: &Normalized, _markdown: bool) -> Lexed {
    let b = n.text.as_bytes();
    let mut out = Lexed::default();
    let mut i = 0usize;
    while i < b.len() {
        if starts(b, i, b"<!--") {
            i = push_delimited(n, &mut out, i, "<!--", "-->", CommentKind::Html);
        } else {
            i += 1;
        }
    }
    out
}
```

Verify: `std::iter::repeat_n` is stable since 1.82. `Vec::extend(repeat_n(..))` is fine. Clippy may flag the closure `r` with five parameters (`too_many_arguments` does not apply to closures); if it does, replace the closure by a small `const fn rules(..)`.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax lexer::` — Expected: PASS (13 tests). The five Task 6 tests do not exist yet.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): lexer for code hosts honouring string and raw-string literals" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Lexer for HTML and Markdown (`lexer.rs`, part 2)

**Rules:** R-SYN-005 (Markdown honours `<!-- fairlead: ... -->` outside fenced and indented code), R-SYN-044 (HTML/XML/SVG: a comment inside an attribute string is not honoured), R-FILE-013.

**Files:**
- Modify: `crates/fairlead-syntax/src/lexer.rs` (replace the stub `lex_html`, add helpers, add tests)

**Interfaces:** Consumes `lex` from Task 5. Produces the same `Lexed` for leaders `[Leader::Html]`; behaviour added: Markdown fenced code (``` and ~~~, closing fence at least as long, up to 3 leading spaces), indented code (4 spaces or a tab after a blank line, continuing while indented), inline code spans (equal-length backtick runs on one line), HTML tag attribute strings.

- [ ] **Step 1: Write the failing test.** Append inside the existing `mod tests` of `lexer.rs`:

```rust
    #[test]
    fn r_syn_005__markdown_comment_outside_fences_only() {
        let src = "# T\n\n```md\n<!-- fairlead: use no.fence -->\n```\n\n<!-- fairlead: use yes.one -->\n\n~~~~\n<!-- fairlead: use no.tilde -->\n~~~\n<!-- still inside -->\n~~~~\n<!-- fairlead: use yes.two -->\n";
        let (n, l) = lex_str(src, "md");
        assert_eq!(texts(&n, &l), vec!["fairlead: use yes.one", "fairlead: use yes.two"]);
    }

    #[test]
    fn r_syn_005__markdown_indented_code_and_inline_code_are_not_scanned() {
        let src = "para\n\n    <!-- fairlead: use no.indented -->\n\nText `<!-- fairlead: use no.inline -->` and <!-- fairlead: use yes.x -->\n";
        let (n, l) = lex_str(src, "md");
        assert_eq!(texts(&n, &l), vec!["fairlead: use yes.x"]);
    }

    #[test]
    fn r_syn_005__indented_text_directly_after_a_paragraph_is_not_code() {
        let src = "para\n    <!-- fairlead: use yes.y -->\n";
        let (n, l) = lex_str(src, "md");
        assert_eq!(texts(&n, &l), vec!["fairlead: use yes.y"]);
    }

    #[test]
    fn r_syn_005__multi_line_html_comment_yields_one_line_per_source_line() {
        let src = "<!-- fairlead: ovr rule.x\n     why=\"a\"\n-->\ntext\n";
        let (n, l) = lex_str(src, "md");
        assert_eq!(l.comments.len(), 1);
        let got: Vec<(u32, &str)> = l.comments[0].lines.iter().map(|x| (x.line, x.text(&n))).collect();
        assert_eq!(got, vec![(1, "fairlead: ovr rule.x"), (2, "why=\"a\""), (3, "")]);
        assert_eq!(l.comments[0].kind, CommentKind::Html);
    }

    #[test]
    fn r_syn_044__html_attribute_strings_do_not_hold_comments() {
        let src = "<a title=\"<!-- fairlead: use no.attr -->\" href='x'>t</a>\n<!-- fairlead: use yes.z -->\n";
        let (n, l) = lex_str(src, "html");
        assert_eq!(texts(&n, &l), vec!["fairlead: use yes.z"]);
    }

    #[test]
    fn r_syn_044__xml_and_svg_use_the_same_comment_syntax() {
        let (n, l) = lex_str("<?xml version=\"1.0\"?>\n<!-- fairlead: def asset.logo -->\n<svg/>\n", "svg");
        assert_eq!(texts(&n, &l), vec!["fairlead: def asset.logo"]);
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_005 r_syn_044` (two filters: run `cargo test -p fairlead-syntax lexer::tests::r_syn_005` then `...r_syn_044`) — Expected: FAIL: assertion mismatch (`yes.one`, `yes.two` expected but fenced comments are also returned because the stub `lex_html` ignores fences; the attribute test returns two comments).

- [ ] **Step 3: Implement.** Replace the stub `lex_html` with:

```rust
/// Skips a backtick code span that starts at `i` (equal-length runs, one line); returns the next index.
fn skip_code_span(b: &[u8], i: usize) -> usize {
    let mut n = 0usize;
    while at(b, i + n) == b'`' {
        n += 1;
    }
    let e = eol(b, i);
    let mut j = i + n;
    while j < e {
        if at(b, j) == b'`' {
            let mut m = 0usize;
            while at(b, j + m) == b'`' {
                m += 1;
            }
            if m == n {
                return j + m;
            }
            j += m;
        } else {
            j += 1;
        }
    }
    i + n
}

/// Skips an HTML start tag at `i` (`<` plus a letter), honouring quoted attribute values.
fn skip_tag(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        match at(b, j) {
            b'>' => return j + 1,
            q @ (b'"' | b'\'') => match find(b, j + 1, &[q]) {
                Some(p) => j = p + 1,
                None => return b.len(),
            },
            _ => j += 1,
        }
    }
    b.len()
}

/// `<!-- -->` comments. For Markdown also: fenced code, indented code, inline code spans (R-SYN-005).
fn lex_html(n: &Normalized, markdown: bool) -> Lexed {
    let b = n.text.as_bytes();
    let mut out = Lexed::default();
    let mut i = 0usize;
    let mut fence: Option<(u8, usize)> = None;
    let mut prev_ok = true; // previous line blank or indented code: an indented code block may start
    while i < b.len() {
        let line_start = i == 0 || at(b, i - 1) == b'\n';
        if markdown && line_start {
            let e = eol(b, i);
            let line = b.get(i..e).unwrap_or(&[]);
            let lead = line.iter().take_while(|c| **c == b' ').count();
            let blank = is_blank(line);
            if let Some((ch, len)) = fence {
                let rest = line.get(lead..).unwrap_or(&[]);
                if lead <= 3 && rest.len() >= len && rest.iter().all(|c| *c == ch) {
                    fence = None;
                }
                prev_ok = false;
                i = e + 1;
                continue;
            }
            let ch = at(line, lead);
            if lead <= 3 && (ch == b'`' || ch == b'~') {
                let run = line.get(lead..).unwrap_or(&[]).iter().take_while(|c| **c == ch).count();
                if run >= 3 {
                    fence = Some((ch, run));
                    prev_ok = false;
                    i = e + 1;
                    continue;
                }
            }
            if !blank && prev_ok && (at(line, 0) == b'\t' || lead >= 4) {
                i = e + 1; // indented code line; prev_ok stays true
                continue;
            }
            prev_ok = blank;
        }
        match at(b, i) {
            b'<' if starts(b, i, b"<!--") => {
                i = push_delimited(n, &mut out, i, "<!--", "-->", CommentKind::Html);
            }
            b'`' if markdown => i = skip_code_span(b, i).max(i + 1),
            b'<' if !markdown && at(b, i + 1).is_ascii_alphabetic() => i = skip_tag(b, i).max(i + 1),
            _ => i += 1,
        }
    }
    out
}
```

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax lexer::` — Expected: PASS (19 tests). Trace for `r_syn_005__indented_text_directly_after_a_paragraph_is_not_code`: line 1 `para` is not blank so `prev_ok = false`; line 2 is indented but `prev_ok` is false, so it is scanned and the comment is found.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): Markdown fences, indented code and HTML attribute strings in the lexer" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

### Task 7: Marker-line classification (`lexer.rs`, part 3)

**Rules:** R-SYN-001 (first token after the leader is `<marker>:`, or `<marker>|` for a continuation), R-SYN-003 (case-sensitive; near-misses are `HN-S005` warn and not directives), R-SYN-004 (`<marker>\:` is ignored silently), R-SYN-002 input (doc flag handled in Task 8).

**Files:**
- Modify: `crates/fairlead-syntax/src/lexer.rs` (add `MarkerLine` and `classify_marker_line`; add tests)

**Interfaces:**
- Consumes: `CLine::text` from Task 5.
- Produces: `pub enum MarkerLine { None, Start { body: usize }, Cont { body: usize }, NearMiss, Escaped }` and `pub fn classify_marker_line(text: &str, marker: &str) -> MarkerLine`. `body` is the byte offset in `text` where the directive body (or continuation tail) begins, after the `:`/`|` and following blanks. The marker is never interpreted as a regex or glob.

- [ ] **Step 1: Write the failing test.** Append inside `mod tests`:

```rust
    #[test]
    fn r_syn_001__start_and_continuation_markers() {
        assert_eq!(classify_marker_line("fairlead: def cmp.x", "fairlead"), MarkerLine::Start { body: 7 });
        assert_eq!(classify_marker_line("fairlead:   use tt.b", "fairlead"), MarkerLine::Start { body: 9 });
        assert_eq!(classify_marker_line("fairlead| why=\"x\"", "fairlead"), MarkerLine::Cont { body: 7 });
        assert_eq!(classify_marker_line("fairlead:", "fairlead"), MarkerLine::Start { body: 6 });
        assert_eq!(classify_marker_line("fairlead: use tt.b", "fairlead"), MarkerLine::Start { body: 10 });
    }

    #[test]
    fn r_syn_001__ordinary_comments_are_not_markers() {
        for t in ["", "fairleads: x", "the fairlead: x", "fairlead is a word", "fairleadX: y", "fairlead", "TODO fairlead: x"] {
            assert_eq!(classify_marker_line(t, "fairlead"), MarkerLine::None, "{t}");
        }
        assert_eq!(classify_marker_line("fairlead: x", ""), MarkerLine::None, "empty marker never matches");
    }

    #[test]
    fn r_syn_003__near_misses_are_reported_not_parsed() {
        for t in ["Fairlead: x", "FAIRLEAD: x", "fairlead; x", "fairlead - x", "fairlead : x", "Fairlead| x", "fairlead ; x"] {
            assert_eq!(classify_marker_line(t, "fairlead"), MarkerLine::NearMiss, "{t}");
        }
        assert_eq!(classify_marker_line("fairlead -- note", "fairlead"), MarkerLine::None);
        assert_eq!(classify_marker_line("Fairlead is fine", "fairlead"), MarkerLine::None);
    }

    #[test]
    fn r_syn_004__backslash_colon_is_silently_ignored() {
        assert_eq!(classify_marker_line("fairlead\\: def x", "fairlead"), MarkerLine::Escaped);
    }

    #[test]
    fn r_syn_001__non_ascii_marker_does_not_panic() {
        assert_eq!(classify_marker_line("h\u{e4}rna: x", "h\u{e4}rna"), MarkerLine::Start { body: 8 });
        assert_eq!(classify_marker_line("h\u{e4}", "fairlead"), MarkerLine::None);
        assert_eq!(classify_marker_line("\u{e9}", "fairlead"), MarkerLine::None);
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_001` and `... r_syn_003` and `... r_syn_004` (three runs) — Expected: FAIL to compile: `cannot find function classify_marker_line`.

- [ ] **Step 3: Implement.** Add to `lexer.rs` above `mod tests`:

```rust
/// What one comment line is, with respect to the marker (R-SYN-001, 003, 004).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MarkerLine {
    None,
    Start { body: usize },
    Cont { body: usize },
    NearMiss,
    Escaped,
}

fn skip_blanks(s: &str, from: usize) -> usize {
    let b = s.as_bytes();
    let mut j = from;
    while matches!(at(b, j), b' ' | b'\t') {
        j += 1;
    }
    j.min(s.len())
}

/// `text` is a [`CLine`] text (outer blanks already trimmed).
pub fn classify_marker_line(text: &str, marker: &str) -> MarkerLine {
    let m = marker.len();
    if m == 0 {
        return MarkerLine::None;
    }
    let (Some(head), Some(rest)) = (text.get(..m), text.get(m..)) else {
        return MarkerLine::None;
    };
    let rb = rest.as_bytes();
    if head == marker {
        return match at(rb, 0) {
            b':' => MarkerLine::Start { body: skip_blanks(text, m + 1) },
            b'|' => MarkerLine::Cont { body: skip_blanks(text, m + 1) },
            b'\\' if at(rb, 1) == b':' => MarkerLine::Escaped,
            b';' => MarkerLine::NearMiss,
            b' ' | b'\t' => {
                let j = skip_blanks(rest, 0);
                match at(rb, j) {
                    b':' | b';' | b'|' => MarkerLine::NearMiss,
                    b'-' if at(rb, j + 1) != b'-' => MarkerLine::NearMiss,
                    _ => MarkerLine::None,
                }
            }
            _ => MarkerLine::None,
        };
    }
    if head.eq_ignore_ascii_case(marker) {
        let j = skip_blanks(rest, 0);
        if matches!(at(rb, j), b':' | b';' | b'|' | b'-') {
            return MarkerLine::NearMiss;
        }
    }
    MarkerLine::None
}
```

Check against the tests: `"fairlead : x"` has `rest = " : x"`, `j = 1`, byte `:` -> NearMiss (the `b' '` arm). `"fairlead - x"` -> `-` followed by space -> NearMiss. `"fairlead -- note"` -> `-` followed by `-` -> None. `"Fairlead is fine"` -> case differs, first non-blank after the marker is `i` -> None. `"fairlead:"` -> `skip_blanks(text, 6)` = 6 (`text.len()`), `body: 6` as asserted. `"h\u{e4}rna"` is 6 bytes (`ä` is 2), so `"h\u{e4}rna: x"` gives `body: 8`.

- [ ] **Step 4: Run** the three filters above — Expected: PASS (5 new tests, 24 in `lexer::`).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): marker-line classification with near-miss and escape handling" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Candidate assembly with continuation lines (`lexer.rs`, part 4)

**Rules:** R-SYN-028 (continuation lines start `marker|`; at most 8 continuation lines and 1 KB per directive, else `HN-S006`), R-SYN-001 (every comment line starting `marker:` is a candidate), R-ARCH-031 input (orphan `marker|` lines are candidates too, so they get a diagnostic, never silent), R-SYN-003 (near-misses collected for `HN-S005`).

**Files:**
- Modify: `crates/fairlead-syntax/src/lexer.rs`

**Interfaces:**
- Consumes: `Lexed`, `Comment`, `CLine`, `classify_marker_line`.
- Produces: `Candidate { comment, first_line, last_line, marker_start, body, segs, orphan, too_long, doc, inline }` with `Candidate::norm_offset(&self, body_off: usize) -> usize`; `Scan { candidates, near_misses, escaped }`; `pub fn scan_markers(n: &Normalized, lexed: &Lexed, marker: &str) -> Scan`. `near_misses` holds the normalized offset of the near-miss text start and `escaped` the count of silently ignored `marker\:` lines.

A continuation joins the directive only when it sits on the physically next line (`line == prev + 1`) in the flattened comment-line stream (so `// fairlead|` lines in separate `//` tokens and consecutive lines of one block comment both work). Joined text uses exactly one space between segments. The first continuation beyond 8, or one that would push the body past 1024 bytes, sets `too_long`; remaining contiguous continuation lines are still consumed so they do not become orphans.

- [ ] **Step 1: Write the failing test.** Append inside `mod tests`:

```rust
    fn scan(src: &str, ext: &str) -> (Normalized, Scan) {
        let (n, l) = lex_str(src, ext);
        let s = scan_markers(&n, &l, "fairlead");
        (n, s)
    }

    #[test]
    fn r_syn_028__continuation_lines_join_with_one_space() {
        let src = "// fairlead: ovr rule.x owner=@a exp=2026-12-31\n// fairlead| why=\"legacy\"\n// fairlead|   value=#ff0000\nlet x = 1;\n";
        let (n, s) = scan(src, "rs");
        assert_eq!(s.candidates.len(), 1);
        let c = &s.candidates[0];
        assert_eq!(c.body, "ovr rule.x owner=@a exp=2026-12-31 why=\"legacy\" value=#ff0000");
        assert_eq!((c.first_line, c.last_line), (1, 3));
        assert!(!c.too_long && !c.orphan && !c.doc);
        let why = c.body.find("why").unwrap();
        assert_eq!(&n.text[c.norm_offset(why)..c.norm_offset(why) + 3], "why");
        let value = c.body.find("value").unwrap();
        assert_eq!(&n.text[c.norm_offset(value)..c.norm_offset(value) + 5], "value");
        assert_eq!(c.norm_offset(0), c.marker_start + "fairlead: ".len());
    }

    #[test]
    fn r_syn_028__more_than_eight_continuations_is_too_long() {
        let mut src = String::from("// fairlead: ovr rule.x why=\"a\"\n");
        for _ in 0..9 {
            src.push_str("// fairlead| k=v\n");
        }
        let (_, s) = scan(&src, "rs");
        assert_eq!(s.candidates.len(), 1, "extra continuations are consumed, not orphaned");
        assert!(s.candidates[0].too_long);
        let mut ok = String::from("// fairlead: ovr rule.x why=\"a\"\n");
        for _ in 0..8 {
            ok.push_str("// fairlead| k=v\n");
        }
        assert!(!scan(&ok, "rs").1.candidates[0].too_long);
    }

    #[test]
    fn r_syn_028__body_over_one_kilobyte_is_too_long() {
        let src = format!("// fairlead: ovr rule.x why=\"{}\"\n// fairlead| k=v\n", "a".repeat(1100));
        let (_, s) = scan(&src, "rs");
        assert!(s.candidates[0].too_long);
    }

    #[test]
    fn r_syn_028__a_gap_ends_the_continuation_and_the_stray_line_is_an_orphan() {
        let src = "// fairlead: use tt.b\n\n// fairlead| k=v\n";
        let (_, s) = scan(src, "rs");
        assert_eq!(s.candidates.len(), 2);
        assert!(!s.candidates[0].orphan);
        assert!(s.candidates[1].orphan);
        assert_eq!(s.candidates[1].body, "k=v");
    }

    #[test]
    fn r_syn_028__block_comment_lines_can_continue() {
        let src = "/*\n * fairlead: use tt.b\n * fairlead| sym=X\n */\n";
        let (_, s) = scan(src, "rs");
        assert_eq!(s.candidates.len(), 1);
        assert_eq!(s.candidates[0].body, "use tt.b sym=X");
        assert_eq!((s.candidates[0].first_line, s.candidates[0].last_line), (2, 3));
    }

    #[test]
    fn r_syn_002__doc_and_inline_flags_reach_the_candidate() {
        let (_, s) = scan("/// fairlead: use tt.b\nlet x = 1; // fairlead: use uu.d\n", "rs");
        assert!(s.candidates[0].doc && !s.candidates[0].inline);
        assert!(!s.candidates[1].doc && s.candidates[1].inline);
    }

    #[test]
    fn r_syn_003__near_misses_and_escapes_are_not_candidates() {
        let src = "// Fairlead: use tt.b\n// fairlead; use tt.b\n// fairlead\\: use tt.b\n// fairlead: use tt.b\n";
        let (n, s) = scan(src, "rs");
        assert_eq!(s.candidates.len(), 1);
        assert_eq!(s.near_misses.len(), 2);
        assert_eq!(&n.text[s.near_misses[0]..s.near_misses[0] + 5], "Fairlead");
        assert_eq!(s.escaped, 1);
    }

    #[test]
    fn r_syn_001__marker_in_a_string_is_not_a_candidate() {
        let (_, s) = scan("let s = \"// fairlead: use tt.b\";\n", "rs");
        assert!(s.candidates.is_empty());
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_028` — Expected: FAIL to compile: `cannot find function scan_markers`.

- [ ] **Step 3: Implement.** Add to `lexer.rs` above `mod tests`:

```rust
/// One marker candidate: a start line plus its joined continuation lines (R-SYN-028).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Candidate {
    /// Index into `Lexed::comments` of the comment holding the first line.
    pub comment: usize,
    pub first_line: u32,
    pub last_line: u32,
    /// Normalized offset of the marker on the first line.
    pub marker_start: usize,
    /// Directive body: text after `marker:`, continuation tails joined with one space.
    pub body: String,
    /// `(body offset, normalized offset)` per joined segment, ascending.
    pub segs: Vec<(usize, usize)>,
    /// A `marker|` line with no directive start before it.
    pub orphan: bool,
    /// More than 8 continuation lines or more than 1 KB (HN-S006).
    pub too_long: bool,
    pub doc: bool,
    pub inline: bool,
}

impl Candidate {
    /// Maps an offset inside `body` to an offset in the normalized text.
    pub fn norm_offset(&self, body_off: usize) -> usize {
        let idx = self.segs.partition_point(|s| s.0 <= body_off);
        match idx.checked_sub(1).and_then(|k| self.segs.get(k)) {
            Some(&(b0, n0)) => n0 + (body_off - b0),
            None => self.marker_start,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Scan {
    pub candidates: Vec<Candidate>,
    /// Normalized offsets of near-miss comment texts (HN-S005).
    pub near_misses: Vec<usize>,
    /// Count of silently ignored `marker\:` lines (R-SYN-004).
    pub escaped: u32,
}

pub const MAX_CONTINUATIONS: usize = 8;
pub const MAX_BODY_BYTES: usize = 1024;

/// Finds every marker candidate in the comment lines of `lexed`.
pub fn scan_markers(n: &Normalized, lexed: &Lexed, marker: &str) -> Scan {
    let mut flat: Vec<(usize, &CLine, MarkerLine)> = Vec::new();
    for (ci, c) in lexed.comments.iter().enumerate() {
        for cl in &c.lines {
            flat.push((ci, cl, classify_marker_line(cl.text(n), marker)));
        }
    }
    let mut out = Scan::default();
    let mut k = 0usize;
    while let Some(&(ci, cl, class)) = flat.get(k) {
        k += 1;
        let Some(comment) = lexed.comments.get(ci) else { continue };
        match class {
            MarkerLine::None => {}
            MarkerLine::NearMiss => out.near_misses.push(cl.start),
            MarkerLine::Escaped => out.escaped += 1,
            MarkerLine::Start { body } | MarkerLine::Cont { body } => {
                let orphan = matches!(class, MarkerLine::Cont { .. });
                let first = slice(cl.text(n), body, cl.text(n).len());
                let mut cand = Candidate {
                    comment: ci,
                    first_line: cl.line,
                    last_line: cl.line,
                    marker_start: cl.start,
                    body: first.to_owned(),
                    segs: vec![(0, cl.start + body)],
                    orphan,
                    too_long: false,
                    doc: comment.doc,
                    inline: comment.inline,
                };
                let mut conts = 0usize;
                while let Some(&(_, cl2, class2)) = flat.get(k) {
                    let MarkerLine::Cont { body: b2 } = class2 else { break };
                    if cl2.line != cand.last_line + 1 {
                        break;
                    }
                    k += 1;
                    conts += 1;
                    cand.last_line = cl2.line;
                    let tail = slice(cl2.text(n), b2, cl2.text(n).len());
                    if conts <= MAX_CONTINUATIONS && cand.body.len() + 1 + tail.len() <= MAX_BODY_BYTES {
                        cand.body.push(' ');
                        let off = cand.body.len();
                        cand.body.push_str(tail);
                        cand.segs.push((off, cl2.start + b2));
                    } else {
                        cand.too_long = true;
                    }
                }
                if cand.body.len() > MAX_BODY_BYTES {
                    cand.too_long = true;
                }
                out.candidates.push(cand);
            }
        }
    }
    out
}
```

Note: for an orphan the `while` loop still joins following contiguous `marker|` lines into the orphan candidate (one diagnostic for the whole group).

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax lexer::` — Expected: PASS (32 tests). If `r_syn_028__body_over_one_kilobyte_is_too_long` fails, check that the first body (1100 bytes) exceeds 1024: `cand.body.len() > MAX_BODY_BYTES` after the loop sets `too_long`.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): marker candidates with continuation assembly and limits" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

### Task 9: Directive types and the winnow body tokenizer (`directive.rs`, part 1)

**Rules:** R-SYN-007 (normative grammar; non-ASCII only inside `quoted` and `note`; NFC; list items are words so `name:type` works), R-SYN-008 (`note` is data), R-SYN-009 (`!` directly before the verb; unknown verb is always `HN-S001`), R-SYN-013 (`k=v` and `k+=a,b`), R-SYN-029 (quote escapes `\"` and `\\` only; otherwise `HN-S007`), R-SYN-032 (every failure carries code, position, expected form, did-you-mean).

**Files:**
- Create: `crates/fairlead-syntax/src/directive.rs`
- Modify: `crates/fairlead-syntax/src/lib.rs` (`pub mod directive;`)

**Interfaces:**
- Consumes: `diag::{codes, did_you_mean, DiagCode, Span}`, `ids::Id`, `normalize::nfc`.
- Produces (contract 4.2, exact): `Verb { Def, Use, Ovr, Flag, Env, End }`, `ArgOp { Set, Append }`, `ArgValue { Word(String), Quoted(String), List(Vec<String>) }`, `Arg { key, op, value, span }`, `Carrier { LineComment, BlockComment, HtmlComment, Frontmatter }`, `Extent { File, Doc, Lines { first, last } }`, `Directive { ordinal, verb, must_understand, ids, bare, args, note, carrier, span, extent, end_span }`, `Directive::arg(&self, key)`. Crate-internal: `Explicit { Lines(u32), Region }`, `ParsedBody`, `BodyError`, `parse_body(&str) -> Result<ParsedBody, BodyError>`. `ParsedBody.args[i].span` is `Span::whole_file()` until `extract` fills it from `arg_pos[i]` (body offsets).

Design: the body (text after `marker:` with continuations joined, or a frontmatter string) is tokenized by small winnow parsers (`take_while`, `alt`, string literals as parsers) driven by a hand-written loop so that every error has an exact byte offset inside the body. Classification of words per verb happens after tokenizing. Tabs are already spaces (Task 3).

- [ ] **Step 1: Write the failing test.** Create `directive.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ok(body: &str) -> ParsedBody {
        parse_body(body).unwrap_or_else(|e| panic!("{body:?}: {e:?}"))
    }
    fn err(body: &str) -> BodyError {
        parse_body(body).expect_err(body)
    }

    #[test]
    fn r_syn_007__pairs_words_extent_and_note() {
        let p = ok("def cmp.sync-button sym=SyncButton layer=molecule +5 -- hello world");
        assert_eq!(p.verb, Verb::Def);
        assert_eq!(p.ids.len(), 1);
        assert_eq!(p.args.len(), 2);
        assert_eq!(p.args[0].key, "sym");
        assert_eq!(p.args[0].value, ArgValue::Word("SyncButton".into()));
        assert_eq!(p.explicit, Some(Explicit::Lines(5)));
        assert_eq!(p.note.as_deref(), Some("hello world"));
        assert_eq!(p.arg_pos[0], (20, 34), "byte range of `sym=SyncButton` inside the body");
    }

    #[test]
    fn r_syn_013__plus_equals_is_the_append_marker() {
        let p = ok("ovr rule.x kw+=a,b why=\"z\"");
        assert_eq!(p.args[0].op, ArgOp::Append);
        assert_eq!(p.args[0].value, ArgValue::List(vec!["a".into(), "b".into()]));
        assert_eq!(p.args[1].op, ArgOp::Set);
    }

    #[test]
    fn r_syn_007__value_forms_word_quoted_list() {
        let p = ok("use tt.b sym=\"x y\" params=id:string,limit:u32 c=#ff0000 o=@platform p=src/a*.rs");
        assert_eq!(p.args[0].value, ArgValue::Quoted("x y".into()));
        assert_eq!(p.args[1].value, ArgValue::List(vec!["id:string".into(), "limit:u32".into()]));
        assert_eq!(p.args[2].value, ArgValue::Word("#ff0000".into()));
        assert_eq!(p.args[3].value, ArgValue::Word("@platform".into()));
        assert_eq!(p.args[4].value, ArgValue::Word("src/a*.rs".into()));
    }

    #[test]
    fn r_syn_029__quote_escapes_are_only_quote_and_backslash() {
        let p = ok(r#"use tt.b why="say \"hi\" \\ done""#);
        assert_eq!(p.args[0].value, ArgValue::Quoted(r#"say "hi" \ done"#.into()));
    }

    #[test]
    fn r_syn_029__unterminated_quote_and_unknown_escape_are_s007() {
        let e = err("use tt.b why=\"abc");
        assert_eq!(e.code, codes::HN_S007);
        assert_eq!(e.at, 17, "points at the end of the text where the quote should close");
        let e2 = err(r#"use tt.b why="a\nb""#);
        assert_eq!(e2.code, codes::HN_S007);
        assert_eq!(e2.at, 15, "points at the backslash");
        assert_eq!(err("use tt.b why=\"a\u{0}b\nc\"").code, codes::HN_S007, "no newline inside quotes");
    }

    #[test]
    fn r_syn_008__note_is_data_and_never_parsed() {
        let p = ok("use tt.b -- def xx.y exp=oops \"unterminated");
        assert_eq!(p.note.as_deref(), Some("def xx.y exp=oops \"unterminated"));
        assert!(p.args.is_empty());
        assert_eq!(ok("use tt.b --").note.as_deref().unwrap_or(""), "");
    }

    #[test]
    fn r_syn_009__unknown_verb_is_s001_with_suggestion() {
        let e = err("frobnicate tt.b");
        assert_eq!(e.code, codes::HN_S001);
        assert_eq!(e.did_you_mean, None);
        let e2 = err("deff tt.b");
        assert_eq!(e2.did_you_mean.as_deref(), Some("def"));
        assert_eq!((e2.at, e2.len), (0, 4));
        assert_eq!(err("!usee tt.b").at, 1, "the bang is not part of the verb");
    }

    #[test]
    fn r_syn_009__bang_sits_directly_before_the_verb() {
        assert!(ok("!def tt.b").must_understand);
        assert!(!ok("def tt.b").must_understand);
        assert_eq!(err("def !tt.b").code, codes::HN_S016);
    }

    #[test]
    fn r_syn_007__stray_characters_are_s016() {
        assert_eq!(err("def tt.b(").code, codes::HN_S016);
        assert_eq!(err("def tt.b k=").code, codes::HN_S016);
        assert_eq!(err("def tt.b k=,").code, codes::HN_S016, "empty list item");
        assert_eq!(err("def tt.b +0").code, codes::HN_S016);
        assert_eq!(err("def tt.b +x").code, codes::HN_S016);
        assert_eq!(err("").code, codes::HN_S016);
        assert_eq!(err("def tt.b k=v=w").code, codes::HN_S016);
    }

    #[test]
    fn r_syn_007__non_ascii_only_inside_quotes_and_note() {
        assert!(parse_body("use tt.b why=\"caf\u{e9}\" -- na\u{ef}ve").is_ok());
        assert_eq!(err("use tt.b k=caf\u{e9}").code, codes::HN_S016);
        assert_eq!(err("use caf\u{e9}.x").code, codes::HN_S016);
    }

    #[test]
    fn r_syn_007__quoted_and_note_are_nfc_normalised() {
        let p = ok("use tt.b why=\"e\u{301}\" -- e\u{301}");
        assert_eq!(p.args[0].value, ArgValue::Quoted("\u{e9}".into()));
        assert_eq!(p.note.as_deref(), Some("\u{e9}"));
    }

    #[test]
    fn r_syn_007__parse_body_is_total() {
        for s in ["!", "!!def", "def \"", "def tt.b +", "def tt.b k+", "def tt.b k+=", "--", "def -- ", "\u{0}", "def tt.b \u{e9}", "= =", "def  tt.b"] {
            let _ = parse_body(s);
        }
    }
}
```

(`"def  tt.b"` with two spaces is accepted: `1*SP` separates parts. `parse_body` receives a trimmed body, so a trailing space never occurs.)

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax directive::` — Expected: FAIL to compile: `cannot find type ParsedBody / function parse_body / enum Verb`.

- [ ] **Step 3: Implement.** Add `pub mod directive;` to `lib.rs`, then above the tests in `directive.rs`:

```rust
//! Directive grammar (winnow) and the directive record types (R-SYN-001..015, 028, 029, 048).
use crate::diag::{codes, did_you_mean, DiagCode, Span};
use crate::ids::{Id, IdError};
use crate::normalize::nfc;
use serde::{Deserialize, Serialize};
use winnow::combinator::alt;
use winnow::prelude::*;
use winnow::token::take_while;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Verb {
    Def,
    Use,
    Ovr,
    Flag,
    Env,
    End,
}

pub const VERBS: [&str; 6] = ["def", "use", "ovr", "flag", "env", "end"];

impl Verb {
    pub fn as_str(self) -> &'static str {
        match self {
            Verb::Def => "def",
            Verb::Use => "use",
            Verb::Ovr => "ovr",
            Verb::Flag => "flag",
            Verb::Env => "env",
            Verb::End => "end",
        }
    }
    pub fn parse(s: &str) -> Option<Verb> {
        Some(match s {
            "def" => Verb::Def,
            "use" => Verb::Use,
            "ovr" => Verb::Ovr,
            "flag" => Verb::Flag,
            "env" => Verb::Env,
            "end" => Verb::End,
            _ => return None,
        })
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ArgOp {
    Set,
    Append,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ArgValue {
    Word(String),
    Quoted(String),
    List(Vec<String>),
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Arg {
    pub key: String,
    pub op: ArgOp,
    pub value: ArgValue,
    pub span: Span,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Carrier {
    LineComment,
    BlockComment,
    HtmlComment,
    Frontmatter,
}

/// R-SYN-016..019, 022. Lines are 1-based inclusive. `Doc` = frontmatter or heading binding.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Extent {
    File,
    Doc,
    Lines { first: u32, last: u32 },
}

/// One attached directive. Dropped/malformed directives never appear here (R-SYN-032, R-ARCH-031).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Directive {
    pub ordinal: u32,
    pub verb: Verb,
    pub must_understand: bool,
    pub ids: Vec<Id>,
    pub bare: Vec<String>,
    pub args: Vec<Arg>,
    pub note: Option<String>,
    pub carrier: Carrier,
    pub span: Span,
    pub extent: Extent,
    pub end_span: Option<Span>,
}

impl Directive {
    pub fn arg(&self, key: &str) -> Option<&Arg> {
        self.args.iter().find(|a| a.key == key)
    }
}

/// The explicit extent word of a directive (`+N` or `+end`), before attachment.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Explicit {
    Lines(u32),
    Region,
}

/// Result of parsing one directive body; offsets are byte offsets inside the body.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParsedBody {
    pub must_understand: bool,
    pub verb: Verb,
    pub verb_at: usize,
    pub ids: Vec<Id>,
    pub bare: Vec<String>,
    pub args: Vec<Arg>,
    pub arg_pos: Vec<(usize, usize)>,
    pub note: Option<String>,
    pub explicit: Option<Explicit>,
}

/// A malformed body: code, byte range in the body, message, expected form, did-you-mean (R-SYN-032).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BodyError {
    pub code: DiagCode,
    pub at: usize,
    pub len: usize,
    pub message: String,
    pub expected: Option<String>,
    pub did_you_mean: Option<String>,
}

impl BodyError {
    pub fn new(code: DiagCode, at: usize, len: usize, message: impl Into<String>) -> BodyError {
        BodyError { code, at, len: len.max(1), message: message.into(), expected: None, did_you_mean: None }
    }
    pub fn expected(mut self, e: impl Into<String>) -> BodyError {
        self.expected = Some(e.into());
        self
    }
    pub fn suggest(mut self, s: Option<String>) -> BodyError {
        self.did_you_mean = s;
        self
    }
}

type PResult<T> = winnow::ModalResult<T>; // Verify: `ModalResult` is the winnow >= 0.7 name (0.6: `PResult`).

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '#' | '/' | '@' | '%' | ':' | '*' | ',')
}
fn is_key(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
}
fn word<'a>(i: &mut &'a str) -> PResult<&'a str> {
    take_while(1.., is_word).parse_next(i)
}
fn key<'a>(i: &mut &'a str) -> PResult<&'a str> {
    take_while(1.., is_key).parse_next(i)
}
fn spaces<'a>(i: &mut &'a str) -> PResult<&'a str> {
    take_while(1.., ' ').parse_next(i)
}
fn assign(i: &mut &str) -> PResult<ArgOp> {
    alt(("+=".value(ArgOp::Append), "=".value(ArgOp::Set))).parse_next(i)
}

fn pos(total: usize, rest: &str) -> usize {
    total.saturating_sub(rest.len())
}

/// `s` starts at the opening quote. Ok((value, bytes consumed)) or Err((offset in `s`, reason)).
fn quoted(s: &str) -> Result<(String, usize), (usize, &'static str)> {
    let mut out = String::new();
    let mut it = s.char_indices();
    it.next();
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Ok((out, i + 1)),
            '\n' => return Err((i, "newline inside quotes (use a continuation line)")),
            '\\' => match it.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some(_) => return Err((i, "unknown escape (only \\\" and \\\\ exist)")),
                None => return Err((s.len(), "unterminated quote")),
            },
            _ => out.push(c),
        }
    }
    Err((s.len(), "unterminated quote"))
}

enum RawVal {
    Word(String),
    Quoted(String),
}

enum Tok<'a> {
    Pair { key: &'a str, op: ArgOp, val: RawVal, at: usize, end: usize },
    Word { text: &'a str, at: usize },
    Extent { x: Explicit },
}

struct Raw<'a> {
    bang: bool,
    verb: &'a str,
    verb_at: usize,
    toks: Vec<Tok<'a>>,
    note: Option<String>,
}

fn boundary(rest: &str, total: usize) -> Result<(), BodyError> {
    match rest.chars().next() {
        None | Some(' ') => Ok(()),
        Some(c) => Err(BodyError::new(codes::HN_S016, pos(total, rest), c.len_utf8(), format!("unexpected character `{c}`"))
            .expected("lowercase ASCII words; text only inside \"quotes\" or after `--`")),
    }
}

fn parse_extent(rest: &mut &str, total: usize) -> Result<Explicit, BodyError> {
    let at = pos(total, rest);
    let r = rest.strip_prefix('+').unwrap_or(rest);
    let n = r.find(' ').unwrap_or(r.len());
    let w = r.get(..n).unwrap_or("");
    *rest = r.get(n..).unwrap_or("");
    if w == "end" {
        return Ok(Explicit::Region);
    }
    match w.parse::<u32>() {
        Ok(k) if k >= 1 && w.bytes().all(|b| b.is_ascii_digit()) => Ok(Explicit::Lines(k)),
        _ => Err(BodyError::new(codes::HN_S016, at, w.len() + 1, "bad extent").expected("+N with N >= 1, or +end")),
    }
}

fn tokenize(body: &str) -> Result<Raw<'_>, BodyError> {
    let total = body.len();
    let mut rest = body;
    let bang = match rest.strip_prefix('!') {
        Some(r) => {
            rest = r;
            true
        }
        None => false,
    };
    let verb_at = pos(total, rest);
    let verb = match take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '-').parse_next(&mut rest) {
        Ok(v) => v,
        Err(_) => {
            return Err(BodyError::new(codes::HN_S016, verb_at, 1, "expected a verb after the marker")
                .expected("def | use | ovr | flag | env | end"))
        }
    };
    let mut toks = Vec::new();
    let mut note = None;
    while !rest.is_empty() {
        if spaces(&mut rest).is_err() {
            return Err(BodyError::new(codes::HN_S016, pos(total, rest), 1, "expected a space between directive parts"));
        }
        if rest.is_empty() {
            break;
        }
        if rest.starts_with("--") && rest.get(2..).is_some_and(|t| t.is_empty() || t.starts_with(' ')) {
            note = Some(nfc(rest.get(2..).unwrap_or("").trim_start()));
            break;
        }
        let at = pos(total, rest);
        if rest.starts_with('+') {
            let x = parse_extent(&mut rest, total)?;
            boundary(rest, total)?;
            toks.push(Tok::Extent { x });
            continue;
        }
        let mut probe = rest;
        let k = key(&mut probe);
        let op = assign(&mut probe);
        if let (Ok(k), Ok(op)) = (k, op) {
            let val = if probe.starts_with('"') {
                match quoted(probe) {
                    Ok((v, used)) => {
                        probe = probe.get(used..).unwrap_or("");
                        RawVal::Quoted(nfc(&v))
                    }
                    Err((o, why)) => {
                        return Err(BodyError::new(codes::HN_S007, pos(total, probe) + o, 1, why)
                            .expected("\"text\" using only \\\" and \\\\ escapes, closed on the same line"))
                    }
                }
            } else {
                match word(&mut probe) {
                    Ok(w) => RawVal::Word(w.to_owned()),
                    Err(_) => {
                        return Err(BodyError::new(codes::HN_S016, pos(total, probe), 1, format!("expected a value after `{k}=`"))
                            .expected("word, list a,b or \"quoted text\""))
                    }
                }
            };
            boundary(probe, total)?;
            rest = probe;
            toks.push(Tok::Pair { key: k, op, val, at, end: pos(total, rest) });
            continue;
        }
        match word(&mut rest) {
            Ok(text) => {
                boundary(rest, total)?;
                toks.push(Tok::Word { text, at });
            }
            Err(_) => {
                boundary(rest, total)?;
                return Err(BodyError::new(codes::HN_S016, pos(total, rest), 1, "expected a word"));
            }
        }
    }
    Ok(Raw { bang, verb, verb_at, toks, note })
}
```

Continue `directive.rs`:

```rust
fn id_error(text: &str, at: usize, e: IdError) -> BodyError {
    if e == IdError::ReservedName {
        return BodyError::new(codes::HN_P001, at, text.len(), format!("`{text}` uses a Windows-reserved name"))
            .expected("a name that is not con prn aux nul com1..9 lpt1..9");
    }
    BodyError::new(codes::HN_S002, at, text.len(), format!("invalid ID \"{text}\": {e}"))
        .expected("[scope:]kind.name with a 2 to 8 letter lowercase kind")
}

fn to_value(v: RawVal, at: usize) -> Result<ArgValue, BodyError> {
    match v {
        RawVal::Quoted(q) => Ok(ArgValue::Quoted(q)),
        RawVal::Word(w) if w.contains(',') => {
            let items: Vec<String> = w.split(',').map(str::to_owned).collect();
            if items.iter().any(String::is_empty) {
                return Err(BodyError::new(codes::HN_S016, at, w.len(), "empty list item").expected("word,word without spaces"));
            }
            Ok(ArgValue::List(items))
        }
        RawVal::Word(w) => Ok(ArgValue::Word(w)),
    }
}

/// Parses one directive body (R-SYN-007 grammar). Pure and total.
pub fn parse_body(body: &str) -> Result<ParsedBody, BodyError> {
    let raw = tokenize(body)?;
    let Some(verb) = Verb::parse(raw.verb) else {
        return Err(BodyError::new(codes::HN_S001, raw.verb_at, raw.verb.len(), format!("unknown verb \"{}\"", raw.verb))
            .expected("one of: def use ovr flag env end")
            .suggest(did_you_mean(raw.verb, &VERBS).map(str::to_owned)));
    };
    let mut p = ParsedBody {
        must_understand: raw.bang,
        verb,
        verb_at: raw.verb_at,
        ids: Vec::new(),
        bare: Vec::new(),
        args: Vec::new(),
        arg_pos: Vec::new(),
        note: raw.note,
        explicit: None,
    };
    for t in raw.toks {
        match t {
            Tok::Extent { x } => p.explicit = Some(x),
            Tok::Pair { key, op, val, at, end } => {
                let value = to_value(val, at)?;
                p.args.push(Arg { key: key.to_owned(), op, value, span: Span::whole_file() });
                p.arg_pos.push((at, end));
            }
            Tok::Word { text, at } => match verb {
                Verb::Def | Verb::Use | Verb::Ovr => p.ids.push(Id::parse(text).map_err(|e| id_error(text, at, e))?),
                Verb::Flag | Verb::Env | Verb::End => p.bare.push(text.to_owned()),
            },
        }
    }
    check_verb(verb, &mut p)?;
    Ok(p)
}

/// Per-verb arity and key rules; each verb's arm is filled by its own task.
fn check_verb(verb: Verb, p: &mut ParsedBody) -> Result<(), BodyError> {
    match verb {
        Verb::Def | Verb::Use => check_def_use(verb, p),
        Verb::Ovr => check_ovr(p),
        Verb::Flag | Verb::Env | Verb::End => check_flag_env_end(verb, p),
    }
}

fn check_def_use(_verb: Verb, _p: &mut ParsedBody) -> Result<(), BodyError> {
    Ok(()) // Task 10
}
fn check_ovr(_p: &mut ParsedBody) -> Result<(), BodyError> {
    Ok(()) // Task 11
}
fn check_flag_env_end(_verb: Verb, _p: &mut ParsedBody) -> Result<(), BodyError> {
    Ok(()) // Task 12
}
```

Verify: (1) in winnow 1.0 `"+=".value(x)` needs the `Parser` trait (in `winnow::prelude`); if the literal-parser form does not resolve use `winnow::token::literal("+=").value(..)`. (2) `take_while(1.., ' ')` accepts a `char` as the set; if not use `|c: char| c == ' '`. (3) `ModalResult` may be spelled `PResult` in older winnow. (4) For `word(&mut probe)` failing, `probe` is unchanged, so `pos(total, probe)` is the position of the missing value.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax directive::` — Expected: PASS (12 tests). Run `cargo clippy -p fairlead-syntax -- -D warnings` (the `_verb`/`_p` stubs are intentional; they are replaced in Tasks 10-12).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): directive types and winnow body tokenizer" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: `def` and `use` grammar (`directive.rs`, part 2)

**Rules:** R-SYN-007 (ID forms in the ID slot), R-SYN-009 (`x-` keys and `!` do not change parsing), R-SYN-014 (`def foreign.<name> tool=<tool> +end` is an ordinary `def` with an explicit region), R-FILE-042 (foreign region bracket), R-SYN-027 (reserved names in IDs report `HN-P001`), R-SYN-032 (every failure reports the expected form).

**Files:** Modify `crates/fairlead-syntax/src/directive.rs`.

**Interfaces:** Consumes `ParsedBody`, `BodyError`. Produces: `def` takes exactly one ID and no bare words; `use` takes one or more IDs and no bare words; both accept any `k=v` pairs and an optional `+N`/`+end`; a bare non-ID word is reported as an invalid ID (`HN-S002`, from Task 9's classification).

- [ ] **Step 1: Write the failing test.** Append inside `mod tests` of `directive.rs`:

```rust
    #[test]
    fn r_syn_007__def_takes_exactly_one_id() {
        assert_eq!(ok("def tok.primary applies=color,background-color min-len=4").ids[0].to_string(), "tok.primary");
        let e = err("def");
        assert_eq!(e.code, codes::HN_S016);
        assert_eq!(e.expected.as_deref(), Some("def ID [k=v..] [+N|+end]"));
        assert_eq!(err("def tt.b uu.d").code, codes::HN_S016);
        assert_eq!(err("def sym=x").code, codes::HN_S016, "pairs alone are not an ID");
    }

    #[test]
    fn r_syn_007__use_takes_one_or_more_ids() {
        let p = ok("use eng.sync-verify eng.sync-purify :tok.primary web:cmp.button sym=Foo");
        assert_eq!(p.ids.len(), 4);
        assert_eq!(p.ids[2].to_string(), ":tok.primary");
        assert_eq!(err("use").code, codes::HN_S016);
        assert_eq!(err("use sym=x").code, codes::HN_S016);
    }

    #[test]
    fn r_syn_007__bad_id_in_the_id_slot_is_s002_with_position() {
        let e = err("def comp.Sync-Button");
        assert_eq!(e.code, codes::HN_S002);
        assert_eq!(e.at, 4);
        let e2 = err("use cmp");
        assert_eq!(e2.code, codes::HN_S002);
        assert_eq!(err("def tauri-command.x").code, codes::HN_S002, "labels are not kinds (R-SYN-024)");
    }

    #[test]
    fn r_syn_027__reserved_windows_name_in_an_id_is_hn_p001() {
        let e = err("def cmp.con");
        assert_eq!(e.code, codes::HN_P001);
        assert_eq!(err("use nul:cmp.x").code, codes::HN_P001);
    }

    #[test]
    fn r_syn_014__def_foreign_with_plus_end_opens_a_region() {
        let p = ok("def foreign.shadcn tool=shadcn +end");
        assert_eq!(p.ids[0].kind.as_str(), "foreign");
        assert_eq!(p.explicit, Some(Explicit::Region));
        assert_eq!(p.args[0].key, "tool");
    }

    #[test]
    fn r_file_042__foreign_region_bracket_parses_like_any_def() {
        let p = ok("def foreign.tailwind tool=tailwind +end -- written by the tool");
        assert_eq!(p.note.as_deref(), Some("written by the tool"));
    }

    #[test]
    fn r_syn_009__extension_keys_and_bang_parse_like_other_keys() {
        let p = ok("!def cmp.x x-owner=team x-note=\"a b\"");
        assert!(p.must_understand);
        assert_eq!(p.args.iter().map(|a| a.key.as_str()).collect::<Vec<_>>(), vec!["x-owner", "x-note"]);
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_007__def` and `r_syn_007__use`, `r_syn_007__bad_id`, `r_syn_027` — Expected: FAIL: the arity tests fail because `check_def_use` accepts everything (e.g. `err("def")` panics with `expected an error`). The ID-position tests may already pass; the ones above prove arity.

- [ ] **Step 3: Implement.** Replace the Task 10 stub in `directive.rs`:

```rust
fn check_def_use(verb: Verb, p: &mut ParsedBody) -> Result<(), BodyError> {
    let form = if verb == Verb::Def { "def ID [k=v..] [+N|+end]" } else { "use ID [ID..] [sym=..]" };
    let bad = |msg: &str| BodyError::new(codes::HN_S016, p.verb_at, verb.as_str().len(), msg.to_owned()).expected(form);
    if !p.bare.is_empty() {
        return Err(bad("unexpected bare word"));
    }
    match (verb, p.ids.len()) {
        (_, 0) => Err(bad(&format!("`{}` needs an ID", verb.as_str()))),
        (Verb::Def, n) if n > 1 => Err(bad("`def` declares exactly one ID")),
        _ => Ok(()),
    }
}
```

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax directive::` — Expected: PASS (19 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): def and use grammar with arity and foreign regions" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: `ovr` grammar (`directive.rs`, part 3)

**Rules:** R-SYN-012 (`exp=` is an ISO date; in edition 1 `until=` and a version-valued `exp` raise `HN-S012` and the `ovr` is dropped), R-SYN-013 (`k=v` replaces, `k+=a,b` appends; no `op=append`), R-SYN-011 (shape only: `why`, `owner`, `exp` presence is checked by `resolve`, C-5), R-SYN-034 (malformed `ovr` fails closed).

**Files:** Modify `crates/fairlead-syntax/src/directive.rs`.

**Interfaces:** Consumes `ParsedBody`. Produces: `ovr` takes exactly one ID; key `until` is `HN-S012` at the key; key `exp` must be a `Word` or `Quoted` `YYYY-MM-DD` that is a real calendar date, else `HN-S012` at the value; `exp+=` is `HN-S012`. Crate-internal `is_iso_date(&str) -> bool`.

- [ ] **Step 1: Write the failing test.** Append inside `mod tests`:

```rust
    #[test]
    fn r_syn_012__exp_must_be_a_real_iso_date() {
        assert!(parse_body("ovr rule.x why=\"w\" owner=@a exp=2026-12-31").is_ok());
        assert!(parse_body("ovr rule.x why=\"w\" exp=\"2024-02-29\"").is_ok(), "leap day, quoted");
        for bad in ["exp=2025-02-29", "exp=2026-13-01", "exp=2026-00-10", "exp=2026-1-5", "exp=1.9.0", "exp=tomorrow", "exp=2026-12-31,2027-01-01"] {
            let e = err(&format!("ovr rule.x why=\"w\" {bad}"));
            assert_eq!(e.code, codes::HN_S012, "{bad}");
        }
    }

    #[test]
    fn r_syn_012__until_is_rejected_in_edition_one() {
        let e = err("ovr rule.x why=\"w\" until=1.9.0");
        assert_eq!(e.code, codes::HN_S012);
        assert_eq!(e.at, 19, "points at the key");
        assert!(e.message.contains("until"));
        assert_eq!(e.expected.as_deref(), Some("exp=YYYY-MM-DD"));
    }

    #[test]
    fn r_syn_013__ovr_keys_replace_or_append() {
        let p = ok("ovr tok.primary why=\"brand\" value=#ff0000 applies+=color,fill");
        assert_eq!(p.args.len(), 3);
        assert_eq!((p.args[2].key.as_str(), p.args[2].op), ("applies", ArgOp::Append));
    }

    #[test]
    fn r_syn_011__ovr_shape_is_one_id() {
        assert_eq!(err("ovr").code, codes::HN_S016);
        assert_eq!(err("ovr tt.b uu.d why=\"w\"").code, codes::HN_S016);
        assert!(parse_body("ovr rule.x").is_ok(), "missing why is resolve's HN-S016 (C-5), not extraction's");
    }

    #[test]
    fn r_syn_012__iso_date_helper() {
        assert!(is_iso_date("2026-02-28") && is_iso_date("2000-02-29") && !is_iso_date("1900-02-29"));
        assert!(!is_iso_date("2026-04-31") && !is_iso_date("26-04-01") && !is_iso_date("2026/04/01"));
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_012` — Expected: FAIL: `cannot find function is_iso_date`, and (after adding the helper) the `exp`/`until` tests fail because `check_ovr` accepts everything.

- [ ] **Step 3: Implement.** Replace the Task 11 stub:

```rust
/// `YYYY-MM-DD` and a real Gregorian calendar date (R-SYN-012).
pub(crate) fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return false;
    }
    let num = |a: usize, z: usize| -> Option<u32> {
        let t = s.get(a..z)?;
        if t.bytes().all(|c| c.is_ascii_digit()) { t.parse().ok() } else { None }
    };
    let (Some(y), Some(m), Some(d)) = (num(0, 4), num(5, 7), num(8, 10)) else {
        return false;
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let dim = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=dim).contains(&d)
}

fn check_ovr(p: &mut ParsedBody) -> Result<(), BodyError> {
    let form = "ovr ID why=\"..\" [owner=@h] [exp=YYYY-MM-DD] [k=v..]";
    if !p.bare.is_empty() || p.ids.len() != 1 {
        return Err(BodyError::new(codes::HN_S016, p.verb_at, 3, "`ovr` names exactly one ID").expected(form));
    }
    for (a, &(at, end)) in p.args.iter().zip(p.arg_pos.iter()) {
        if a.key == "until" {
            return Err(BodyError::new(codes::HN_S012, at, a.key.len(), "`until=` needs seals (edition 1 has none)")
                .expected("exp=YYYY-MM-DD"));
        }
        if a.key == "exp" {
            let date = match (&a.op, &a.value) {
                (ArgOp::Set, ArgValue::Word(w) | ArgValue::Quoted(w)) => Some(w.as_str()),
                _ => None,
            };
            if !date.is_some_and(is_iso_date) {
                return Err(BodyError::new(codes::HN_S012, at, end.saturating_sub(at), "`exp` must be an ISO date")
                    .expected("exp=YYYY-MM-DD"));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax directive::` — Expected: PASS (24 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): ovr grammar with ISO exp and until rejection" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 12: `flag`, `env`, `end` grammar (`directive.rs`, part 4)

**Rules:** R-SYN-048 (`flag WORD [ID] [to=ID] [target=VERSION] [by-date=DATE] [since=VERSION]`; the word is user-defined; `until` is not special-cased: unknown-key handling is `resolve`'s HN-S011), R-SYN-009 (`env a,b`), R-SYN-019 (`end` closes the innermost region), R-SYN-014 (no `flag foreign`).

**Files:** Modify `crates/fairlead-syntax/src/directive.rs`.

**Interfaces:** Produces: for `flag`, `bare = [word]` and `ids` = zero or one ID (the second word); for `env`, `bare` = the names, split on `,`, each `[a-z][a-z0-9_-]*`; for `end`, no words, no pairs, no extent (a note is allowed). `Verb::Env`/`Verb::Flag` may carry `+N`/`+end`.

- [ ] **Step 1: Write the failing test.** Append inside `mod tests`:

```rust
    #[test]
    fn r_syn_048__flag_word_then_optional_id() {
        let p = ok("flag deprecate cmp.sync-menu to=cmp.sync-menu2 target=2.0.0 since=1.4.0");
        assert_eq!(p.verb, Verb::Flag);
        assert_eq!(p.bare, vec!["deprecate"]);
        assert_eq!(p.ids.len(), 1);
        assert_eq!(p.args.iter().map(|a| a.key.as_str()).collect::<Vec<_>>(), vec!["to", "target", "since"]);
        let q = ok("flag dep");
        assert!(q.ids.is_empty() && q.bare == vec!["dep"]);
    }

    #[test]
    fn r_syn_048__flag_shape_errors() {
        assert_eq!(err("flag").code, codes::HN_S016);
        assert_eq!(err("flag Deprecate").code, codes::HN_S016, "flag words are lowercase");
        assert_eq!(err("flag dep cmp.x cmp.y").code, codes::HN_S016);
        assert_eq!(err("flag cmp.x").code, codes::HN_S016, "the first word is the flag word, not an ID");
        assert_eq!(err("flag dep not-an-id").code, codes::HN_S002);
    }

    #[test]
    fn r_syn_048__until_on_flag_is_parsed_and_left_to_resolve() {
        let p = ok("flag deprecate cmp.x until=2.0.0");
        assert_eq!(p.args[0].key, "until", "HN-S011 is raised by resolve (C-5), error under !");
    }

    #[test]
    fn r_syn_009__env_lists_names() {
        assert_eq!(ok("env dev").bare, vec!["dev"]);
        assert_eq!(ok("env dev,staging +end").bare, vec!["dev", "staging"]);
        assert_eq!(ok("env dev,staging +end").explicit, Some(Explicit::Region));
        assert_eq!(ok("env a b").bare, vec!["a", "b"]);
        assert_eq!(err("env").code, codes::HN_S016);
        assert_eq!(err("env Dev").code, codes::HN_S016);
        assert_eq!(err("env dev,,prod").code, codes::HN_S016);
        assert_eq!(err("env dev k=v").code, codes::HN_S016, "env takes no keys");
    }

    #[test]
    fn r_syn_019__end_takes_nothing_but_a_note() {
        let p = ok("end -- closes the shadcn block");
        assert_eq!(p.verb, Verb::End);
        assert_eq!(p.note.as_deref(), Some("closes the shadcn block"));
        assert_eq!(err("end cmp.x").code, codes::HN_S016);
        assert_eq!(err("end +3").code, codes::HN_S016);
        assert_eq!(err("end k=v").code, codes::HN_S016);
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_048` and `r_syn_009__env` and `r_syn_019` — Expected: FAIL (`p.bare` holds raw words, the shape checks accept everything; `p.ids` is empty for `flag`).

- [ ] **Step 3: Implement.** Replace the Task 12 stub:

```rust
fn is_lower_word(w: &str, extra: &[u8]) -> bool {
    let b = w.as_bytes();
    b.first().is_some_and(u8::is_ascii_lowercase)
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-' || extra.contains(c))
}

fn check_flag_env_end(verb: Verb, p: &mut ParsedBody) -> Result<(), BodyError> {
    let shape = |msg: &str, form: &str| BodyError::new(codes::HN_S016, p.verb_at, verb.as_str().len(), msg.to_owned()).expected(form.to_owned());
    match verb {
        Verb::Flag => {
            let words = std::mem::take(&mut p.bare);
            let mut it = words.into_iter();
            let Some(flag_word) = it.next() else {
                return Err(shape("`flag` needs a word", "flag WORD [ID] [to=ID] [target=VERSION]"));
            };
            if !is_lower_word(&flag_word, b"") || flag_word.len() > 32 {
                return Err(shape("flag words are lowercase `a-z0-9-`", "flag WORD [ID]"));
            }
            let rest: Vec<String> = it.collect();
            if rest.len() > 1 {
                return Err(shape("`flag` takes one word and at most one ID", "flag WORD [ID]"));
            }
            for t in &rest {
                p.ids.push(Id::parse(t).map_err(|e| id_error(t, p.verb_at + 5 + flag_word.len(), e))?);
            }
            p.bare = vec![flag_word];
            Ok(())
        }
        Verb::Env => {
            let mut names = Vec::new();
            for w in std::mem::take(&mut p.bare) {
                for n in w.split(',') {
                    if !is_lower_word(n, b"_") {
                        return Err(shape("environment names are lowercase `a-z0-9_-`", "env a,b"));
                    }
                    names.push(n.to_owned());
                }
            }
            if names.is_empty() || !p.args.is_empty() {
                return Err(shape("`env` takes only environment names", "env a,b [+N|+end]"));
            }
            p.bare = names;
            Ok(())
        }
        _ => {
            if p.bare.is_empty() && p.args.is_empty() && p.explicit.is_none() {
                Ok(())
            } else {
                Err(shape("`end` takes no arguments", "end"))
            }
        }
    }
}
```

The `flag dep not-an-id` case reports `HN-S002` through `id_error`; the offset is approximate (start of the second word is not tracked) and is fine for the caret because `extract` clamps and the message names the word. Verify: `u8::is_ascii_lowercase` takes `&self`, so `.is_some_and(u8::is_ascii_lowercase)` on `Option<&u8>` works.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax directive::` — Expected: PASS (29 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): flag, env and end grammar" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Known keys and stable rendering (`directive.rs`, part 5)

**Rules:** R-SYN-015 (built-in keys per verb in edition 1), R-SYN-009 (`x-` keys always known), R-SYN-048 (flag keys), R-ARCH-150 table (parse, render, parse is stable).

**Files:** Modify `crates/fairlead-syntax/src/directive.rs`.

**Interfaces (contract 4.2):** `pub fn known_keys(verb: Verb) -> &'static [&'static str]`; `pub fn render_directive_body(d: &Directive) -> String`. Additive: `pub fn is_extension_key(k: &str) -> bool`; crate-internal `render_parts(...)` shared with the fuzz round-trip. `render_directive_body` omits `+N` (the explicit extent word is not stored in `Directive`; heuristic extents are re-derived) and renders `+end` iff `end_span` is set.

- [ ] **Step 1: Write the failing test.** Append inside `mod tests`:

```rust
    fn dir(body: &str) -> Directive {
        let p = ok(body);
        Directive {
            ordinal: 0,
            verb: p.verb,
            must_understand: p.must_understand,
            ids: p.ids,
            bare: p.bare,
            args: p.args,
            note: p.note,
            carrier: Carrier::LineComment,
            span: Span::whole_file(),
            extent: Extent::Lines { first: 1, last: 1 },
            end_span: if p.explicit == Some(Explicit::Region) { Some(Span::whole_file()) } else { None },
        }
    }

    #[test]
    fn r_syn_015__known_keys_per_verb() {
        assert!(known_keys(Verb::Def).contains(&"gen") && known_keys(Verb::Def).contains(&"min-len"));
        assert!(known_keys(Verb::Def).contains(&"tool"), "def foreign tool= (R-SYN-014)");
        assert_eq!(known_keys(Verb::Use), &["sym"]);
        assert_eq!(known_keys(Verb::Ovr), &["why", "owner", "exp", "op", "base"]);
        assert_eq!(known_keys(Verb::Flag), &["to", "target", "by-date", "since"]);
        assert!(known_keys(Verb::Env).is_empty() && known_keys(Verb::End).is_empty());
        assert!(!known_keys(Verb::Flag).contains(&"until"), "D-49");
        assert!(is_extension_key("x-owner") && !is_extension_key("xowner") && !is_extension_key("x-"));
    }

    #[test]
    fn r_arch_150__render_then_parse_is_stable() {
        for body in [
            "def cmp.sync-button sym=SyncButton layer=molecule",
            "!def foreign.shadcn tool=shadcn +end",
            "use eng.a eng.b sym=\"has \\\"q\\\" and \\\\\" -- note text",
            "ovr rule.x why=\"w\" owner=@a exp=2026-12-31 kw+=a,b",
            "flag deprecate cmp.x target=2.0.0",
            "env dev,staging",
            "end",
        ] {
            let d1 = dir(body);
            let r1 = render_directive_body(&d1);
            let d2 = dir(&r1);
            assert_eq!(render_directive_body(&d2), r1, "{body}");
            assert_eq!((d1.verb, &d1.ids, &d1.bare, &d1.args, &d1.note), (d2.verb, &d2.ids, &d2.bare, &d2.args, &d2.note), "{body}");
        }
    }

    #[test]
    fn r_arch_150__render_is_canonical() {
        assert_eq!(render_directive_body(&dir("def   tt.b   k=\"v\"   +end")), "def tt.b k=\"v\" +end");
        assert_eq!(render_directive_body(&dir("flag dep cmp.x")), "flag dep cmp.x");
        assert!(!render_directive_body(&dir("use tt.b")).contains('\n'));
    }

    #[test]
    fn r_syn_008__directive_arg_lookup() {
        let d = dir("def tt.b sym=X sym=Y");
        assert_eq!(d.arg("sym").map(|a| &a.value), Some(&ArgValue::Word("X".into())), "first wins");
        assert!(d.arg("nope").is_none());
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_015` and `r_arch_150` — Expected: FAIL to compile: `cannot find function known_keys / render_directive_body / is_extension_key`.

- [ ] **Step 3: Implement.** Add above `mod tests`:

```rust
const DEF_KEYS: &[&str] = &["st", "sum", "kw", "sym", "of", "policy", "value", "applies", "min-len", "home", "gen", "tool"];
const USE_KEYS: &[&str] = &["sym"];
const OVR_KEYS: &[&str] = &["why", "owner", "exp", "op", "base"];
const FLAG_KEYS: &[&str] = &["to", "target", "by-date", "since"];

/// R-SYN-015, R-SYN-013, R-SYN-048: built-in keys of edition 1 (kind-declared keys and `x-` keys are extra).
pub fn known_keys(verb: Verb) -> &'static [&'static str] {
    match verb {
        Verb::Def => DEF_KEYS,
        Verb::Use => USE_KEYS,
        Verb::Ovr => OVR_KEYS,
        Verb::Flag => FLAG_KEYS,
        Verb::Env | Verb::End => &[],
    }
}

/// `x-` user extension keys are always known and never warned (R-SYN-009).
pub fn is_extension_key(k: &str) -> bool {
    k.len() > 2 && k.starts_with("x-")
}

fn render_value(v: &ArgValue, out: &mut String) {
    match v {
        ArgValue::Word(w) => out.push_str(w),
        ArgValue::List(items) => out.push_str(&items.join(",")),
        ArgValue::Quoted(q) => {
            out.push('"');
            for c in q.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' | '\r' => out.push(' '),
                    _ => out.push(c),
                }
            }
            out.push('"');
        }
    }
}

/// Stable re-rendering used by the parse-render-parse fuzz invariant (R-ARCH-150 table).
pub fn render_directive_body(d: &Directive) -> String {
    let mut s = String::new();
    if d.must_understand {
        s.push('!');
    }
    s.push_str(d.verb.as_str());
    if d.verb == Verb::Env {
        if !d.bare.is_empty() {
            s.push(' ');
            s.push_str(&d.bare.join(","));
        }
    } else {
        for w in &d.bare {
            s.push(' ');
            s.push_str(w);
        }
    }
    for id in &d.ids {
        s.push(' ');
        s.push_str(&id.to_string());
    }
    for a in &d.args {
        s.push(' ');
        s.push_str(&a.key);
        s.push_str(if a.op == ArgOp::Append { "+=" } else { "=" });
        render_value(&a.value, &mut s);
    }
    if d.end_span.is_some() {
        s.push_str(" +end");
    }
    if let Some(n) = &d.note {
        s.push_str(" -- ");
        s.push_str(&n.replace(['\n', '\r'], " "));
    }
    s
}
```

Note: for `flag`, `bare` (the word) precedes `ids` (the target), matching the grammar order. A `Word` value that contains a comma cannot occur (it parses to `List`), so the round trip is lossless.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax directive::` — Expected: PASS (33 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): known keys per verb and stable directive rendering" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

### Task 14: Frontmatter (`frontmatter.rs`)

**Rules:** R-SYN-036 (a doc starts with `+++`, TOML, `+++`; the key `directives` is an array of strings, each a full directive body without leader and marker), R-SYN-037 (invalid TOML or `directives` not an array of strings is `HN-S004`; other keys are the author's and ignored), R-SYN-022 (frontmatter directives bind to the doc), D-05.

**Files:**
- Create: `crates/fairlead-syntax/src/frontmatter.rs`
- Modify: `crates/fairlead-syntax/src/lib.rs` (`pub mod frontmatter;`)

**Interfaces:**
- Consumes: `normalize::{Normalized, slice}`, `diag::Span`.
- Produces (contract 4.2): `Frontmatter { span, table: toml::Table, directive_bodies: Vec<String> }`, `pub fn split_frontmatter(text: &str) -> Option<(Range<usize>, Range<usize>)>` (outer range from the opening `+++` to the end of the closing `+++`; inner range = the TOML text between the two fence lines). Additive: `FmOutcome { Absent, Parsed(ParsedFrontmatter), Bad(FmError) }`, `ParsedFrontmatter { fm, offsets, end }` (`offsets[i]` = normalized offset where body `i` starts, for diagnostics), `FmError { at, len, message }`, `pub fn parse_frontmatter(n: &Normalized) -> FmOutcome`. `Frontmatter` derives `PartialEq` but not `Eq` (`toml::Value` holds floats; see Contract issues).

Directive bodies that span several lines (a TOML `'''` string) are joined with single spaces so `parse_body` sees one line.

- [ ] **Step 1: Write the failing test.** Create `frontmatter.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_syn_036__splitter_finds_both_fences() {
        let t = "+++\ntitle = \"x\"\n+++\n# H\n";
        let (outer, inner) = split_frontmatter(t).unwrap();
        assert_eq!(&t[inner.clone()], "title = \"x\"\n");
        assert_eq!(&t[outer], "+++\ntitle = \"x\"\n+++");
    }

    #[test]
    fn r_syn_036__only_a_leading_fence_line_counts() {
        for t in ["# H\n+++\na=1\n+++\n", " +++\na=1\n+++\n", "++++\na=1\n+++\n", "+++\na=1\n", "+++", ""] {
            assert_eq!(split_frontmatter(t), None, "{t:?}");
        }
        assert!(split_frontmatter("+++  \na=1\n+++ \nbody").is_some(), "trailing blanks on fence lines are fine");
        assert!(split_frontmatter("+++\n+++\nbody").is_some(), "empty frontmatter");
    }

    fn parsed(src: &str) -> ParsedFrontmatter {
        match parse_frontmatter(&Normalized::new(src)) {
            FmOutcome::Parsed(p) => p,
            other => panic!("expected Parsed, got {other:?}"),
        }
    }

    #[test]
    fn r_syn_036__directives_array_of_strings() {
        let src = "+++\ntitle = \"Design tokens\"\ndirectives = [\n  'def tok.primary applies=color,background-color min-len=4',\n  'use rule.no-raw-hex',\n]\n+++\n\n# Colour\n";
        let p = parsed(src);
        assert_eq!(p.fm.directive_bodies, vec!["def tok.primary applies=color,background-color min-len=4", "use rule.no-raw-hex"]);
        assert!(p.fm.table.contains_key("title"), "other keys are kept and ignored");
        let n = Normalized::new(src);
        assert!(n.text[p.offsets[0]..].starts_with("def tok.primary"));
        assert!(n.text[p.offsets[1]..].starts_with("use rule.no-raw-hex"));
        assert_eq!(p.fm.span.line, 1);
        assert_eq!(&n.text[..p.end], "+++\ntitle = \"Design tokens\"\ndirectives = [\n  'def tok.primary applies=color,background-color min-len=4',\n  'use rule.no-raw-hex',\n]\n+++");
    }

    #[test]
    fn r_syn_036__multi_line_bodies_join_with_one_space() {
        let p = parsed("+++\ndirectives = ['''\novr rule.x\n   why=\"legacy\"\n''']\n+++\n");
        assert_eq!(p.fm.directive_bodies, vec!["ovr rule.x why=\"legacy\""]);
    }

    #[test]
    fn r_syn_036__no_directives_key_is_fine_and_no_fence_is_absent() {
        assert!(parsed("+++\ntitle = \"x\"\n+++\n").fm.directive_bodies.is_empty());
        assert!(matches!(parse_frontmatter(&Normalized::new("# Plain\n")), FmOutcome::Absent));
        assert!(matches!(parse_frontmatter(&Normalized::new("+++x\n")), FmOutcome::Absent));
    }

    #[test]
    fn r_syn_036__crlf_files_split_after_normalisation() {
        let p = parsed("+++\r\ndirectives = ['use tt.b']\r\n+++\r\nbody\r\n");
        assert_eq!(p.fm.directive_bodies, vec!["use tt.b"]);
    }

    fn bad(src: &str) -> FmError {
        match parse_frontmatter(&Normalized::new(src)) {
            FmOutcome::Bad(e) => e,
            other => panic!("expected Bad, got {other:?}"),
        }
    }

    #[test]
    fn r_syn_037__invalid_toml_is_bad_with_a_position_inside_the_block() {
        let e = bad("+++\ntitle = \n+++\n");
        assert!(e.message.starts_with("frontmatter is not valid TOML"), "{}", e.message);
        assert!((4..=13).contains(&e.at), "{}", e.at);
    }

    #[test]
    fn r_syn_037__directives_must_be_an_array_of_strings() {
        assert!(bad("+++\ndirectives = \"use tt.b\"\n+++\n").message.contains("array of strings"));
        assert!(bad("+++\ndirectives = [1, 2]\n+++\n").message.contains("array of strings"));
        assert!(bad("+++\ndirectives = [['use tt.b']]\n+++\n").message.contains("array of strings"));
    }

    #[test]
    fn r_syn_037__unterminated_frontmatter_is_bad() {
        let e = bad("+++\ntitle = \"x\"\n# no closing fence\n");
        assert!(e.message.contains("unterminated"));
        assert_eq!((e.at, e.len), (0, 3));
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax frontmatter::` — Expected: FAIL to compile: `cannot find function split_frontmatter / type ParsedFrontmatter`.

- [ ] **Step 3: Implement.** Above the tests:

```rust
//! TOML frontmatter delimited by `+++` (R-SYN-036, R-SYN-037, D-05).
use crate::diag::Span;
use crate::normalize::{slice, Normalized};
use core::ops::Range;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Frontmatter {
    pub span: Span,
    pub table: toml::Table,
    pub directive_bodies: Vec<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ParsedFrontmatter {
    pub fm: Frontmatter,
    /// Normalized offset of each body (start of its string in the source; the `directives` key as fallback).
    pub offsets: Vec<usize>,
    /// Normalized offset just after the closing `+++`.
    pub end: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FmError {
    pub at: usize,
    pub len: usize,
    pub message: String,
}

#[derive(Clone, PartialEq, Debug)]
pub enum FmOutcome {
    Absent,
    Parsed(ParsedFrontmatter),
    Bad(FmError),
}

fn is_fence(line: &str) -> bool {
    line.trim_end_matches(['\r', ' ', '\t']) == "+++"
}

/// `(outer, inner)`: outer runs from the opening `+++` to the end of the closing `+++`, inner is the TOML text.
pub fn split_frontmatter(text: &str) -> Option<(Range<usize>, Range<usize>)> {
    let bom = if text.starts_with('\u{feff}') { '\u{feff}'.len_utf8() } else { 0 };
    let t = text.get(bom..)?;
    let mut lines = t.split_inclusive('\n');
    let first = lines.next()?;
    if !is_fence(first.trim_end_matches('\n')) {
        return None;
    }
    let inner_start = bom + first.len();
    let mut off = inner_start;
    for line in lines {
        if is_fence(line.trim_end_matches('\n')) {
            return Some((bom..off + 3, inner_start..off));
        }
        off += line.len();
    }
    None
}

fn bad(at: usize, len: usize, message: impl Into<String>) -> FmOutcome {
    FmOutcome::Bad(FmError { at, len: len.max(1), message: message.into() })
}

/// Parses the frontmatter of a normalized document. Pure and total.
pub fn parse_frontmatter(n: &Normalized) -> FmOutcome {
    let text = n.text.as_str();
    if !text.lines().next().is_some_and(is_fence) {
        return FmOutcome::Absent;
    }
    let Some((outer, inner)) = split_frontmatter(text) else {
        return bad(0, 3, "unterminated frontmatter: no closing `+++` line");
    };
    let body = slice(text, inner.start, inner.end);
    let table = match toml::from_str::<toml::Table>(body) {
        Ok(t) => t,
        Err(e) => {
            let r = e.span().unwrap_or(0..0);
            let at = (inner.start + r.start).min(inner.end);
            return bad(at, r.len(), format!("frontmatter is not valid TOML: {}", e.message()));
        }
    };
    let key_at = inner.start + body.find("directives").unwrap_or(0);
    let mut bodies = Vec::new();
    let mut offsets = Vec::new();
    match table.get("directives") {
        None => {}
        Some(toml::Value::Array(items)) => {
            let mut cursor = 0usize;
            for item in items {
                let toml::Value::String(raw) = item else {
                    return bad(key_at, 10, "`directives` must be an array of strings");
                };
                let at = match body.get(cursor..).and_then(|r| r.find(raw.as_str())) {
                    Some(p) => {
                        cursor += p + raw.len();
                        inner.start + cursor - raw.len()
                    }
                    None => key_at,
                };
                let joined = raw.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
                bodies.push(joined);
                offsets.push(at);
            }
        }
        Some(_) => return bad(key_at, 10, "`directives` must be an array of strings"),
    }
    FmOutcome::Parsed(ParsedFrontmatter {
        fm: Frontmatter { span: n.span(outer.start, outer.end), table, directive_bodies: bodies },
        offsets,
        end: outer.end,
    })
}
```

Verify: `toml::de::Error::span()` returns `Option<Range<usize>>` and `message()` returns `&str` in toml 1.x; if `toml::from_str::<toml::Table>` is not available use `body.parse::<toml::Table>()`. In `r_syn_037__invalid_toml...` the error lies on line 2 (`title = ` has no value): the span starts at byte 12 of the source (4 for `+++\n`, 8 for `title = `), inside the asserted range `4..=13`.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax frontmatter::` — Expected: PASS (10 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): +++ frontmatter splitter and directives array" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 15: Attachment, part 1: file form (`attach.rs`)

**Rules:** R-SYN-016 (file form: the first comment block of the file, after BOM, shebang, `<?php`, `"use client"`, `'use strict'`, `@ts-check` and `[scan] header-skip` patterns, followed by a blank line or EOF; a license banner counts as the first block; no `+file`), Edge cases 5, 6 of chapter 12.

**Files:**
- Create: `crates/fairlead-syntax/src/attach.rs`
- Modify: `crates/fairlead-syntax/src/lib.rs` (`pub mod attach;`)

**Interfaces:**
- Consumes: `normalize::Normalized`, `lexer::{Lexed, lex, scan_markers}`, `directive::{Extent, Explicit, Verb, parse_body}`, `delimiters::CommentTable` (tests).
- Produces (crate API used by `extract`): `Site { first_line, last_line, inline, explicit: Option<Explicit>, is_end }`, `Outcome { Bound { extent, end_site: Option<usize> }, ClosesRegion, NothingToBind, UnmatchedEnd, UnclosedRegion }`, `pub fn attach(n: &Normalized, lexed: &Lexed, ext: &str, header_skip: &[String], sites: &[Site]) -> Vec<Outcome>` (one outcome per site, same order). In this task only the file form is implemented; every other site gets `NothingToBind` until Tasks 16 and 17 (the tests of this task use only file-form inputs plus one negative case).

Method: build per-line kinds (`Blank`, `CommentOnly`, `Code`) from the lexer's comment ranges; skip header lines from line 1; the first block is the maximal run of `CommentOnly` lines starting at the first non-skipped line; it is a file block iff the next line is blank or the file ends. `mask` replaces comment (and optionally string) bytes by spaces, keeping newlines, so offsets and line numbers are unchanged.

- [ ] **Step 1: Write the failing test.** Create `attach.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::delimiters::CommentTable;
    use crate::directive::{parse_body, Verb};
    use crate::lexer::{lex, scan_markers};

    pub(crate) fn run_with(src: &str, ext: &str, header_skip: &[String]) -> Vec<Outcome> {
        let n = Normalized::new(src);
        let table = CommentTable::builtin();
        let leaders = table.leaders_for(ext).unwrap().to_vec();
        let lexed = lex(&n, ext, &leaders);
        let scan = scan_markers(&n, &lexed, "fairlead");
        let sites: Vec<Site> = scan
            .candidates
            .iter()
            .map(|c| {
                let p = parse_body(&c.body).unwrap();
                Site { first_line: c.first_line, last_line: c.last_line, inline: c.inline, explicit: p.explicit, is_end: p.verb == Verb::End }
            })
            .collect();
        attach(&n, &lexed, ext, header_skip, &sites)
    }
    pub(crate) fn run(src: &str, ext: &str) -> Vec<Outcome> {
        run_with(src, ext, &[])
    }
    pub(crate) fn file() -> Outcome {
        Outcome::Bound { extent: Extent::File, end_site: None }
    }

    #[test]
    fn r_syn_016__file_form_after_license_banner() {
        let src = "/*\n * License\n */\n// fairlead: use tok.a\n\nfn main() {}\n";
        assert_eq!(run(src, "rs"), vec![file()]);
        assert_eq!(run("// Copyright 2026\n// fairlead: def cmp.x\n\nfn main() {}\n", "rs"), vec![file()]);
    }

    #[test]
    fn r_syn_016__skips_bom_shebang_use_client_and_ts_check() {
        assert_eq!(run("\u{feff}// fairlead: use tt.b\n\ncode\n", "rs"), vec![file()]);
        assert_eq!(run("#!/usr/bin/env python\n# fairlead: use tt.b\n\nimport os\n", "py"), vec![file()]);
        assert_eq!(run("\"use client\";\n// fairlead: use tok.a\n\nexport {}\n", "tsx"), vec![file()]);
        assert_eq!(run("'use strict'\n// fairlead: use tok.a\n\nx()\n", "js"), vec![file()]);
        assert_eq!(run("// @ts-check\n// fairlead: use tt.b\n\nx\n", "ts"), vec![file()]);
        assert_eq!(run("<?php\n// fairlead: use tt.b\n\nx();\n", "rs"), vec![file()], "<?php line is skipped regardless of host");
        assert_eq!(run("<?xml version=\"1.0\"?>\n<!doctype svg>\n<!-- fairlead: def asset.logo -->\n\n<svg/>\n", "svg"), vec![file()]);
    }

    #[test]
    fn r_syn_016__header_skip_patterns_use_star_wildcards() {
        let skip = vec!["// eslint-disable*".to_owned(), "/* generated by * */".to_owned()];
        assert_eq!(run_with("// eslint-disable-next-line\n// fairlead: use tt.b\n\ncode\n", "ts", &skip), vec![file()]);
        assert_eq!(run_with("/* generated by tool 1.2 */\n// fairlead: use tt.b\n\ncode\n", "ts", &skip), vec![file()]);
        assert_eq!(run("// eslint-disable-next-line\n// fairlead: use tt.b\n\ncode\n", "ts"), vec![file()], "without the pattern both lines form the block");
    }

    #[test]
    fn r_syn_016__eof_also_ends_the_block_and_directives_stack() {
        assert_eq!(run("// fairlead: use tt.b", "rs"), vec![file()]);
        assert_eq!(run("// fairlead: def tt.b\n// fairlead: use uu.d\n\ncode\n", "rs"), vec![file(), file()]);
    }

    #[test]
    fn r_syn_016__block_directly_followed_by_code_is_not_file_form() {
        let out = run("// fairlead: use tt.b\nfn f() {}\n", "rs");
        assert_ne!(out, vec![file()], "edge case 5: standalone form, not file form");
    }

    #[test]
    fn r_syn_016__later_block_is_not_file_form() {
        let out = run("fn a() {}\n\n// fairlead: use tt.b\n\nfn b() {}\n", "rs");
        assert_ne!(out, vec![file()]);
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax attach::` — Expected: FAIL to compile: `cannot find type Site / Outcome / function attach`.

- [ ] **Step 3: Implement.** Add `pub mod attach;` to `lib.rs`, then above the tests in `attach.rs`:

```rust
//! Attachment of directives to code: file, trailing, standalone, explicit range, regions (R-SYN-016..023, 045, 046).
use crate::directive::{Explicit, Extent};
use crate::lexer::Lexed;
use crate::normalize::{slice, Normalized};
use core::ops::Range;

/// One parsed directive as `attach` sees it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Site {
    pub first_line: u32,
    pub last_line: u32,
    /// Code shares the comment's line (trailing form, R-SYN-017).
    pub inline: bool,
    pub explicit: Option<Explicit>,
    /// The verb is `end`.
    pub is_end: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Bound { extent: Extent, end_site: Option<usize> },
    /// An `end` that closed a `+end` region; it is structure, not a directive record.
    ClosesRegion,
    NothingToBind,
    UnmatchedEnd,
    UnclosedRegion,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Kind {
    Blank,
    CommentOnly,
    Code,
}

fn blank_out(b: &mut [u8], r: Range<usize>) {
    for k in r {
        if let Some(x) = b.get_mut(k) {
            if *x != b'\n' {
                *x = b' ';
            }
        }
    }
}

/// Copy of the text with comment bytes (and string bytes when `strings`) replaced by spaces.
fn mask(n: &Normalized, lexed: &Lexed, strings: bool) -> String {
    let mut b = n.text.as_bytes().to_vec();
    for c in &lexed.comments {
        blank_out(&mut b, c.start..c.end);
    }
    if strings {
        for s in &lexed.strings {
            blank_out(&mut b, s.clone());
        }
    }
    String::from_utf8(b).unwrap_or_else(|e| {
        let mut raw = e.into_bytes();
        for x in raw.iter_mut().filter(|x| !x.is_ascii()) {
            *x = b' ';
        }
        String::from_utf8(raw).unwrap_or_default()
    })
}

/// `*` matches any run of characters; everything else is literal.
fn wildcard(pattern: &str, text: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let Some(mut rest) = text.strip_prefix(first) else { return false };
    let all: Vec<&str> = parts.collect();
    let Some((last, mids)) = all.split_last() else { return rest.is_empty() };
    for m in mids {
        rest = match rest.find(m) {
            Some(i) => rest.get(i + m.len()..).unwrap_or(""),
            None => return false,
        };
    }
    rest.ends_with(last)
}

struct Ctx<'a> {
    n: &'a Normalized,
    ext: String,
    kinds: Vec<Kind>,
    /// Comments and strings masked (bracket matching, indentation).
    code: String,
    count: u32,
}

impl<'a> Ctx<'a> {
    fn new(n: &'a Normalized, lexed: &Lexed, ext: &str) -> Ctx<'a> {
        let no_comments = mask(n, lexed, false);
        let code = mask(n, lexed, true);
        let count = n.line_count();
        let kinds = (1..=count)
            .map(|l| {
                if n.line_text(l).trim().is_empty() {
                    Kind::Blank
                } else if slice(&no_comments, n.line_start(l), n.line_end(l)).trim().is_empty() {
                    Kind::CommentOnly
                } else {
                    Kind::Code
                }
            })
            .collect();
        Ctx { n, ext: ext.trim_start_matches('.').to_ascii_lowercase(), kinds, code, count }
    }

    fn kind(&self, line: u32) -> Kind {
        line.checked_sub(1)
            .and_then(|i| self.kinds.get(i as usize))
            .copied()
            .unwrap_or(Kind::Blank)
    }

    fn is_html_family(&self) -> bool {
        matches!(self.ext.as_str(), "html" | "xml" | "svg")
    }

    fn header_line(&self, l: u32, header_skip: &[String]) -> bool {
        let t = self.n.line_text(l).trim();
        t.is_empty()
            || (l == 1 && t.starts_with("#!"))
            || t.starts_with("<?php")
            || matches!(t.trim_end_matches(';'), "\"use client\"" | "'use client'" | "\"use strict\"" | "'use strict'")
            || (self.is_html_family() && (t.to_ascii_lowercase().starts_with("<!doctype") || t.starts_with("<?xml")))
            || (self.kind(l) == Kind::CommentOnly && t.contains("@ts-check"))
            || header_skip.iter().any(|p| wildcard(p, t))
    }

    /// The first comment block, when it is followed by a blank line or EOF (R-SYN-016).
    fn file_block(&self, header_skip: &[String]) -> Option<(u32, u32)> {
        let mut l = 1;
        while l <= self.count && self.header_line(l, header_skip) {
            l += 1;
        }
        if l > self.count || self.kind(l) != Kind::CommentOnly {
            return None;
        }
        let start = l;
        while l < self.count && self.kind(l + 1) == Kind::CommentOnly {
            l += 1;
        }
        (l == self.count || self.kind(l + 1) == Kind::Blank).then_some((start, l))
    }
}

/// One outcome per site, in order. File form only in this task; the remaining forms follow in Tasks 16 and 17.
pub fn attach(n: &Normalized, lexed: &Lexed, ext: &str, header_skip: &[String], sites: &[Site]) -> Vec<Outcome> {
    let cx = Ctx::new(n, lexed, ext);
    let block = cx.file_block(header_skip);
    sites
        .iter()
        .map(|s| match block {
            Some((a, b)) if !s.is_end && !s.inline && s.explicit.is_none() && s.first_line >= a && s.last_line <= b => {
                Outcome::Bound { extent: Extent::File, end_site: None }
            }
            _ => Outcome::NothingToBind,
        })
        .collect()
}
```

Check against the tests: for `"/*\n * License\n */\n// fairlead: use tok.a\n\nfn main() {}\n"` lines 1-4 are `CommentOnly` (the block comment's lines are masked to spaces; line 4 is a `//` comment), line 5 is blank, so the block is `(1, 4)` and the site at line 4 is inside. For `"<?php\n// fairlead..."` the `<?php` line is skipped by `header_line` for every host (the spec lists it without a host condition). For the SVG case the lexer sees only the `<!-- ... -->` comment; lines 1-2 are skipped (`<?xml`, `<!doctype`), the comment is line 3, line 4 blank. For `without the pattern` both `// eslint-disable-next-line` and `// fairlead: ...` are comment-only lines forming one block (1-2) followed by a blank line, so the site is file form.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax attach::` — Expected: PASS (6 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): file-form attachment with header skipping" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

### Task 16: Attachment, part 2: trailing and standalone forms (`attach.rs`)

**Rules:** R-SYN-017 (a directive in a comment on a line that also holds code binds to that line), R-SYN-018 (standalone: binds to the next code line plus its extent: unbalanced `{ ( [` run to the matching close, capped at 500 lines; a line ending in `:` or followed by a more indented line takes the indented block; otherwise one line; consecutive directive lines stack on the same code line), R-SYN-045 (attribute, decorator and doc-comment lines between the directive and the code are skipped, and the extent includes them), R-SYN-046 (a multi-line signature followed by a `{` body binds as one item), R-SYN-023 (heuristics decide where, never what).

**Files:** Modify `crates/fairlead-syntax/src/attach.rs`.

**Interfaces:** `attach` now binds every non-`end`, non-explicit site: inline sites to `Lines { first: l, last: l }`, standalone sites to `Lines { first, last }`, or `NothingToBind`. Added private `Ctx` methods: `code_line`, `next_nonblank`, `is_attribute`, `attr_end`, `bind_target`, `extent_last`, `indent_extent(b, from, cap)`. Constant `MAX_EXTENT: u32 = 500`.

Interpretation notes (also listed under Contract issues): blank lines and ordinary comment-only lines between a standalone directive and its code line are skipped; the extent's `first` is the first non-blank skipped comment/attribute line directly above the code line (a blank line resets it), excluding lines that hold directives themselves.

- [ ] **Step 1: Write the failing test.** Append inside `mod tests` of `attach.rs`:

```rust
    pub(crate) fn lines(first: u32, last: u32) -> Outcome {
        Outcome::Bound { extent: Extent::Lines { first, last }, end_site: None }
    }

    #[test]
    fn r_syn_017__trailing_directive_binds_to_its_own_line() {
        assert_eq!(run("let x = 1; // fairlead: use tt.b\nlet y = 2;\n", "rs"), vec![lines(1, 1)]);
        assert_eq!(run("x = 1  # fairlead: use tt.b\ny = 2\n", "py"), vec![lines(1, 1)]);
        let jsx = "<Button>\n  {/* fairlead: use tok.surface */}\n  <Icon />\n</Button>\n";
        assert_eq!(run(jsx, "tsx"), vec![lines(2, 2)], "JSX comment sits between braces on its own line");
    }

    #[test]
    fn r_syn_018__standalone_one_liner_and_blank_lines() {
        assert_eq!(run("fn a() {}\n// fairlead: use tt.b\nlet x = 1;\n", "rs"), vec![lines(3, 3)]);
        assert_eq!(run("fn a() {}\n\n// fairlead: use tt.b\n\nfn b() {}\n", "rs"), vec![lines(5, 5)]);
    }

    #[test]
    fn r_syn_018__unbalanced_opener_runs_to_the_matching_close() {
        let src = "fn pre() {}\n\n// fairlead: def cmp.x\nfn run() {\n    if x {\n        y();\n    }\n}\nfn after() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 8)]);
        let ts = "const a = 1;\n\n// fairlead: def cmp.b\nexport function B() {\n  return (\n    <div/>\n  );\n}\n";
        assert_eq!(run(ts, "tsx"), vec![lines(4, 8)]);
    }

    #[test]
    fn r_syn_018__colon_or_deeper_indent_takes_the_indented_block() {
        let py = "import os\n\n# fairlead: use tt.b\ndef f(x):\n    return x\n\ndef g(): pass\n";
        assert_eq!(run(py, "py"), vec![lines(4, 5)]);
        let yaml = "a: 1\n\n# fairlead: use tt.b\nservices:\n  web:\n    image: x\n\nother: 2\n";
        assert_eq!(run(yaml, "yaml"), vec![lines(4, 6)]);
        let toml = "[package]\nname = \"x\"\n\n# fairlead: use eng.sync-verify\nsync-verify = { path = \"../sync-verify\" }\n";
        assert_eq!(run(toml, "toml"), vec![lines(5, 5)]);
    }

    #[test]
    fn r_syn_018__consecutive_directives_stack_on_one_code_line() {
        let src = "x\n\n// fairlead: def tt.b\n// fairlead: use uu.d\nfn f() {\n}\n";
        assert_eq!(run(src, "rs"), vec![lines(5, 6), lines(5, 6)]);
    }

    #[test]
    fn r_syn_045__attribute_lines_are_skipped_and_included() {
        let src = "fn pre() {}\n\n// fairlead: def cmd.x\n#[tauri::command]\npub async fn sync_now(\n    a: i32,\n) -> Result<(), E>\n{\n    body();\n}\nfn post() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 10)]);
        let doc = "x\n\n// fairlead: def cmd.y\n/// Docs\n#[attr]\nfn y() {}\n";
        assert_eq!(run(doc, "rs"), vec![lines(4, 6)]);
        let dec = "x\n\n// fairlead: def cmp.z\n@Component({\n  a: 1,\n})\nexport class Z {\n}\n";
        assert_eq!(run(dec, "ts"), vec![lines(4, 8)]);
        let cs = "x;\n\n// fairlead: def cmd.w\n[HttpGet]\npublic void W() {\n}\n";
        assert_eq!(run(cs, "cs"), vec![lines(4, 6)]);
    }

    #[test]
    fn r_syn_046__multi_line_signature_binds_with_its_body() {
        let src = "x\n\n// fairlead: def fn.a\nfn a(\n    p: i32,\n) -> i32 {\n    p\n}\nfn b() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 8)]);
        let decl = "x\n\n// fairlead: def fn.c\nfn c(\n    p: i32,\n);\nfn d() {}\n";
        assert_eq!(run(decl, "rs"), vec![lines(4, 6)], "a `;` at depth 0 ends the item");
    }

    #[test]
    fn r_syn_018__extent_is_capped_at_500_lines() {
        let mut src = String::from("x\n\n// fairlead: def tt.b\nfn big() {\n");
        for _ in 0..600 {
            src.push_str("    y();\n");
        }
        src.push_str("}\n");
        assert_eq!(run(&src, "rs"), vec![lines(4, 503)]);
    }

    #[test]
    fn r_syn_023__brackets_in_strings_and_comments_are_ignored() {
        let src = "x\n\n// fairlead: def tt.b\nfn f() {\n    let s = \"}\"; // }\n}\nfn g() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 6)]);
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax attach::` — Expected: FAIL: every new test asserts `lines(..)` but `attach` returns `NothingToBind` (e.g. `left: [NothingToBind]`, `right: [Bound { extent: Lines { first: 1, last: 1 }, .. }]`).

- [ ] **Step 3: Implement.** In `attach.rs` add `use std::collections::BTreeSet;`, the constant, these methods inside `impl<'a> Ctx<'a>`, and replace `attach`:

```rust
pub const MAX_EXTENT: u32 = 500;

fn indent_of(s: &str) -> usize {
    let mut w = 0;
    for c in s.chars() {
        match c {
            ' ' => w += 1,
            '\t' => w += 4,
            _ => break,
        }
    }
    w
}
```

```rust
    fn code_line(&self, l: u32) -> &str {
        slice(&self.code, self.n.line_start(l), self.n.line_end(l))
    }

    /// First line at or after `from` that holds code (comments and strings masked).
    fn next_nonblank(&self, from: u32) -> Option<u32> {
        (from..=self.count).find(|&l| !self.code_line(l).trim().is_empty())
    }

    /// Attribute and decorator lines (R-SYN-045).
    fn is_attribute(&self, l: u32) -> bool {
        let t = self.code_line(l).trim();
        match self.ext.as_str() {
            "rs" => t.starts_with("#[") || t.starts_with("#!["),
            "cs" => t.starts_with('[') && t.ends_with(']'),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "java" | "kt" | "scala" | "swift" | "py" => t
                .strip_prefix('@')
                .is_some_and(|r| r.chars().next().is_some_and(|c| c.is_ascii_alphabetic())),
            _ => false,
        }
    }

    /// Last line of an attribute that may span lines (bracket balanced, at most 50 lines).
    fn attr_end(&self, l: u32) -> u32 {
        let cap = l.saturating_add(50).min(self.count);
        let mut depth = 0i32;
        let mut k = l;
        while k <= cap {
            for c in self.code_line(k).bytes() {
                match c {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    _ => {}
                }
            }
            if depth <= 0 {
                return k;
            }
            k += 1;
        }
        cap
    }

    /// `(first, bind)`: the first skipped comment/attribute line directly above the code line (or the code line)
    /// and the code line itself. `dirs` are lines occupied by directives (they never start a run).
    fn bind_target(&self, after: u32, dirs: &BTreeSet<u32>) -> Option<(u32, u32)> {
        let mut run: Option<u32> = None;
        let mut l = after + 1;
        while l <= self.count {
            match self.kind(l) {
                Kind::Blank => run = None,
                Kind::CommentOnly => {
                    if !dirs.contains(&l) && run.is_none() {
                        run = Some(l);
                    }
                }
                Kind::Code => {
                    if self.is_attribute(l) {
                        run.get_or_insert(l);
                        l = self.attr_end(l);
                    } else {
                        return Some((run.unwrap_or(l), l));
                    }
                }
            }
            l += 1;
        }
        None
    }

    /// Last line of the item that starts on code line `b` (R-SYN-018, R-SYN-046).
    fn extent_last(&self, b: u32) -> u32 {
        let cap = b.saturating_add(MAX_EXTENT - 1).min(self.count);
        let (mut depth, mut opened, mut sig_checked) = (0i32, false, false);
        let mut l = b;
        while l <= cap {
            let mut semi = false;
            for c in self.code_line(l).bytes() {
                match c {
                    b'{' | b'(' | b'[' => {
                        depth += 1;
                        opened = true;
                    }
                    b'}' | b')' | b']' => depth = (depth - 1).max(0),
                    b';' if depth == 0 && opened => semi = true,
                    _ => {}
                }
            }
            if depth > 0 {
                l += 1;
                continue;
            }
            if !opened {
                return self.indent_extent(b, l, cap);
            }
            if semi || sig_checked {
                return l;
            }
            match self.next_nonblank(l + 1) {
                Some(nl) if nl <= cap && self.code_line(nl).trim_start().starts_with('{') => {
                    sig_checked = true;
                    l = nl;
                }
                _ => return self.indent_extent(b, l, cap),
            }
        }
        cap
    }

    /// Indented-block rule: item header on `b`, its last bracket-balanced line is `from`. A trailing `:` on `from`
    /// or a deeper-indented next line extends the extent over the indented block (R-SYN-018).
    fn indent_extent(&self, b: u32, from: u32, cap: u32) -> u32 {
        let base = indent_of(self.code_line(b));
        let colon = self.code_line(from).trim_end().ends_with(':');
        let deeper = self.next_nonblank(from + 1).is_some_and(|nl| indent_of(self.code_line(nl)) > base);
        if !(colon || deeper) {
            return from;
        }
        let mut last = from;
        let mut l = from + 1;
        while l <= cap {
            let s = self.code_line(l);
            if s.trim().is_empty() {
                l += 1;
            } else if indent_of(s) > base {
                last = l;
                l += 1;
            } else {
                break;
            }
        }
        last
    }
```

```rust
/// One outcome per site, in order. Explicit ranges and regions follow in Task 17.
pub fn attach(n: &Normalized, lexed: &Lexed, ext: &str, header_skip: &[String], sites: &[Site]) -> Vec<Outcome> {
    let cx = Ctx::new(n, lexed, ext);
    let block = cx.file_block(header_skip);
    let dirs: BTreeSet<u32> = sites.iter().flat_map(|s| s.first_line..=s.last_line).collect();
    sites
        .iter()
        .map(|s| {
            let bound = |first: u32, last: u32| Outcome::Bound { extent: Extent::Lines { first, last }, end_site: None };
            match block {
                Some((a, b)) if !s.is_end && !s.inline && s.explicit.is_none() && s.first_line >= a && s.last_line <= b => {
                    Outcome::Bound { extent: Extent::File, end_site: None }
                }
                _ if s.is_end || s.explicit.is_some() => Outcome::NothingToBind,
                _ if s.inline => bound(s.first_line, s.first_line),
                _ => match cx.bind_target(s.last_line, &dirs) {
                    Some((first, b)) => bound(first, cx.extent_last(b)),
                    None => Outcome::NothingToBind,
                },
            }
        })
        .collect()
}
```

Trace of the two tricky tests. Signature test: line 4 `fn a(` opens, depth 1; line 5 `p: i32,` depth 1; line 6 `) -> i32 {` closes to 0 then `{` reopens: depth 1 at end of line, so continue; line 7 `p`; line 8 `}` depth 0, `sig_checked` false, `semi` false, next non-blank line 9 `fn b() {}` does not start with `{`, return 8, so `lines(4, 8)`. Declaration test: line 6 `);` closes and `;` at depth 0 with `opened`, so `semi` and return 6. Attribute test: line 4 `#[tauri::command]` is an attribute (run starts at 4, `attr_end` = 4), line 5 is the code line; `extent_last(5)`: depth 1 after `(`, line 6 depth 1, line 7 `) -> Result<(), E>` ends at depth 0 (close, open, close), no `;`, next non-blank line 8 is `{` so `sig_checked`, line 8 depth 1, line 9 `body();` is at depth 1 so no `semi`, line 10 `}` depth 0 and `sig_checked` so return 10. The 500-line cap: binding line 4, `cap = 4 + 499 = 503`, the loop ends with `depth > 0` and returns `cap = 503`. The `#`-decorator for Python `@` and the TS decorator `@Component({ ... })`: `attr_end` follows the balanced `(` `{` `}` `)` to line 6, code line 7 `export class Z {` to 8.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax attach::` — Expected: PASS (15 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): trailing and standalone attachment with attributes and signatures" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 17: Attachment, part 3: explicit ranges, regions, binds-nothing (`attach.rs`)

**Rules:** R-SYN-019 (`+N` sets the extent to N lines; `+end` opens a nestable region closed by a later `end`; an `end` with no open region and a region unclosed at EOF are `HN-S008`; a region opens only by an explicit `+end`), R-SYN-021 (a directive that binds to nothing is dropped, `HN-S010`), R-SYN-014 (foreign regions), R-FILE-042.

**Files:** Modify `crates/fairlead-syntax/src/attach.rs`.

**Interfaces:** `attach` rewritten: explicit `+N` binds N lines from the extent's first line (clamped to the file end); `+end` sites push onto a stack; an `end` site pops it, giving the opener `Bound { extent: Lines { first: start, last }, end_site: Some(i) }` and itself `ClosesRegion`; empty `end` stack gives `UnmatchedEnd`; leftovers give `UnclosedRegion`. A region opened by a standalone directive starts on the line after the directive's last line; by a trailing one, on its own line. The region ends on the line before the `end`; an empty region is the opener's last line.

- [ ] **Step 1: Write the failing test.** Append inside `mod tests`:

```rust
    #[test]
    fn r_syn_019__plus_n_sets_the_extent_to_n_lines() {
        let src = "x\n\n// fairlead: use tt.b +3\nfn a() {\n  x();\n}\nfn b() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 6)]);
        assert_eq!(run("let a = 1; // fairlead: use tt.b +2\nlet b = 2;\nlet c = 3;\n", "rs"), vec![lines(1, 2)]);
        assert_eq!(run("x\n\n// fairlead: use tt.b +9\nfn a() {}\n", "rs"), vec![lines(4, 4)], "clamped to the last line");
    }

    #[test]
    fn r_syn_019__plus_end_region_closes_at_matching_end() {
        let css = ":root { --primary: #2563eb; }\n/* fairlead: env dev +end */\n.debug-grid { outline: 1px solid red; }\n.more { x: y; }\n/* fairlead: end */\n.after {}\n";
        assert_eq!(
            run(css, "css"),
            vec![Outcome::Bound { extent: Extent::Lines { first: 3, last: 4 }, end_site: Some(1) }, Outcome::ClosesRegion]
        );
    }

    #[test]
    fn r_syn_019__regions_nest_and_close_innermost_first() {
        let src = "x\n/* fairlead: env a +end */\nA\n/* fairlead: env b +end */\nB\n/* fairlead: end */\nC\n/* fairlead: end */\nD\n";
        let out = run(src, "css");
        assert_eq!(out[0], Outcome::Bound { extent: Extent::Lines { first: 3, last: 7 }, end_site: Some(3) });
        assert_eq!(out[1], Outcome::Bound { extent: Extent::Lines { first: 5, last: 5 }, end_site: Some(2) });
        assert_eq!(out[2], Outcome::ClosesRegion);
        assert_eq!(out[3], Outcome::ClosesRegion);
    }

    #[test]
    fn r_syn_019__foreign_region_in_css() {
        let src = "a {}\n/* fairlead: def foreign.shadcn tool=shadcn +end */\n.shadcn-block { color: #111; }\n/* fairlead: end */\n";
        let out = run(src, "css");
        assert_eq!(out[0], Outcome::Bound { extent: Extent::Lines { first: 3, last: 3 }, end_site: Some(1) });
    }

    #[test]
    fn r_syn_019__unmatched_end_and_unclosed_region_are_flagged() {
        assert_eq!(run("a {}\n/* fairlead: end */\n", "css"), vec![Outcome::UnmatchedEnd]);
        assert_eq!(run("a {}\n/* fairlead: env dev +end */\nb {}\n", "css"), vec![Outcome::UnclosedRegion]);
    }

    #[test]
    fn r_syn_019__a_directive_never_becomes_a_region_by_lookahead() {
        let out = run("a {}\n/* fairlead: env dev */\nb { x: y; }\n/* fairlead: end */\n", "css");
        assert_eq!(out[0], lines(3, 3), "no +end: ordinary standalone binding");
        assert_eq!(out[1], Outcome::UnmatchedEnd);
    }

    #[test]
    fn r_syn_019__empty_region_is_the_openers_last_line() {
        let out = run("a {}\n/* fairlead: env dev +end */\n/* fairlead: end */\nb {}\n", "css");
        assert_eq!(out[0], Outcome::Bound { extent: Extent::Lines { first: 2, last: 2 }, end_site: Some(1) });
    }

    #[test]
    fn r_syn_021__directive_with_nothing_after_it_binds_to_nothing() {
        assert_eq!(run("fn a() {}\n// fairlead: use tt.b\n", "rs"), vec![Outcome::NothingToBind]);
        assert_eq!(run("fn a() {}\n// fairlead: use tt.b\n\n\n", "rs"), vec![Outcome::NothingToBind]);
        assert_eq!(run("fn a() {}\n// fairlead: use tt.b\n// plain comment\n", "rs"), vec![Outcome::NothingToBind]);
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax r_syn_019 r_syn_021` — run each filter separately: `cargo test -p fairlead-syntax attach::tests::r_syn_019` and `attach::tests::r_syn_021`. Expected: FAIL: explicit and `end` sites return `NothingToBind`.

- [ ] **Step 3: Implement.** Replace `attach` in `attach.rs` with:

```rust
fn region_start(s: &Site) -> u32 {
    if s.inline { s.first_line } else { s.last_line + 1 }
}

/// One outcome per site, in order (R-SYN-016..019, 021, 045, 046).
pub fn attach(n: &Normalized, lexed: &Lexed, ext: &str, header_skip: &[String], sites: &[Site]) -> Vec<Outcome> {
    let cx = Ctx::new(n, lexed, ext);
    let block = cx.file_block(header_skip);
    let dirs: BTreeSet<u32> = sites.iter().flat_map(|s| s.first_line..=s.last_line).collect();
    let bound = |first: u32, last: u32, end_site: Option<usize>| Outcome::Bound { extent: Extent::Lines { first, last }, end_site };
    let mut out: Vec<Option<Outcome>> = vec![None; sites.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (i, s) in sites.iter().enumerate() {
        let result = if s.is_end {
            match stack.pop().and_then(|o| sites.get(o).map(|op| (o, op))) {
                None => Outcome::UnmatchedEnd,
                Some((o, op)) => {
                    let last = s.first_line.saturating_sub(1).max(op.last_line);
                    let first = region_start(op).min(last);
                    if let Some(slot) = out.get_mut(o) {
                        *slot = Some(bound(first, last, Some(i)));
                    }
                    Outcome::ClosesRegion
                }
            }
        } else if s.explicit == Some(Explicit::Region) {
            stack.push(i);
            continue;
        } else if let Some(Explicit::Lines(k)) = s.explicit {
            let target = if s.inline { Some((s.first_line, s.first_line)) } else { cx.bind_target(s.last_line, &dirs) };
            match target {
                Some((first, _)) => bound(first, first.saturating_add(k - 1).min(cx.count), None),
                None => Outcome::NothingToBind,
            }
        } else if let (Some((a, b)), false) = (block, s.inline) {
            if s.first_line >= a && s.last_line <= b {
                Outcome::Bound { extent: Extent::File, end_site: None }
            } else {
                standalone(&cx, s, &dirs)
            }
        } else if s.inline {
            bound(s.first_line, s.first_line, None)
        } else {
            standalone(&cx, s, &dirs)
        };
        if let Some(slot) = out.get_mut(i) {
            *slot = Some(result);
        }
    }
    for o in stack {
        if let Some(slot) = out.get_mut(o) {
            *slot = Some(Outcome::UnclosedRegion);
        }
    }
    out.into_iter().map(|o| o.unwrap_or(Outcome::NothingToBind)).collect()
}

fn standalone(cx: &Ctx<'_>, s: &Site, dirs: &BTreeSet<u32>) -> Outcome {
    match cx.bind_target(s.last_line, dirs) {
        Some((first, b)) => Outcome::Bound { extent: Extent::Lines { first, last: cx.extent_last(b) }, end_site: None },
        None => Outcome::NothingToBind,
    }
}
```

Traces. Nested regions: sites 0..3 are `env a +end` (line 2), `env b +end` (line 4), `end` (line 6), `end` (line 8). The first `end` pops site 1: `last = max(5, 4) = 5`, `first = min(5, 5) = 5`, so `lines(5, 5)`; the second pops site 0: `last = max(7, 2) = 7`, `first = min(3, 7) = 3`, giving `Lines{3, 7}` as asserted. Empty region: opener line 2, `end` line 3: `last = max(2, 2) = 2`, `first = min(3, 2) = 2`. The unmatched-end-by-lookahead test: the first directive has no `+end` so it is standalone and binds line 3; the later `end` has an empty stack, so `UnmatchedEnd`. `r_syn_021` third case: the next lines are comment-only/blank, so `bind_target` reaches EOF and returns `None`.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax attach::` — Expected: PASS (23 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): explicit ranges, +end regions and bind-to-nothing" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 18: Attachment, part 4: Markdown doc form (`attach.rs`)

**Rules:** R-SYN-022 (a body comment `<!-- fairlead: ... -->` binds to the next block (paragraph, list, table or code fence); before a heading it binds to that heading's section; frontmatter directives bind to the doc), R-SYN-005 (the lexer already ignored fenced and indented code), R-SYN-016 (Markdown has no file form).

**Files:** Modify `crates/fairlead-syntax/src/attach.rs`.

**Interfaces:** For `ext` `md`/`markdown` the file form is disabled and non-explicit, non-inline sites bind through `md_bind`: a heading yields the section (`Lines { first: heading line, last: last non-blank line before the next heading of the same or higher level }`; a level-1 heading yields `Extent::Doc`, see Contract issues); a code fence yields fence to closing fence; any other line yields the paragraph/list/table up to the next blank line. Inline comments bind their own line.

- [ ] **Step 1: Write the failing test.** Append inside `mod tests`:

```rust
    #[test]
    fn r_syn_022__comment_before_a_paragraph_binds_the_block() {
        let src = "# T\n\n<!-- fairlead: use tt.b -->\nSome text\nmore text\n\nnext\n";
        assert_eq!(run(src, "md"), vec![lines(4, 5)]);
        let list = "<!-- fairlead: use tt.b -->\n- one\n- two\n\nafter\n";
        assert_eq!(run(list, "md"), vec![lines(2, 3)]);
        let table = "intro\n\n<!-- fairlead: use tt.b -->\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        assert_eq!(run(table, "md"), vec![lines(4, 6)]);
    }

    #[test]
    fn r_syn_022__comment_before_a_code_fence_binds_the_fence() {
        let src = "<!-- fairlead: use tt.b -->\n```rs\ncode\n```\nafter\n";
        assert_eq!(run(src, "md"), vec![lines(2, 4)]);
        assert_eq!(run("<!-- fairlead: use tt.b -->\n~~~~\nopen forever\n", "md"), vec![lines(2, 3)]);
    }

    #[test]
    fn r_syn_022__comment_before_a_heading_binds_the_section() {
        let src = "<!-- fairlead: use tt.b -->\n## Sec\ntext\n\n### Sub\nmore\n\n## Next\nx\n";
        assert_eq!(run(src, "md"), vec![lines(2, 6)]);
        let title = "<!-- fairlead: use tok.primary -->\n# Colour\n\ntext\n";
        assert_eq!(run(title, "md"), vec![Outcome::Bound { extent: Extent::Doc, end_site: None }]);
    }

    #[test]
    fn r_syn_022__inline_comment_binds_its_line_and_no_target_binds_nothing() {
        assert_eq!(run("Text <!-- fairlead: use tt.b --> more\nnext\n", "md"), vec![lines(1, 1)]);
        assert_eq!(run("# T\n<!-- fairlead: use tt.b -->\n", "md"), vec![Outcome::NothingToBind]);
    }

    #[test]
    fn r_syn_016__markdown_has_no_file_form() {
        let out = run("<!-- fairlead: use tt.b -->\n\nBody text\n", "md");
        assert_eq!(out, vec![lines(3, 3)]);
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax attach::tests::r_syn_022` and `r_syn_016__markdown` — Expected: FAIL: the Markdown comments are bound by the code-oriented rules (for example the paragraph test yields `lines(4, 4)` and the heading tests yield `lines(2, 2)`).

- [ ] **Step 3: Implement.** Add to `attach.rs`:

```rust
fn heading_level(line: &str) -> Option<usize> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let level = t.bytes().take_while(|b| *b == b'#').count();
    let after = t.as_bytes().get(level).copied();
    ((1..=6).contains(&level) && matches!(after, None | Some(b' ') | Some(b'\t'))).then_some(level)
}

fn fence_of(line: &str) -> Option<(u8, usize)> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let ch = *t.as_bytes().first()?;
    let run = t.bytes().take_while(|b| *b == ch).count();
    ((ch == b'`' || ch == b'~') && run >= 3).then_some((ch, run))
}
```

Inside `impl<'a> Ctx<'a>`:

```rust
    fn is_markdown(&self) -> bool {
        matches!(self.ext.as_str(), "md" | "markdown")
    }

    /// R-SYN-022: the block, fence or heading section that follows a body comment.
    fn md_bind(&self, s: &Site) -> Option<Extent> {
        if s.inline {
            return Some(Extent::Lines { first: s.first_line, last: s.first_line });
        }
        let mut l = s.last_line + 1;
        while l <= self.count && self.kind(l) != Kind::Code {
            l += 1;
        }
        if l > self.count {
            return None;
        }
        let first_text = self.n.line_text(l);
        if let Some(level) = heading_level(first_text) {
            let mut last = l;
            for k in l + 1..=self.count {
                let t = self.n.line_text(k);
                if heading_level(t).is_some_and(|h| h <= level) {
                    break;
                }
                if !t.trim().is_empty() {
                    last = k;
                }
            }
            return Some(if level == 1 { Extent::Doc } else { Extent::Lines { first: l, last } });
        }
        if let Some((ch, len)) = fence_of(first_text) {
            let mut last = self.count;
            for k in l + 1..=self.count {
                let t = self.n.line_text(k).trim();
                if t.len() >= len && t.bytes().all(|b| b == ch) {
                    last = k;
                    break;
                }
            }
            return Some(Extent::Lines { first: l, last });
        }
        let mut last = l;
        while last < self.count && !self.n.line_text(last + 1).trim().is_empty() {
            last += 1;
        }
        Some(Extent::Lines { first: l, last })
    }
```

In `attach`, change the file-block line and the two `standalone` fall-throughs: set `let block = if cx.is_markdown() { None } else { cx.file_block(header_skip) };` and replace the body of `fn standalone` with

```rust
fn standalone(cx: &Ctx<'_>, s: &Site, dirs: &BTreeSet<u32>) -> Outcome {
    if cx.is_markdown() {
        return match cx.md_bind(s) {
            Some(extent) => Outcome::Bound { extent, end_site: None },
            None => Outcome::NothingToBind,
        };
    }
    match cx.bind_target(s.last_line, dirs) {
        Some((first, b)) => Outcome::Bound { extent: Extent::Lines { first, last: cx.extent_last(b) }, end_site: None },
        None => Outcome::NothingToBind,
    }
}
```

and in the `s.inline` arm of `attach` call `standalone` too when markdown is not involved: the existing `bound(s.first_line, s.first_line, None)` already gives `Lines{l,l}` for inline Markdown comments, so no further change is needed there.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax attach::` — Expected: PASS (28 tests).

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): Markdown doc-form attachment" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

### Task 19: `ExtractConfig`, `Extraction` and the skip statuses (`extract.rs`, part 1)

**Rules:** R-SYN-030 (files over `max-file` and files with a NUL in the first 8 KB are skipped with `HN-I001`; non-UTF-8 is skipped with `HN-I003`), R-FILE-015 (never decoded by guess), R-ARCH-032 (no panic on any byte input), R-SYN-006 (unknown extension: one `HN-I002` with the hint line to add; JSON carries no directives), R-ARCH-030 (pure function of bytes, extension, config), R-SYN-016 config (`header_skip`), contract 4.2 (`config_fingerprint`).

**Files:**
- Create: `crates/fairlead-syntax/src/extract.rs`
- Modify: `crates/fairlead-syntax/src/lib.rs` (`pub mod extract;` and re-exports `extract`, `ExtractConfig`, `Extraction`, `ScanStatus`, `render_directive_body`, `known_keys`, plus `pub use` of the types listed in contract 4.2)

**Interfaces:**
- Consumes: `delimiters::{CommentTable, Leader}`, `diag::*`, `directive::Directive`, `frontmatter::Frontmatter`.
- Produces (contract 4.2, exact): `ExtractConfig { marker, comments, header_skip, max_file, max_directives }`, `ExtractConfig::config_fingerprint(&self) -> [u8; 32]`, `ScanStatus { Scanned, NoMarker, SkippedTooLarge, SkippedBinary, SkippedNotUtf8 }`, `Extraction { status, directives, frontmatter, diags, candidates, dropped }`, `pub fn extract(bytes: &[u8], ext: &str, cfg: &ExtractConfig) -> Extraction`. Additive: `impl Default for ExtractConfig` (marker `"fairlead"`, built-in table, `max_file` 2_000_000, `max_directives` 500, no header skips), `ExtractConfig::canonical_bytes`, `Extraction::empty`. In this task `extract` returns before scanning for every status except `Scanned`/`NoMarker`; the scanning pipeline arrives in Task 20 behind the private function `scan_text`.

Marker prefilter (R-ARCH-030 step 1): a case-insensitive ASCII search for the marker (`memchr2` on the first byte, then `eq_ignore_ascii_case`) so that near-miss spellings such as `Fairlead:` still reach the scanner and produce `HN-S005`. Markdown files whose first line is `+++` always pass (their frontmatter has no marker).

`config_fingerprint`: `fairlead-syntax` has no hash crate in its dependency list (contract Section 2), so the fingerprint is four FNV-1a 64-bit lanes (distinct seeds) over `canonical_bytes()`, which is a length-prefixed encoding that also includes `SYNTAX_VERSION` and `GRAMMAR_EDITION`. It is a cache key, not a security primitive; see Contract issues for the alternative.

- [ ] **Step 1: Write the failing test.** Create `extract.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::delimiters::Leader;
    use crate::diag::Severity;

    fn cfg() -> ExtractConfig {
        ExtractConfig::default()
    }

    #[test]
    fn r_syn_030__default_limits_and_marker() {
        let c = cfg();
        assert_eq!((c.marker.as_str(), c.max_file, c.max_directives), ("fairlead", 2_000_000, 500));
        assert!(c.header_skip.is_empty());
    }

    #[test]
    fn r_syn_030__too_large_is_skipped_with_i001() {
        let mut c = cfg();
        c.max_file = 10;
        let ex = extract(b"// fairlead: use tt.b\n", "rs", &c);
        assert_eq!(ex.status, ScanStatus::SkippedTooLarge);
        assert_eq!(ex.diags.len(), 1);
        assert_eq!((ex.diags[0].code, ex.diags[0].severity), (codes::HN_I001, Severity::Info));
        assert!(ex.directives.is_empty() && ex.candidates == 0 && ex.dropped == 0);
        assert_eq!(extract(b"0123456789", "rs", &c).status, ScanStatus::NoMarker, "exactly max-file is fine");
    }

    #[test]
    fn r_syn_030__nul_in_the_first_8k_means_binary() {
        let mut bytes = b"// fairlead: use tt.b\n".to_vec();
        bytes.resize(100, b' ');
        bytes[50] = 0;
        let ex = extract(&bytes, "rs", &cfg());
        assert_eq!(ex.status, ScanStatus::SkippedBinary);
        assert_eq!(ex.diags[0].code, codes::HN_I001);
        let mut late = vec![b' '; 9000];
        late[8500] = 0;
        assert_eq!(extract(&late, "rs", &cfg()).status, ScanStatus::NoMarker, "a NUL after 8 KB does not count");
    }

    #[test]
    fn r_file_015__non_utf8_is_skipped_with_i003_and_never_decoded() {
        let ex = extract(b"// fairlead: use tt.b \xe9\xe8\n", "rs", &cfg());
        assert_eq!(ex.status, ScanStatus::SkippedNotUtf8);
        assert_eq!((ex.diags[0].code, ex.diags[0].severity), (codes::HN_I003, Severity::Warn));
        assert!(ex.directives.is_empty());
    }

    #[test]
    fn r_arch_032__any_bytes_are_accepted() {
        for b in [&b""[..], b"\xff", b"\xef\xbb\xbf", b"\x00", b"\r", b"\xc3", b"fairlead", b"// fairlead:"] {
            let _ = extract(b, "rs", &cfg());
            let _ = extract(b, "md", &cfg());
            let _ = extract(b, "", &cfg());
        }
    }

    #[test]
    fn r_arch_030__no_marker_skips_scanning_entirely() {
        let ex = extract(b"fn main() {}\n// just a comment\n", "rs", &cfg());
        assert_eq!(ex.status, ScanStatus::NoMarker);
        assert!(ex.diags.is_empty() && ex.directives.is_empty());
        assert_eq!(extract(b"", "rs", &cfg()).status, ScanStatus::NoMarker);
    }

    #[test]
    fn r_syn_006__unknown_extension_yields_one_i002_only_when_the_marker_is_present() {
        let ex = extract(b"x ; fairlead: use tt.b\n", "zzz", &cfg());
        assert_eq!(ex.status, ScanStatus::Scanned);
        assert_eq!(ex.diags.len(), 1);
        assert_eq!((ex.diags[0].code, ex.diags[0].severity), (codes::HN_I002, Severity::Info));
        assert!(ex.diags[0].hint.contains("[comments]"));
        assert_eq!(extract(b"nothing here\n", "zzz", &cfg()).diags.len(), 0);
    }

    #[test]
    fn r_syn_006__json_carries_no_directives() {
        let ex = extract(b"{\"a\": \"// fairlead: use tt.b\"}\n", "json", &cfg());
        assert_eq!(ex.status, ScanStatus::Scanned);
        assert!(ex.diags.is_empty() && ex.directives.is_empty() && ex.candidates == 0);
    }

    #[test]
    fn r_syn_003__prefilter_is_case_insensitive_so_near_misses_reach_the_scanner() {
        let ex = extract(b"// Fairlead: use tt.b\n", "rs", &cfg());
        assert_eq!(ex.status, ScanStatus::Scanned);
    }

    #[test]
    fn r_syn_006__fingerprint_is_stable_and_sensitive_to_every_field() {
        let base = cfg();
        assert_eq!(base.config_fingerprint(), cfg().config_fingerprint());
        let mut variants = Vec::new();
        let mut a = cfg();
        a.marker = "fairlead".into();
        variants.push(a);
        let mut a = cfg();
        a.header_skip.push("// eslint*".into());
        variants.push(a);
        let mut a = cfg();
        a.max_file = 1;
        variants.push(a);
        let mut a = cfg();
        a.max_directives = 1;
        variants.push(a);
        let mut a = cfg();
        a.comments = a.comments.with_overrides("foo", vec![Leader::Line(";".into())]);
        variants.push(a);
        for v in &variants {
            assert_ne!(v.config_fingerprint(), base.config_fingerprint());
        }
        let prints: std::collections::BTreeSet<[u8; 32]> = variants.iter().map(|v| v.config_fingerprint()).collect();
        assert_eq!(prints.len(), variants.len());
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax extract::` — Expected: FAIL to compile: `cannot find type ExtractConfig / function extract in this scope`.

- [ ] **Step 3: Implement.** Add `pub mod extract;` to `lib.rs`. Above the tests in `extract.rs`:

```rust
//! The single extraction entry point (R-ARCH-030..033). Pure, total, deterministic.
use crate::delimiters::{CommentTable, Leader};
use crate::diag::{codes, Span, SyntaxDiag};
use crate::directive::Directive;
use crate::frontmatter::Frontmatter;
use crate::{GRAMMAR_EDITION, SYNTAX_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExtractConfig {
    /// `[fairlead] marker`, default `"fairlead"`.
    pub marker: String,
    pub comments: CommentTable,
    /// `[scan] header-skip` patterns (`*` wildcard) skipped before the file-form block (R-SYN-016).
    pub header_skip: Vec<String>,
    /// Default 2_000_000 (R-SYN-030).
    pub max_file: u64,
    /// Default 500 (R-SYN-031).
    pub max_directives: u32,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        ExtractConfig {
            marker: "fairlead".to_owned(),
            comments: CommentTable::builtin(),
            header_skip: Vec::new(),
            max_file: 2_000_000,
            max_directives: 500,
        }
    }
}

fn put(b: &mut Vec<u8>, tag: u8, s: &[u8]) {
    b.push(tag);
    b.extend_from_slice(&(s.len() as u64).to_le_bytes());
    b.extend_from_slice(s);
}

fn fnv_lane(data: &[u8], basis: u64) -> u64 {
    let mut h = basis;
    for &x in data {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

impl ExtractConfig {
    /// Length-prefixed, order-stable encoding of everything that influences extraction.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        put(&mut b, b'V', &SYNTAX_VERSION.to_le_bytes());
        put(&mut b, b'E', &GRAMMAR_EDITION.to_le_bytes());
        put(&mut b, b'M', self.marker.as_bytes());
        for (ext, leaders) in self.comments.iter() {
            put(&mut b, b'X', ext.as_bytes());
            for l in leaders {
                match l {
                    Leader::Line(s) => put(&mut b, b'L', s.as_bytes()),
                    Leader::Block { open, close } => {
                        put(&mut b, b'O', open.as_bytes());
                        put(&mut b, b'C', close.as_bytes());
                    }
                    Leader::Html => put(&mut b, b'H', b""),
                }
            }
        }
        for p in &self.header_skip {
            put(&mut b, b'S', p.as_bytes());
        }
        put(&mut b, b'F', &self.max_file.to_le_bytes());
        put(&mut b, b'D', &self.max_directives.to_le_bytes());
        b
    }

    /// Cache-key part (contract 4.2). Not a security primitive.
    pub fn config_fingerprint(&self) -> [u8; 32] {
        let data = self.canonical_bytes();
        let mut out = [0u8; 32];
        for (k, chunk) in out.chunks_mut(8).enumerate() {
            let basis = 0xcbf2_9ce4_8422_2325u64 ^ (k as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            chunk.copy_from_slice(&fnv_lane(&data, basis).to_le_bytes());
        }
        out
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ScanStatus {
    Scanned,
    NoMarker,
    SkippedTooLarge,
    SkippedBinary,
    SkippedNotUtf8,
}

/// Cached, keyed by content hash + config (R-ARCH-073).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Extraction {
    pub status: ScanStatus,
    pub directives: Vec<Directive>,
    pub frontmatter: Option<Frontmatter>,
    pub diags: Vec<SyntaxDiag>,
    /// Marker candidates seen (a well-formed `end` that closes a `+end` region is structure, not counted).
    pub candidates: u32,
    /// Invariant: `candidates == directives.len() + dropped` (R-ARCH-031).
    pub dropped: u32,
}

impl Extraction {
    pub fn empty(status: ScanStatus) -> Extraction {
        Extraction { status, directives: Vec::new(), frontmatter: None, diags: Vec::new(), candidates: 0, dropped: 0 }
    }
    fn skipped(status: ScanStatus, diag: SyntaxDiag) -> Extraction {
        let mut e = Extraction::empty(status);
        e.diags.push(diag);
        e
    }
}

/// Case-insensitive ASCII search for the marker (so near-misses still reach the scanner).
fn contains_marker_ci(hay: &[u8], marker: &str) -> bool {
    let m = marker.as_bytes();
    let Some(&first) = m.first() else { return false };
    let (lo, up) = (first.to_ascii_lowercase(), first.to_ascii_uppercase());
    let mut from = 0usize;
    while let Some(p) = memchr::memchr2(lo, up, hay.get(from..).unwrap_or(&[])) {
        let at = from + p;
        if hay.get(at..at + m.len()).is_some_and(|w| w.eq_ignore_ascii_case(m)) {
            return true;
        }
        from = at + 1;
    }
    false
}

/// THE entry point. Pure function of bytes, extension and config (R-ARCH-030). Total: never panics,
/// never loops, any byte input (R-ARCH-032). Offsets in results are original-byte offsets.
pub fn extract(bytes: &[u8], ext: &str, cfg: &ExtractConfig) -> Extraction {
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if len > cfg.max_file || u32::try_from(bytes.len()).is_err() {
        let d = SyntaxDiag::new(
            codes::HN_I001,
            Span::whole_file(),
            format!("file skipped: {len} bytes exceed max-file {}", cfg.max_file),
            "raise [scan] max-file, or add the path to [scan] skip",
        );
        return Extraction::skipped(ScanStatus::SkippedTooLarge, d);
    }
    if bytes.iter().take(8192).any(|&b| b == 0) {
        let d = SyntaxDiag::new(
            codes::HN_I001,
            Span::whole_file(),
            "file skipped: contains a NUL byte in the first 8 KB (binary)",
            "add the path to [scan] skip or exclude to silence this notice",
        );
        return Extraction::skipped(ScanStatus::SkippedBinary, d);
    }
    let Ok(src) = std::str::from_utf8(bytes) else {
        let d = SyntaxDiag::new(
            codes::HN_I003,
            Span::whole_file(),
            "file skipped: not valid UTF-8 (never decoded by guess)",
            "re-save the file as UTF-8, or add it to [scan] skip",
        );
        return Extraction::skipped(ScanStatus::SkippedNotUtf8, d);
    };
    let ext_l = ext.trim_start_matches('.').to_ascii_lowercase();
    let body = src.strip_prefix('\u{feff}').unwrap_or(src);
    let has_frontmatter = ext_l == "md" && body.starts_with("+++");
    if !has_frontmatter && !contains_marker_ci(bytes, &cfg.marker) {
        return Extraction::empty(ScanStatus::NoMarker);
    }
    let Some(leaders) = cfg.comments.leaders_for(&ext_l) else {
        let d = SyntaxDiag::new(
            codes::HN_I002,
            Span::whole_file(),
            format!("no comment syntax known for extension `.{ext_l}`; the file was not scanned"),
            format!("add `{ext_l} = [\"//\"]` (or the right leader) under [comments] in the root control file"),
        );
        let mut e = Extraction::empty(ScanStatus::Scanned);
        e.diags.push(d);
        return e;
    };
    if leaders.is_empty() {
        return Extraction::empty(ScanStatus::Scanned); // comment-free host such as JSON (R-SYN-006)
    }
    scan_text(src, &ext_l, cfg, leaders)
}

/// The scanning pipeline (Task 20).
fn scan_text(_src: &str, _ext: &str, _cfg: &ExtractConfig, _leaders: &[Leader]) -> Extraction {
    Extraction::empty(ScanStatus::Scanned)
}
```

Add to `lib.rs`:

```rust
pub use delimiters::{CommentTable, Leader};
pub use directive::{known_keys, render_directive_body, Arg, ArgOp, ArgValue, Carrier, Directive, Extent, Verb};
pub use extract::{extract, ExtractConfig, Extraction, ScanStatus};
pub use frontmatter::{split_frontmatter, Frontmatter};
pub use normalize::lf_bytes;
```

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax extract::` — Expected: PASS (10 tests). Then `cargo clippy -p fairlead-syntax --all-targets -- -D warnings`; the `_`-prefixed parameters of `scan_text` are replaced in Task 20.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): ExtractConfig, Extraction and skip statuses" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 20: The extraction pipeline for code hosts (`extract.rs`, part 2)

**Rules:** R-ARCH-031 (every marker candidate yields a directive or a diagnostic), R-SYN-032 (malformed directive removed with code, caret position, expected form, did-you-mean, hint), R-SYN-034 (fails closed: unknown verb, bad ID, unterminated quote, bad `ovr`/`exp`, unmatched `end`, oversize continuation, doc-comment carrier; warns only: near-miss marker, bind-to-nothing, `sym` absent, directive-count explosion), R-SYN-001..004, R-SYN-002 (`HN-S013`), R-SYN-020 (`sym` text-presence check, `HN-S014`), R-SYN-021, R-SYN-023, R-SYN-031 (`HN-S015`), R-ARCH-033 (original-byte offsets, CRLF/LF equivalence), R-SYN-014 and R-FILE-042 (foreign regions), R-SYN-035 (parsing is read-only: the pipeline performs no I/O).

**Files:**
- Modify: `crates/fairlead-syntax/src/extract.rs` (replace `scan_text`)
- Modify: `crates/fairlead-syntax/src/attach.rs` (add `pub(crate) fn code_text`)

**Interfaces:** Consumes everything above. Produces the real `scan_text`; `candidates` counts marker candidates minus well-formed region-closing `end`s; `dropped` counts every candidate turned into a diagnostic. Output ordering: directives by original start offset with `ordinal` = position; diagnostics by `(line, col, code, message)`.

- [ ] **Step 1: Write the failing test.** Append inside `mod tests` of `extract.rs`:

```rust
    fn ex(src: &str, ext: &str) -> Extraction {
        extract(src.as_bytes(), ext, &cfg())
    }
    fn codes_of(e: &Extraction) -> Vec<&'static str> {
        e.diags.iter().map(|d| d.code.0).collect()
    }
    fn extent(e: &Extraction, i: usize) -> Extent {
        e.directives[i].extent
    }
    fn assert_invariant(e: &Extraction) {
        assert_eq!(e.candidates as usize, e.directives.len() + e.dropped as usize, "R-ARCH-031: {e:?}");
    }

    #[test]
    fn r_arch_031__every_candidate_is_a_directive_or_a_diagnostic() {
        let src = "// fairlead: def cmp.a\nfn a() {}\n// fairlead: frobnicate xx.y\n// fairlead: use bad\n// fairlead: use cmp.a\nfn b() {}\n// fairlead|  orphan\n";
        let e = ex(src, "rs");
        assert_eq!((e.candidates, e.directives.len(), e.dropped), (5, 2, 3));
        assert_eq!(codes_of(&e), vec!["HN-S001", "HN-S002", "HN-S016"]);
        assert_eq!(e.diags.iter().map(|d| d.span.line).collect::<Vec<_>>(), vec![3, 4, 7]);
        assert_invariant(&e);
    }

    #[test]
    fn r_syn_034__fail_closed_set_drops_the_directive_with_an_error() {
        let mut nine = String::from("// fairlead: use tt.b\n");
        for _ in 0..9 {
            nine.push_str("// fairlead| k=v\n");
        }
        nine.push_str("fn f() {}\n");
        let table: Vec<(String, &str)> = vec![
            ("// fairlead: frobnicate tt.b\nfn f() {}\n".into(), "HN-S001"),
            ("// fairlead: use Cmp.x\nfn f() {}\n".into(), "HN-S002"),
            ("// fairlead: use tt.b why=\"abc\nfn f() {}\n".into(), "HN-S007"),
            ("// fairlead: ovr rule.x why=\"w\" exp=tomorrow\nfn f() {}\n".into(), "HN-S012"),
            ("// fairlead: ovr rule.x why=\"w\" until=1.9.0\nfn f() {}\n".into(), "HN-S012"),
            ("fn f() {}\n// fairlead: end\n".into(), "HN-S008"),
            (nine, "HN-S006"),
            ("/// fairlead: use tt.b\nfn f() {}\n".into(), "HN-S013"),
            ("//! fairlead: use tt.b\nfn f() {}\n".into(), "HN-S013"),
            ("/** fairlead: use tt.b */\nfn f() {}\n".into(), "HN-S013"),
            ("// fairlead: use cmp.con\nfn f() {}\n".into(), "HN-P001"),
        ];
        for (src, code) in table {
            let e = ex(&src, "rs");
            assert!(e.directives.is_empty(), "{code}: {e:?}");
            assert_eq!((e.candidates, e.dropped), (1, 1), "{code}");
            assert_eq!(codes_of(&e), vec![code], "{src}");
            assert_eq!(e.diags[0].severity, Severity::Error, "{code}");
            assert!(!e.diags[0].hint.is_empty() && !e.diags[0].message.is_empty());
        }
    }

    #[test]
    fn r_syn_032__diagnostic_carries_position_expected_form_and_suggestion() {
        let e = ex("fn a() {}\n// fairlead: deff cmp.a\nfn b() {}\n", "rs");
        let d = &e.diags[0];
        assert_eq!(d.code, codes::HN_S001);
        assert_eq!((d.span.line, d.span.col), (2, 11), "points at the verb");
        assert_eq!(d.did_you_mean.as_deref(), Some("def"));
        assert_eq!(d.expected.as_deref(), Some("one of: def use ovr flag env end"));
        assert_eq!(&"fn a() {}\n// fairlead: deff cmp.a\nfn b() {}\n"[d.span.start as usize..d.span.end as usize], "deff");
    }

    #[test]
    fn r_syn_034__warn_only_set() {
        let e = ex("// Fairlead: use tt.b\nfn f() {}\n", "rs");
        assert_eq!((e.candidates, e.directives.len()), (0, 0));
        assert_eq!((codes_of(&e), e.diags[0].severity), (vec!["HN-S005"], Severity::Warn));
        assert_eq!(e.diags[0].did_you_mean.as_deref(), Some("fairlead:"));
        let e2 = ex("fn f() {}\n// fairlead: use tt.b\n", "rs");
        assert_eq!((codes_of(&e2), e2.diags[0].severity), (vec!["HN-S010"], Severity::Warn));
        assert_eq!((e2.candidates, e2.dropped), (1, 1), "bind-to-nothing is dropped (R-SYN-021)");
        assert_invariant(&e2);
    }

    #[test]
    fn r_syn_004__escaped_marker_is_silent() {
        let e = ex("// fairlead\\: use tt.b\nfn f() {}\n", "rs");
        assert!(e.diags.is_empty() && e.candidates == 0);
    }

    #[test]
    fn r_file_013__marker_inside_a_string_literal_does_not_register() {
        let e = ex("let s = \"// fairlead: use tt.b\";\nlet r = r#\"// fairlead: use uu.d\"#;\n", "rs");
        assert_eq!((e.status, e.candidates, e.diags.len()), (ScanStatus::Scanned, 0, 0));
    }

    #[test]
    fn r_syn_020__sym_is_a_text_presence_check_inside_the_extent() {
        let ok = ex("x\n\n// fairlead: def cmd.x sym=sync_now\nfn sync_now() {}\n", "rs");
        assert!(ok.diags.is_empty(), "{:?}", ok.diags);
        let missing = ex("x\n\n// fairlead: def cmd.x sym=other_name\nfn sync_now() {}\n", "rs");
        assert_eq!(codes_of(&missing), vec!["HN-S014"]);
        assert_eq!(missing.diags[0].severity, Severity::Warn);
        assert_eq!(missing.directives.len(), 1, "sym never drops or reshapes the directive");
        assert_eq!(extent(&missing, 0), Extent::Lines { first: 4, last: 4 });
    }

    #[test]
    fn r_syn_031__directive_count_explosion_warns_once_and_keeps_everything() {
        let mut c = cfg();
        c.max_directives = 2;
        let src = "x\n\n// fairlead: use tt.b\nA\n// fairlead: use tt.c\nB\n// fairlead: use tt.d\nC\n";
        let e = extract(src.as_bytes(), "rs", &c);
        assert_eq!(e.directives.len(), 3);
        assert_eq!(codes_of(&e), vec!["HN-S015"]);
        assert_eq!(e.diags[0].severity, Severity::Warn);
    }

    #[test]
    fn r_syn_007__ordinals_follow_source_order_and_notes_are_data() {
        let src = "x\n\n// fairlead: use tt.b -- first\nA\n// fairlead: use tt.c\nB\n";
        let e = ex(src, "rs");
        assert_eq!(e.directives.iter().map(|d| d.ordinal).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(e.directives[0].note.as_deref(), Some("first"));
        assert_eq!(e.directives[0].carrier, Carrier::LineComment);
    }

    #[test]
    fn r_syn_023__where_a_directive_binds_never_changes_what_it_says() {
        let file = ex("// fairlead: use cmp.a layer=molecule\n\nfn f() {}\n", "rs");
        let standalone = ex("x\n\n// fairlead: use cmp.a layer=molecule\nfn f() {}\n", "rs");
        let trailing = ex("fn f() {} // fairlead: use cmp.a layer=molecule\n", "rs");
        assert_eq!(extent(&file, 0), Extent::File);
        assert_eq!(extent(&standalone, 0), Extent::Lines { first: 4, last: 4 });
        assert_eq!(extent(&trailing, 0), Extent::Lines { first: 1, last: 1 });
        for e in [&standalone, &trailing] {
            assert_eq!((e.directives[0].verb, &e.directives[0].ids), (file.directives[0].verb, &file.directives[0].ids));
            let strip = |d: &Directive| d.args.iter().map(|a| (a.key.clone(), a.value.clone())).collect::<Vec<_>>();
            assert_eq!(strip(&e.directives[0]), strip(&file.directives[0]));
        }
    }

    #[test]
    fn r_syn_014__foreign_regions_and_env_regions_in_css() {
        let css = "/* fairlead: def tok.primary sym=--primary */\n:root { --primary: #2563eb; }\n.btn { color: var(--primary); } /* fairlead: use tok.primary */\n/* fairlead: env dev +end */\n.debug-grid { outline: 1px solid red; }\n/* fairlead: end */\n/* fairlead: def foreign.shadcn tool=shadcn +end */\n.shadcn-block { color: #111; }\n/* fairlead: end */\n";
        let e = ex(css, "css");
        assert!(e.diags.is_empty(), "{:?}", e.diags);
        assert_eq!((e.candidates, e.directives.len(), e.dropped), (4, 4, 0));
        assert_eq!(extent(&e, 0), Extent::Lines { first: 2, last: 2 });
        assert_eq!(extent(&e, 1), Extent::Lines { first: 3, last: 3 });
        assert_eq!(extent(&e, 2), Extent::Lines { first: 5, last: 5 });
        assert_eq!(e.directives[2].end_span.map(|s| s.line), Some(6));
        assert_eq!(extent(&e, 3), Extent::Lines { first: 8, last: 8 });
        assert_eq!(e.directives[3].ids[0].to_string(), "foreign.shadcn");
        assert_eq!(e.directives[0].carrier, Carrier::BlockComment);
        assert_invariant(&e);
    }

    #[test]
    fn r_arch_033__crlf_and_lf_checkouts_extract_identically() {
        let lf = "// License\n// fairlead: def cmp.a\n\nfn a() {}\n// fairlead: use cmp.a sym=zz\nfn b() {}\n// fairlead: nope\n";
        let crlf = lf.replace('\n', "\r\n");
        let (a, b) = (ex(lf, "rs"), ex(&crlf, "rs"));
        let shape = |e: &Extraction| {
            (
                e.directives.iter().map(|d| (d.ordinal, d.verb, d.ids.clone(), d.extent, d.span.line, d.span.col)).collect::<Vec<_>>(),
                e.diags.iter().map(|d| (d.code, d.span.line, d.span.col, d.message.clone())).collect::<Vec<_>>(),
                (e.candidates, e.dropped),
            )
        };
        assert_eq!(shape(&a), shape(&b));
    }

    #[test]
    fn r_arch_033__offsets_refer_to_the_original_bytes() {
        let src = "// x\r\n// fairlead: def cmp.a\r\n\r\nfn a() {}\r\n";
        let e = ex(src, "rs");
        let d = &e.directives[0];
        assert_eq!(&src.as_bytes()[d.span.start as usize..d.span.start as usize + 6], b"fairlead:");
        assert_eq!(&src[d.span.start as usize..d.span.end as usize], "fairlead: def cmp.a");
        assert_eq!((d.span.line, d.span.col), (2, 4));
        let bom = ex("\u{feff}// fairlead: def cmp.a\n\nfn a() {}\n", "rs");
        assert_eq!(bom.directives[0].span.start, 6, "BOM bytes count in original offsets");
    }

    #[test]
    fn r_syn_016__file_form_through_extract() {
        let e = ex("#!/usr/bin/env python\n# fairlead: use tt.b\n\nimport os\n", "py");
        assert_eq!(extent(&e, 0), Extent::File);
    }

    #[test]
    fn r_syn_006__jsx_comment_form_is_trailing() {
        let e = ex("<Button>\n  {/* fairlead: use tok.surface */}\n  <Icon />\n</Button>\n", "tsx");
        assert_eq!(extent(&e, 0), Extent::Lines { first: 2, last: 2 });
    }

    #[test]
    fn r_syn_044__blanks_around_the_leader_and_marker_are_tolerated() {
        let e = ex("x

	// 	fairlead:	use tok.a	 
fn f() {}
", "rs");
        assert_eq!(e.directives.len(), 1, "{:?}", e.diags);
        assert_eq!(e.directives[0].ids[0].to_string(), "tok.a");
        assert_eq!(extent(&e, 0), Extent::Lines { first: 4, last: 4 });
    }

    #[test]
    fn r_syn_035__extraction_is_pure_and_repeatable() {
        let src = "x

// fairlead: use tok.a
fn f() {}
// fairlead: frobnicate
";
        assert_eq!(ex(src, "rs"), ex(src, "rs"));
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax extract::tests::r_` — Expected: FAIL: the stub `scan_text` returns an empty `Scanned` extraction (assertions such as `left: 0, right: 5` for `candidates`).

- [ ] **Step 3: Implement.** In `attach.rs` add:

```rust
/// Text with comment bytes blanked (strings kept): the haystack of the `sym=` presence check (R-SYN-020).
pub(crate) fn code_text(n: &Normalized, lexed: &Lexed) -> String {
    mask(n, lexed, false)
}
```

In `extract.rs` add the imports and replace the stub `scan_text`:

```rust
use crate::attach::{attach, code_text, Outcome, Site};
use crate::diag::DiagCode;
use crate::directive::{parse_body, ArgValue, BodyError, Carrier, Extent, ParsedBody, Verb};
use crate::lexer::{lex, scan_markers, Candidate, CommentKind};
use crate::normalize::{slice, tabs_to_spaces_outside_quotes, Normalized};

fn hint(code: DiagCode) -> &'static str {
    match code.0 {
        "HN-S001" => "the six verbs are def, use, ovr, flag, env, end (R-SYN-009)",
        "HN-S002" => "IDs are [scope:]kind.name in ASCII lowercase; kinds have 2 to 8 letters",
        "HN-S004" => "frontmatter must be valid TOML and `directives` an array of strings",
        "HN-S005" => "the marker is case-sensitive and `:` must follow it directly (R-SYN-003)",
        "HN-S006" => "shorten the directive: at most 8 continuation lines and 1 KB",
        "HN-S007" => "quote with \"...\"; only \\\" and \\\\ are escapes and a quote may not span lines",
        "HN-S008" => "close every `+end` region with `end`, and write no `end` without one",
        "HN-S010" => "put code after the directive, or give it +N or +end",
        "HN-S012" => "edition 1 uses exp=YYYY-MM-DD; until= needs seals (0.2)",
        "HN-S013" => "use `//` or `/* */` instead of a doc comment so the text never reaches rustdoc or TSDoc",
        "HN-S014" => "fix the sym= spelling or move the directive onto the item",
        "HN-S015" => "split the file or raise [scan] max-directives",
        "HN-S016" => "see `fairlead explain syntax` for the directive forms",
        "HN-P001" => "rename: con prn aux nul com1..9 lpt1..9 and names ending in `.` or space are reserved",
        _ => "see `fairlead explain` for this code",
    }
}

fn plain(n: &Normalized, code: DiagCode, a: usize, b: usize, message: impl Into<String>) -> SyntaxDiag {
    SyntaxDiag::new(code, n.span(a, b), message, hint(code))
}

fn body_diag(n: &Normalized, c: &Candidate, e: &BodyError) -> SyntaxDiag {
    let a = c.norm_offset(e.at);
    let b = (c.norm_offset(e.at.saturating_add(e.len).saturating_sub(1)) + 1).max(a + 1);
    let mut d = plain(n, e.code, a, b, e.message.clone());
    d.expected = e.expected.clone();
    d.did_you_mean = e.did_you_mean.clone();
    d
}

struct Pending {
    body: ParsedBody,
    span: Span,
    carrier: Carrier,
}

fn extent_text<'a>(n: &Normalized, masked: &'a str, extent: Extent) -> Option<&'a str> {
    match extent {
        Extent::File => Some(masked),
        Extent::Doc => None,
        Extent::Lines { first, last } => Some(slice(masked, n.line_start(first), n.line_end(last))),
    }
}

fn scan_text(src: &str, ext: &str, cfg: &ExtractConfig, leaders: &[Leader]) -> Extraction {
    let n = Normalized::new(src);
    let mut ex = Extraction::empty(ScanStatus::Scanned);
    let mut candidates = 0u32;
    let mut dropped = 0u32;
    let fm_end = 0usize; // Task 21 replaces this with the frontmatter handling

    let mut lexed = lex(&n, ext, leaders);
    lexed.comments.retain(|c| c.start >= fm_end);
    let scan = scan_markers(&n, &lexed, &cfg.marker);

    for &off in &scan.near_misses {
        let mut d = plain(&n, codes::HN_S005, off, off + cfg.marker.len(), "comment looks like a directive but the marker does not match");
        d.expected = Some(format!("{}:", cfg.marker));
        d.did_you_mean = Some(format!("{}:", cfg.marker));
        ex.diags.push(d);
    }

    let mut pend: Vec<Pending> = Vec::new();
    let mut sites: Vec<Site> = Vec::new();
    for c in &scan.candidates {
        candidates += 1;
        let line_end = n.line_end(c.first_line);
        let whole = |code: DiagCode, msg: &str| plain(&n, code, c.marker_start, line_end, msg);
        if c.doc {
            ex.diags.push(whole(codes::HN_S013, "directive in a doc comment would leak into rustdoc/TSDoc and is ignored"));
            dropped += 1;
            continue;
        }
        if c.orphan {
            let mut d = whole(codes::HN_S016, "continuation line has no directive to continue");
            d.expected = Some("marker: verb ...".to_owned());
            ex.diags.push(d);
            dropped += 1;
            continue;
        }
        if c.too_long {
            ex.diags.push(whole(codes::HN_S006, "directive exceeds 8 continuation lines or 1 KB"));
            dropped += 1;
            continue;
        }
        let body = tabs_to_spaces_outside_quotes(&c.body);
        match parse_body(&body) {
            Err(e) => {
                ex.diags.push(body_diag(&n, c, &e));
                dropped += 1;
            }
            Ok(mut pb) => {
                for (a, &(s, e)) in pb.args.iter_mut().zip(pb.arg_pos.iter()) {
                    let (x, y) = (c.norm_offset(s), c.norm_offset(e.saturating_sub(1)) + 1);
                    a.span = n.span(x, y);
                }
                let carrier = match lexed.comments.get(c.comment).map(|k| k.kind) {
                    Some(CommentKind::Block) => Carrier::BlockComment,
                    Some(CommentKind::Html) => Carrier::HtmlComment,
                    _ => Carrier::LineComment,
                };
                sites.push(Site { first_line: c.first_line, last_line: c.last_line, inline: c.inline, explicit: pb.explicit, is_end: pb.verb == Verb::End });
                pend.push(Pending { body: pb, span: n.span(c.marker_start, line_end), carrier });
            }
        }
    }

    let outcomes = attach(&n, &lexed, ext, &cfg.header_skip, &sites);
    let masked = code_text(&n, &lexed);
    let spans: Vec<Span> = pend.iter().map(|p| p.span).collect();
    let mut consumed = 0u32;
    for (p, o) in pend.into_iter().zip(outcomes) {
        match o {
            Outcome::Bound { extent, end_site } => {
                let d = Directive {
                    ordinal: 0,
                    verb: p.body.verb,
                    must_understand: p.body.must_understand,
                    ids: p.body.ids,
                    bare: p.body.bare,
                    args: p.body.args,
                    note: p.body.note,
                    carrier: p.carrier,
                    span: p.span,
                    extent,
                    end_span: end_site.and_then(|j| spans.get(j).copied()),
                };
                if matches!(d.verb, Verb::Def | Verb::Use) {
                    if let (Some(a), Some(hay)) = (d.arg("sym"), extent_text(&n, &masked, extent)) {
                        let wanted: Vec<&str> = match &a.value {
                            ArgValue::Word(w) | ArgValue::Quoted(w) => vec![w.as_str()],
                            ArgValue::List(v) => v.iter().map(String::as_str).collect(),
                        };
                        if let Some(missing) = wanted.iter().find(|w| !hay.contains(**w)) {
                            ex.diags.push(SyntaxDiag::new(codes::HN_S014, a.span, format!("sym `{missing}` not found in the directive's extent"), hint(codes::HN_S014)));
                        }
                    }
                }
                ex.directives.push(d);
            }
            Outcome::ClosesRegion => consumed += 1,
            Outcome::NothingToBind => {
                ex.diags.push(SyntaxDiag::new(codes::HN_S010, p.span, "directive binds to nothing and was dropped", hint(codes::HN_S010)));
                dropped += 1;
            }
            Outcome::UnmatchedEnd => {
                ex.diags.push(SyntaxDiag::new(codes::HN_S008, p.span, "`end` without an open `+end` region", hint(codes::HN_S008)));
                dropped += 1;
            }
            Outcome::UnclosedRegion => {
                ex.diags.push(SyntaxDiag::new(codes::HN_S008, p.span, "`+end` region is never closed; the directive was dropped", hint(codes::HN_S008)));
                dropped += 1;
            }
        }
    }

    ex.directives.sort_by_key(|d| d.span.start);
    for (i, d) in ex.directives.iter_mut().enumerate() {
        d.ordinal = u32::try_from(i).unwrap_or(u32::MAX);
    }
    if u32::try_from(ex.directives.len()).unwrap_or(u32::MAX) > cfg.max_directives {
        ex.diags.push(SyntaxDiag::new(
            codes::HN_S015,
            Span::whole_file(),
            format!("{} directives in one file exceed max-directives {}", ex.directives.len(), cfg.max_directives),
            hint(codes::HN_S015),
        ));
    }
    ex.diags.sort_by(|a, b| (a.span.line, a.span.col, a.code.0, &a.message).cmp(&(b.span.line, b.span.col, b.code.0, &b.message)));
    ex.candidates = candidates - consumed;
    ex.dropped = dropped;
    ex
}
```

Trace for the numbers asserted. Test 1: candidates are the five `fairlead` lines (def, frobnicate, `use bad`, `use cmp.a`, orphan); the first `def` sits at line 1 followed directly by code (not file form) so it binds line 2; `use cmp.a` binds line 6. Diagnostics land on lines 3, 4, 7 in that order after sorting. Test `r_syn_032`: line 2 is `// fairlead: deff cmp.a`; `deff` starts at normalized column 11 (`// fairlead: ` is 10 characters), the span covers 4 bytes. Test `r_syn_014`: the `end` directives are consumed, so `candidates = 6 - 2 = 4`; the first `def` binds line 2 (`:root { --primary: #2563eb; }`, whose `;` sits at depth 1, no continuation), its `sym=--primary` is present in the masked extent; the foreign region is lines 8..8. Test CRLF: the shapes compare line/col only; offsets legitimately differ. Test BOM: `\u{feff}` is 3 bytes, `//` at 3, space at 5, so the marker starts at original byte 6 and `d.span.start` is 6.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax extract::` — Expected: PASS (all tests of Tasks 19 and 20). Then `cargo test -p fairlead-syntax` for the whole crate.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): extraction pipeline for comment hosts with diagnostics and invariants" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 21: Markdown frontmatter inside `extract` (`extract.rs`, part 3)

**Rules:** R-SYN-036 (frontmatter `directives` strings are directive bodies), R-SYN-037 (`HN-S004`), R-SYN-022 (frontmatter directives bind to the doc: `Extent::Doc`, `Carrier::Frontmatter`), R-SYN-005 (body comments outside fences), R-SYN-019 (`end` and extents have no meaning in frontmatter), R-ARCH-031 (frontmatter bodies are candidates too), D-32.

**Files:** Modify `crates/fairlead-syntax/src/extract.rs`.

**Interfaces:** For `ext == "md"`, `parse_frontmatter` runs first. `Parsed`: `ex.frontmatter = Some(..)`, each body is a candidate (`candidates += 1`); `Bad`: one `HN-S004` error at the reported position and no candidates; comments starting inside the frontmatter block are discarded (`fm_end`). Frontmatter directives never accept `end`, `+N` or `+end` (they would be unmatched or meaningless): such a body is dropped with `HN-S008` (`end`) or `HN-S016` (extent).

- [ ] **Step 1: Write the failing test.** Append inside `mod tests`:

```rust
    #[test]
    fn r_syn_036__frontmatter_directives_bind_to_the_doc() {
        let src = "+++\ntitle = \"T\"\ndirectives = [\n  'def tok.primary applies=color',\n  'use rule.no-raw-hex',\n  'end',\n  'frobnicate xx.y',\n]\n+++\n\n# Colour\n<!-- fairlead: use tok.primary -- example only -->\nSome text\n";
        let e = ex(src, "md");
        assert_eq!((e.candidates, e.directives.len(), e.dropped), (5, 3, 2));
        assert_eq!(codes_of(&e), vec!["HN-S008", "HN-S001"]);
        let fm = &e.directives[0];
        assert_eq!((fm.carrier, fm.extent, fm.ordinal), (Carrier::Frontmatter, Extent::Doc, 0));
        assert_eq!((fm.span.line, fm.span.col), (4, 4));
        assert_eq!(e.directives[1].ids[0].to_string(), "rule.no-raw-hex");
        let body = &e.directives[2];
        assert_eq!((body.carrier, body.extent, body.note.as_deref()), (Carrier::HtmlComment, Extent::Lines { first: 13, last: 13 }, Some("example only")));
        assert!(e.frontmatter.as_ref().unwrap().table.contains_key("title"));
        assert_eq!(e.frontmatter.as_ref().unwrap().directive_bodies.len(), 4);
        assert_invariant(&e);
    }

    #[test]
    fn r_syn_037__bad_frontmatter_is_s004_and_loses_no_body_comment() {
        let src = "+++\ndirectives = 5\n+++\n\nText\n<!-- fairlead: use tt.b -->\nMore\n";
        let e = ex(src, "md");
        assert_eq!(codes_of(&e), vec!["HN-S004"]);
        assert_eq!(e.diags[0].severity, Severity::Error);
        assert!(e.frontmatter.is_none());
        assert_eq!(e.directives.len(), 1, "the body comment still extracts");
        let e2 = ex("+++\ntitle = \"x\"\n", "md");
        assert_eq!(codes_of(&e2), vec!["HN-S004"], "unterminated frontmatter");
    }

    #[test]
    fn r_syn_036__frontmatter_without_a_marker_anywhere_is_still_scanned() {
        let e = ex("+++\ndirectives = ['use tt.b']\n+++\n# T\n", "md");
        assert_eq!((e.status, e.directives.len()), (ScanStatus::Scanned, 1));
    }

    #[test]
    fn r_syn_019__extents_are_meaningless_in_frontmatter() {
        let e = ex("+++\ndirectives = ['use tt.b +3', 'env dev +end']\n+++\n", "md");
        assert_eq!((e.candidates, e.directives.len(), e.dropped), (2, 0, 2));
        assert_eq!(codes_of(&e), vec!["HN-S016", "HN-S016"]);
    }

    #[test]
    fn r_syn_005__markdown_fenced_comments_do_not_register_through_extract() {
        let src = "# T\n\n```md\n<!-- fairlead: use no.fence -->\n```\n\n<!-- fairlead: use yes.one -->\ntext\n";
        let e = ex(src, "md");
        assert_eq!(e.directives.len(), 1);
        assert_eq!(e.directives[0].ids[0].to_string(), "yes.one");
        assert_eq!(extent(&e, 0), Extent::Lines { first: 8, last: 8 });
    }

    #[test]
    fn r_syn_005__comments_inside_frontmatter_strings_are_not_body_comments() {
        let src = "+++\ntitle = \"<!-- fairlead: use no.fm -->\"\n+++\n\ntext\n";
        let e = ex(src, "md");
        assert!(e.directives.is_empty() && e.candidates == 0 && e.diags.is_empty());
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax extract::tests::r_syn_036 r_syn_037` (run `r_syn_036`, `r_syn_037`, `r_syn_019__extents`, `r_syn_005` separately) — Expected: FAIL (`candidates` is 1 instead of 5; no frontmatter handling).

- [ ] **Step 3: Implement.** In `extract.rs` add imports `use crate::frontmatter::{parse_frontmatter, FmOutcome};` and replace the line `let fm_end = 0usize; // Task 21 ...` in `scan_text` with:

```rust
    let mut fm_end = 0usize;
    if ext == "md" {
        match parse_frontmatter(&n) {
            FmOutcome::Absent => {}
            FmOutcome::Bad(e) => ex.diags.push(plain(&n, codes::HN_S004, e.at, e.at + e.len, e.message)),
            FmOutcome::Parsed(p) => {
                fm_end = p.end;
                for (body_text, &off) in p.fm.directive_bodies.iter().zip(p.offsets.iter()) {
                    candidates += 1;
                    let end = off + body_text.len();
                    match parse_body(&tabs_to_spaces_outside_quotes(body_text)) {
                        Err(e) => {
                            let d = SyntaxDiag::new(e.code, n.span(off + e.at, off + e.at + e.len), e.message.clone(), hint(e.code));
                            ex.diags.push(SyntaxDiag { expected: e.expected.clone(), did_you_mean: e.did_you_mean.clone(), ..d });
                            dropped += 1;
                        }
                        Ok(pb) if pb.verb == Verb::End => {
                            ex.diags.push(plain(&n, codes::HN_S008, off, end, "`end` has no region to close in frontmatter"));
                            dropped += 1;
                        }
                        Ok(pb) if pb.explicit.is_some() => {
                            let d = plain(&n, codes::HN_S016, off, end, "frontmatter directives bind to the doc and take no +N or +end");
                            ex.diags.push(SyntaxDiag { expected: Some("verb ID [k=v..]".to_owned()), ..d });
                            dropped += 1;
                        }
                        Ok(mut pb) => {
                            for (a, &(s, e)) in pb.args.iter_mut().zip(pb.arg_pos.iter()) {
                                a.span = n.span(off + s, off + e);
                            }
                            ex.directives.push(Directive {
                                ordinal: 0,
                                verb: pb.verb,
                                must_understand: pb.must_understand,
                                ids: pb.ids,
                                bare: pb.bare,
                                args: pb.args,
                                note: pb.note,
                                carrier: Carrier::Frontmatter,
                                span: n.span(off, end),
                                extent: Extent::Doc,
                                end_span: None,
                            });
                        }
                    }
                }
                ex.frontmatter = Some(p.fm);
            }
        }
    }
```

`ex.diags.push(plain(..., e.message))` moves `e.message` out of `e`; that is fine because `e` is owned. The later `ex.directives.sort_by_key(span.start)` places frontmatter directives (smaller offsets) before body comments, so ordinals run in source order.

Trace of the first test: lines are 1 `+++`, 2 `title`, 3 `directives = [`, 4 `  'def tok.primary applies=color',`, 5 `use rule.no-raw-hex`, 6 `'end'`, 7 `'frobnicate xx.y'`, 8 `]`, 9 `+++`, 10 blank, 11 `# Colour`, 12 the comment, 13 `Some text`. `offsets[0]` points at the `d` of `def` (column 4: two spaces and a quote precede it). The comment binds the paragraph at line 13. Diagnostics sort by line: `end` (line 6, `HN-S008`) then `frobnicate` (line 7, `HN-S001`). Candidates: four frontmatter bodies plus one comment.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax extract::` — Expected: PASS. Then run `cargo test -p fairlead-syntax` and `cargo clippy -p fairlead-syntax --all-targets -- -D warnings`.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax
git commit -m "feat(syntax): Markdown frontmatter directives inside extract" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

### Task 22: Golden conformance fixtures and runner (`tests/conformance.rs`, `fixtures/syntax/`)

**Rules:** R-SYN-046 (the attachment rules R-SYN-016..019, 045, 046 are confirmed by a conformance suite of golden fixtures per host language; a fixture that disagrees with the text is a spec bug), R-SYN-044 (per-host placement table), R-FILE-013 (strings and raw strings), R-FILE-042 (foreign region bracket), R-ARCH-033 (every fixture also runs as a CRLF checkout with identical results), contract Section 7 (syntax conformance row).

**Files:**
- Create: `crates/fairlead-syntax/tests/conformance.rs`
- Create: `fixtures/syntax/<host>/<case>.<ext>` and `fixtures/syntax/<host>/<case>.expect` (list below)

**Interfaces:** Consumes the public API only: `fairlead_syntax::{extract, ExtractConfig, Extraction, Extent, ScanStatus}`. Produces the `.expect` format: one line per surviving directive in ordinal order, `<verb> <ids and bare words joined by ,, or -> <extent>` with extent `file`, `doc` or `L<first>-<last>`; then one line per diagnostic `<code> <line>:<col>`; a first line `status <Name>` only when the status is not `Scanned`. Blank lines and lines starting `#` in a `.expect` file are comments. `fixtures/** -text` in `.gitattributes` (P1) keeps the bytes of the fixtures untouched by `core.autocrlf`.

The runner is read-only on `fixtures/`; to change an expectation, edit the `.expect` file by hand (the failure message prints the actual rendering).

- [ ] **Step 1: Write the failing test.** Create `crates/fairlead-syntax/tests/conformance.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
//! Golden conformance suite (R-SYN-046). Reads `fixtures/syntax/<host>/<case>.<ext>` + `<case>.expect`.
use fairlead_syntax::{extract, Extent, ExtractConfig, Extraction, ScanStatus};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_HOSTS: [&str; 11] =
    ["css", "html", "javascript", "json", "markdown", "rust", "shell", "sql", "toml", "typescript", "yaml"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/syntax")
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    v.sort();
    v
}

fn render(ex: &Extraction) -> String {
    let mut out = String::new();
    if ex.status != ScanStatus::Scanned {
        out.push_str(&format!("status {:?}\n", ex.status));
    }
    for d in &ex.directives {
        let mut names: Vec<String> = d.ids.iter().map(|i| i.to_string()).collect();
        names.extend(d.bare.iter().cloned());
        let names = if names.is_empty() { "-".to_owned() } else { names.join(",") };
        let extent = match d.extent {
            Extent::File => "file".to_owned(),
            Extent::Doc => "doc".to_owned(),
            Extent::Lines { first, last } => format!("L{first}-{last}"),
        };
        out.push_str(&format!("{} {} {}\n", serde_verb(d.verb), names, extent));
    }
    for d in &ex.diags {
        out.push_str(&format!("{} {}:{}\n", d.code.0, d.span.line, d.span.col));
    }
    out
}

fn serde_verb(v: fairlead_syntax::Verb) -> &'static str {
    v.as_str()
}

fn expected(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap();
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| format!("{l}\n"))
        .collect()
}

#[test]
fn r_syn_046__every_required_host_has_fixtures() {
    for h in REQUIRED_HOSTS {
        let dir = root().join(h);
        assert!(dir.is_dir(), "missing fixture directory {}", dir.display());
        let cases = sorted_entries(&dir).iter().filter(|p| p.extension().is_some_and(|e| e != "expect")).count();
        assert!(cases > 0, "no fixtures for host {h}");
    }
}

#[test]
fn r_syn_046__conformance_fixtures_match_expectations() {
    let cfg = ExtractConfig::default();
    let mut failures = Vec::new();
    let mut hosts = BTreeSet::new();
    let mut count = 0;
    for host_dir in sorted_entries(&root()).into_iter().filter(|p| p.is_dir()) {
        let host = host_dir.file_name().unwrap().to_string_lossy().into_owned();
        for case in sorted_entries(&host_dir) {
            let ext = case.extension().unwrap().to_string_lossy().into_owned();
            if ext == "expect" {
                continue;
            }
            let want = expected(&case.with_extension("expect"));
            let bytes = fs::read(&case).unwrap();
            let got = render(&extract(&bytes, &ext, &cfg));
            let crlf = String::from_utf8(bytes.clone()).unwrap().replace('\n', "\r\n");
            let got_crlf = render(&extract(crlf.as_bytes(), &ext, &cfg));
            if got != want {
                failures.push(format!("{host}/{}: LF mismatch\n--- expected\n{want}--- actual\n{got}", case.file_name().unwrap().to_string_lossy()));
            }
            if got_crlf != want {
                failures.push(format!("{host}/{}: CRLF checkout differs\n--- actual\n{got_crlf}", case.file_name().unwrap().to_string_lossy()));
            }
            hosts.insert(host.clone());
            count += 1;
        }
    }
    assert!(failures.is_empty(), "{} of {count} fixtures failed:\n{}", failures.len(), failures.join("\n"));
    assert!(count >= 20, "suite shrank: only {count} fixtures");
    assert!(REQUIRED_HOSTS.iter().all(|h| hosts.contains(*h)));
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax --test conformance` — Expected: FAIL: `missing fixture directory .../fixtures/syntax/css` (the suite has no fixtures yet).

- [ ] **Step 3: Create the fixtures.** Every file below ends with exactly one trailing `\n`. Paths are repository-relative. Each `.expect` shows its content under the input.

`fixtures/syntax/rust/file_form_banner.rs`:

````text
// Copyright 2026 Example
// fairlead: def eng.sync-core

use std::fmt;
````

`fixtures/syntax/rust/file_form_banner.expect`: `def eng.sync-core file`

`fixtures/syntax/rust/standalone_signature.rs` (attribute skipping, multi-line signature, R-SYN-045 and R-SYN-046):

````text
use std::io;

// fairlead: def cmd.sync-now sym=sync_now
#[tauri::command]
pub async fn sync_now(
    id: u32,
) -> Result<(), Error>
{
    run();
}

// fairlead: ovr rule.no-unwrap why="startup invariant" owner=@platform exp=2026-12-31
let cfg = load().unwrap();
````

`.expect` (`standalone_signature.expect`):

````text
def cmd.sync-now L4-10
ovr rule.no-unwrap L13-13
````

`fixtures/syntax/rust/trailing_and_stack.rs`:

````text
fn pre() {}

// fairlead: use eng.sync-verify
// fairlead: use eng.sync-purify -- stacked
pub fn run() {
    work(); // fairlead: use tok.accent
}
````

`.expect`:

````text
use eng.sync-verify L5-7
use eng.sync-purify L5-7
use tok.accent L6-6
````

`fixtures/syntax/rust/strings_hide_markers.rs` (R-FILE-013):

````text
let a = "// fairlead: use no.string";
let b = r#"// fairlead: use no.raw"#;
let c = '"'; // fairlead: use tok.real
````

`.expect`: `use tok.real L3-3`

`fixtures/syntax/rust/doc_comment_carrier.rs` (R-SYN-002):

````text
/// fairlead: use tok.leak
pub fn documented() {}
````

`.expect`: `HN-S013 1:5`

`fixtures/syntax/rust/unmatched_and_binds_nothing.rs`:

````text
fn a() {}
// fairlead: end
fn b() {}
// fairlead: use tok.last
````

`.expect`:

````text
HN-S008 2:4
HN-S010 4:4
````

`fixtures/syntax/rust/near_miss.rs` (R-SYN-003):

````text
// Fairlead: use tok.nearmiss
// fairlead: use tok.ok
fn f() {}
````

`.expect`:

````text
use tok.ok L3-3
HN-S005 1:4
````

`fixtures/syntax/typescript/jsx_children.tsx` (R-SYN-006 JSX form, `"use client"` header, file form):

````text
"use client";
// fairlead: def cmp.sync-button sym=SyncButton layer=molecule
// fairlead: use tok.primary cmp.button

export function SyncButton() {
  return (
    <Button>
      {/* fairlead: use tok.surface */}
      <Icon />
    </Button>
  );
}
````

`.expect`:

````text
def cmp.sync-button file
use tok.primary,cmp.button file
use tok.surface L8-8
````

`fixtures/syntax/javascript/strict_header.js`:

````text
'use strict';
/* fairlead: def lib.util */

module.exports = {};
````

`.expect`: `def lib.util file`

`fixtures/syntax/javascript/template_literal.js` (R-FILE-013, multi-line template):

````text
const t = `// fairlead: use no.template
// fairlead: use no.second`;
// fairlead: use lib.real
go();
````

`.expect`: `use lib.real L4-4`

`fixtures/syntax/css/regions.css` (chapter 12 CSS example: sym, trailing, env region, foreign region):

````text
/* fairlead: def tok.primary sym=--primary */
:root { --primary: #2563eb; }
.btn { color: var(--primary); } /* fairlead: use tok.primary */
/* fairlead: env dev +end */
.debug-grid { outline: 1px solid red; }
/* fairlead: end */
/* fairlead: def foreign.shadcn tool=shadcn +end */
.shadcn-block { color: #111; }
/* fairlead: end */
````

`.expect`:

````text
def tok.primary L2-2
use tok.primary L3-3
env dev L5-5
def foreign.shadcn L8-8
````

`fixtures/syntax/css/string_hides_comment.css`:

````text
a::after { content: "/* fairlead: use no.css */"; }
/* fairlead: use tok.accent */
b { color: red; }
````

`.expect`: `use tok.accent L3-3`

`fixtures/syntax/markdown/frontmatter_and_body.md` (R-SYN-036, R-SYN-022, R-SYN-005):

`````text
+++
title = "Design tokens"
directives = [
  'def tok.primary applies=color,background-color min-len=4',
  'use rule.no-raw-hex',
]
+++

# Colour

<!-- fairlead: use tok.primary -- example only -->
Primary text goes here.

```md
<!-- fairlead: use no.fence -->
```
`````

`.expect`:

````text
def tok.primary doc
use rule.no-raw-hex doc
use tok.primary L12-12
````

`fixtures/syntax/markdown/heading_section.md`:

````text
<!-- fairlead: use tok.primary -->
## Section
text

### Sub
more

## Next
x
````

`.expect`: `use tok.primary L2-6`

`fixtures/syntax/toml/cargo.toml` (file form needs the blank line):

````text
# fairlead: def crate.sync-core

[package]
name = "sync-core"

[dependencies]
# fairlead: use eng.sync-verify
sync-verify = { path = "../sync-verify" }
````

`.expect`:

````text
def crate.sync-core file
use eng.sync-verify L8-8
````

`fixtures/syntax/shell/script.sh` (shebang header, `#` only at a word start):

````text
#!/bin/sh
# fairlead: use tool.build

echo ${#PATH} a#b
# fairlead: def fn.build
build() {
  echo hi
}
````

`.expect`:

````text
use tool.build file
def fn.build L6-8
````

`fixtures/syntax/yaml/compose.yaml` (indented block):

````text
version: 1

# fairlead: use tok.accent
services:
  web:
    image: nginx

other: 2
````

`.expect`: `use tok.accent L4-6`

`fixtures/syntax/html/comment_in_attribute.html` (R-SYN-044):

````text
<a title="<!-- fairlead: use no.attr -->" href="x">link</a>
<!-- fairlead: use tok.link -->
<p>text</p>
````

`.expect`: `use tok.link L3-3`

`fixtures/syntax/sql/dash_comments.sql`:

````text
-- fairlead: def tbl.users

select '-- fairlead: use no.sql' as s; -- fairlead: use tbl.users
````

`.expect`:

````text
def tbl.users file
use tbl.users L3-3
````

`fixtures/syntax/json/no_comments.json` (R-SYN-006: JSON carries no directives in 0.1):

````text
{"note": "// fairlead: use no.json"}
````

`.expect`: `# json carries no directives in edition 1 (R-SYN-006)` (a comment line only; the expected rendering is empty).

That is 20 fixtures across 11 host directories (the runner asserts at least 20).

Hand-check of the two non-obvious ones. `standalone_signature.rs`: line 3 is the directive, line 4 the attribute (kept in the extent), line 5 `pub async fn sync_now(` opens a parenthesis, line 7 `) -> Result<(), Error>` closes at depth 0 with no `;`, line 8 is `{` so the extent continues through line 10 `}`: `L4-10`; the `sym` name occurs in line 5. `frontmatter_and_body.md`: lines 1-7 are the frontmatter (the two bodies are at lines 4-5), line 11 holds the comment, line 12 the paragraph, line 13 is blank so the extent is `L12-12`; the commented line inside the fence (line 15) is never lexed.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax --test conformance` — Expected: PASS (2 tests). A mismatch prints expected and actual renderings; per R-SYN-046 decide whether the fixture or the implementation is wrong and justify the choice in the commit message.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax/tests fixtures/syntax
git commit -m "test(syntax): golden conformance fixtures for eleven hosts" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 23: Property tests (`tests/prop_extract.rs`)

**Rules:** R-ARCH-032 (never panics, any bytes), R-ARCH-031 (candidates equal directives plus dropped), R-ARCH-033 (CRLF and LF equivalence; offsets map to original bytes), R-ARCH-150 table (parse, render, parse is stable). These make the API fuzz-ready; the `cargo-fuzz` targets and the CI job belong to P9.

**Files:**
- Create: `crates/fairlead-syntax/tests/prop_extract.rs`

**Interfaces:** Public API only. `proptest` is a dev-dependency (Task 1 manifest).

- [ ] **Step 1: Write the failing test.** Create `crates/fairlead-syntax/tests/prop_extract.rs` with the generators and properties:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
//! Property tests for `fairlead_syntax::extract` (R-ARCH-031/032/033, R-ARCH-150).
use fairlead_syntax::{extract, render_directive_body, Carrier, ExtractConfig, Extraction};
use proptest::prelude::*;

const FRAGMENTS: &[&str] = &[
    "// fairlead: use cmp.a",
    "// fairlead: def cmp.b sym=run layer=molecule",
    "// fairlead: ovr rule.x why=\"w\" owner=@a exp=2026-12-31 kw+=a,b",
    "// fairlead: ovr rule.x why=\"w\" until=1.9.0",
    "// fairlead: flag deprecate cmp.a target=2.0.0 -- note \u{e9}",
    "// fairlead: env dev,staging",
    "// fairlead: frobnicate x.y",
    "// fairlead: use Bad.Id",
    "// fairlead: use cmp.a why=\"unterminated",
    "// fairlead: def foreign.t tool=t +end",
    "// fairlead: use cmp.c +3",
    "/* fairlead: env dev +end */",
    "/* fairlead: end */",
    "// fairlead: end",
    "// fairlead| k=v",
    "// fairlead|  why=\"more\"",
    "/// fairlead: use doc.leak",
    "//! fairlead: use doc.leak",
    "// Fairlead: use near.miss",
    "// fairlead\\: escaped",
    "let s = \"// fairlead: use in.string\";",
    "let r = r#\"// fairlead: use in.raw\"#;",
    "let c = '\"'; // fairlead: use tok.trail",
    "#[attr]",
    "fn f() {",
    "    body();",
    "}",
    "fn g(",
    ") -> T",
    "",
    "",
    "\t",
    "/*",
    " * fairlead: use cmp.block",
    " */",
    "'",
    "\"",
    "`",
    "r#\"",
];

fn doc_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(FRAGMENTS.to_vec()), 0..40).prop_map(|v| v.join("\n") + "\n")
}

const EXTS: &[&str] = &["rs", "ts", "tsx", "md", "py", "css", "html", "sh", "toml", "json", "zzz", ""];

fn shape(e: &Extraction) -> String {
    let mut s = format!("{:?} {} {}\n", e.status, e.candidates, e.dropped);
    for d in &e.directives {
        s.push_str(&format!("{} {} {:?} {}:{} {:?}\n", d.ordinal, render_directive_body(d), d.extent, d.span.line, d.span.col, d.carrier));
    }
    for d in &e.diags {
        s.push_str(&format!("{} {:?} {}:{} {}\n", d.code.0, d.severity, d.span.line, d.span.col, d.message));
    }
    s
}

fn line_of(bytes: &[u8], off: usize) -> u32 {
    let (mut line, mut i) = (1u32, 0usize);
    while i < off {
        match bytes[i] {
            b'\n' => line += 1,
            b'\r' => {
                line += 1;
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    line
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_extract_never_panics_and_accounts_for_every_candidate(
        bytes in prop::collection::vec(any::<u8>(), 0..1500),
        ext in prop::sample::select(EXTS.to_vec()),
    ) {
        let e = extract(&bytes, ext, &ExtractConfig::default());
        prop_assert_eq!(e.candidates as usize, e.directives.len() + e.dropped as usize);
    }

    #[test]
    fn prop_structured_documents_account_for_every_candidate(
        doc in doc_strategy(),
        ext in prop::sample::select(EXTS.to_vec()),
    ) {
        let e = extract(doc.as_bytes(), ext, &ExtractConfig::default());
        prop_assert_eq!(e.candidates as usize, e.directives.len() + e.dropped as usize);
        let ords: Vec<u32> = e.directives.iter().map(|d| d.ordinal).collect();
        prop_assert_eq!(ords, (0..e.directives.len() as u32).collect::<Vec<_>>());
    }

    #[test]
    fn prop_crlf_and_lf_checkouts_are_equivalent(doc in doc_strategy(), ext in prop::sample::select(EXTS.to_vec())) {
        let lf = extract(doc.as_bytes(), ext, &ExtractConfig::default());
        let crlf_text = doc.replace('\n', "\r\n");
        let crlf = extract(crlf_text.as_bytes(), ext, &ExtractConfig::default());
        prop_assert_eq!(shape(&lf), shape(&crlf));
        let cr_text = doc.replace('\n', "\r");
        let cr = extract(cr_text.as_bytes(), ext, &ExtractConfig::default());
        prop_assert_eq!(shape(&lf), shape(&cr));
    }

    #[test]
    fn prop_offsets_map_to_original_bytes(doc in doc_strategy()) {
        let crlf = doc.replace('\n', "\r\n");
        let bytes = crlf.as_bytes();
        let e = extract(bytes, "rs", &ExtractConfig::default());
        for d in &e.directives {
            let (s, t) = (d.span.start as usize, d.span.end as usize);
            prop_assert!(s <= t && t <= bytes.len());
            prop_assert_eq!(d.span.line, line_of(bytes, s));
            if d.carrier != Carrier::Frontmatter {
                prop_assert!(bytes[s..].starts_with(b"fairlead:"), "directive span must start at the marker");
            }
        }
        for g in &e.diags {
            prop_assert!(g.span.start <= g.span.end && g.span.end as usize <= bytes.len());
            if g.span.line > 0 {
                prop_assert_eq!(g.span.line, line_of(bytes, g.span.start as usize));
            }
        }
    }

    #[test]
    fn prop_render_then_parse_is_stable(doc in doc_strategy()) {
        let e = extract(doc.as_bytes(), "rs", &ExtractConfig::default());
        for d in &e.directives {
            let body = render_directive_body(d);
            let tail = if d.end_span.is_some() { "// fairlead: end\n" } else { "" };
            let again = format!("x\n\n// fairlead: {body}\nfn x() {{}}\n{tail}");
            let e2 = extract(again.as_bytes(), "rs", &ExtractConfig::default());
            prop_assert_eq!(e2.directives.len(), 1, "{} => {:?}", body, e2.diags);
            prop_assert_eq!(render_directive_body(&e2.directives[0]), body);
        }
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-syntax --test prop_extract` — Expected: PASS on a complete implementation. This task adds tests only; there is no production code to write, so a failure is a real defect in an earlier task. Without Tasks 19 to 21 the file fails to compile (`unresolved import fairlead_syntax::extract`), which is the red state of this task.

- [ ] **Step 3: Fix what the properties find.** On a failing property, proptest prints a shrunk counter-example and writes a seed under `crates/fairlead-syntax/proptest-regressions/` (commit that directory). Fix the earlier module, and add the shrunk input as a conformance fixture in Task 22 first (R-ARCH-152). Known sharp edges: (a) `prop_offsets_map_to_original_bytes` requires `Directive.span.start` to be the marker's original offset, so it uses extension `rs` and CRLF text; (b) `prop_render_then_parse_is_stable` appends `// fairlead: end` after the item for directives that carry `end_span`, otherwise the re-extraction reports an unclosed region; (c) a note with trailing blanks renders trimmed and parses back identically.

- [ ] **Step 4: Run** `cargo test -p fairlead-syntax` (whole crate), `cargo clippy -p fairlead-syntax --all-targets -- -D warnings`, `cargo fmt --all --check`. Expected: all PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/fairlead-syntax/tests/prop_extract.rs
git commit -m "test(syntax): property tests for totality, CRLF equivalence, offsets and render stability" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Traceability

Every P2 rule of contract Section 6 maps to a task and at least one test function (names are the tests' real names; `mod` is the module holding them). Conformance fixtures (Task 22) add cross-checks for R-SYN-002, 003, 006, 016, 017, 018, 019, 022, 036, 044, 045, 046, R-FILE-013, R-FILE-042.

| Rule | Task | Test functions |
|---|---|---|
| R-SYN-001 | 7, 8 | `r_syn_001__start_and_continuation_markers`, `r_syn_001__ordinary_comments_are_not_markers`, `r_syn_001__non_ascii_marker_does_not_panic`, `r_syn_001__marker_in_a_string_is_not_a_candidate` |
| R-SYN-002 | 5, 8, 20 | `r_syn_002__doc_comment_leaders_are_flagged`, `r_syn_002__doc_and_inline_flags_reach_the_candidate`, `r_syn_034__fail_closed_set_drops_the_directive_with_an_error` (S013 rows), fixture `rust/doc_comment_carrier` |
| R-SYN-003 | 7, 8, 19, 20 | `r_syn_003__near_misses_are_reported_not_parsed`, `r_syn_003__near_misses_and_escapes_are_not_candidates`, `r_syn_003__prefilter_is_case_insensitive_so_near_misses_reach_the_scanner`, `r_syn_034__warn_only_set`, fixture `rust/near_miss` |
| R-SYN-004 | 7, 8, 20 | `r_syn_004__backslash_colon_is_silently_ignored`, `r_syn_004__escaped_marker_is_silent` |
| R-SYN-005 | 6, 21 | `r_syn_005__markdown_comment_outside_fences_only`, `r_syn_005__markdown_indented_code_and_inline_code_are_not_scanned`, `r_syn_005__indented_text_directly_after_a_paragraph_is_not_code`, `r_syn_005__multi_line_html_comment_yields_one_line_per_source_line`, `r_syn_005__markdown_fenced_comments_do_not_register_through_extract`, `r_syn_005__comments_inside_frontmatter_strings_are_not_body_comments` |
| R-SYN-006 | 4, 5, 19, 20 | `r_syn_006__builtin_table_matches_the_spec`, `r_syn_006__json_has_no_leaders_and_unknown_is_none`, `r_syn_006__overrides_add_and_replace`, `r_syn_006__from_spec_reads_control_file_strings`, `r_syn_006__custom_leader_from_overrides`, `r_syn_006__unknown_extension_yields_one_i002_only_when_the_marker_is_present`, `r_syn_006__json_carries_no_directives`, `r_syn_006__fingerprint_is_stable_and_sensitive_to_every_field`, `r_syn_006__jsx_comment_form_is_trailing` |
| R-SYN-007 | 2, 3, 9, 10 | `r_syn_007__id_forms_bare_root_and_scoped`, `r_syn_007__id_is_ascii_lowercase_only`, `r_syn_007__id_shape_errors`, `r_syn_007__id_length_limits`, `r_syn_007__ord_is_bytewise_on_display`, `r_syn_007__parse_never_panics_on_junk`, `r_syn_007__bom_is_stripped_and_offsets_stay_original`, `r_syn_007__nfc_composes_and_ascii_is_untouched`, `r_syn_007__tabs_outside_quotes_become_one_space`, `r_syn_007__pairs_words_extent_and_note`, `r_syn_007__value_forms_word_quoted_list`, `r_syn_007__stray_characters_are_s016`, `r_syn_007__non_ascii_only_inside_quotes_and_note`, `r_syn_007__quoted_and_note_are_nfc_normalised`, `r_syn_007__parse_body_is_total`, `r_syn_007__def_takes_exactly_one_id`, `r_syn_007__use_takes_one_or_more_ids`, `r_syn_007__bad_id_in_the_id_slot_is_s002_with_position`, `r_syn_007__ordinals_follow_source_order_and_notes_are_data` |
| R-SYN-008 | 9, 13 | `r_syn_008__note_is_data_and_never_parsed`, `r_syn_008__directive_arg_lookup` |
| R-SYN-009 | 9, 10, 12 | `r_syn_009__unknown_verb_is_s001_with_suggestion`, `r_syn_009__bang_sits_directly_before_the_verb`, `r_syn_009__extension_keys_and_bang_parse_like_other_keys`, `r_syn_009__env_lists_names` |
| R-SYN-012 | 11 | `r_syn_012__exp_must_be_a_real_iso_date`, `r_syn_012__until_is_rejected_in_edition_one`, `r_syn_012__iso_date_helper` |
| R-SYN-014 | 10, 17, 20 | `r_syn_014__def_foreign_with_plus_end_opens_a_region`, `r_syn_019__foreign_region_in_css`, `r_syn_014__foreign_regions_and_env_regions_in_css` |
| R-SYN-015 | 13 | `r_syn_015__known_keys_per_verb` |
| R-SYN-016 | 15, 18, 20 | `r_syn_016__file_form_after_license_banner`, `r_syn_016__skips_bom_shebang_use_client_and_ts_check`, `r_syn_016__header_skip_patterns_use_star_wildcards`, `r_syn_016__eof_also_ends_the_block_and_directives_stack`, `r_syn_016__block_directly_followed_by_code_is_not_file_form`, `r_syn_016__later_block_is_not_file_form`, `r_syn_016__markdown_has_no_file_form`, `r_syn_016__file_form_through_extract` |
| R-SYN-017 | 5, 16 | `r_syn_017__inline_flag_marks_comments_sharing_a_line_with_code`, `r_syn_017__trailing_directive_binds_to_its_own_line` |
| R-SYN-018 | 16 | `r_syn_018__standalone_one_liner_and_blank_lines`, `r_syn_018__unbalanced_opener_runs_to_the_matching_close`, `r_syn_018__colon_or_deeper_indent_takes_the_indented_block`, `r_syn_018__consecutive_directives_stack_on_one_code_line`, `r_syn_018__extent_is_capped_at_500_lines` |
| R-SYN-019 | 12, 17, 21 | `r_syn_019__end_takes_nothing_but_a_note`, `r_syn_019__plus_n_sets_the_extent_to_n_lines`, `r_syn_019__plus_end_region_closes_at_matching_end`, `r_syn_019__regions_nest_and_close_innermost_first`, `r_syn_019__unmatched_end_and_unclosed_region_are_flagged`, `r_syn_019__a_directive_never_becomes_a_region_by_lookahead`, `r_syn_019__empty_region_is_the_openers_last_line`, `r_syn_019__extents_are_meaningless_in_frontmatter` |
| R-SYN-020 | 20 | `r_syn_020__sym_is_a_text_presence_check_inside_the_extent` |
| R-SYN-021 | 17, 20 | `r_syn_021__directive_with_nothing_after_it_binds_to_nothing`, `r_syn_034__warn_only_set` |
| R-SYN-022 | 18, 21 | `r_syn_022__comment_before_a_paragraph_binds_the_block`, `r_syn_022__comment_before_a_code_fence_binds_the_fence`, `r_syn_022__comment_before_a_heading_binds_the_section`, `r_syn_022__inline_comment_binds_its_line_and_no_target_binds_nothing`, `r_syn_036__frontmatter_directives_bind_to_the_doc` |
| R-SYN-023 | 16, 20 | `r_syn_023__brackets_in_strings_and_comments_are_ignored`, `r_syn_023__where_a_directive_binds_never_changes_what_it_says` |
| R-SYN-027 | 2, 10 | `r_syn_027__windows_reserved_names`, `r_syn_027__reserved_scope_and_name_segments_are_rejected`, `r_syn_027__reserved_windows_name_in_an_id_is_hn_p001` |
| R-SYN-028 | 8 | `r_syn_028__continuation_lines_join_with_one_space`, `r_syn_028__more_than_eight_continuations_is_too_long`, `r_syn_028__body_over_one_kilobyte_is_too_long`, `r_syn_028__a_gap_ends_the_continuation_and_the_stray_line_is_an_orphan`, `r_syn_028__block_comment_lines_can_continue` |
| R-SYN-029 | 9 | `r_syn_029__quote_escapes_are_only_quote_and_backslash`, `r_syn_029__unterminated_quote_and_unknown_escape_are_s007` |
| R-SYN-030 | 5, 19 | `r_syn_030__block_comment_lines_strip_leading_star`, `r_syn_030__unterminated_block_runs_to_eof`, `r_syn_030__default_limits_and_marker`, `r_syn_030__too_large_is_skipped_with_i001`, `r_syn_030__nul_in_the_first_8k_means_binary` |
| R-SYN-031 | 20 | `r_syn_031__directive_count_explosion_warns_once_and_keeps_everything` |
| R-SYN-032 | 1, 20 | `r_syn_032__registry_has_each_syntax_code_once_and_never_s003`, `r_syn_032__severities_match_table_13`, `r_syn_032__did_you_mean_finds_close_verbs_only`, `r_syn_032__diag_code_deserializes_through_the_registry`, `r_syn_032__new_diag_takes_severity_from_registry`, `r_syn_032__diagnostic_carries_position_expected_form_and_suggestion` |
| R-SYN-034 | 20 | `r_syn_034__fail_closed_set_drops_the_directive_with_an_error`, `r_syn_034__warn_only_set` |
| R-SYN-035 | 20 | `r_syn_035__extraction_is_pure_and_repeatable` (read-only by construction: the crate has no I/O API) |
| R-SYN-036 | 14, 21 | `r_syn_036__splitter_finds_both_fences`, `r_syn_036__only_a_leading_fence_line_counts`, `r_syn_036__directives_array_of_strings`, `r_syn_036__multi_line_bodies_join_with_one_space`, `r_syn_036__no_directives_key_is_fine_and_no_fence_is_absent`, `r_syn_036__crlf_files_split_after_normalisation`, `r_syn_036__frontmatter_directives_bind_to_the_doc`, `r_syn_036__frontmatter_without_a_marker_anywhere_is_still_scanned` |
| R-SYN-037 | 14, 21 | `r_syn_037__invalid_toml_is_bad_with_a_position_inside_the_block`, `r_syn_037__directives_must_be_an_array_of_strings`, `r_syn_037__unterminated_frontmatter_is_bad`, `r_syn_037__bad_frontmatter_is_s004_and_loses_no_body_comment` |
| R-SYN-044 | 4, 6, 20 | `r_syn_044__lookup_ignores_leading_dot_and_case`, `r_syn_044__html_attribute_strings_do_not_hold_comments`, `r_syn_044__xml_and_svg_use_the_same_comment_syntax`, `r_syn_044__blanks_around_the_leader_and_marker_are_tolerated` |
| R-SYN-045 | 16 | `r_syn_045__attribute_lines_are_skipped_and_included`, fixture `rust/standalone_signature` |
| R-SYN-046 | 16, 22 | `r_syn_046__multi_line_signature_binds_with_its_body`, `r_syn_046__every_required_host_has_fixtures`, `r_syn_046__conformance_fixtures_match_expectations` |
| R-SYN-048 | 12 | `r_syn_048__flag_word_then_optional_id`, `r_syn_048__flag_shape_errors`, `r_syn_048__until_on_flag_is_parsed_and_left_to_resolve` |
| R-FILE-013 | 5, 20, 22 | `r_file_013__double_quoted_string_hides_comment_marker`, `r_file_013__rust_raw_strings_hide_comments`, `r_file_013__rust_lifetimes_and_char_literals_are_not_strings`, `r_file_013__js_quotes_and_templates`, `r_file_013__unterminated_single_line_string_ends_at_newline`, `r_file_013__hash_hosts_python_shell_toml`, `r_file_013__css_and_sql_hosts`, `r_file_013__marker_inside_a_string_literal_does_not_register` |
| R-FILE-015 | 19 | `r_file_015__non_utf8_is_skipped_with_i003_and_never_decoded` |
| R-FILE-042 | 10, 20, 22 | `r_file_042__foreign_region_bracket_parses_like_any_def`, `r_syn_014__foreign_regions_and_env_regions_in_css`, fixture `css/regions` |
| R-ARCH-031 | 20, 23 | `r_arch_031__every_candidate_is_a_directive_or_a_diagnostic`, `prop_extract_never_panics_and_accounts_for_every_candidate`, `prop_structured_documents_account_for_every_candidate` |
| R-ARCH-032 | 3, 5, 19, 23 | `r_arch_032__slice_never_panics`, `r_arch_032__lexer_is_total_on_hostile_input`, `r_arch_032__any_bytes_are_accepted`, `prop_extract_never_panics_and_accounts_for_every_candidate` |
| R-ARCH-033 | 3, 20, 22, 23 | `r_arch_033__crlf_and_lone_cr_become_lf_with_original_offsets`, `r_arch_033__span_columns_count_scalar_values`, `r_arch_033__line_helpers`, `r_arch_033__lf_bytes_strips_bom_and_cr`, `r_arch_033__crlf_and_lf_checkouts_extract_identically`, `r_arch_033__offsets_refer_to_the_original_bytes`, `prop_crlf_and_lf_checkouts_are_equivalent`, `prop_offsets_map_to_original_bytes`, conformance CRLF pass |

Cross-checks outside the P2 rule list that this phase also supports: R-ARCH-150 (`r_arch_150__render_then_parse_is_stable`, `r_arch_150__render_is_canonical`, `prop_render_then_parse_is_stable`), R-CLI-031 constant (`r_cli_031__placeholder_constant`), R-SYN-024 lexical shape (kind 2 to 8 letters, in `r_syn_007__id_shape_errors`).

---

## Contract issues

Ambiguities and additions met while planning P2. None of them renames anything in contract Section 4; where the contract is silent the choice below is the narrowest one, and each item should be confirmed (or turned into a contract amendment) before P3 starts consuming the types.

1. **`HN_P001` in `fairlead-syntax`.** Section 4.1 says the syntax constants are "HN-S*/HN-I*" but Section 4.9 says P2 raises `HN-P001` for reserved names in IDs (R-SYN-027). The plan adds `codes::HN_P001` to `fairlead-syntax` (Task 1); `fairlead-core::diag::codes` must re-export it, not define a second constant.
2. **No code for "malformed directive shape" or "lexically bad ID".** R-SYN-034 says a bad ID and other malformed bodies fail closed, but chapter 13 allocates no code for them (`HN-S002` is "unknown kind", `HN-S016` is "incomplete `ovr`"). The plan uses `HN-S002` for a lexically invalid ID or kind (message distinguishes it from an undeclared kind) and `HN-S016` for any other malformed body: missing verb or ID, wrong arity, stray characters, orphan `marker|` line, `end` with arguments, an extent in frontmatter. P4 also raises both codes for their semantic meanings. Recommend a one-line amendment to chapter 13.
3. **`DiagCode` cannot derive `Deserialize`.** It wraps `&'static str`, but `SyntaxDiag` and `Extraction` are cached (Section 4). The plan implements `Serialize`/`Deserialize` by hand, deserialising through the registry; an unknown code in a cache file is a deserialisation error, which P3's cache treats as a miss (R-ARCH-072).
4. **`Eq` on `Frontmatter` and `Extraction`.** Section 4 says every type derives `Eq` unless stated, but `toml::Table` holds floats, so `Frontmatter`, `Extraction` and `ParsedFrontmatter` derive `PartialEq` only. Tests compare with `PartialEq`.
5. **Meaning of `candidates`.** The invariant `candidates == directives.len() + dropped` (R-ARCH-031) cannot hold if a well-formed `end` that closes a `+end` region counts as a candidate, because it is folded into the opener's `end_span` and is not a `Directive`. The plan does not count such `end`s. Consequently `Verb::End` never appears in `Extraction.directives`.
6. **`render_directive_body` loses `+N`.** `Directive` has no field for the explicit extent word, so `+N` cannot be rendered; `+end` is rendered iff `end_span` is set. Parse-render-parse stability holds for the rendered text (verified by `prop_render_then_parse_is_stable`), but the original extent is re-derived. Adding `explicit: Option<..>` to `Directive` would remove the gap.
7. **`Extent::Doc` for headings.** Section 4.2 says "`Doc` = frontmatter or heading binding". R-SYN-022 says a comment before a heading binds the heading's section. The plan returns `Doc` for frontmatter and for a comment before a level-1 heading (the doc title), and `Lines { first, last }` (the section) for deeper headings, so no line information is lost for sections.
8. **R-SYN-018/045 interpretation.** The text does not say whether blank lines or ordinary comment lines may separate a standalone directive from its code line. The plan skips both, and `Extent.first` starts at the first non-blank comment or attribute line directly above the code line (a blank line resets it; lines holding directives are ignored). The conformance fixtures pin this down; if the spec author disagrees the fixture is the place to change (R-SYN-046).
9. **R-SYN-016 header list.** The plan skips `<?php` for every host and, for `html/xml/svg`, `<?xml ...?>` and `<!doctype` lines, which the spec does not list but an XML or SVG file cannot otherwise have its first comment treated as the file block. `header-skip` patterns are matched with a `*` wildcard against the trimmed line; the spec says only "pattern".
10. **R-SYN-006 JSX warning.** "A warning fires when the previous line starts `<` or ends `>`" for a bare `//` between JSX children has no allocated code, so P2 does not emit it. Needs a code in chapter 13 (or deletion of the sentence).
11. **Config fingerprint.** `fairlead-syntax` lists no hashing crate (Section 2), so `config_fingerprint` is a four-lane FNV-1a over `canonical_bytes()` (additive method). Recommended: `fairlead-core` builds its cache key from `blake3(canonical_bytes())` and treats `config_fingerprint` as a convenience.
12. **`impl Default for ExtractConfig`** is added (Section 4.2 gives the defaults only as comments).
13. **`marker:` without a space.** The grammar requires `1*SP` after `:`; the plan accepts `// fairlead:def a.b` leniently (the directive still parses). Strictness would turn a harmless typo into a dropped directive.
14. **Property test location.** Section 7 names `crates/fairlead-core/tests/prop_*.rs` for property tests; the syntax properties live in `crates/fairlead-syntax/tests/prop_extract.rs` because `fairlead-syntax` must not depend on `fairlead-core`. `proptest` is a dev-dependency of `fairlead-syntax`.
15. **Column of line 1 with a BOM.** C-16 says columns count scalar values of the original text; a leading BOM is not counted (columns on line 1 start after it). Offsets do include the BOM bytes.
16. **Files over 4 GiB.** Offsets are `u32` (Section 4), so a file larger than `u32::MAX` bytes is reported `SkippedTooLarge` even if `max_file` is larger.
17. **Ownership of `crates/fairlead-syntax/Cargo.toml`.** P1 owns workspace files; the plan creates the crate manifest in Task 1 and says to merge if P1 already did. `fairlead/control.toml` dogfood declarations for the crate (R-ARCH-162) belong to P9.
18. **`known_keys` contents.** Edition 1 `def` keys include `tool` (needed by `def foreign.<name> tool=`, R-SYN-014, although R-SYN-015 does not list it); `classes` (0.2) and the 0.3 keys (`exposure`, `role`, `dyn`, `pin`, ...) are excluded, so they warn as unknown in this build, as R-SYN-039's principle says for control keys.
19. **`.expect` format.** Section 7 says only "terse list of `verb ids extent` lines and diag codes"; the exact format is defined in Task 22.
20. **Unverified dependencies.** `unicode-normalization` (assumed 0.1.x), `memchr` (2.x) and `proptest` (1.x) are UNVERIFIED per Section 2; winnow 1.0 names (`ModalResult`, `take_while`, literal-as-parser `.value()`), `toml` 1.1 (`from_str::<Table>`, `Error::span/message`) carry `Verify:` notes. The executing engineer records the verified versions in the 50-architecture Section 11 table (R-ARCH-120).

---

## Execution handoff

**Order.** Execute Tasks 1 to 23 strictly in order; each task's tests and commit are self-contained, and no later task edits an earlier task's tests. Tasks 5 to 8 share `lexer.rs`, Tasks 9 to 13 share `directive.rs`, Tasks 15 to 18 share `attach.rs`, Tasks 19 to 21 share `extract.rs`; every one of these appends tests and replaces only the stub or function the task names.

**Verification before declaring P2 done** (run from the repository root, nothing here is run by this plan):
- `cargo test -p fairlead-syntax --locked` (all unit tests, `tests/conformance.rs`, `tests/prop_extract.rs`).
- `cargo clippy -p fairlead-syntax --all-targets --locked -- -D warnings` and `cargo fmt --all --check`. Library code must show no `unwrap`, `expect`, `panic`, indexing or `HashMap`/`HashSet` (grep the `src/` tree outside `#[cfg(test)]` as a final check: `rg -n "unwrap\(|expect\(|panic!|HashMap|HashSet|println!" crates/fairlead-syntax/src` should only hit test modules).
- MSRV: `cargo +1.88 test -p fairlead-syntax --locked` (R-ARCH-113 job). `Option::is_none_or` (1.82), `std::iter::repeat_n` (1.82) and let-else (1.65) are the newest features used; no let-chains are used.
- Fuzz-readiness: `extract(&[u8], &str, &ExtractConfig) -> Extraction` is total and pure, `parse_body` (crate-internal) is total, `split_frontmatter` is total, `lex` is total. P9's fuzz targets `directive`, `frontmatter` and `lexer` can call `fairlead_syntax::extract` with extensions `rs`, `md` and `py` and check `candidates == directives.len() + dropped` and `render_directive_body` stability; if P9 needs the lower-level entry points (`parse_body`, `lex`, `parse_frontmatter`) it should ask for them to be re-exported (they are `pub` in their modules today).

**What the other phases consume from P2** (Section 8 seam P2 to P3): `Extraction`, `Directive`, `ExtractConfig`, `CommentTable` (with `Leader::from_spec` for `[comments]`), `Frontmatter`, `SyntaxDiag`, `DiagCode`, `Severity`, `Span`, `Id`, `ScopeSlug`, `Kind`, `Name`, `PLACEHOLDER`, and `lf_bytes` for `fairlead-core::hash`. P3's `extract_file` builds `ExtractConfig { marker, comments, header_skip, max_file, max_directives }` from the root control file and must skip scanning for reference, vendor-owned and external-generated classes before calling `extract` (R-FILE-004/005/009). P4 raises `HN-S002` (undeclared kind), `HN-S009`, `HN-S011` (using `known_keys` plus `is_extension_key`) and `HN-S016` (`ovr` without `why`/`owner`/`exp`) from the `Directive`s this crate produces.

**Open decisions to resolve with the contract owner before or during execution:** items 2, 5, 6, 7 and 11 under Contract issues change observable behaviour of P3/P4 consumers; the rest are local to P2.

**Estimated size:** about 3,800 lines of Rust including tests, 20 fixtures, 23 commits.
