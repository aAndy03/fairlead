# P3 Core Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Run `cargo` only as `cargo test -p fairlead-core <name>`, `cargo check -p fairlead-core --all-targets` and the final workspace gate; no test may spawn `cargo` or `rustc`.

**Goal:** Deliver the foundation modules of `crates/fairlead-core` exactly as named by the 0.1 contract: `error`, `outcome`, `diag`, `time`, `hash`, `path`, `discover`, `walk`, `classify`, `extract` (mod + styles), `manifest`, `git`, `facts`, `config` (lenient `[fairlead]` header pass, typed schema, rules, approvals) and `cache`. At the end of P3 a caller can run discover, walk, `load_controls`, `extract_files` and `collect_facts` over a real directory and receive the values that P4 (registry), P5 (check), P6 (context) and P7 (write layer) consume.

**Architecture:** P3 is the I/O and parsing floor of the pipeline `discover -> walk -> load_controls -> classify+extract -> collect_facts` (contract Section 5, seams S1 to S5). Every module is a small file with a narrow public surface; pure functions (`pin_check`, `search_upward`, `parse_control`, `parse_approvals`, `classify`, `extract_file`, the styles extractor) sit next to thin I/O wrappers (`discover`, `walk`, `load_controls`, `extract_files`, `collect_facts`). All collections that reach output are `BTreeMap`/`BTreeSet`/sorted `Vec`. Errors are typed (`Error`), never `process::exit`. The cache is JSON under `.fairlead/`, written atomically, and every failure degrades to uncached operation.

**Tech Stack:** Rust 2024, MSRV 1.88; `fairlead-syntax` (P2), `ignore` 0.4.33, `globset` 0.4.20, `regex`, `blake3` 1.8.7, `rayon` 1.12.0, `git2` 0.21.0 (no network features), `toml` 1.1.7, `serde`, `serde_json`, `jiff` 0.2.38, `camino`, `dunce`, `tempfile`, `semver`, `thiserror`, `unicode-normalization`. No other crate (contract Section 2).

**Spec:** [contract](01-contract.md) (law: Sections 3, 4.3 to 4.7, 5, 7, 8, 9) | [decisions](../../decisions.md) | [conventions](../../conventions.md) | [architecture](../../50-architecture.md) Sections 4.1 to 4.3, 6, 7, 8, 9, 10, 13 | [files and classes](../../11-files-and-classes.md) | [directive syntax and control file](../../12-directive-syntax.md) | [scopes](../../10-scopes-and-inheritance.md) | [CLI](../../15-cli.md) R-CLI-004/005/010 | [git](../../23-git-integration.md) R-GIT-007, R-GIT-070 to R-GIT-078.

## Global Constraints

These apply to every task; a task that must break one stops and reports instead.

- **MSRV 1.88.** Edition 2024. Use only language and std features stable in 1.88; no `if let` chains.
- **`#![forbid(unsafe_code)]`** in `fairlead-core` (set in `lib.rs`, Task 1). No `unsafe` anywhere, including tests.
- **No `HashMap`/`HashSet` in output paths.** Output-reaching collections are `BTreeMap`, `BTreeSet` or explicitly sorted `Vec` (R-ARCH-060). `clippy.toml` forbids the hash types in core; add no local `#[allow]`.
- **LF-only.** Every string this phase renders into a file or a diagnostic uses `\n`; every content hash is over `lf_bytes` (BOM stripped, CR/CRLF to LF) except raw hashing of non-UTF-8 files (R-PORT-016).
- **Determinism.** Same tree, config, binary and `AsOf` give the same bytes: sorted walk (R-ARCH-024), sorted maps, parallel results gathered then sorted, identical records with a cold, warm or deleted cache (R-ARCH-071) and with any thread count (R-ARCH-080).
- **Read-only readers.** Nothing in P3 writes outside `<root>/.fairlead/` (the cache). No P3 function commits, stages, pushes or writes git state (R-CLI-010, R-ARCH-140). Tests write only inside `tempfile` directories.
- **Clock only in `time.rs`.** `jiff::Timestamp::now` / `SystemTime::now` appear only in `AsOf::today_utc`. File mtimes read from metadata are data, not clock reads.
- **Env only in `discover.rs`.** `std::env::var*` appears only in `discover.rs` (`read_env_root`). Every other function takes explicit values.
- No `unwrap`, `expect`, `panic!`, slice indexing, `println!`/`eprintln!` in non-test code (workspace lints). Tests may use `unwrap`/`expect`.
- Test function names start with the snake-case rule ID (`r_arch_024__...`, contract Section 7). A test that only supports a contract name uses the nearest P3 rule ID.
- Commit after every task, ending the message with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## File structure (what P3 creates)

```text
crates/fairlead-core/
  Cargo.toml
  src/lib.rs   src/testutil.rs (cfg(test))
  src/hash.rs  path.rs  diag.rs  outcome.rs  error.rs  time.rs
  src/discover.rs  walk.rs  classify.rs  manifest.rs  git.rs  facts.rs
  src/config/{mod.rs,header.rs,schema.rs,rules.rs,approvals.rs}
  src/extract/{mod.rs,styles.rs}
  src/cache/{mod.rs,files.rs,key.rs}
  tests/foundation_pipeline.rs
```

`extract/literals.rs`, `secrets.rs`, `registry/*`, `check/*` belong to other phases. Where P3 must call P5 code (`scan_literals`, `scan_secrets`) Task 21 states the compile-time fallback.

## Execution order and prerequisites

P1 (workspace, lints, CI) and P2's contract-signature skeleton (every `pub` item of contract 4.1 and 4.2 compiles, bodies may be `todo!()`) must exist before Task 1. Run the tasks in the order below; each task compiles and passes on its own. Where a later task extends a file created earlier, the step says "append" or shows the exact edit.

## P2 API as consumed

P3 uses exactly these `fairlead-syntax` items, at these paths. If P2's module layout differs, change only the `use` paths in P3; never rename a contract item (contract Section 4.1 and 4.2 are law). Items marked ADDITION are not named by the contract and are requested from P2 (Contract issues, item 5).

| Path | Items |
|---|---|
| `fairlead_syntax::normalize` | `lf_bytes(&[u8]) -> Cow<'_, [u8]>` |
| `fairlead_syntax::ids` | `Id` (`parse`, `kind_name`, `Display`, public `qualifier`), `IdQualifier`, `ScopeSlug` (`parse`, `as_str`), `IdError` (`Debug`), `is_reserved_windows_name` |
| `fairlead_syntax::diag` | `DiagCode`, `Severity`, `Span`, `SyntaxDiag`, `codes::*` (every `HN_S*`, `HN_I001`, `HN_I003`) |
| `fairlead_syntax::directive` | `ArgValue`, `Extent`, `Verb`, `Directive` |
| `fairlead_syntax::delimiters` | `CommentTable` (`builtin`, `with_overrides`), `Leader::parse(&str) -> Result<Leader, impl Debug>` (ADDITION) |
| `fairlead_syntax::extract` | `extract(&[u8], &str, &ExtractConfig) -> Extraction`, `ExtractConfig` (public fields, `config_fingerprint`), `Extraction` (`Serialize + Deserialize + Clone + Eq + Debug`), `ScanStatus` |

Behaviour P3 relies on: `extract` emits `HN-I003` (warning) for a non-UTF-8 file with status `SkippedNotUtf8`; a `def foreign.<name> +end` region carries `Extent::Lines { first, last }`; `Extraction` round-trips through `serde_json` unchanged.

---

### Task 1: fairlead-core manifest, lib.rs, test utilities

**Files:**
- Create or modify: `crates/fairlead-core/Cargo.toml`
- Create or modify: `crates/fairlead-core/src/lib.rs`
- Create: `crates/fairlead-core/src/testutil.rs`

**Interfaces:**
- Consumes: `[workspace.package]` and `[workspace.lints]` from P1.
- Produces: the crate shell; `testutil::{utf8_tmp, write_file, write_bytes}` used by all filesystem tests.

- [ ] **Step 1: Write `Cargo.toml`.** If P1 already created the file, keep P1's `[package]` and merge every dependency below; record the UNVERIFIED crates in `50-architecture.md` Section 11 in this same commit (R-ARCH-120).

```toml
[package]
name = "fairlead-core"
description = "Core engine of fairlead: discovery, walk, extraction, registry, checks, context, writes"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[features]
default = ["vendored-libgit2"]
# R-GIT-077: never require a system libgit2.
vendored-libgit2 = ["git2/vendored-libgit2"]

[dependencies]
fairlead-syntax = { path = "../fairlead-syntax" }
ignore = "0.4.33"
globset = "0.4.20"
regex = "1"                      # UNVERIFIED version: confirm MSRV 1.88 and licence (C-2)
blake3 = "1.8.7"
rayon = "1.12.0"
git2 = { version = "0.21.0", default-features = false }   # no https/ssh: no network stack (R-ARCH-123)
toml = { version = "1.1.7", default-features = false, features = ["std", "parse", "display", "serde"] }   # UNVERIFIED feature names
toml_edit = "0.25.16"            # used by P7; declared once so the lockfile is stable
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
jiff = { version = "0.2.38", default-features = false, features = ["std"] }   # UNVERIFIED feature names
similar = "3.2.0"                # used by P5
camino = "1.2.6"
dunce = "1.0.5"
tempfile = "3.27.0"
semver = "1.0.28"
thiserror = "2"                  # UNVERIFIED (C-2)
unicode-normalization = "0.1"    # UNVERIFIED (C-2): RepoPath::fold_key and control-file path hygiene
```

Verify: if the `toml` feature names differ in 1.1.7, the intent is `std` + `FromStr for Table` + `Display` + serde. If `jiff` needs other features for `Timestamp::now` and `civil::Date` parsing, keep `std` and add only what compiles.

- [ ] **Step 2: Write `src/lib.rs`.**

```rust
#![forbid(unsafe_code)]
//! Core library of fairlead. P3 owns the foundation modules declared below; later phases append
//! their own `pub mod` lines. Module list and signatures: plans/0.1/01-contract.md Sections 3 and 4.

#[cfg(test)]
pub(crate) mod testutil;
```

- [ ] **Step 3: Write `src/testutil.rs`.**

```rust
//! Test-only helpers. Filesystem tests use `tempfile` directories only.
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;

/// A temporary directory and its UTF-8 path. Keep the guard alive for the whole test.
pub fn utf8_tmp() -> (tempfile::TempDir, Utf8PathBuf) {
    let td = tempfile::tempdir().expect("create tempdir");
    let p = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).expect("utf-8 temp path");
    (td, p)
}

/// Write `text` to `dir/rel`, creating parent directories.
pub fn write_file(dir: &Utf8Path, rel: &str, text: &str) {
    write_bytes(dir, rel, text.as_bytes());
}

/// Write raw bytes to `dir/rel`, creating parent directories.
pub fn write_bytes(dir: &Utf8Path, rel: &str, bytes: &[u8]) {
    let full = dir.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(full.as_std_path(), bytes).expect("write file");
}
```

- [ ] **Step 4: Run** `cargo check -p fairlead-core --all-targets`. Expected: PASS (an empty crate with dependencies resolved). If `fairlead-syntax` cannot be found, stop: P2's skeleton is a prerequisite.

- [ ] **Step 5: Commit** `feat(core): fairlead-core manifest, lib shell and test utilities`.

---

### Task 2: hash.rs: blake3 hashing, Crockford base32, fingerprints

Rules: R-ARCH-076, R-FILE-024, R-PORT-016, R-CHK-031 (types only; the fingerprint algorithm is P5), C-7, C-17.

**Files:**
- Create: `crates/fairlead-core/src/hash.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (add `pub mod hash;`)

**Interfaces:**
- Consumes: `fairlead_syntax::normalize::lf_bytes(&[u8]) -> Cow<'_, [u8]>` (contract 4.2 note). Verify the module path; if P2 re-exports it at the crate root, use that.
- Produces (contract 4.3, names fixed): `ContentHash(pub [u8; 32])`, `hash_normalised`, `hash_raw`, `Fingerprint`, `Digest`, `BodyHash`, `base32`. Additions (not renames): `ContentHash::{to_hex, from_hex}`, `hash_content`, `combine_hashes`, `Fingerprint::{from_hash, parse, as_str}`, `Digest::{from_hash, as_str}`, `BodyHash::{from_hash, as_str}`, `HashParseError`.

- [ ] **Step 1: Write the failing tests.** Create `hash.rs` containing only this test module.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_chk_031__base32_known_vectors() {
        // 10101 01011 => 'n' (21) 'b' (11); lowercase Crockford alphabet (C-7).
        assert_eq!(base32(&[0b1010_1010, 0b1100_0000], 2), "nb");
        assert_eq!(base32(&[0xff, 0xff], 3), "zzw");
        assert_eq!(base32(&[0x00], 1), "0");
        assert_eq!(base32(&[], 4), "0000", "missing bits pad with zeros");
        assert_eq!(base32(&[0xff; 32], 12), "zzzzzzzzzzzz");
    }

    #[test]
    fn r_chk_031__fingerprint_digest_bodyhash_lengths() {
        let h = hash_raw(b"abc");
        assert_eq!(Fingerprint::from_hash(&h).as_str().len(), 12);
        assert_eq!(Digest::from_hash(&h).as_str().len(), 8);
        assert_eq!(BodyHash::from_hash(&h).as_str().len(), 20);
        // The short forms share one bit stream, so each is a prefix of the longer ones.
        assert!(BodyHash::from_hash(&h).as_str().starts_with(Fingerprint::from_hash(&h).as_str()));
        assert!(Fingerprint::from_hash(&h).as_str().starts_with(Digest::from_hash(&h).as_str()));
    }

    #[test]
    fn r_chk_031__fingerprint_parse_validates_alphabet_and_length() {
        assert!(Fingerprint::parse("0123456789ab").is_ok());
        assert_eq!(Fingerprint::parse("0123456789a"), Err(HashParseError::Length { want: 12, got: 11 }));
        // 'i' is not in the Crockford alphabet; uppercase is outside the lowercase alphabet.
        assert_eq!(Fingerprint::parse("0123456789ai"), Err(HashParseError::BadChar { at: 11 }));
        assert_eq!(Fingerprint::parse("0123456789AB"), Err(HashParseError::BadChar { at: 10 }));
    }

    #[test]
    fn r_arch_076__crlf_lf_and_bom_hash_equal() {
        let lf = hash_normalised(b"a\nb\n");
        assert_eq!(lf, hash_normalised(b"a\r\nb\r\n"));
        assert_eq!(lf, hash_normalised(b"a\rb\r"));
        assert_eq!(lf, hash_normalised(b"\xEF\xBB\xBFa\nb\n"));
        assert_ne!(lf, hash_raw(b"a\r\nb\r\n"));
    }

    #[test]
    fn r_file_024__known_blake3_vector_and_hex_roundtrip() {
        let h = hash_raw(b""); // blake3 of the empty input
        assert_eq!(h.to_hex(), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262");
        assert_eq!(ContentHash::from_hex(&h.to_hex()), Ok(h));
        assert_eq!(ContentHash::from_hex("zz"), Err(HashParseError::Length { want: 64, got: 2 }));
    }

    #[test]
    fn r_port_016__non_utf8_is_hashed_over_raw_bytes() {
        let latin = b"caf\xE9\r\n"; // not UTF-8
        assert_eq!(hash_content(latin), hash_raw(latin));
        let utf8 = b"cafe\r\n";
        assert_eq!(hash_content(utf8), hash_normalised(utf8));
    }

    #[test]
    fn r_chk_031__content_hash_serde_is_hex_string() {
        let h = hash_raw(b"x");
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, format!("\"{}\"", h.to_hex()));
        assert_eq!(serde_json::from_str::<ContentHash>(&json).unwrap(), h);
    }

    #[test]
    fn r_arch_060__combine_hashes_is_order_sensitive_and_deterministic() {
        let a = hash_raw(b"a");
        let b = hash_raw(b"b");
        assert_eq!(combine_hashes(&[a, b]), combine_hashes(&[a, b]));
        assert_ne!(combine_hashes(&[a, b]), combine_hashes(&[b, a]));
    }
}
```

- [ ] **Step 2: Add `pub mod hash;` to `lib.rs`. Run** `cargo test -p fairlead-core r_chk_031`. Expected: FAIL (compile error: `cannot find function base32` / `Fingerprint` in this scope).

- [ ] **Step 3: Write the implementation** (insert above the test module).

```rust
//! blake3 everywhere (D-50). Base32 is the lowercase Crockford alphabet, hand-implemented (C-7).
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

fn sym(index: u32) -> char {
    usize::try_from(index)
        .ok()
        .and_then(|i| ALPHABET.get(i))
        .copied()
        .map(char::from)
        .unwrap_or('0')
}

fn in_alphabet(c: char) -> bool {
    c.is_ascii() && ALPHABET.contains(&(c as u8))
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HashParseError {
    Length { want: usize, got: usize },
    BadChar { at: usize },
}

impl fmt::Display for HashParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HashParseError::Length { want, got } => write!(f, "expected {want} characters, got {got}"),
            HashParseError::BadChar { at } => write!(f, "invalid character at position {at}"),
        }
    }
}

impl std::error::Error for HashParseError {}

/// 32-byte blake3 digest. Serialises as 64 lowercase hex characters.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
            s.push(char::from_digit(u32::from(b & 15), 16).unwrap_or('0'));
        }
        s
    }

    pub fn from_hex(s: &str) -> Result<Self, HashParseError> {
        if s.len() != 64 {
            return Err(HashParseError::Length { want: 64, got: s.len() });
        }
        let mut out = [0u8; 32];
        for (i, slot) in out.iter_mut().enumerate() {
            let pair = s.get(i * 2..i * 2 + 2).ok_or(HashParseError::BadChar { at: i * 2 })?;
            *slot = u8::from_str_radix(pair, 16).map_err(|_| HashParseError::BadChar { at: i * 2 })?;
        }
        Ok(ContentHash(out))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        ContentHash::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// Hash after BOM strip and CRLF/CR to LF, so `core.autocrlf` never changes a hash (R-ARCH-076).
pub fn hash_normalised(bytes: &[u8]) -> ContentHash {
    let lf = fairlead_syntax::normalize::lf_bytes(bytes);
    ContentHash(*blake3::hash(&lf).as_bytes())
}

/// Hash of the bytes as they are. For non-UTF-8 files (R-PORT-016).
pub fn hash_raw(bytes: &[u8]) -> ContentHash {
    ContentHash(*blake3::hash(bytes).as_bytes())
}

/// Normalised hash for UTF-8 text, raw hash otherwise. The one entry point for file content.
pub fn hash_content(bytes: &[u8]) -> ContentHash {
    if std::str::from_utf8(bytes).is_ok() {
        hash_normalised(bytes)
    } else {
        hash_raw(bytes)
    }
}

/// Order-sensitive combination of hashes (config hashes, cache keys).
pub fn combine_hashes(parts: &[ContentHash]) -> ContentHash {
    let mut h = blake3::Hasher::new();
    for p in parts {
        h.update(&p.0);
    }
    ContentHash(*h.finalize().as_bytes())
}

/// First `chars` symbols of the lowercase Crockford base32 of `bytes`, most significant bit first.
/// Missing input bits are zero-padded, so the result always has exactly `chars` symbols.
pub fn base32(bytes: &[u8], chars: usize) -> String {
    let mut out = String::with_capacity(chars);
    let mut acc: u32 = 0;
    let mut nbits: u32 = 0;
    let mut iter = bytes.iter();
    for _ in 0..chars {
        while nbits < 5 {
            match iter.next() {
                Some(b) => {
                    acc = (acc << 8) | u32::from(*b);
                    nbits += 8;
                }
                None => {
                    acc <<= 5;
                    nbits += 5;
                }
            }
        }
        nbits -= 5;
        out.push(sym((acc >> nbits) & 31));
        acc &= (1u32 << nbits) - 1;
    }
    out
}

macro_rules! short_id {
    ($(#[$m:meta])* $name:ident, $len:expr) => {
        $(#[$m])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub const LEN: usize = $len;
            pub fn from_hash(h: &ContentHash) -> Self {
                Self(base32(&h.0, Self::LEN))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

short_id!(
    /// 12 characters = first 60 bits (R-CHK-031); also the approval key (R-CLI-037).
    Fingerprint, 12
);
short_id!(
    /// 8 characters = first 40 bits (context digest, R-CTX-020).
    Digest, 8
);
short_id!(
    /// 20 characters = first 100 bits; generated-file header body hash (C-17).
    BodyHash, 20
);

impl Fingerprint {
    /// Parse the 12-character text form (approvals, baseline lines).
    pub fn parse(s: &str) -> Result<Self, HashParseError> {
        let n = s.chars().count();
        if n != Self::LEN {
            return Err(HashParseError::Length { want: Self::LEN, got: n });
        }
        if let Some((at, _)) = s.chars().enumerate().find(|(_, c)| !in_alphabet(*c)) {
            return Err(HashParseError::BadChar { at });
        }
        Ok(Self(s.to_string()))
    }
}
```

- [ ] **Step 4: Run** `cargo test -p fairlead-core hash::`. Expected: PASS (all eight tests).

- [ ] **Step 5: Commit** `feat(core): blake3 hashing, base32 and short ids`.

---

### Task 3: path.rs: RepoPath, case-fold key, hygiene, case collisions

Rules: R-SCOPE-007, R-SYN-040 (hygiene half), R-ARCH-024 (ordering), 50-architecture Section 10 (path form, canonical casing, reserved names).

**Files:**
- Create: `crates/fairlead-core/src/path.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod path;`)

**Interfaces:**
- Consumes: `fairlead_syntax::ids::is_reserved_windows_name(&str) -> bool`; `unicode_normalization`.
- Produces (contract 4.3, names fixed): `RepoPath` with `new, as_str, fold_key, parent, join, extension, is_within`; `PathError`. Additions: `RepoPath::{root, is_root, file_name, from_native_rel, hygiene}`, `PathIssue`, `find_case_collisions`, `Display`.

Design: the empty string is the repository root (`RepoPath::root()`), so `ControlFile.dir` of the root scope is representable. `new` checks structure only (no `..`, no leading `/`, no backslash, no empty segment, no control characters, no drive letter). Non-NFC names, Windows-reserved names and trailing dot or space are hygiene issues (`HN-P001`, R-SYN-040) reported by `hygiene()` and not refused by `new`: a Linux checkout may legitimately contain them and the tool must report, not crash. `Ord` is the derived bytewise order of the string (R-ARCH-024).

- [ ] **Step 1: Write the failing tests.** Create `path.rs` containing only this module.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn rp(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }

    #[test]
    fn r_arch_024__repo_path_orders_bytewise() {
        let mut v: Vec<RepoPath> = ["b", "a/b", "a", "a/a", "B"].iter().map(|s| rp(s)).collect();
        v.sort();
        let got: Vec<&str> = v.iter().map(RepoPath::as_str).collect();
        assert_eq!(got, ["B", "a", "a/a", "a/b", "b"]);
    }

    #[test]
    fn r_syn_040__new_rejects_structural_garbage() {
        for bad in ["/abs", "a\\b", "a//b", "../x", "a/../b", "a/./b", "C:/x", "a/", "x\u{0}y", "x\ny"] {
            assert!(RepoPath::new(bad).is_err(), "{bad:?} must be rejected");
        }
        assert!(RepoPath::new("").unwrap().is_root());
        assert!(RepoPath::new("src/lib.rs").is_ok());
    }

    #[test]
    fn r_syn_040__hygiene_reports_nfc_reserved_and_trailing() {
        assert_eq!(rp("cafe\u{301}.txt").hygiene(), vec![PathIssue::NotNfc]);
        assert_eq!(rp("src/CON.txt").hygiene(), vec![PathIssue::ReservedName { segment: "CON.txt".into() }]);
        assert_eq!(
            rp("docs/readme ").hygiene(),
            vec![PathIssue::TrailingDotOrSpace { segment: "readme ".into() }]
        );
        assert!(rp("src/lib.rs").hygiene().is_empty());
    }

    #[test]
    fn r_scope_007__case_only_directories_collide() {
        let ps: Vec<RepoPath> = ["Ui/a.rs", "ui/a.rs", "ui/b.rs", "docs/x.md"].iter().map(|s| rp(s)).collect();
        let groups = find_case_collisions(ps.iter());
        assert_eq!(groups.len(), 1, "only the Ui/ui pair is reported, not Ui/a.rs vs ui/a.rs");
        let names: Vec<&str> = groups[0].iter().map(RepoPath::as_str).collect();
        assert_eq!(names, ["Ui", "ui"]);
    }

    #[test]
    fn r_scope_007__case_only_files_collide() {
        let ps = [rp("a/README.md"), rp("a/readme.md")];
        assert_eq!(find_case_collisions(ps.iter()), vec![vec![rp("a/README.md"), rp("a/readme.md")]]);
    }

    #[test]
    fn r_scope_007__fold_key_is_nfc_and_case_insensitive() {
        let a = rp("Src/Cafe\u{301}.RS"); // NFD
        let b = rp("src/caf\u{e9}.rs"); // NFC
        assert_eq!(a.fold_key(), b.fold_key());
        assert_ne!(a, b, "the on-disk spelling is preserved for display");
    }

    #[test]
    fn r_arch_024__parent_join_extension_within() {
        let p = rp("a/b/c.test.ts");
        assert_eq!(p.parent().unwrap().as_str(), "a/b");
        assert_eq!(rp("a").parent().unwrap(), RepoPath::root());
        assert!(RepoPath::root().parent().is_none());
        assert_eq!(p.extension(), "ts");
        assert_eq!(rp(".gitignore").extension(), "gitignore");
        assert_eq!(rp("Makefile").extension(), "");
        assert_eq!(RepoPath::root().join("x").unwrap().as_str(), "x");
        assert_eq!(rp("a").join("b.rs").unwrap().as_str(), "a/b.rs");
        assert!(rp("a").join("../b").is_err());
        let a = rp("a");
        assert!(p.is_within(&a));
        assert!(a.is_within(&a));
        assert!(!rp("ab/c").is_within(&a), "a name prefix is not a parent");
        assert!(p.is_within(&RepoPath::root()));
    }

    #[test]
    fn r_arch_024__from_native_rel_converts_separators() {
        assert_eq!(RepoPath::from_native_rel(camino::Utf8Path::new("a\\b\\c.rs")).unwrap().as_str(), "a/b/c.rs");
        assert_eq!(RepoPath::from_native_rel(camino::Utf8Path::new("./x.rs")).unwrap().as_str(), "x.rs");
    }

    #[test]
    fn r_arch_024__serde_rejects_invalid_paths() {
        assert!(serde_json::from_str::<RepoPath>("\"../x\"").is_err());
        assert_eq!(serde_json::from_str::<RepoPath>("\"a/b\"").unwrap().as_str(), "a/b");
        assert_eq!(serde_json::to_string(&rp("a/b")).unwrap(), "\"a/b\"");
    }
}
```

- [ ] **Step 2: Add `pub mod path;`. Run** `cargo test -p fairlead-core path::`. Expected: FAIL (compile error: `cannot find type RepoPath`).

- [ ] **Step 3: Write the implementation** (above the tests).

```rust
//! Repo-relative, UTF-8, forward-slash paths (50-architecture Section 10).
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use unicode_normalization::{is_nfc, UnicodeNormalization};

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum PathError {
    #[error("path must be repo-relative (no leading '/')")]
    Absolute,
    #[error("path must use '/' separators")]
    Backslash,
    #[error("path has an empty segment")]
    EmptySegment,
    #[error("path has a '.' segment")]
    DotSegment,
    #[error("path has a '..' segment")]
    ParentSegment,
    #[error("path contains a control character")]
    ControlChar,
    #[error("path starts with a drive letter")]
    DriveLetter,
    #[error("a joined segment must be a single path segment")]
    BadSegment,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoPath(String);

/// One `HN-P001` reason (R-SYN-040, R-SYN-027).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PathIssue {
    NotNfc,
    ReservedName { segment: String },
    TrailingDotOrSpace { segment: String },
}

fn validate(s: &str) -> Result<(), PathError> {
    if s.is_empty() {
        return Ok(());
    }
    if s.starts_with('/') {
        return Err(PathError::Absolute);
    }
    if s.contains('\\') {
        return Err(PathError::Backslash);
    }
    if s.chars().any(char::is_control) {
        return Err(PathError::ControlChar);
    }
    let b = s.as_bytes();
    if let (Some(first), Some(second)) = (b.first(), b.get(1)) {
        if *second == b':' && first.is_ascii_alphabetic() {
            return Err(PathError::DriveLetter);
        }
    }
    for seg in s.split('/') {
        match seg {
            "" => return Err(PathError::EmptySegment),
            "." => return Err(PathError::DotSegment),
            ".." => return Err(PathError::ParentSegment),
            _ => {}
        }
    }
    Ok(())
}

impl RepoPath {
    pub fn new(s: &str) -> Result<Self, PathError> {
        validate(s)?;
        Ok(Self(s.to_string()))
    }

    /// The repository root (empty path).
    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Last segment; empty for the root.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or("")
    }

    /// Convert a native relative path (possibly with `\`) into a `RepoPath`.
    pub fn from_native_rel(p: &camino::Utf8Path) -> Result<Self, PathError> {
        let s = p.as_str().replace('\\', "/");
        let s = s.strip_prefix("./").unwrap_or(&s);
        Self::new(s)
    }

    /// NFC, simple case folding, NFC again: the lookup key for scope membership (R-SCOPE-007).
    /// Verify: full Unicode case folding (sharp s to "ss") is not applied; NTFS and APFS use simple
    /// case mapping, which `char::to_lowercase` approximates.
    pub fn fold_key(&self) -> String {
        let nfc: String = self.0.nfc().collect();
        let lower: String = nfc.chars().flat_map(char::to_lowercase).collect();
        lower.nfc().collect()
    }

    pub fn parent(&self) -> Option<RepoPath> {
        if self.0.is_empty() {
            return None;
        }
        match self.0.rsplit_once('/') {
            Some((head, _)) => Some(Self(head.to_string())),
            None => Some(Self::root()),
        }
    }

    pub fn join(&self, seg: &str) -> Result<RepoPath, PathError> {
        if seg.contains('/') {
            return Err(PathError::BadSegment);
        }
        let joined = if self.0.is_empty() { seg.to_string() } else { format!("{}/{}", self.0, seg) };
        Self::new(&joined)
    }

    /// Extension without the dot; a dotfile such as `.gitignore` yields `gitignore` (R-SYN-006 table).
    pub fn extension(&self) -> &str {
        match self.file_name().rsplit_once('.') {
            Some((_, ext)) => ext,
            None => "",
        }
    }

    /// True when `self` equals `dir` or lies below it (every path is within the root).
    pub fn is_within(&self, dir: &RepoPath) -> bool {
        dir.is_root()
            || self.0 == dir.0
            || (self.0.starts_with(&dir.0) && self.0.as_bytes().get(dir.0.len()) == Some(&b'/'))
    }

    /// `HN-P001` reasons (R-SYN-040): non-NFC, Windows-reserved, trailing dot or space.
    pub fn hygiene(&self) -> Vec<PathIssue> {
        let mut out = Vec::new();
        if !is_nfc(&self.0) {
            out.push(PathIssue::NotNfc);
        }
        for seg in self.0.split('/').filter(|s| !s.is_empty()) {
            if seg.ends_with('.') || seg.ends_with(' ') {
                out.push(PathIssue::TrailingDotOrSpace { segment: seg.to_string() });
                continue;
            }
            let lower = seg.to_lowercase();
            let stem = lower.split('.').next().unwrap_or("");
            if fairlead_syntax::ids::is_reserved_windows_name(&lower)
                || fairlead_syntax::ids::is_reserved_windows_name(stem)
            {
                out.push(PathIssue::ReservedName { segment: seg.to_string() });
            }
        }
        out
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for RepoPath {
    type Error = PathError;
    fn try_from(s: String) -> Result<Self, PathError> {
        validate(&s)?;
        Ok(Self(s))
    }
}

impl From<RepoPath> for String {
    fn from(p: RepoPath) -> String {
        p.0
    }
}

/// Groups of paths (files or directories) that differ only in case or normalisation form and share
/// one parent, sorted by fold key then bytewise (R-SCOPE-007). A collision caused only by a
/// colliding ancestor is not repeated.
pub fn find_case_collisions<'a, I>(paths: I) -> Vec<Vec<RepoPath>>
where
    I: IntoIterator<Item = &'a RepoPath>,
{
    let mut groups: BTreeMap<String, BTreeSet<RepoPath>> = BTreeMap::new();
    for p in paths {
        let mut cur = p.clone();
        while !cur.is_root() {
            groups.entry(cur.fold_key()).or_default().insert(cur.clone());
            match cur.parent() {
                Some(next) => cur = next,
                None => break,
            }
        }
    }
    groups
        .into_values()
        .filter(|g| g.len() > 1)
        .filter(|g| {
            let mut parents = g.iter().map(RepoPath::parent);
            let first = parents.next();
            parents.all(|p| Some(p) == first)
        })
        .map(|g| g.into_iter().collect())
        .collect()
}
```

Verify: `is_reserved_windows_name` is called with the lowercased segment and its stem. `unicode_normalization::is_nfc` exists as a free function; if not, use `s.nfc().eq(s.chars())`.

- [ ] **Step 4: Run** `cargo test -p fairlead-core path::`. Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): RepoPath with fold key, hygiene and case-collision groups`.

---

### Task 4: diag.rs: Diagnostic, Location, DiagCode constants for every HN code

Rules: R-ARCH-103 (shape), R-ARCH-055 and contract invariant 10 (codes only here and in syntax), R-SYN-039 (C002), R-SYN-041 (C003), R-SYN-040 (P001).

**Files:**
- Create: `crates/fairlead-core/src/diag.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod diag;`)

**Interfaces:**
- Consumes: `fairlead_syntax::diag::{DiagCode, Severity, Span, SyntaxDiag}` and `fairlead_syntax::diag::codes::*` (every `HN-S*` and `HN-I*` constant, contract 4.1).
- Produces (contract 4.9, names fixed): `Diagnostic`, `Location`, `impl From<SyntaxDiag> for Diagnostic`, `diag::codes` (re-exporting the syntax constants and adding every other code of chapter 13). Additions: `Location::{file, at}`, `Diagnostic::{new, with_hint, with_expected, with_text, with_scope, from_syntax}`, `sort_diagnostics`, `codes::CORE_CODES`.

Why `From<SyntaxDiag>` is lossy: a `SyntaxDiag` has a `Span` but no path, while `Location` needs a `RepoPath`. The `From` impl therefore uses the root path as a placeholder; every caller that knows the file uses `Diagnostic::from_syntax(path, d)` (see Contract issues, item 1).

- [ ] **Step 1: Write the failing tests.** Create `diag.rs` with only this module.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn rp(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }

    #[test]
    fn r_arch_055__core_codes_are_unique_and_well_formed() {
        let mut seen = std::collections::BTreeSet::new();
        for c in codes::CORE_CODES {
            let s = c.0;
            let b = s.as_bytes();
            assert_eq!(b.len(), 7, "{s}");
            assert!(s.starts_with("HN-"), "{s}");
            assert!(b[3].is_ascii_uppercase(), "{s}");
            assert!(b[4..].iter().all(u8::is_ascii_digit), "{s}");
            assert!(seen.insert(s), "duplicate code {s}");
        }
        assert!(seen.len() > 100, "chapter 13 allocates well over 100 core codes");
    }

    #[test]
    fn r_syn_041__control_family_codes_are_the_frozen_ones() {
        assert_eq!(codes::HN_C001.0, "HN-C001");
        assert_eq!(codes::HN_C002.0, "HN-C002");
        assert_eq!(codes::HN_C003.0, "HN-C003");
        assert_eq!(codes::HN_C004.0, "HN-C004");
        assert_eq!(codes::HN_C005.0, "HN-C005");
        assert_eq!(codes::HN_C006.0, "HN-C006");
        assert_eq!(codes::HN_P001.0, "HN-P001");
        assert_eq!(codes::HN_V013.0, "HN-V013");
    }

    #[test]
    fn r_arch_103__from_syntax_keeps_every_field_and_sets_path() {
        let sd = SyntaxDiag {
            code: codes::HN_S001,
            severity: Severity::Error,
            span: Span { start: 10, end: 14, line: 2, col: 4 },
            message: "unknown verb".into(),
            expected: Some("one of def use ovr flag env end".into()),
            did_you_mean: Some("def".into()),
            hint: "fix the verb".into(),
        };
        let d = Diagnostic::from_syntax(&rp("src/a.rs"), sd.clone());
        assert_eq!(d.code, sd.code);
        assert_eq!(d.location, Location { path: rp("src/a.rs"), line: 2, col: 4 });
        assert_eq!(d.message, "unknown verb");
        assert_eq!(d.expected.as_deref(), Some("one of def use ovr flag env end"));
        assert_eq!(d.did_you_mean.as_deref(), Some("def"));
        assert_eq!(d.hint, "fix the verb");
        let placeholder: Diagnostic = sd.into();
        assert!(placeholder.location.path.is_root());
    }

    #[test]
    fn r_arch_103__sort_is_path_line_col_code_message() {
        let mk = |p: &str, l: u32, c: u32, code: DiagCode, m: &str| {
            Diagnostic::new(code, Severity::Warn, Location::at(rp(p), l, c), m)
        };
        let mut v = vec![
            mk("b.rs", 1, 1, codes::HN_C001, "x"),
            mk("a.rs", 2, 1, codes::HN_C001, "x"),
            mk("a.rs", 1, 9, codes::HN_C001, "x"),
            mk("a.rs", 1, 1, codes::HN_C002, "x"),
            mk("a.rs", 1, 1, codes::HN_C001, "z"),
            mk("a.rs", 1, 1, codes::HN_C001, "y"),
        ];
        sort_diagnostics(&mut v);
        let got: Vec<(String, u32, u32, &str, String)> = v
            .iter()
            .map(|d| (d.location.path.to_string(), d.location.line, d.location.col, d.code.0, d.message.clone()))
            .collect();
        let want: Vec<(String, u32, u32, &str, String)> = vec![
            ("a.rs".into(), 1, 1, "HN-C001", "y".into()),
            ("a.rs".into(), 1, 1, "HN-C001", "z".into()),
            ("a.rs".into(), 1, 1, "HN-C002", "x".into()),
            ("a.rs".into(), 1, 9, "HN-C001", "x".into()),
            ("a.rs".into(), 2, 1, "HN-C001", "x".into()),
            ("b.rs".into(), 1, 1, "HN-C001", "x".into()),
        ];
        assert_eq!(got, want);
    }

    #[test]
    fn r_arch_103__whole_file_location_is_line_zero() {
        let l = Location::file(rp("a/b.toml"));
        assert_eq!((l.line, l.col), (0, 0));
    }
}
```

- [ ] **Step 2: Add `pub mod diag;`. Run** `cargo test -p fairlead-core diag::`. Expected: FAIL (compile error: `cannot find value HN_C001 in module codes` / `Diagnostic`).

- [ ] **Step 3: Write the implementation** (above the tests). The list below is a mechanical transcription of the tables of 13-checks-and-diagnostics.md (rows C001 to M012). `HN-S*`, `HN-I*` come from `fairlead-syntax`; `HN-P001` is defined here as well because P3 emits it for paths (a local definition shadows the glob re-export, the string is identical).

```rust
//! Pre-finding diagnostics and the `HN-` code constants (13-checks-and-diagnostics R-CHK-042).
use crate::path::RepoPath;
pub use fairlead_syntax::diag::{DiagCode, Severity, Span, SyntaxDiag};

/// Code constants. No string literal `"HN-..."` may appear outside this file and fairlead-syntax.
pub mod codes {
    pub use fairlead_syntax::diag::codes::*;

    use super::DiagCode;

    macro_rules! core_codes {
        ($($name:ident = $text:literal),* $(,)?) => {
            $(pub const $name: DiagCode = DiagCode($text);)*
            /// Every code owned by core (the HN-S* and HN-I* codes live in fairlead-syntax).
            pub const CORE_CODES: &[DiagCode] = &[$($name),*];
        };
    }

    core_codes! {
        HN_C001 = "HN-C001", HN_C002 = "HN-C002", HN_C003 = "HN-C003", HN_C004 = "HN-C004",
        HN_C005 = "HN-C005", HN_C006 = "HN-C006", HN_C007 = "HN-C007", HN_C008 = "HN-C008",
        HN_C009 = "HN-C009", HN_C011 = "HN-C011", HN_C012 = "HN-C012", HN_C013 = "HN-C013",
        HN_C014 = "HN-C014",
        HN_R001 = "HN-R001", HN_R002 = "HN-R002", HN_R004 = "HN-R004", HN_R005 = "HN-R005",
        HN_R006 = "HN-R006", HN_R007 = "HN-R007", HN_R008 = "HN-R008",
        HN_D001 = "HN-D001", HN_D002 = "HN-D002", HN_D003 = "HN-D003", HN_D004 = "HN-D004",
        HN_D005 = "HN-D005", HN_D006 = "HN-D006",
        HN_T001 = "HN-T001", HN_T002 = "HN-T002", HN_T003 = "HN-T003", HN_T004 = "HN-T004",
        HN_T005 = "HN-T005", HN_T006 = "HN-T006", HN_T007 = "HN-T007", HN_T008 = "HN-T008",
        HN_T009 = "HN-T009", HN_T010 = "HN-T010",
        HN_U001 = "HN-U001",
        HN_G001 = "HN-G001", HN_G002 = "HN-G002", HN_G003 = "HN-G003", HN_G004 = "HN-G004",
        HN_G005 = "HN-G005", HN_G006 = "HN-G006",
        HN_X001 = "HN-X001", HN_X002 = "HN-X002", HN_X003 = "HN-X003", HN_X004 = "HN-X004",
        HN_X005 = "HN-X005", HN_X006 = "HN-X006", HN_X007 = "HN-X007", HN_X008 = "HN-X008",
        HN_X009 = "HN-X009", HN_X010 = "HN-X010", HN_X011 = "HN-X011", HN_X012 = "HN-X012",
        HN_X013 = "HN-X013", HN_X014 = "HN-X014", HN_X015 = "HN-X015", HN_X016 = "HN-X016",
        HN_B001 = "HN-B001", HN_B002 = "HN-B002",
        HN_A001 = "HN-A001", HN_A002 = "HN-A002", HN_A003 = "HN-A003", HN_A004 = "HN-A004",
        HN_N001 = "HN-N001", HN_N002 = "HN-N002",
        HN_F001 = "HN-F001",
        HN_P001 = "HN-P001",
        HN_V001 = "HN-V001", HN_V002 = "HN-V002", HN_V003 = "HN-V003", HN_V004 = "HN-V004",
        HN_V005 = "HN-V005", HN_V006 = "HN-V006", HN_V007 = "HN-V007", HN_V008 = "HN-V008",
        HN_V009 = "HN-V009", HN_V010 = "HN-V010", HN_V011 = "HN-V011", HN_V012 = "HN-V012",
        HN_V013 = "HN-V013", HN_V014 = "HN-V014", HN_V015 = "HN-V015", HN_V016 = "HN-V016",
        HN_E001 = "HN-E001", HN_E002 = "HN-E002", HN_E003 = "HN-E003", HN_E004 = "HN-E004",
        HN_E005 = "HN-E005", HN_E006 = "HN-E006", HN_E007 = "HN-E007",
        HN_L001 = "HN-L001", HN_L002 = "HN-L002",
        HN_H001 = "HN-H001", HN_H002 = "HN-H002", HN_H003 = "HN-H003", HN_H004 = "HN-H004",
        HN_H005 = "HN-H005", HN_H006 = "HN-H006", HN_H007 = "HN-H007", HN_H008 = "HN-H008",
        HN_H009 = "HN-H009", HN_H010 = "HN-H010", HN_H011 = "HN-H011", HN_H012 = "HN-H012",
        HN_H013 = "HN-H013", HN_H014 = "HN-H014", HN_H015 = "HN-H015", HN_H016 = "HN-H016",
        HN_H017 = "HN-H017", HN_H018 = "HN-H018", HN_H019 = "HN-H019", HN_H020 = "HN-H020",
        HN_H021 = "HN-H021", HN_H022 = "HN-H022", HN_H023 = "HN-H023", HN_H024 = "HN-H024",
        HN_H025 = "HN-H025",
        HN_W001 = "HN-W001", HN_W002 = "HN-W002", HN_W003 = "HN-W003", HN_W004 = "HN-W004",
        HN_K001 = "HN-K001",
        HN_M001 = "HN-M001", HN_M002 = "HN-M002", HN_M003 = "HN-M003", HN_M004 = "HN-M004",
        HN_M005 = "HN-M005", HN_M006 = "HN-M006", HN_M007 = "HN-M007", HN_M008 = "HN-M008",
        HN_M009 = "HN-M009", HN_M010 = "HN-M010", HN_M011 = "HN-M011", HN_M012 = "HN-M012",
    }
}

/// 1-based line and column (Unicode scalar values of the original text, C-16); line 0 = whole file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Location {
    pub path: RepoPath,
    pub line: u32,
    pub col: u32,
}

impl Location {
    pub fn file(path: RepoPath) -> Self {
        Self { path, line: 0, col: 0 }
    }
    pub fn at(path: RepoPath, line: u32, col: u32) -> Self {
        Self { path, line, col }
    }
}

/// A pre-finding: what every stage before `check` emits (syntax, config, walk, registry).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    pub code: DiagCode,
    pub severity: Severity,
    pub location: Location,
    pub scope: Option<fairlead_syntax::ids::ScopeSlug>,
    pub rule: Option<String>,
    pub related: Option<String>,
    pub message: String,
    pub expected: Option<String>,
    pub did_you_mean: Option<String>,
    pub hint: String,
    pub text: String,
}

impl Diagnostic {
    pub fn new(code: DiagCode, severity: Severity, location: Location, message: impl Into<String>) -> Self {
        Self {
            code,
            severity,
            location,
            scope: None,
            rule: None,
            related: None,
            message: message.into(),
            expected: None,
            did_you_mean: None,
            hint: String::new(),
            text: String::new(),
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Offending text, used later for the finding fingerprint.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn with_scope(mut self, scope: fairlead_syntax::ids::ScopeSlug) -> Self {
        self.scope = Some(scope);
        self
    }

    /// The lossless conversion: the caller knows the file, `SyntaxDiag` does not.
    pub fn from_syntax(path: &RepoPath, d: SyntaxDiag) -> Self {
        Self {
            code: d.code,
            severity: d.severity,
            location: Location::at(path.clone(), d.span.line, d.span.col),
            scope: None,
            rule: None,
            related: None,
            message: d.message,
            expected: d.expected,
            did_you_mean: d.did_you_mean,
            hint: d.hint,
            text: String::new(),
        }
    }
}

/// Placeholder-path conversion required by the contract; prefer [`Diagnostic::from_syntax`].
impl From<SyntaxDiag> for Diagnostic {
    fn from(d: SyntaxDiag) -> Self {
        Diagnostic::from_syntax(&RepoPath::root(), d)
    }
}

/// Total order (path, line, col, code, message): R-ARCH-051 applied to pre-findings.
pub fn sort_diagnostics(v: &mut [Diagnostic]) {
    v.sort_by(|a, b| {
        a.location
            .path
            .cmp(&b.location.path)
            .then(a.location.line.cmp(&b.location.line))
            .then(a.location.col.cmp(&b.location.col))
            .then(a.code.cmp(&b.code))
            .then_with(|| a.message.cmp(&b.message))
    });
}
```

Verify: `fairlead_syntax::ids::ScopeSlug` must derive `Clone, PartialEq, Eq, Debug` (contract 4 preamble: every type does). If `fairlead_syntax::diag::codes` does not define `HN_P001`, the local definition above is the only one; if it does, the local one shadows the glob without error.

- [ ] **Step 4: Run** `cargo test -p fairlead-core diag::`. Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): Diagnostic, Location and all HN code constants`.

---

### Task 5: outcome.rs: ExitCode and VerbOutcome

Rules: supports R-CLI-003 (P8 owns the CLI mapping), R-ARCH-100. Contract 4.3.

**Files:**
- Create: `crates/fairlead-core/src/outcome.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod outcome;`)

**Interfaces:**
- Produces (names fixed): `ExitCode { Ok = 0, Findings = 1, Usage = 2, StaleGenerated = 3, Refused = 4, Internal = 5 }`, `ExitCode::resolve(&[ExitCode]) -> ExitCode`, `VerbOutcome { ok, signal, merge, exit_code }`. Addition: `ExitCode::as_u8`.

Note: the derived `Ord` of `ExitCode` is the numeric order; the precedence 5, 2, 4, 1, 3, 0 is a different order and lives only in `resolve`.

- [ ] **Step 1: Write the failing tests.**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ExitCode::*;

    #[test]
    fn r_cli_003__resolve_precedence_is_5_2_4_1_3_0() {
        assert_eq!(ExitCode::resolve(&[]), Ok);
        assert_eq!(ExitCode::resolve(&[Ok, Ok]), Ok);
        assert_eq!(ExitCode::resolve(&[StaleGenerated, Ok]), StaleGenerated);
        assert_eq!(ExitCode::resolve(&[Findings, StaleGenerated]), Findings);
        assert_eq!(ExitCode::resolve(&[Refused, Findings]), Refused);
        assert_eq!(ExitCode::resolve(&[Usage, Refused, Findings]), Usage);
        assert_eq!(ExitCode::resolve(&[Internal, Usage, Refused, Findings, StaleGenerated]), Internal);
        assert_eq!(ExitCode::resolve(&[StaleGenerated, Internal]), Internal);
    }

    #[test]
    fn r_cli_003__resolve_is_order_independent() {
        let all = [Ok, Findings, Usage, StaleGenerated, Refused, Internal];
        for a in all {
            for b in all {
                assert_eq!(ExitCode::resolve(&[a, b]), ExitCode::resolve(&[b, a]));
            }
        }
    }

    #[test]
    fn r_cli_003__numeric_values_are_stable() {
        assert_eq!(
            [Ok, Findings, Usage, StaleGenerated, Refused, Internal].map(ExitCode::as_u8),
            [0, 1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn r_cli_003__verb_outcome_merges_and_resolves() {
        let mut a = VerbOutcome::ok();
        assert_eq!(a.exit_code(), Ok);
        a.signal(StaleGenerated);
        let mut b = VerbOutcome::default();
        b.signal(Findings);
        a.merge(b);
        assert_eq!(a.exit_code(), Findings);
        a.signal(Internal);
        assert_eq!(a.exit_code(), Internal);
    }
}
```

- [ ] **Step 2: Add `pub mod outcome;`. Run** `cargo test -p fairlead-core r_cli_003`. Expected: FAIL (compile error: `cannot find type ExitCode`).

- [ ] **Step 3: Write the implementation.**

```rust
//! Exit codes are defined only in 15-cli.md R-CLI-003; this module names them.

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ExitCode {
    Ok = 0,
    Findings = 1,
    Usage = 2,
    StaleGenerated = 3,
    Refused = 4,
    Internal = 5,
}

impl ExitCode {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Rank in the precedence 5, 2, 4, 1, 3, 0 (higher wins).
    const fn precedence(self) -> u8 {
        match self {
            ExitCode::Internal => 5,
            ExitCode::Usage => 4,
            ExitCode::Refused => 3,
            ExitCode::Findings => 2,
            ExitCode::StaleGenerated => 1,
            ExitCode::Ok => 0,
        }
    }

    /// The winning code of several signals; `Ok` when there are none (R-CLI-003).
    pub fn resolve(codes: &[ExitCode]) -> ExitCode {
        codes.iter().copied().max_by_key(|c| c.precedence()).unwrap_or(ExitCode::Ok)
    }
}

/// What a verb reports besides text; the bin turns it into the process exit code.
#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct VerbOutcome {
    signals: Vec<ExitCode>,
}

impl VerbOutcome {
    pub fn ok() -> Self {
        Self::default()
    }

    pub fn signal(&mut self, c: ExitCode) {
        self.signals.push(c);
    }

    pub fn merge(&mut self, o: VerbOutcome) {
        self.signals.extend(o.signals);
    }

    pub fn exit_code(&self) -> ExitCode {
        ExitCode::resolve(&self.signals)
    }
}
```

- [ ] **Step 4: Run** `cargo test -p fairlead-core r_cli_003`. Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): ExitCode precedence and VerbOutcome`.

---

### Task 6: discover.rs (part 1): Edition, Pin, pin_check

Rules: R-CLI-005, R-ARCH-013, C-12, C-34 (edition half).

**Files:**
- Create: `crates/fairlead-core/src/discover.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod discover;`)

**Interfaces (contract 4.4, names fixed):**
- Produces: `Edition(pub u32)`, `SUPPORTED_EDITION`, `Pin { version: Option<semver::Version>, edition: Edition }`, `PinVerdict { Ok, OkNewerBinary { info }, Refused(PinRefusal) }`, `PinRefusal { install_line, reason }`, `pin_check(&Version, Edition, &Pin) -> PinVerdict`. Additions: `DEFAULT_FOLDER`, `parse_pin_version`.
- Task 15 appends the rest of `discover.rs` (root lookup, I/O); the `Edition`/`Pin` types come first because `config` and `error` need them.

Rules of the check: a different edition always refuses (whichever side is newer); on the same edition a binary older than the pin refuses and a newer binary proceeds with one info line. Pre-release and build metadata of the binary are ignored in the comparison (a `0.1.0-alpha.2` build satisfies the pin `0.1`). `fairlead-version = "0.1"` is padded to `0.1.0` (C-12).

- [ ] **Step 1: Write the failing tests.** Create `discover.rs` with only this module.

```rust
#[cfg(test)]
mod pin_tests {
    use super::*;
    use semver::Version;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }
    fn pin(version: Option<&str>, edition: u32) -> Pin {
        Pin { version: version.map(v), edition: Edition(edition) }
    }

    #[test]
    fn r_cli_005__older_binary_is_refused_with_exact_install_line() {
        match pin_check(&v("0.1.0"), SUPPORTED_EDITION, &pin(Some("0.2.0"), 1)) {
            PinVerdict::Refused(r) => {
                assert_eq!(r.install_line, "cargo install fairlead --version 0.2.0 --locked");
                assert!(r.reason.contains("older"), "{}", r.reason);
            }
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn r_cli_005__newer_binary_same_edition_proceeds_with_info_line() {
        match pin_check(&v("0.3.1"), SUPPORTED_EDITION, &pin(Some("0.2.0"), 1)) {
            PinVerdict::OkNewerBinary { info } => assert!(info.contains("0.3.1") && info.contains("0.2.0")),
            other => panic!("expected OkNewerBinary, got {other:?}"),
        }
    }

    #[test]
    fn r_cli_005__equal_version_and_missing_pin_are_ok() {
        assert_eq!(pin_check(&v("0.2.0"), SUPPORTED_EDITION, &pin(Some("0.2.0"), 1)), PinVerdict::Ok);
        assert_eq!(pin_check(&v("0.2.0"), SUPPORTED_EDITION, &pin(None, 1)), PinVerdict::Ok);
    }

    #[test]
    fn r_cli_005__other_edition_is_refused_whichever_side_is_newer() {
        for e in [0u32, 2] {
            match pin_check(&v("9.0.0"), SUPPORTED_EDITION, &pin(Some("0.1.0"), e)) {
                PinVerdict::Refused(r) => {
                    assert!(r.reason.contains(&format!("edition {e}")), "{}", r.reason);
                    assert_eq!(r.install_line, "cargo install fairlead --version 0.1.0 --locked");
                }
                other => panic!("expected refusal, got {other:?}"),
            }
        }
        match pin_check(&v("1.0.0"), SUPPORTED_EDITION, &pin(None, 2)) {
            PinVerdict::Refused(r) => assert_eq!(r.install_line, "cargo install fairlead --locked"),
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn r_cli_005__prerelease_binary_satisfies_its_release_pin() {
        assert_eq!(pin_check(&v("0.1.0-alpha.2"), SUPPORTED_EDITION, &pin(Some("0.1.0"), 1)), PinVerdict::Ok);
    }

    #[test]
    fn r_cli_005__pin_version_is_padded_to_three_parts() {
        assert_eq!(parse_pin_version("0.1"), Some(v("0.1.0")));
        assert_eq!(parse_pin_version("1"), Some(v("1.0.0")));
        assert_eq!(parse_pin_version(" 1.2.3 "), Some(v("1.2.3")));
        assert_eq!(parse_pin_version("x.y"), None);
        assert_eq!(parse_pin_version(""), None);
    }

    #[test]
    fn r_arch_013__supported_edition_is_one() {
        assert_eq!(SUPPORTED_EDITION, Edition(1));
    }
}
```

- [ ] **Step 2: Add `pub mod discover;`. Run** `cargo test -p fairlead-core r_cli_005`. Expected: FAIL (compile error: `cannot find type Pin`).

- [ ] **Step 3: Write the implementation** (above the tests).

```rust
//! Root discovery and the pin check (R-CLI-004, R-CLI-005, R-ARCH-010..013, D-31, D-33).
//! Part 1: the pure pin types. Task 15 appends root lookup and I/O.
use semver::Version;
use std::cmp::Ordering;

/// Integer edition (D-45).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Edition(pub u32);

/// The edition this build reads and writes.
pub const SUPPORTED_EDITION: Edition = Edition(1);

/// Default managerial folder name (a config key, D-30).
pub const DEFAULT_FOLDER: &str = "fairlead";

/// `[settings] fairlead-version` plus the header edition. "0.1" is padded to 0.1.0 (C-12).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pin {
    pub version: Option<Version>,
    pub edition: Edition,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PinRefusal {
    pub install_line: String,
    pub reason: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PinVerdict {
    Ok,
    OkNewerBinary { info: String },
    Refused(PinRefusal),
}

/// Parse a pin such as `0.1`, `1` or `1.2.3`; `None` when it is not a version.
pub fn parse_pin_version(s: &str) -> Option<Version> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let padded = match t.matches('.').count() {
        0 => format!("{t}.0.0"),
        1 => format!("{t}.0"),
        _ => t.to_string(),
    };
    Version::parse(&padded).ok()
}

fn install_line(v: Option<&Version>) -> String {
    match v {
        Some(v) => format!("cargo install fairlead --version {v} --locked"),
        None => "cargo install fairlead --locked".to_string(),
    }
}

/// Pure (R-CLI-005, D-50(4)): another edition or an older binary refuses; a newer binary on the same
/// edition proceeds. Pre-release and build metadata of the binary are ignored.
pub fn pin_check(binary: &Version, supported: Edition, pin: &Pin) -> PinVerdict {
    if pin.edition != supported {
        return PinVerdict::Refused(PinRefusal {
            install_line: install_line(pin.version.as_ref()),
            reason: format!(
                "this binary supports edition {} but the codebase is pinned to edition {}",
                supported.0, pin.edition.0
            ),
        });
    }
    let Some(pinned) = &pin.version else {
        return PinVerdict::Ok;
    };
    let release = Version::new(binary.major, binary.minor, binary.patch);
    match release.cmp(pinned) {
        Ordering::Less => PinVerdict::Refused(PinRefusal {
            install_line: install_line(Some(pinned)),
            reason: format!("binary {binary} is older than the fairlead-version pin {pinned}"),
        }),
        Ordering::Equal => PinVerdict::Ok,
        Ordering::Greater => PinVerdict::OkNewerBinary {
            info: format!(
                "binary {binary} is newer than the fairlead-version pin {pinned} (same edition {}); proceeding",
                supported.0
            ),
        },
    }
}
```

- [ ] **Step 4: Run** `cargo test -p fairlead-core r_cli_005 r_arch_013` (two invocations if needed). Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): edition, pin and pin_check`.

---

### Task 7: config/mod.rs (ConfigFailure only) and error.rs

Rules: R-ARCH-100, R-ARCH-102; contract 4.3 `Error`, `ErrorClass`, `Refusal`, `RefusalKind`; C-10.

**Files:**
- Create: `crates/fairlead-core/src/config/mod.rs` (only `ConfigFailure` now; Tasks 9 to 14 and 17 extend it)
- Create: `crates/fairlead-core/src/error.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod config; pub mod error;`)

**Interfaces:**
- Consumes: `diag::Diagnostic`, `discover::PinRefusal`, `path::RepoPath`.
- Produces (names fixed): `Error` (ten variants), `ErrorClass { Usage, Refused, Internal }`, `Error::class`, `Error::diagnostics`, `Refusal`, `RefusalKind` (nine kinds), `ConfigFailure { path, diags }`. Additions: `IoFailure`, `Error::io`, `ConfigFailure::with_path`, `impl From<ConfigFailure> for Error`.

Class mapping (contract 4.3 and C-10): `NoRoot`, `PathOutsideRoot`, `Usage`, `Reserved`, `Config` are Usage; `Pin` and `Refused` are Refused; `Io` is Usage only when `kind == NotFound` and the path was user-supplied, else Internal; `CannotDetermine` and `Internal` are Internal.

- [ ] **Step 1: Write `config/mod.rs`** (the minimal file; later tasks add modules and items to it).

```rust
//! Control-file parsing (lenient header pass, typed schema, rules, approvals). See Tasks 9 to 14, 17.
use crate::diag::Diagnostic;
use crate::path::RepoPath;

/// A control file (or approvals file) that cannot be used: the scope it governs is unresolved
/// (R-SYN-033, fail closed).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfigFailure {
    pub path: RepoPath,
    pub diags: Vec<Diagnostic>,
}

impl ConfigFailure {
    /// Re-label every location with the real file path (header parsing does not know it).
    pub fn with_path(mut self, path: &RepoPath) -> Self {
        self.path = path.clone();
        for d in &mut self.diags {
            d.location.path = path.clone();
        }
        self
    }
}
```

- [ ] **Step 2: Write the failing tests** (new `error.rs` with only this module).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigFailure;
    use crate::diag::{codes, Diagnostic, Location, Severity};
    use crate::discover::PinRefusal;
    use crate::path::RepoPath;

    fn io(kind: std::io::ErrorKind, user_path: bool) -> Error {
        Error::io("some/file", "read", &std::io::Error::new(kind, "boom"), user_path)
    }

    #[test]
    fn r_arch_100__class_mapping() {
        use ErrorClass::*;
        assert_eq!(Error::NoRoot { searched_from: "x".into() }.class(), Usage);
        assert_eq!(Error::PathOutsideRoot { path: "x".into() }.class(), Usage);
        assert_eq!(Error::Usage("x".into()).class(), Usage);
        assert_eq!(Error::Reserved { verb: "seal", milestone: "0.2" }.class(), Usage);
        let cf = ConfigFailure { path: RepoPath::root(), diags: vec![] };
        assert_eq!(Error::Config(Box::new(cf)).class(), Usage);
        let pin = PinRefusal { install_line: "i".into(), reason: "r".into() };
        assert_eq!(Error::Pin(pin).class(), Refused);
        let r = crate::error::Refusal { kind: RefusalKind::LockHeld, message: "m".into(), hint: "h".into() };
        assert_eq!(Error::Refused(r).class(), Refused);
        assert_eq!(Error::CannotDetermine("x".into()).class(), Internal);
        assert_eq!(Error::Internal("x".into()).class(), Internal);
    }

    #[test]
    fn r_arch_100__io_not_found_on_user_path_is_usage_else_internal() {
        assert_eq!(io(std::io::ErrorKind::NotFound, true).class(), ErrorClass::Usage);
        assert_eq!(io(std::io::ErrorKind::NotFound, false).class(), ErrorClass::Internal);
        assert_eq!(io(std::io::ErrorKind::PermissionDenied, true).class(), ErrorClass::Internal);
    }

    #[test]
    fn r_arch_102__cannot_determine_is_internal_and_says_so() {
        let e = Error::CannotDetermine("git state unreadable".into());
        assert_eq!(e.class(), ErrorClass::Internal);
        assert!(e.to_string().contains("git state unreadable"));
    }

    #[test]
    fn r_arch_100__config_error_exposes_diagnostics() {
        let d = Diagnostic::new(codes::HN_C001, Severity::Error, Location::file(RepoPath::new("fairlead/control.toml").unwrap()), "bad");
        let e = Error::from(ConfigFailure { path: RepoPath::new("fairlead/control.toml").unwrap(), diags: vec![d.clone()] });
        assert_eq!(e.diagnostics(), &[d]);
        assert!(Error::Usage("x".into()).diagnostics().is_empty());
        assert!(e.to_string().contains("fairlead/control.toml"));
    }

    #[test]
    fn r_arch_100__reserved_verb_message_names_the_milestone() {
        assert_eq!(Error::Reserved { verb: "seal", milestone: "0.2" }.to_string(), "seal arrives in 0.2");
    }

    #[test]
    fn r_arch_100__core_never_calls_process_exit() {
        // Built from two literals so this file does not contain the needle itself.
        let needle = concat!("process", "::exit");
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        let mut checked = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let p = entry.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    assert!(!text.contains(needle), "{} calls the process-exit function", p.display());
                    checked += 1;
                }
            }
        }
        assert!(checked > 5);
    }
}
```

- [ ] **Step 3: Add `pub mod config; pub mod error;`. Run** `cargo test -p fairlead-core r_arch_100 r_arch_102`. Expected: FAIL (compile error: `cannot find type Error`).

- [ ] **Step 4: Write the implementation** (above the tests).

```rust
//! Typed errors; the library never exits the process (R-ARCH-100). The bin maps `class()` to codes.
use crate::config::ConfigFailure;
use crate::diag::Diagnostic;
use crate::discover::PinRefusal;
use std::fmt;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("no root managerial folder found searching up from {searched_from}; run `fairlead init` to create one")]
    NoRoot { searched_from: String },
    #[error("path is outside the root: {path}")]
    PathOutsideRoot { path: String },
    #[error("{0}")]
    Usage(String),
    #[error("{verb} arrives in {milestone}")]
    Reserved { verb: &'static str, milestone: &'static str },
    #[error("control file {} is invalid", .0.path)]
    Config(Box<ConfigFailure>),
    #[error("refused by pin: {}", .0.reason)]
    Pin(PinRefusal),
    #[error("refused: {}", .0.message)]
    Refused(Refusal),
    #[error("{0}")]
    Io(IoFailure),
    #[error("cannot determine: {0}")]
    CannotDetermine(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ErrorClass {
    Usage,
    Refused,
    Internal,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RefusalKind {
    PinMismatch,
    LockHeld,
    NoClobber,
    HandEditedGenerated,
    PlanNotConfirmed,
    NotATty,
    CiSet,
    UnbalancedBlock,
    Floor,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Refusal {
    pub kind: RefusalKind,
    pub message: String,
    pub hint: String,
}

/// A failed file-system operation (C-10).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IoFailure {
    pub path: String,
    pub op: &'static str,
    pub kind: std::io::ErrorKind,
    pub detail: String,
    /// True when the path came from the user (flag, argument), so NotFound is a usage error.
    pub user_path: bool,
}

impl fmt::Display for IoFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot {} {}: {}", self.op, self.path, self.detail)
    }
}

impl Error {
    pub fn io(path: impl Into<String>, op: &'static str, err: &std::io::Error, user_path: bool) -> Error {
        Error::Io(IoFailure { path: path.into(), op, kind: err.kind(), detail: err.to_string(), user_path })
    }

    pub fn class(&self) -> ErrorClass {
        match self {
            Error::NoRoot { .. }
            | Error::PathOutsideRoot { .. }
            | Error::Usage(_)
            | Error::Reserved { .. }
            | Error::Config(_) => ErrorClass::Usage,
            Error::Pin(_) | Error::Refused(_) => ErrorClass::Refused,
            Error::Io(f) => {
                if f.kind == std::io::ErrorKind::NotFound && f.user_path {
                    ErrorClass::Usage
                } else {
                    ErrorClass::Internal
                }
            }
            Error::CannotDetermine(_) | Error::Internal(_) => ErrorClass::Internal,
        }
    }

    /// Diagnostics printed like findings (R-ARCH-103); empty for errors that carry none.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Error::Config(c) => &c.diags,
            _ => &[],
        }
    }
}

impl From<ConfigFailure> for Error {
    fn from(f: ConfigFailure) -> Self {
        Error::Config(Box::new(f))
    }
}
```

Verify: thiserror's `.0.path` field-access shorthand inside `#[error("...", .0.path)]` needs thiserror 1.0.5x or 2.x; `RepoPath` implements `Display` (Task 3).

- [ ] **Step 5: Run** `cargo test -p fairlead-core r_arch_100 r_arch_102`. Expected: PASS. **Commit** `feat(core): typed Error, classes and refusals`.

---

### Task 8: time.rs: AsOf

Rules: R-CHK-014 (only time input is `--as-of`, default UTC today), R-ARCH-090 (as-of is explicit), R-ARCH-077 (cache key part).

**Files:**
- Create: `crates/fairlead-core/src/time.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod time;`)

**Interfaces (contract 4.3):** `AsOf(pub jiff::civil::Date)`, `AsOf::parse(&str) -> Result<AsOf, Error>`, `AsOf::today_utc() -> AsOf` (the only clock read in the crate). Additions: `Display` (`YYYY-MM-DD`), `Copy/Ord/Hash` derives.

Test policy: tests never read the clock, so `today_utc` has no test; it is a one-line call exercised by P8's CLI tests with a fixed `--as-of`.

- [ ] **Step 1: Write the failing tests.**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_chk_014__parse_accepts_iso_dates_only() {
        let a = AsOf::parse("2026-10-09").unwrap();
        assert_eq!(a.to_string(), "2026-10-09");
        assert_eq!(a.0.year(), 2026);
        assert_eq!(a.0.month(), 10);
        assert_eq!(a.0.day(), 9);
        for bad in ["20261009", "2026-1-9", "2026-13-01", "2026-02-30", "2027-02-29", "", "today", "2026-10-09T00:00:00Z", " 2026-10-09"] {
            let err = AsOf::parse(bad).unwrap_err();
            assert_eq!(err.class(), crate::error::ErrorClass::Usage, "{bad:?}");
        }
        assert!(AsOf::parse("2028-02-29").is_ok(), "leap day");
    }

    #[test]
    fn r_chk_014__as_of_orders_by_date() {
        assert!(AsOf::parse("2026-10-09").unwrap() < AsOf::parse("2026-10-10").unwrap());
        assert_eq!(AsOf::parse("2026-10-09").unwrap(), AsOf::parse("2026-10-09").unwrap());
    }
}
```

- [ ] **Step 2: Add `pub mod time;`. Run** `cargo test -p fairlead-core r_chk_014`. Expected: FAIL (compile error: `cannot find type AsOf`).

- [ ] **Step 3: Write the implementation.**

```rust
//! The only clock access in the crate (R-CHK-014, contract invariant 9).
use crate::error::Error;
use std::fmt;

/// The reference date of a run (`--as-of`). Date-dependent results and cache entries carry it.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AsOf(pub jiff::civil::Date);

impl AsOf {
    /// Strict `YYYY-MM-DD`; any other shape is a usage error.
    pub fn parse(s: &str) -> Result<Self, Error> {
        let b = s.as_bytes();
        let shape_ok = b.len() == 10
            && b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() });
        if !shape_ok {
            return Err(Error::Usage(format!("--as-of expects YYYY-MM-DD, got {s:?}")));
        }
        s.parse::<jiff::civil::Date>()
            .map(AsOf)
            .map_err(|e| Error::Usage(format!("invalid --as-of date {s:?}: {e}")))
    }

    /// UTC today. The ONLY wall-clock read in fairlead-core.
    /// Verify: `Timestamp::now()`, `Timestamp::to_zoned` and `TimeZone::UTC` exist in jiff 0.2 with `std`.
    pub fn today_utc() -> Self {
        let zoned = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC);
        AsOf(zoned.date())
    }
}

impl fmt::Display for AsOf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

- [ ] **Step 4: Run** `cargo test -p fairlead-core r_chk_014`. Expected: PASS (jiff's `Display` for `civil::Date` prints `2026-10-09`).

- [ ] **Step 5: Commit** `feat(core): AsOf with strict parsing and the single clock read`.

---

### Task 9: config/header.rs: the lenient `[fairlead]` header pass

Rules: R-SYN-038, R-SYN-039 (duplicate keys and wrong types are `HN-C001`), D-33, C-34 (edition half), R-CLI-005 input.

**Files:**
- Create: `crates/fairlead-core/src/config/header.rs`
- Modify: `crates/fairlead-core/src/config/mod.rs` (add `pub mod header; pub use header::{parse_header_lenient, parse_pin_lenient, FairleadHeader};`)

**Interfaces:**
- Consumes: `ScopeSlug::parse`, `fairlead_syntax::normalize::lf_bytes`, `discover::{Edition, Pin, parse_pin_version}`.
- Produces (contract 4.6, fixed): `FairleadHeader { scope, edition: Option<Edition>, root, marker }`, `parse_header_lenient(&[u8]) -> Result<FairleadHeader, ConfigFailure>`. Additions: `parse_pin_lenient(&[u8]) -> Result<(FairleadHeader, Pin), ConfigFailure>` (root file: requires `edition`, reads `[settings] fairlead-version`), and crate-private helpers `decode_control`, `parse_table`, `line_col`, `locate_key`, `error_diag`, `failure`, `toml_failure`, `header_from_table`, `HEADER_KEYS` used by Tasks 10 to 14.

"Lenient" means: the whole file is read as a generic TOML table (so any later-edition table is tolerated), and only `[fairlead]` (and, for the pin, `[settings] fairlead-version`) is interpreted. The failure `path` is the root path; callers re-label with `ConfigFailure::with_path`.

- [ ] **Step 1: Write the failing tests.** Create `header.rs` with only this module.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;

    const ROOT: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n";

    #[test]
    fn r_syn_038__header_reads_all_four_frozen_keys() {
        let h = parse_header_lenient(ROOT.as_bytes()).unwrap();
        assert_eq!(h.scope.as_str(), "root");
        assert_eq!(h.edition, Some(Edition(1)));
        assert!(h.root);
        assert_eq!(h.marker.as_deref(), Some("fairlead"));
    }

    #[test]
    fn r_syn_038__non_root_header_carries_only_scope() {
        let h = parse_header_lenient(b"[fairlead]\nscope = \"web\"\n").unwrap();
        assert_eq!((h.edition, h.root, h.marker), (None, false, None));
    }

    #[test]
    fn r_syn_038__header_requires_a_valid_scope() {
        for bad in ["[fairlead]\n", "[fairlead]\nscope = 3\n", "[fairlead]\nscope = \"Bad_Scope\"\n", "[fairlead]\nscope = \"con\"\n", "[settings]\nfolder = \"fairlead\"\n"] {
            let f = parse_header_lenient(bad.as_bytes()).unwrap_err();
            assert_eq!(f.diags[0].code, codes::HN_C001, "{bad:?}");
        }
    }

    #[test]
    fn r_syn_038__wrong_types_in_header_are_hc001() {
        for bad in [
            "[fairlead]\nscope = \"a\"\nedition = \"1\"\n",
            "[fairlead]\nscope = \"a\"\nedition = -1\n",
            "[fairlead]\nscope = \"a\"\nroot = \"yes\"\n",
            "[fairlead]\nscope = \"a\"\nmarker = \"\"\n",
            "[fairlead]\nscope = \"a\"\nmarker = \"has space\"\n",
        ] {
            assert!(parse_header_lenient(bad.as_bytes()).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn r_syn_038__lenient_pass_ignores_later_tables_and_unknown_header_keys() {
        let text = "[fairlead]\nscope = \"a\"\nedition = 9\nfuture-key = [1, 2]\n\n[settings]\nfairlead-version = 5\n\n[future.table]\nx = 1\n";
        let h = parse_header_lenient(text.as_bytes()).unwrap();
        assert_eq!(h.edition, Some(Edition(9)), "a later edition still yields its header (D-33)");
    }

    #[test]
    fn r_syn_039__duplicate_keys_are_a_parse_error_with_a_line() {
        let f = parse_header_lenient(b"[fairlead]\nscope = \"a\"\nscope = \"b\"\n").unwrap_err();
        assert_eq!(f.diags[0].code, codes::HN_C001);
        assert!(f.diags[0].location.line >= 2, "{:?}", f.diags[0].location);
    }

    #[test]
    fn r_syn_039__invalid_toml_is_hc001() {
        let f = parse_header_lenient(b"[fairlead\nscope = \n").unwrap_err();
        assert_eq!(f.diags[0].code, codes::HN_C001);
        assert!(f.diags[0].message.contains("TOML"), "{}", f.diags[0].message);
    }

    #[test]
    fn r_syn_038__bom_crlf_and_non_utf8() {
        let crlf = format!("\u{feff}{}", ROOT.replace('\n', "\r\n"));
        assert!(parse_header_lenient(crlf.as_bytes()).is_ok());
        let f = parse_header_lenient(b"[fairlead]\nscope = \"a\xFF\"\n").unwrap_err();
        assert!(f.diags[0].message.contains("UTF-8"), "{}", f.diags[0].message);
    }

    #[test]
    fn r_syn_038__pin_needs_edition_and_pads_the_version() {
        let text = format!("{ROOT}\n[settings]\nfairlead-version = \"0.1\"\n");
        let (h, pin) = parse_pin_lenient(text.as_bytes()).unwrap();
        assert!(h.root);
        assert_eq!(pin.edition, Edition(1));
        assert_eq!(pin.version, Some(semver::Version::new(0, 1, 0)));
        let (_, none) = parse_pin_lenient(ROOT.as_bytes()).unwrap();
        assert_eq!(none.version, None);
        let f = parse_pin_lenient(b"[fairlead]\nscope = \"root\"\nroot = true\n").unwrap_err();
        assert!(f.diags[0].message.contains("edition"), "C-34: a root without edition is HN-C001");
        let f = parse_pin_lenient(format!("{ROOT}\n[settings]\nfairlead-version = \"abc\"\n").as_bytes()).unwrap_err();
        assert!(f.diags[0].message.contains("fairlead-version"));
    }

    #[test]
    fn r_syn_039__line_col_counts_scalar_values() {
        assert_eq!(line_col("ab\ncd", 4), (2, 2));
        assert_eq!(line_col("é=1\nx", 4), (1, 4), "a multi-byte char counts once");
        assert_eq!(locate_key("[a]\nfoo = 1\n[b]\nbar = 2\n", "b.bar"), 4);
        assert_eq!(locate_key("x = 1\n", "missing"), 1);
    }
}
```

- [ ] **Step 2: Add the `pub mod header;` line to `config/mod.rs`. Run** `cargo test -p fairlead-core r_syn_038 r_syn_039`. Expected: FAIL (compile error: `cannot find function parse_header_lenient`).

- [ ] **Step 3: Write the implementation** (above the tests).

```rust
//! Frozen `[fairlead]` header, parsed in a first lenient pass (D-33, R-SYN-038).
use crate::config::ConfigFailure;
use crate::diag::{codes, Diagnostic, Location, Severity};
use crate::discover::{parse_pin_version, Edition, Pin};
use crate::path::RepoPath;
use fairlead_syntax::ids::ScopeSlug;

/// The four frozen keys of `[fairlead]`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FairleadHeader {
    pub scope: ScopeSlug,
    pub edition: Option<Edition>,
    pub root: bool,
    pub marker: Option<String>,
}

pub(crate) const HEADER_KEYS: [&str; 4] = ["scope", "edition", "root", "marker"];

/// BOM stripped, CRLF and CR to LF, must be UTF-8.
pub(crate) fn decode_control(bytes: &[u8]) -> Result<String, String> {
    let lf = fairlead_syntax::normalize::lf_bytes(bytes);
    String::from_utf8(lf.into_owned())
        .map_err(|e| format!("control file is not valid UTF-8 (first bad byte at {})", e.utf8_error().valid_up_to()))
}

/// 1-based line and column (scalar values) of a byte offset.
pub(crate) fn line_col(text: &str, offset: usize) -> (u32, u32) {
    let (mut line, mut col) = (1u32, 1u32);
    for (i, c) in text.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Best-effort 1-based line of a dotted key path: the first `key =` line or table header that
/// mentions the last segment. `toml::Table` carries no spans, so this is a documented heuristic.
pub(crate) fn locate_key(text: &str, dotted: &str) -> u32 {
    let last = dotted.rsplit('.').next().unwrap_or(dotted);
    if last.is_empty() {
        return 1;
    }
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim_start();
        let bare = line.strip_prefix(last).is_some_and(|rest| rest.trim_start().starts_with('='));
        let quoted = line
            .strip_prefix('"')
            .and_then(|r| r.strip_prefix(last))
            .is_some_and(|r| r.starts_with('"'));
        let header = line.starts_with('[') && line.contains(last);
        if bare || quoted || header {
            return u32::try_from(i + 1).unwrap_or(1);
        }
    }
    1
}

pub(crate) fn error_diag(line: u32, col: u32, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(codes::HN_C001, Severity::Error, Location::at(RepoPath::root(), line, col), message)
        .with_hint("fix the control file; `fairlead explain syntax` shows the schema")
}

/// A failure at the line of `key` (path is the root placeholder; callers use `with_path`).
pub(crate) fn failure(text: &str, key: &str, message: impl Into<String>) -> ConfigFailure {
    ConfigFailure { path: RepoPath::root(), diags: vec![error_diag(locate_key(text, key), 1, message)] }
}

pub(crate) fn toml_failure(text: &str, e: &toml::de::Error) -> ConfigFailure {
    let (line, col) = e.span().map(|s| line_col(text, s.start)).unwrap_or((1, 1));
    ConfigFailure {
        path: RepoPath::root(),
        diags: vec![error_diag(line, col, format!("control file is not valid TOML: {}", e.message()))],
    }
}

/// Decode and parse into a generic table (any later-edition content is tolerated).
pub(crate) fn parse_table(bytes: &[u8]) -> Result<(String, toml::Table), ConfigFailure> {
    let text = decode_control(bytes).map_err(|m| failure("", "", m))?;
    let table = text.parse::<toml::Table>().map_err(|e| toml_failure(&text, &e))?;
    Ok((text, table))
}

pub(crate) fn header_from_table(text: &str, h: &toml::Table) -> Result<FairleadHeader, ConfigFailure> {
    let scope_s = match h.get("scope") {
        Some(toml::Value::String(s)) => s.as_str(),
        Some(_) => return Err(failure(text, "fairlead.scope", "[fairlead] scope must be a string")),
        None => return Err(failure(text, "fairlead", "[fairlead] scope is required in every control file")),
    };
    let scope = ScopeSlug::parse(scope_s)
        .map_err(|e| failure(text, "fairlead.scope", format!("invalid scope id {scope_s:?}: {e:?}")))?;
    let edition = match h.get("edition") {
        None => None,
        Some(toml::Value::Integer(n)) => match u32::try_from(*n) {
            Ok(v) => Some(Edition(v)),
            Err(_) => return Err(failure(text, "fairlead.edition", "edition must be a non-negative integer")),
        },
        Some(_) => return Err(failure(text, "fairlead.edition", "edition must be an integer such as 1")),
    };
    let root = match h.get("root") {
        None => false,
        Some(toml::Value::Boolean(b)) => *b,
        Some(_) => return Err(failure(text, "fairlead.root", "root must be true or false")),
    };
    let marker = match h.get("marker") {
        None => None,
        Some(toml::Value::String(m))
            if !m.is_empty() && m.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') =>
        {
            Some(m.clone())
        }
        Some(_) => return Err(failure(text, "fairlead.marker", "marker must be a non-empty word (letters, digits, - and _)")),
    };
    Ok(FairleadHeader { scope, edition, root, marker })
}

fn fairlead_table<'a>(text: &str, t: &'a toml::Table) -> Result<&'a toml::Table, ConfigFailure> {
    match t.get("fairlead") {
        Some(toml::Value::Table(h)) => Ok(h),
        _ => Err(failure(text, "fairlead", "a control file must contain a [fairlead] header table")),
    }
}

/// First, lenient pass: only `[fairlead]` is interpreted (D-33).
pub fn parse_header_lenient(bytes: &[u8]) -> Result<FairleadHeader, ConfigFailure> {
    let (text, table) = parse_table(bytes)?;
    let h = fairlead_table(&text, &table)?;
    header_from_table(&text, h)
}

/// Root-file variant used by discovery before anything else (R-ARCH-013): the header plus the pin.
/// A root without `edition` is `HN-C001` (C-34).
pub fn parse_pin_lenient(bytes: &[u8]) -> Result<(FairleadHeader, Pin), ConfigFailure> {
    let (text, table) = parse_table(bytes)?;
    let header = header_from_table(&text, fairlead_table(&text, &table)?)?;
    let Some(edition) = header.edition else {
        return Err(failure(&text, "fairlead", "the root control file requires `edition` in [fairlead]"));
    };
    let version = match table.get("settings") {
        Some(toml::Value::Table(s)) => match s.get("fairlead-version") {
            None => None,
            Some(toml::Value::String(v)) => Some(parse_pin_version(v).ok_or_else(|| {
                failure(&text, "settings.fairlead-version", format!("fairlead-version {v:?} must look like \"0.1\" or \"0.1.0\""))
            })?),
            Some(_) => return Err(failure(&text, "settings.fairlead-version", "fairlead-version must be a string")),
        },
        _ => None,
    };
    Ok((header, Pin { version, edition }))
}
```

Verify: `toml::de::Error::span()` and `message()` exist in toml 1.x; if `message()` is absent use `e.to_string()`. In the test `line_col("é=1\nx", 4)` offset 4 is the byte of `\n` (é is 2 bytes), so three scalar values precede it and the column is 4.

- [ ] **Step 4: Run** `cargo test -p fairlead-core r_syn_038 r_syn_039`. Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): lenient [fairlead] header pass and pin extraction`.

---

### Task 10: config/schema.rs (part 1): typed schema, reader helpers, simple tables

Rules: R-SYN-038/039/040/051, R-ARCH-021 (default excludes are config, not code), D-33, C-4.

**Files:**
- Create: `crates/fairlead-core/src/config/schema.rs`
- Modify: `crates/fairlead-core/src/config/mod.rs` (`pub mod schema; pub use schema::*;`)

**Interfaces (contract 4.6, names fixed):** `Policy`, `Stage`, `OwnerKind`, `Owner`, `Settings`, `ScanConfig`, `KindDecl`, `FileKindDecl`, `Layout`, `FileItemDecl`, `VendorDecl`, `OvrDecl`, `ContextConfig`, `BaselineConfig`, `SecretsConfig`, `ExceptionsConfig`, `EligibilityConfig`, `ConventionDecl`, `UnknownKey`. In this task: all type definitions and the parsers for `[settings]`, `[scan]`, `[comments]`, `[agents]`, `[layout]`. Tasks 11 and 12 add parsers for the rest.

Choices fixed here (see Contract issues): `OvrOp` is defined in `schema.rs` (the config types need it; P4's `registry/ovr.rs` re-exports it); the contract's `Layout.gen` is written `r#gen` because `gen` is a reserved keyword in edition 2024; auxiliary types the contract leaves to the owner get the obvious fields: `UnknownKey { path, line }`, `KeyDecl { ty, values }`, `IndexValues`, `KindShape`, `ImplLimit`, `SevWeight`, `ConventionAction`.

- [ ] **Step 1: Write the failing tests** (new `schema.rs` containing the types' tests only; the helpers they use are defined in Step 3).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;

    fn cx(text: &str) -> (Ctx<'_>, toml::Table) {
        let table: toml::Table = text.parse().unwrap();
        (Ctx::new(&RepoPath::new("fairlead/control.toml").unwrap(), text), table)
    }

    #[test]
    fn r_arch_021__defaults_for_scan_live_in_config_not_in_code() {
        let (mut c, mut t) = cx("");
        let s = parse_scan(&mut c, &mut t);
        assert!(s.exclude.is_empty(), "no hard-coded default excludes: `init` writes them into control.toml");
        assert_eq!((s.max_file, s.max_directives), (2_000_000, 500));
        assert!(c.diags.is_empty() && c.unknown.is_empty());
    }

    #[test]
    fn r_syn_039__scan_values_parse() {
        let text = "[scan]\nexclude = [\"target/\", \"node_modules/\"]\nskip = [\"tests/fixtures/**\"]\nheader-skip = [\"^<\\\\?php\"]\nmax-file = 1000\nmax-directives = 7\n";
        let (mut c, mut t) = cx(text);
        let s = parse_scan(&mut c, &mut t);
        assert_eq!(s.exclude, ["target/", "node_modules/"]);
        assert_eq!(s.skip, ["tests/fixtures/**"]);
        assert_eq!(s.header_skip.len(), 1);
        assert_eq!((s.max_file, s.max_directives), (1000, 7));
        assert!(c.diags.is_empty());
    }

    #[test]
    fn r_syn_039__unknown_keys_are_collected_not_fatal() {
        let (mut c, mut t) = cx("[scan]\nbogus = 1\nexclude = []\n");
        let _ = parse_scan(&mut c, &mut t);
        assert!(c.diags.is_empty());
        assert_eq!(c.unknown, vec![UnknownKey { path: "scan.bogus".into(), line: 2 }]);
    }

    #[test]
    fn r_syn_039__wrong_type_is_hc001() {
        let (mut c, mut t) = cx("[scan]\nexclude = \"target/\"\nmax-file = -5\n");
        let _ = parse_scan(&mut c, &mut t);
        assert_eq!(c.diags.len(), 2);
        assert!(c.diags.iter().all(|d| d.code == codes::HN_C001));
    }

    #[test]
    fn r_syn_051__settings_use_fairlead_range_not_requires() {
        let (mut c, mut t) = cx("[settings]\nfairlead-range = \">=0.1\"\nrequires = \"x\"\nfairlead-version = \"0.1\"\nfolder = \"fairlead\"\nindex-values = \"hash\"\npurpose = \"Sync app\"\n");
        let s = parse_settings(&mut c, &mut t);
        assert_eq!(s.fairlead_range.as_deref(), Some(">=0.1"));
        assert_eq!(s.fairlead_version.as_deref(), Some("0.1"));
        assert_eq!(s.index_values, IndexValues::Hash);
        assert_eq!(s.purpose.as_deref(), Some("Sync app"));
        assert_eq!(c.unknown.iter().map(|u| u.path.as_str()).collect::<Vec<_>>(), ["settings.requires"]);
        assert!(c.diags.is_empty());
    }

    #[test]
    fn r_syn_038__settings_owners_accept_handles_and_inline_tables() {
        let (mut c, mut t) = cx("[settings]\nowners = [\"@platform\", { handle = \"@bot\", kind = \"agent\" }, { handle = \"@d\", kind = \"human\" }]\n");
        let s = parse_settings(&mut c, &mut t);
        assert_eq!(
            s.owners,
            vec![
                Owner { handle: "@platform".into(), kind: OwnerKind::Human },
                Owner { handle: "@bot".into(), kind: OwnerKind::Agent },
                Owner { handle: "@d".into(), kind: OwnerKind::Human },
            ]
        );
        assert!(c.diags.is_empty());
        let (mut c2, mut t2) = cx("[settings]\nowners = [{ handle = \"@x\", kind = \"robot\" }, 5]\n");
        let _ = parse_settings(&mut c2, &mut t2);
        assert_eq!(c2.diags.len(), 2);
    }

    #[test]
    fn r_syn_038__settings_purpose_is_at_most_160_chars_and_index_values_is_checked() {
        let long = "x".repeat(161);
        let text = format!("[settings]\npurpose = \"{long}\"\nindex-values = \"both\"\nfairlead-version = \"zzz\"\n");
        let (mut c, mut t) = cx(&text);
        let _ = parse_settings(&mut c, &mut t);
        assert_eq!(c.diags.len(), 3);
    }

    #[test]
    fn r_syn_040__layout_names_and_paths_get_hygiene() {
        let (mut c, mut t) = cx("[layout]\ngen = \"CON\"\ndocs = \"a/b\"\nref = \"reference\"\n");
        let l = parse_layout(&mut c, &mut t);
        assert_eq!(c.diags.len(), 2, "reserved name and multi-segment name are errors");
        assert!(c.diags.iter().any(|d| d.code == codes::HN_P001));
        assert_eq!(l.reference, "reference");
        assert_eq!(l.contract, "contract", "unset keys keep their defaults (R-FILE-020)");
    }

    #[test]
    fn r_syn_040__scan_patterns_are_nfc_and_forward_slashes() {
        let (mut c, mut t) = cx("[scan]\nexclude = [\"build\\\\out/\", \"cafe\\u0301/\"]\n");
        let s = parse_scan(&mut c, &mut t);
        assert_eq!(s.exclude, ["build/out/", "caf\u{e9}/"]);
    }

    #[test]
    fn r_syn_039__comments_and_agents_tables() {
        let (mut c, mut t) = cx("[comments]\n\".Vue\" = [\"<!-- -->\", \"//\"]\nrs = []\n\n[agents]\ntargets = [\"AGENTS.md\", \"CLAUDE.md\", \"../x\"]\n");
        let m = parse_comments(&mut c, &mut t);
        assert_eq!(m.get("vue").unwrap(), &vec!["<!-- -->".to_string(), "//".to_string()]);
        let a = parse_agents(&mut c, &mut t);
        assert_eq!(a, ["AGENTS.md", "CLAUDE.md"]);
        assert_eq!(c.diags.len(), 2, "empty leader list and a `..` target are errors");
    }

    #[test]
    fn r_syn_041__policy_and_stage_orders() {
        assert!(Policy::Open < Policy::NeedsException && Policy::NeedsException < Policy::Sealed);
        assert!(Stage::Observe < Stage::Warn && Stage::Warn < Stage::Enforce);
    }
}
```

- [ ] **Step 2: Add `pub mod schema; pub use schema::*;` to `config/mod.rs`. Run** `cargo test -p fairlead-core r_syn_039 r_syn_051 r_arch_021 r_syn_040`. Expected: FAIL (compile error: `cannot find struct Ctx`).

- [ ] **Step 3: Write the implementation** (above the tests).

```rust
//! Typed control-file schema and the reader helpers (12-directive-syntax schema index, 0.1 rows).
use crate::config::header::locate_key;
use crate::diag::{codes, DiagCode, Diagnostic, Location, Severity};
use crate::path::RepoPath;
use fairlead_syntax::directive::ArgValue;
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

// ---------------------------------------------------------------- vocabulary types
/// Override policy; "stricter" order Open < NeedsException < Sealed (R-SCOPE-030).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Policy {
    Open,
    NeedsException,
    Sealed,
}

/// The ordered ramp observe, warn, enforce (R-CHK-033).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Stage {
    Observe,
    Warn,
    Enforce,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OwnerKind {
    Human,
    Agent,
}

/// R-SCOPE-040. Defined here because the control-file `[[ovr]]` table needs it; the registry
/// re-exports it (contract issue 3).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum OvrOp {
    Loosen,
    Tighten,
    Set,
    Replace,
    Add,
    Remove,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Owner {
    pub handle: String,
    pub kind: OwnerKind,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum IndexValues {
    #[default]
    Value,
    Hash,
}

/// `[settings]`; a non-root file may set only `purpose` (C-4).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Settings {
    pub folder: Option<String>,
    pub fairlead_version: Option<String>,
    pub fairlead_range: Option<String>,
    pub index_values: IndexValues,
    pub purpose: Option<String>,
    pub owners: Vec<Owner>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScanConfig {
    pub exclude: Vec<String>,
    pub skip: Vec<String>,
    pub header_skip: Vec<String>,
    pub max_file: u64,
    pub max_directives: u32,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self { exclude: Vec::new(), skip: Vec::new(), header_skip: Vec::new(), max_file: 2_000_000, max_directives: 500 }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum KindShape {
    Doc,
    Code,
    #[default]
    Either,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ImplLimit {
    #[default]
    Any,
    None,
    N(u32),
}

/// One `[kinds.K.keys]` entry, for example `layer = { type = "enum", values = [..] }`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KeyDecl {
    pub ty: String,
    pub values: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KindDecl {
    pub title: Option<String>,
    pub shape: KindShape,
    pub impl_limit: ImplLimit,
    pub unused: Option<Severity>,
    pub dir: Option<String>,
    pub source_generate: bool,
    pub keys: BTreeMap<String, KeyDecl>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileKindDecl {
    pub extractor: String,
    pub binding: String,
    pub import_template: Option<String>,
}

/// Sub-folder names of a managerial folder (`[layout]`, R-FILE-020). `gen` is a reserved word in
/// edition 2024, hence `r#gen`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Layout {
    pub docs: String,
    pub contract: String,
    pub r#gen: String,
    pub ledger: String,
    pub reference: String,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            docs: "docs".into(),
            contract: "contract".into(),
            r#gen: "gen".into(),
            ledger: "ledger".into(),
            reference: "ref".into(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileItemDecl {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub extends: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VendorDecl {
    pub path: String,
    pub tool: String,
}

/// `[[ovr]]`: the keys of the `ovr` directive. `set` holds every other key as a field assignment;
/// a key written with a trailing `+` (`"tags+" = [..]`) is the append form `k+=` (TOML cannot spell
/// `+=`); P4 interprets it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OvrDecl {
    pub target: String,
    pub op: OvrOp,
    pub applies: Option<String>,
    pub set: BTreeMap<String, ArgValue>,
    pub why: Option<String>,
    pub owner: Option<String>,
    pub exp: Option<String>,
    pub base: Option<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SevWeight {
    pub error: u32,
    pub warn: u32,
    pub info: u32,
}

impl Default for SevWeight {
    fn default() -> Self {
        Self { error: 4, warn: 2, info: 1 }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContextConfig {
    pub budget_start: u32,
    pub budget_path: u32,
    pub bytes_per_token: u32,
    pub tokenizer: Option<String>,
    pub sev_weight: SevWeight,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self { budget_start: 500, budget_path: 700, bytes_per_token: 4, tokenizer: None, sev_weight: SevWeight::default() }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BaselineConfig {
    /// Default `<folder>/gen/baseline.txt`, filled in by `parse_control` once the folder is known.
    pub file: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SecretsConfig {
    pub allow: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExceptionsConfig {
    pub max_days: Option<u32>,
    pub max_live_per_rule: u32,
}

impl Default for ExceptionsConfig {
    fn default() -> Self {
        Self { max_days: None, max_live_per_rule: 10 }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EligibilityConfig {
    pub min_lines: u32,
    pub min_files: u32,
}

impl Default for EligibilityConfig {
    fn default() -> Self {
        Self { min_lines: 5, min_files: 3 }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConventionAction {
    Transparent,
    Class(crate::classify::FileClass),
    /// 0.2; never produced by a 0.1 parse (a 0.1 build reports `own` as an unknown key).
    Own(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConventionDecl {
    pub glob: String,
    pub action: ConventionAction,
}

/// An unknown control key or table (`HN-C002`, R-SYN-039). `line` is a best-effort 1-based line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnknownKey {
    pub path: String,
    pub line: u32,
}

// ---------------------------------------------------------------- reader context
pub(crate) fn join_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// Collects hard errors (`HN-C001`, `HN-P001`) and unknown keys while the tables are consumed.
pub(crate) struct Ctx<'a> {
    pub path: RepoPath,
    pub text: &'a str,
    pub diags: Vec<Diagnostic>,
    pub unknown: Vec<UnknownKey>,
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(path: &RepoPath, text: &'a str) -> Self {
        Self { path: path.clone(), text, diags: Vec::new(), unknown: Vec::new() }
    }

    pub(crate) fn error(&mut self, key: &str, message: impl Into<String>) {
        self.error_code(codes::HN_C001, key, message);
    }

    pub(crate) fn error_code(&mut self, code: DiagCode, key: &str, message: impl Into<String>) {
        let line = locate_key(self.text, key);
        self.diags.push(
            Diagnostic::new(code, Severity::Error, Location::at(self.path.clone(), line, 1), message)
                .with_hint("fix the control file; `fairlead explain syntax` shows the schema"),
        );
    }

    /// Everything left in `rest` was not consumed: unknown (R-SYN-039).
    pub(crate) fn unknown_in(&mut self, prefix: &str, rest: &toml::Table) {
        for key in rest.keys() {
            let path = join_key(prefix, key);
            let line = locate_key(self.text, &path);
            self.unknown.push(UnknownKey { path, line });
        }
    }
}

pub(crate) fn take_string(cx: &mut Ctx<'_>, t: &mut toml::Table, prefix: &str, key: &str) -> Option<String> {
    match t.remove(key) {
        None => None,
        Some(toml::Value::String(s)) => Some(s),
        Some(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` must be a string"));
            None
        }
    }
}

pub(crate) fn take_bool(cx: &mut Ctx<'_>, t: &mut toml::Table, prefix: &str, key: &str) -> Option<bool> {
    match t.remove(key) {
        None => None,
        Some(toml::Value::Boolean(b)) => Some(b),
        Some(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` must be true or false"));
            None
        }
    }
}

pub(crate) fn take_u32(cx: &mut Ctx<'_>, t: &mut toml::Table, prefix: &str, key: &str) -> Option<u32> {
    take_u64(cx, t, prefix, key).and_then(|v| match u32::try_from(v) {
        Ok(v) => Some(v),
        Err(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` is too large"));
            None
        }
    })
}

pub(crate) fn take_u64(cx: &mut Ctx<'_>, t: &mut toml::Table, prefix: &str, key: &str) -> Option<u64> {
    match t.remove(key) {
        None => None,
        Some(toml::Value::Integer(n)) => match u64::try_from(n) {
            Ok(v) => Some(v),
            Err(_) => {
                let k = join_key(prefix, key);
                cx.error(&k, format!("`{k}` must be a non-negative integer"));
                None
            }
        },
        Some(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` must be an integer"));
            None
        }
    }
}

pub(crate) fn string_array(v: &toml::Value) -> Option<Vec<String>> {
    v.as_array()?.iter().map(|x| x.as_str().map(str::to_string)).collect()
}

pub(crate) fn take_string_list(cx: &mut Ctx<'_>, t: &mut toml::Table, prefix: &str, key: &str) -> Option<Vec<String>> {
    match t.remove(key) {
        None => None,
        Some(v) => match string_array(&v) {
            Some(list) => Some(list),
            None => {
                let k = join_key(prefix, key);
                cx.error(&k, format!("`{k}` must be a list of strings"));
                None
            }
        },
    }
}

pub(crate) fn take_table(cx: &mut Ctx<'_>, t: &mut toml::Table, prefix: &str, key: &str) -> Option<toml::Table> {
    match t.remove(key) {
        None => None,
        Some(toml::Value::Table(x)) => Some(x),
        Some(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` must be a table"));
            None
        }
    }
}

/// `[[name]]` arrays: every element must be a table.
pub(crate) fn take_table_array(cx: &mut Ctx<'_>, t: &mut toml::Table, prefix: &str, key: &str) -> Vec<toml::Table> {
    let k = join_key(prefix, key);
    match t.remove(key) {
        None => Vec::new(),
        Some(toml::Value::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    toml::Value::Table(tb) => out.push(tb),
                    _ => cx.error(&k, format!("every `{k}` entry must be a table")),
                }
            }
            out
        }
        Some(_) => {
            cx.error(&k, format!("`{k}` must be an array of tables"));
            Vec::new()
        }
    }
}

/// A date given as a quoted string or a bare TOML date; returned as `YYYY-MM-DD` text, validated.
pub(crate) fn take_date_string(cx: &mut Ctx<'_>, t: &mut toml::Table, prefix: &str, key: &str) -> Option<String> {
    let k = join_key(prefix, key);
    let raw = match t.remove(key) {
        None => return None,
        Some(toml::Value::String(s)) => s,
        Some(toml::Value::Datetime(d)) => d.to_string(),
        Some(_) => {
            cx.error(&k, format!("`{k}` must be a date such as 2026-12-31"));
            return None;
        }
    };
    match raw.parse::<jiff::civil::Date>() {
        Ok(_) if raw.len() == 10 => Some(raw),
        _ => {
            cx.error(&k, format!("`{k}` must be a date YYYY-MM-DD, got {raw:?}"));
            None
        }
    }
}

/// NFC and `/` separators; rejects `..`. Used for scan patterns (gitignore syntax: leading `/`,
/// trailing `/` and `!` are legal).
pub(crate) fn clean_pattern(cx: &mut Ctx<'_>, key: &str, raw: &str) -> Option<String> {
    let s: String = raw.replace('\\', "/").nfc().collect();
    if s.split('/').any(|seg| seg == "..") {
        cx.error(key, format!("pattern {raw:?} must not contain `..`"));
        return None;
    }
    Some(s)
}

/// Declared repo-relative paths and globs: NFC, `/` separators, no `..`, no leading `/`, and no
/// Windows-reserved literal segment (`HN-P001`, R-SYN-040).
pub(crate) fn clean_rel_path(cx: &mut Ctx<'_>, key: &str, raw: &str) -> Option<String> {
    let s: String = raw.replace('\\', "/").nfc().collect();
    let s = s.strip_prefix("./").unwrap_or(&s).to_string();
    if s.is_empty() || s.starts_with('/') || s.split('/').any(|seg| seg == "..") {
        cx.error(key, format!("path {raw:?} must be non-empty, relative and free of `..`"));
        return None;
    }
    for seg in s.split('/') {
        if seg.is_empty() || seg.contains(['*', '?', '[', '{']) {
            continue;
        }
        let lower = seg.to_lowercase();
        let stem = lower.split('.').next().unwrap_or("");
        if fairlead_syntax::ids::is_reserved_windows_name(&lower) || fairlead_syntax::ids::is_reserved_windows_name(stem) {
            cx.error_code(codes::HN_P001, key, format!("path segment {seg:?} is a Windows-reserved name"));
            return None;
        }
    }
    Some(s)
}

/// A glob that must compile (globset syntax, `/` is literal); invalid is `HN-C001` at load so that
/// classification (`ClassifyRules`) can be infallible.
pub(crate) fn check_glob(cx: &mut Ctx<'_>, key: &str, pat: &str) -> bool {
    let probe = if pat.ends_with('/') { format!("{pat}**") } else { pat.to_string() };
    match globset::GlobBuilder::new(&probe).literal_separator(true).build() {
        Ok(_) => true,
        Err(e) => {
            cx.error(key, format!("invalid glob {pat:?}: {e}"));
            false
        }
    }
}

/// One folder name: a clean relative path with exactly one segment.
pub(crate) fn clean_segment(cx: &mut Ctx<'_>, key: &str, raw: &str) -> Option<String> {
    let s = clean_rel_path(cx, key, raw)?;
    if s.contains('/') {
        cx.error(key, format!("{raw:?} must be a single folder name"));
        return None;
    }
    Some(s)
}

// ---------------------------------------------------------------- simple tables
pub(crate) fn parse_scan(cx: &mut Ctx<'_>, root: &mut toml::Table) -> ScanConfig {
    let mut out = ScanConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "scan") {
        if let Some(list) = take_string_list(cx, &mut t, "scan", "exclude") {
            out.exclude = list.iter().filter_map(|p| clean_pattern(cx, "scan.exclude", p)).collect();
        }
        if let Some(list) = take_string_list(cx, &mut t, "scan", "skip") {
            out.skip = list
                .iter()
                .filter_map(|p| clean_pattern(cx, "scan.skip", p).filter(|s| check_glob(cx, "scan.skip", s)))
                .collect();
        }
        if let Some(list) = take_string_list(cx, &mut t, "scan", "header-skip") {
            out.header_skip = list;
        }
        if let Some(v) = take_u64(cx, &mut t, "scan", "max-file") {
            out.max_file = v;
        }
        if let Some(v) = take_u32(cx, &mut t, "scan", "max-directives") {
            out.max_directives = v;
        }
        cx.unknown_in("scan", &t);
    }
    out
}

fn take_owners(cx: &mut Ctx<'_>, t: &mut toml::Table) -> Vec<Owner> {
    let Some(v) = t.remove("owners") else { return Vec::new() };
    let Some(items) = v.as_array() else {
        cx.error("settings.owners", "`settings.owners` must be a list of handles");
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        match item {
            toml::Value::String(h) if !h.is_empty() => out.push(Owner { handle: h.clone(), kind: OwnerKind::Human }),
            toml::Value::Table(tb) => {
                let handle = tb.get("handle").and_then(toml::Value::as_str).filter(|h| !h.is_empty());
                let kind = match tb.get("kind").and_then(toml::Value::as_str) {
                    None | Some("human") => Some(OwnerKind::Human),
                    Some("agent") => Some(OwnerKind::Agent),
                    Some(_) => None,
                };
                match (handle, kind) {
                    (Some(h), Some(k)) => out.push(Owner { handle: h.to_string(), kind: k }),
                    _ => cx.error("settings.owners", "an owners entry needs a `handle` and kind \"human\" or \"agent\""),
                }
            }
            _ => cx.error("settings.owners", "an owners entry is a handle string or { handle, kind }"),
        }
    }
    out
}

pub(crate) fn parse_settings(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Settings {
    let mut out = Settings::default();
    if let Some(mut t) = take_table(cx, root, "", "settings") {
        if let Some(f) = take_string(cx, &mut t, "settings", "folder") {
            out.folder = clean_segment(cx, "settings.folder", &f);
        }
        if let Some(v) = take_string(cx, &mut t, "settings", "fairlead-version") {
            if crate::discover::parse_pin_version(&v).is_some() {
                out.fairlead_version = Some(v);
            } else {
                cx.error("settings.fairlead-version", format!("fairlead-version {v:?} must look like \"0.1\" or \"0.1.0\""));
            }
        }
        out.fairlead_range = take_string(cx, &mut t, "settings", "fairlead-range");
        if let Some(v) = take_string(cx, &mut t, "settings", "index-values") {
            match v.as_str() {
                "value" => out.index_values = IndexValues::Value,
                "hash" => out.index_values = IndexValues::Hash,
                _ => cx.error("settings.index-values", "index-values must be \"value\" or \"hash\""),
            }
        }
        if let Some(p) = take_string(cx, &mut t, "settings", "purpose") {
            if p.chars().count() > 160 {
                cx.error("settings.purpose", "purpose is at most 160 characters");
            } else {
                out.purpose = Some(p);
            }
        }
        out.owners = take_owners(cx, &mut t);
        cx.unknown_in("settings", &t);
    }
    out
}

pub(crate) fn parse_comments(cx: &mut Ctx<'_>, root: &mut toml::Table) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    if let Some(t) = take_table(cx, root, "", "comments") {
        for (ext, v) in t {
            let key = join_key("comments", &ext);
            match string_array(&v) {
                Some(list) if !list.is_empty() => {
                    let mut ok = true;
                    for spec in &list {
                        // Verify: `Leader::parse` is a P2 addition (see Contract issues, item 5).
                        if let Err(e) = fairlead_syntax::delimiters::Leader::parse(spec) {
                            cx.error(&key, format!("invalid comment leader {spec:?}: {e:?}"));
                            ok = false;
                        }
                    }
                    if ok {
                        out.insert(ext.trim_start_matches('.').to_ascii_lowercase(), list);
                    }
                }
                _ => cx.error(&key, format!("`{key}` must be a non-empty list of comment leaders")),
            }
        }
    }
    out
}

pub(crate) fn parse_agents(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(mut t) = take_table(cx, root, "", "agents") {
        if let Some(list) = take_string_list(cx, &mut t, "agents", "targets") {
            out = list.iter().filter_map(|p| clean_rel_path(cx, "agents.targets", p)).collect();
        }
        cx.unknown_in("agents", &t);
    }
    out
}

pub(crate) fn parse_layout(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Layout {
    let mut out = Layout::default();
    if let Some(mut t) = take_table(cx, root, "", "layout") {
        for (key, slot) in [
            ("docs", &mut out.docs),
            ("contract", &mut out.contract),
            ("gen", &mut out.r#gen),
            ("ledger", &mut out.ledger),
            ("ref", &mut out.reference),
        ] {
            if let Some(v) = take_string(cx, &mut t, "layout", key) {
                if let Some(clean) = clean_segment(cx, &join_key("layout", key), &v) {
                    *slot = clean;
                }
            }
        }
        cx.unknown_in("layout", &t);
    }
    out
}
```

Verify: the contract's `ConventionAction::Class(FileClass)` makes `schema.rs` depend on `classify::FileClass`; Task 18 defines `FileClass`. To keep every task compiling, **Task 10 also creates `src/classify.rs` containing only the `FileClass` enum** (copy it from Task 18 Step 3, first block) and adds `pub mod classify;`. Task 18 then appends the rest to that file. `toml::Table::keys()` iterates sorted keys (BTreeMap-backed; do not enable `toml`'s `preserve_order`).

- [ ] **Step 4: Create `src/classify.rs` with the enum only** and add `pub mod classify;` to `lib.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Who may write a file and what lines it may hold (D-19, R-FILE-001).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum FileClass {
    Authored,
    Generated,
    Reference,
    VendorOwned,
    Ledger,
    ExternalGenerated,
}
```

Run `cargo test -p fairlead-core r_syn_039 r_syn_051 r_syn_038 r_syn_040 r_syn_041 r_arch_021`. Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): control-file schema types, reader helpers and simple tables`.

---

### Task 11: config/schema.rs (part 2): kinds, file kinds, `[[file]]`, `[[vendor]]`, `[[ovr]]`

Rules: R-SCOPE-065 (`[kinds.exc]`), R-FILE-031 (`source = "generate"`), R-FILE-050 (`[[file]]`), R-FILE-005/R-FILE-040 (`[[vendor]]` declaration), R-SYN-050 (kinds are tables), C-6.

**Files:**
- Modify: `crates/fairlead-core/src/config/schema.rs` (append the parsers; append tests to its test module)

**Interfaces:** consumes the types and helpers of Task 10 and `fairlead_syntax::ids::Id::parse`. Produces crate-private `parse_kinds`, `parse_file_kinds`, `parse_files`, `parse_vendors`, `parse_ovrs`.

Behaviour fixed here:
- A kind name is 2 to 8 ASCII lowercase letters (R-SYN-024). `[kinds.exc]` is rejected (`HN-C001`, "reserved built-in kind", C-6).
- `source = "generate"` is rejected in 0.1 with `HN-C001` (C-6: no generator ships in 0.1; the same message also covers a vendor-owned target). The flag is still parsed so the diagnostic names it.
- `[[file]]` needs `id` (an unqualified `kind.name`), `kind` and `path`; `extends` may be qualified (`root:file.styles`).
- `[[vendor]]` needs `path` (a path or glob) and `tool`.
- `[[ovr]]` needs `target` (an ID) and `why` is validated later (P4/P5); `op` defaults to `Set`; every key that is not `target op applies why owner exp base` is a field assignment.

- [ ] **Step 1: Append these tests to the `tests` module of `schema.rs`.**

```rust
    #[test]
    fn r_scope_065__kinds_exc_is_rejected_at_load() {
        let (mut c, mut t) = cx("[kinds.exc]\ntitle = \"x\"\n\n[kinds.tok]\ntitle = \"design token\"\n");
        let kinds = parse_kinds(&mut c, &mut t);
        assert_eq!(c.diags.len(), 1);
        assert_eq!(c.diags[0].code, codes::HN_C001);
        assert!(c.diags[0].message.contains("exc"), "{}", c.diags[0].message);
        assert!(kinds.contains_key("tok") && !kinds.contains_key("exc"));
    }

    #[test]
    fn r_syn_050__kind_names_are_two_to_eight_lowercase_letters() {
        let (mut c, mut t) = cx("[kinds.a]\n[kinds.tauri-command]\n[kinds.Cmp]\n[kinds.variant]\n");
        let kinds = parse_kinds(&mut c, &mut t);
        assert_eq!(c.diags.len(), 3);
        assert_eq!(kinds.keys().collect::<Vec<_>>(), ["variant"]);
    }

    #[test]
    fn r_syn_050__kind_fields_parse() {
        let text = "[kinds.cmp]\ntitle = \"component\"\nshape = \"code\"\nimpl = 1\nunused = \"warn\"\ndir = \"docs/components/\"\n[kinds.cmp.keys]\nlayer = { type = \"enum\", values = [\"atom\", \"molecule\"] }\n\n[kinds.adr]\nshape = \"doc\"\nimpl = \"none\"\n";
        let (mut c, mut t) = cx(text);
        let kinds = parse_kinds(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let cmp = &kinds["cmp"];
        assert_eq!((cmp.shape, cmp.impl_limit, cmp.unused), (KindShape::Code, ImplLimit::N(1), Some(Severity::Warn)));
        assert_eq!(cmp.dir.as_deref(), Some("docs/components/"));
        assert_eq!(cmp.keys["layer"], KeyDecl { ty: "enum".into(), values: vec!["atom".into(), "molecule".into()] });
        let adr = &kinds["adr"];
        assert_eq!((adr.shape, adr.impl_limit), (KindShape::Doc, ImplLimit::None));
        assert_eq!(KindDecl::default().shape, KindShape::Either);
    }

    #[test]
    fn r_file_031__source_generate_is_rejected_in_0_1() {
        let (mut c, mut t) = cx("[kinds.tok]\nsource = \"generate\"\n");
        let kinds = parse_kinds(&mut c, &mut t);
        assert_eq!(c.diags.len(), 1);
        assert!(c.diags[0].message.contains("generator"), "{}", c.diags[0].message);
        assert!(kinds["tok"].source_generate);
        let (mut c2, mut t2) = cx("[kinds.tok]\nsource = \"hand\"\n");
        let _ = parse_kinds(&mut c2, &mut t2);
        assert_eq!(c2.diags.len(), 1);
    }

    #[test]
    fn r_file_050__file_items_parse_with_extends() {
        let text = "[[file]]\nid = \"file.console-styles\"\nkind = \"stylesheet\"\npath = \"app\\\\overrides.css\"\nextends = \"root:file.styles\"\n\n[[file]]\nid = \"file.styles\"\nkind = \"stylesheet\"\npath = \"app/globals.css\"\n";
        let (mut c, mut t) = cx(text);
        let files = parse_files(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "app/overrides.css");
        assert_eq!(files[0].extends.as_deref(), Some("root:file.styles"));
        assert_eq!(files[1].extends, None);
    }

    #[test]
    fn r_file_050__file_item_requires_id_kind_path_and_a_bare_id() {
        let (mut c, mut t) = cx("[[file]]\nkind = \"stylesheet\"\npath = \"a.css\"\n\n[[file]]\nid = \"web:file.x\"\nkind = \"k\"\npath = \"a.css\"\n\n[[file]]\nid = \"nodot\"\nkind = \"k\"\npath = \"a.css\"\n");
        let files = parse_files(&mut c, &mut t);
        assert!(files.is_empty());
        assert_eq!(c.diags.len(), 3);
    }

    #[test]
    fn r_file_005__vendor_decl_needs_path_and_tool() {
        let (mut c, mut t) = cx("[[vendor]]\npath = \"components.json\"\ntool = \"shadcn\"\n\n[[vendor]]\npath = \"ui\\\\**\"\n\n[[vendor]]\ntool = \"x\"\n\n[[vendor]]\npath = \"[bad\"\ntool = \"t\"\n");
        let v = parse_vendors(&mut c, &mut t);
        assert_eq!(v, vec![VendorDecl { path: "components.json".into(), tool: "shadcn".into() }]);
        assert_eq!(c.diags.len(), 3, "missing tool, missing path, and a glob that does not compile");
    }

    #[test]
    fn r_file_020__file_kinds_need_extractor_and_binding() {
        let (mut c, mut t) = cx("[file-kinds.stylesheet]\nextractor = \"css-custom-properties\"\nbinding = \"import\"\nimport-template = \"@import '{path}';\"\n\n[file-kinds.broken]\nextractor = \"x\"\n");
        let fk = parse_file_kinds(&mut c, &mut t);
        assert_eq!(fk["stylesheet"].import_template.as_deref(), Some("@import '{path}';"));
        assert_eq!(fk.len(), 1);
        assert_eq!(c.diags.len(), 1);
    }

    #[test]
    fn r_syn_039__ovr_table_parses_known_keys_and_field_sets() {
        let text = "[[ovr]]\ntarget = \"rule.no-raw-hex\"\nop = \"loosen\"\napplies = \"src/legacy/**\"\nwhy = \"legacy\"\nowner = \"@platform\"\nexp = 2026-12-31\nstage = \"warn\"\n\"globs+\" = [\"a\", \"b\"]\nnote = \"two words\"\n";
        let (mut c, mut t) = cx(text);
        let o = parse_ovrs(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(o.len(), 1);
        assert_eq!((o[0].op, o[0].exp.as_deref(), o[0].owner.as_deref()), (OvrOp::Loosen, Some("2026-12-31"), Some("@platform")));
        assert_eq!(o[0].set["stage"], ArgValue::Word("warn".into()));
        assert_eq!(o[0].set["globs+"], ArgValue::List(vec!["a".into(), "b".into()]));
        assert_eq!(o[0].set["note"], ArgValue::Quoted("two words".into()));
    }

    #[test]
    fn r_syn_039__ovr_defaults_and_errors() {
        let (mut c, mut t) = cx("[[ovr]]\ntarget = \"rule.x\"\nwhy = \"w\"\n\n[[ovr]]\nwhy = \"no target\"\n\n[[ovr]]\ntarget = \"rule.y\"\nop = \"explode\"\nexp = \"soon\"\n");
        let o = parse_ovrs(&mut c, &mut t);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].op, OvrOp::Set);
        assert_eq!(c.diags.len(), 3, "missing target, bad op, bad exp");
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-core r_scope_065 r_syn_050 r_file_031 r_file_050 r_file_005 r_file_020 r_syn_039`. Expected: FAIL (compile error: `cannot find function parse_kinds`).

- [ ] **Step 3: Append the implementation to `schema.rs`** (above the test module).

```rust
// ---------------------------------------------------------------- kinds, file kinds, items
fn valid_kind_name(s: &str) -> bool {
    (2..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase())
}

fn parse_key_decls(cx: &mut Ctx<'_>, kind: &str, t: &mut toml::Table) -> BTreeMap<String, KeyDecl> {
    let mut out = BTreeMap::new();
    let prefix = format!("kinds.{kind}.keys");
    if let Some(keys) = take_table(cx, t, &format!("kinds.{kind}"), "keys") {
        for (name, v) in keys {
            let key = join_key(&prefix, &name);
            let Some(mut decl) = v.as_table().cloned() else {
                cx.error(&key, format!("`{key}` must be an inline table such as {{ type = \"enum\", values = [..] }}"));
                continue;
            };
            let ty = take_string(cx, &mut decl, &key, "type");
            let values = take_string_list(cx, &mut decl, &key, "values").unwrap_or_default();
            cx.unknown_in(&key, &decl);
            match ty {
                Some(ty) => {
                    out.insert(name, KeyDecl { ty, values });
                }
                None => cx.error(&key, format!("`{key}` needs a `type`")),
            }
        }
    }
    out
}

pub(crate) fn parse_kinds(cx: &mut Ctx<'_>, root: &mut toml::Table) -> BTreeMap<String, KindDecl> {
    let mut out = BTreeMap::new();
    let Some(kinds) = take_table(cx, root, "", "kinds") else { return out };
    for (name, v) in kinds {
        let key = join_key("kinds", &name);
        if !valid_kind_name(&name) {
            cx.error(&key, format!("kind name {name:?} must be 2 to 8 lowercase letters; put a readable label in `title`"));
            continue;
        }
        if name == "exc" {
            cx.error(&key, "`exc` is a reserved built-in kind and cannot be declared (R-SCOPE-065)");
            continue;
        }
        let Some(mut t) = v.as_table().cloned() else {
            cx.error(&key, format!("`{key}` must be a table"));
            continue;
        };
        let mut decl = KindDecl { title: take_string(cx, &mut t, &key, "title"), ..KindDecl::default() };
        if let Some(s) = take_string(cx, &mut t, &key, "shape") {
            match s.as_str() {
                "doc" => decl.shape = KindShape::Doc,
                "code" => decl.shape = KindShape::Code,
                "either" => decl.shape = KindShape::Either,
                _ => cx.error(&key, "shape must be \"doc\", \"code\" or \"either\""),
            }
        }
        match t.remove("impl") {
            None => {}
            Some(toml::Value::Integer(n)) => match u32::try_from(n) {
                Ok(n) => decl.impl_limit = ImplLimit::N(n),
                Err(_) => cx.error(&key, "impl must be a non-negative integer, \"any\" or \"none\""),
            },
            Some(toml::Value::String(s)) if s == "any" => decl.impl_limit = ImplLimit::Any,
            Some(toml::Value::String(s)) if s == "none" => decl.impl_limit = ImplLimit::None,
            Some(_) => cx.error(&key, "impl must be a non-negative integer, \"any\" or \"none\""),
        }
        if let Some(s) = take_string(cx, &mut t, &key, "unused") {
            match s.as_str() {
                "info" => decl.unused = Some(Severity::Info),
                "warn" => decl.unused = Some(Severity::Warn),
                "error" => decl.unused = Some(Severity::Error),
                _ => cx.error(&key, "unused must be \"info\", \"warn\" or \"error\""),
            }
        }
        if let Some(d) = take_string(cx, &mut t, &key, "dir") {
            decl.dir = clean_rel_path(cx, &key, &d);
        }
        if let Some(s) = take_string(cx, &mut t, &key, "source") {
            if s == "generate" {
                decl.source_generate = true;
                cx.error(&key, "source = \"generate\" needs a generator and none ships in 0.1; it is also refused for vendor-owned files (C-6, R-FILE-031)");
            } else {
                cx.error(&key, "source must be \"generate\" when present");
            }
        }
        decl.keys = parse_key_decls(cx, &name, &mut t);
        cx.unknown_in(&key, &t);
        out.insert(name, decl);
    }
    out
}

pub(crate) fn parse_file_kinds(cx: &mut Ctx<'_>, root: &mut toml::Table) -> BTreeMap<String, FileKindDecl> {
    let mut out = BTreeMap::new();
    let Some(fks) = take_table(cx, root, "", "file-kinds") else { return out };
    for (name, v) in fks {
        let key = join_key("file-kinds", &name);
        let Some(mut t) = v.as_table().cloned() else {
            cx.error(&key, format!("`{key}` must be a table"));
            continue;
        };
        let extractor = take_string(cx, &mut t, &key, "extractor");
        let binding = take_string(cx, &mut t, &key, "binding");
        let import_template = take_string(cx, &mut t, &key, "import-template");
        cx.unknown_in(&key, &t);
        match (extractor, binding) {
            (Some(extractor), Some(binding)) => {
                out.insert(name, FileKindDecl { extractor, binding, import_template });
            }
            _ => cx.error(&key, format!("`{key}` needs both `extractor` and `binding`")),
        }
    }
    out
}

fn check_id(cx: &mut Ctx<'_>, key: &str, raw: &str, allow_qualified: bool) -> bool {
    use fairlead_syntax::ids::{Id, IdQualifier};
    match Id::parse(raw) {
        Ok(id) if allow_qualified || matches!(id.qualifier, IdQualifier::Bare) => true,
        Ok(_) => {
            cx.error(key, format!("{raw:?} must be an unqualified kind.name here"));
            false
        }
        Err(e) => {
            cx.error(key, format!("{raw:?} is not a valid ID: {e:?}"));
            false
        }
    }
}

pub(crate) fn parse_files(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<FileItemDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "file") {
        let before = cx.diags.len();
        let id = take_string(cx, &mut t, "file", "id");
        let kind = take_string(cx, &mut t, "file", "kind");
        let path = take_string(cx, &mut t, "file", "path");
        let extends = take_string(cx, &mut t, "file", "extends");
        cx.unknown_in("file", &t);
        let (Some(id), Some(kind), Some(path)) = (id, kind, path) else {
            if cx.diags.len() == before {
                cx.error("file", "a [[file]] needs `id`, `kind` and `path`");
            }
            continue;
        };
        if !check_id(cx, "file.id", &id, false) {
            continue;
        }
        if let Some(e) = &extends {
            if !check_id(cx, "file.extends", e, true) {
                continue;
            }
        }
        if let Some(path) = clean_rel_path(cx, "file.path", &path) {
            out.push(FileItemDecl { id, kind, path, extends });
        }
    }
    out
}

pub(crate) fn parse_vendors(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<VendorDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "vendor") {
        let before = cx.diags.len();
        let path = take_string(cx, &mut t, "vendor", "path");
        let tool = take_string(cx, &mut t, "vendor", "tool");
        cx.unknown_in("vendor", &t);
        match (path, tool) {
            (Some(path), Some(tool)) => {
                if let Some(path) = clean_rel_path(cx, "vendor.path", &path) {
                    if check_glob(cx, "vendor.path", &path) {
                        out.push(VendorDecl { path, tool });
                    }
                }
            }
            _ => {
                if cx.diags.len() == before {
                    cx.error("vendor", "a [[vendor]] needs `path` and `tool`");
                }
            }
        }
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-_.#/@%:*".contains(c)
}

fn to_arg_value(v: &toml::Value) -> Option<ArgValue> {
    match v {
        toml::Value::String(s) if !s.is_empty() && s.chars().all(is_word_char) => Some(ArgValue::Word(s.clone())),
        toml::Value::String(s) => Some(ArgValue::Quoted(s.clone())),
        toml::Value::Integer(i) => Some(ArgValue::Word(i.to_string())),
        toml::Value::Boolean(b) => Some(ArgValue::Word(b.to_string())),
        toml::Value::Array(_) => string_array(v).map(ArgValue::List),
        _ => None,
    }
}

pub(crate) fn parse_ovrs(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<OvrDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "ovr") {
        let before = cx.diags.len();
        let target = take_string(cx, &mut t, "ovr", "target");
        let op = match take_string(cx, &mut t, "ovr", "op").as_deref() {
            None | Some("set") => Some(OvrOp::Set),
            Some("loosen") => Some(OvrOp::Loosen),
            Some("tighten") => Some(OvrOp::Tighten),
            Some("replace") => Some(OvrOp::Replace),
            Some("add") => Some(OvrOp::Add),
            Some("remove") => Some(OvrOp::Remove),
            Some(other) => {
                cx.error("ovr.op", format!("unknown op {other:?}; use loosen, tighten, set, replace, add or remove"));
                None
            }
        };
        let applies = take_string(cx, &mut t, "ovr", "applies");
        let why = take_string(cx, &mut t, "ovr", "why");
        let owner = take_string(cx, &mut t, "ovr", "owner");
        let exp = take_date_string(cx, &mut t, "ovr", "exp");
        let base = take_string(cx, &mut t, "ovr", "base");
        let mut set = BTreeMap::new();
        for (k, v) in std::mem::take(&mut t) {
            match to_arg_value(&v) {
                Some(av) => {
                    set.insert(k, av);
                }
                None => cx.error(&join_key("ovr", &k), format!("`ovr.{k}` must be a string, number, bool or list of strings")),
            }
        }
        match (target, op) {
            (Some(target), Some(op)) => {
                if check_id(cx, "ovr.target", &target, true) && cx.diags.len() == before {
                    out.push(OvrDecl { target, op, applies, set, why, owner, exp, base });
                }
            }
            _ => {
                if cx.diags.len() == before {
                    cx.error("ovr", "an [[ovr]] needs a `target`");
                }
            }
        }
    }
    out
}
```

Verify: `Id::parse("web:file.x")` returns `qualifier = IdQualifier::Scope(..)` and `Id::parse("rule.no-raw-hex")` returns `Bare`; `IdQualifier` is a public enum (contract 4.1). A TOML bare date (`exp = 2026-12-31`) arrives as `toml::Value::Datetime`, whose `to_string()` is `2026-12-31`.

- [ ] **Step 4: Run** the same command as Step 2. Expected: PASS (all earlier schema tests still pass).

- [ ] **Step 5: Commit** `feat(core): kinds, file kinds, file items, vendors and scope-level ovr tables`.

---

### Task 12: config/rules.rs: rules, policies, grants, requests, conventions and the small root tables

Rules: R-CHK-005 (`[rules.<name>]`), R-SYN-053 (field-level policies), R-SCOPE-018 (`[[convention]]`), R-SCOPE-056 (`[[allow]]`, `[[req]]`), C-18/C-19/C-24/C-25/C-26/C-27 (defaults), C-30 (regex validated at load).

**Files:**
- Create: `crates/fairlead-core/src/config/rules.rs`
- Modify: `crates/fairlead-core/src/config/mod.rs` (`pub mod rules; pub use rules::*;`)

**Interfaces (contract 4.6, names fixed):** `RuleDecl`, `RuleCheckDecl { Leak { tokens }, Pattern { forbid, msg }, Builtin }`, `PolicyTable { by_pattern, waiver, fields }`, `WaiverPolicy { max_exp_days: 90, budget: 25, max_renewals: 2, approval_required: true }`, `FieldPolicy { target, fields, policy }`, `AllowDecl { target, field, for_scopes, why, exp }`, `ReqDecl { to, target, set, why, blocking }`. Crate-private parsers: `parse_rules`, `parse_policies`, `parse_allows`, `parse_reqs`, `parse_conventions`, `parse_context`, `parse_baseline`, `parse_secrets`, `parse_exceptions`, `parse_eligibility`.

Behaviour fixed here: a rule without `check` is `Builtin`; `leak` needs a non-empty `tokens` list; `pattern` needs `forbid` (a Rust regex, compiled with a 1 MB size limit; invalid is `HN-C001`, C-30) and `msg`; `since` is a date; `[policies]` string values are key-pattern policies, a sub-table named `waiver` is the waiver policy, any other sub-table is a field-level policy keyed by its free label (R-SYN-053); `[policies.waiver] max-exp` accepts `90` or `"90d"`, `approval` accepts `"required"`, `"none"` or a bool; a `[[convention]]` has `glob` plus exactly one of `transparent = true` or `class = "external-generated" | "vendor-owned"`; `own` is a 0.2 key and is reported unknown by a 0.1 parse.

- [ ] **Step 1: Write the failing tests.** Create `rules.rs` with only this module.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::FileClass;
    use crate::config::schema::UnknownKey;
    use crate::diag::codes;
    use crate::path::RepoPath;

    fn cx(text: &str) -> (Ctx<'_>, toml::Table) {
        let table: toml::Table = text.parse().unwrap();
        (Ctx::new(&RepoPath::new("fairlead/control.toml").unwrap(), text), table)
    }

    #[test]
    fn r_chk_005__leak_rule_parses_with_all_keys() {
        let text = "[rules.no-raw-hex]\ncheck = \"leak\"\ntokens = [\"tok.*\"]\nsev = \"error\"\nstage = \"observe\"\nglob = [\"**/*.css\", \"**/*.tsx\"]\nexclude = [\"dist/\"]\nwaivable = true\nwhy = \"colours come from tokens\"\nsince = \"2026-10-01\"\n";
        let (mut c, mut t) = cx(text);
        let rules = parse_rules(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let r = &rules["no-raw-hex"];
        assert_eq!(r.check, RuleCheckDecl::Leak { tokens: vec!["tok.*".into()] });
        assert_eq!((r.sev, r.stage, r.waivable), (Some(Severity::Error), Some(Stage::Observe), true));
        assert_eq!(r.glob, ["**/*.css", "**/*.tsx"]);
        assert_eq!(r.exclude, ["dist/"]);
        assert_eq!(r.why.as_deref(), Some("colours come from tokens"));
        assert_eq!(r.since.map(|d| d.to_string()).as_deref(), Some("2026-10-01"));
        assert_eq!(r.name, "no-raw-hex");
    }

    #[test]
    fn r_chk_005__pattern_rule_compiles_its_regex_at_load() {
        let (mut c, mut t) = cx("[rules.no-unwrap]\ncheck = \"pattern\"\nforbid = '\\.unwrap\\(\\)'\nmsg = \"no unwrap\"\n");
        let rules = parse_rules(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert!(matches!(&rules["no-unwrap"].check, RuleCheckDecl::Pattern { msg, .. } if msg == "no unwrap"));
        let (mut c2, mut t2) = cx("[rules.bad]\ncheck = \"pattern\"\nforbid = \"(unclosed\"\nmsg = \"m\"\n");
        let rules2 = parse_rules(&mut c2, &mut t2);
        assert!(rules2.is_empty());
        assert_eq!(c2.diags.len(), 1);
        assert!(c2.diags[0].message.contains("regex"), "{}", c2.diags[0].message);
    }

    #[test]
    fn r_chk_005__rule_errors_and_builtin_default() {
        let text = "[rules.a-leak]\ncheck = \"leak\"\n\n[rules.a-pattern]\ncheck = \"pattern\"\nforbid = \"x\"\n\n[rules.Bad_Name]\ncheck = \"leak\"\ntokens = [\"t\"]\n\n[rules.unknown-check]\ncheck = \"magic\"\n\n[rules.fine]\nsev = \"warn\"\nstage = \"enforce\"\n\n[rules.bad-sev]\nsev = \"loud\"\n";
        let (mut c, mut t) = cx(text);
        let rules = parse_rules(&mut c, &mut t);
        assert_eq!(c.diags.len(), 5, "{:?}", c.diags);
        assert_eq!(rules.keys().collect::<Vec<_>>(), ["bad-sev", "fine"].iter().collect::<Vec<_>>());
        assert_eq!(rules["fine"].check, RuleCheckDecl::Builtin);
    }

    #[test]
    fn r_syn_053__field_level_policy_form() {
        let text = "[policies]\n\"rules.*\" = \"sealed\"\n\"rules.no-raw-hex\" = \"needs-exception\"\n\n[policies.\"cargo.dep-version\"]\ntarget = \"cargo\"\nfields = [\"dependencies.*.version\", \"workspace.dependencies.*.version\"]\npolicy = \"sealed\"\n\n[policies.waiver]\nmax-exp = \"60d\"\nbudget = 10\n";
        let (mut c, mut t) = cx(text);
        let p = parse_policies(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(p.by_pattern["rules.*"], Policy::Sealed);
        assert_eq!(p.by_pattern["rules.no-raw-hex"], Policy::NeedsException);
        let f = &p.fields["cargo.dep-version"];
        assert_eq!((f.target.as_deref(), f.policy, f.fields.len()), (Some("cargo"), Policy::Sealed, 2));
        assert!(!p.fields.contains_key("waiver"), "the label `waiver` is the waiver table, never a policy id");
        assert_eq!((p.waiver.max_exp_days, p.waiver.budget, p.waiver.max_renewals, p.waiver.approval_required), (60, 10, 2, true));
    }

    #[test]
    fn r_syn_053__policy_errors() {
        let (mut c, mut t) = cx("[policies]\n\"a.b\" = \"maybe\"\n\n[policies.\"x.y\"]\npolicy = \"sealed\"\n\n[policies.\"z.w\"]\nfields = [\"a\"]\n\n[policies.waiver]\nmax-exp = \"soon\"\napproval = \"sometimes\"\n");
        let _ = parse_policies(&mut c, &mut t);
        assert_eq!(c.diags.len(), 5, "{:?}", c.diags);
    }

    #[test]
    fn r_chk_005__waiver_defaults_match_the_contract() {
        let w = WaiverPolicy::default();
        assert_eq!((w.max_exp_days, w.budget, w.max_renewals, w.approval_required), (90, 25, 2, true));
        let (mut c, mut t) = cx("[policies.waiver]\nmax-exp = 30\napproval = \"none\"\n");
        let p = parse_policies(&mut c, &mut t);
        assert_eq!((p.waiver.max_exp_days, p.waiver.approval_required), (30, false));
    }

    #[test]
    fn r_scope_018__convention_actions() {
        let text = "[[convention]]\nglob = \"app/(*)/**\"\ntransparent = true\n\n[[convention]]\nglob = \"**/*.gen.ts\"\nclass = \"external-generated\"\n\n[[convention]]\nglob = \"ui/**\"\nclass = \"vendor-owned\"\n";
        let (mut c, mut t) = cx(text);
        let v = parse_conventions(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(v[0].action, ConventionAction::Transparent);
        assert_eq!(v[1].action, ConventionAction::Class(FileClass::ExternalGenerated));
        assert_eq!(v[2].action, ConventionAction::Class(FileClass::VendorOwned));
    }

    #[test]
    fn r_scope_018__convention_needs_exactly_one_action_and_own_is_unknown_in_0_1() {
        let text = "[[convention]]\nglob = \"a/**\"\n\n[[convention]]\nglob = \"b/**\"\ntransparent = true\nclass = \"external-generated\"\n\n[[convention]]\nglob = \"c/**\"\nclass = \"authored\"\n\n[[convention]]\nglob = \"d/**\"\ntransparent = true\nown = \"web\"\n";
        let (mut c, mut t) = cx(text);
        let v = parse_conventions(&mut c, &mut t);
        assert_eq!(c.diags.len(), 3);
        assert_eq!(v.len(), 1);
        assert_eq!(c.unknown, vec![UnknownKey { path: "convention.own".into(), line: 16 }]);
    }

    #[test]
    fn r_scope_056__allow_and_req_tables() {
        let text = "[[allow]]\nfor = [\"web\", \"api\"]\ntarget = \"rules.no-raw-hex.stage\"\nfield = \"stage\"\nwhy = \"web is migrating\"\nexp = 2027-03-31\n\n[[req]]\nto = \"root\"\ntarget = \"rule.no-raw-hex\"\nset = \"warn\"\nwhy = \"need it\"\nblocking = true\n";
        let (mut c, mut t) = cx(text);
        let a = parse_allows(&mut c, &mut t);
        let r = parse_reqs(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(a, vec![AllowDecl {
            target: "rules.no-raw-hex.stage".into(), field: Some("stage".into()),
            for_scopes: vec!["web".into(), "api".into()], why: "web is migrating".into(), exp: "2027-03-31".into(),
        }]);
        assert_eq!(r, vec![ReqDecl { to: "root".into(), target: "rule.no-raw-hex".into(), set: Some("warn".into()), why: "need it".into(), blocking: true }]);
    }

    #[test]
    fn r_scope_056__allow_and_req_need_their_keys_and_valid_scope_ids() {
        let (mut c, mut t) = cx("[[allow]]\ntarget = \"x\"\nwhy = \"w\"\nexp = \"2027-01-01\"\n\n[[allow]]\nfor = [\"Bad Scope\"]\ntarget = \"x\"\nwhy = \"w\"\nexp = \"2027-01-01\"\n\n[[req]]\nto = \"root\"\ntarget = \"x\"\n");
        assert!(parse_allows(&mut c, &mut t).is_empty());
        assert!(parse_reqs(&mut c, &mut t).is_empty());
        assert_eq!(c.diags.len(), 3);
    }

    #[test]
    fn r_syn_039__context_table_parses_and_defaults() {
        let d = ContextConfig::default();
        assert_eq!((d.budget_start, d.budget_path, d.bytes_per_token), (500, 700, 4));
        assert_eq!(d.sev_weight, SevWeight { error: 4, warn: 2, info: 1 });
        let (mut c, mut t) = cx("[context]\nbudget-start = 300\ntokenizer = \"chars4\"\nbytes-per-token = 3\n[context.sev-weight]\nerror = 9\n");
        let x = parse_context(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!((x.budget_start, x.budget_path, x.bytes_per_token), (300, 700, 3));
        assert_eq!((x.tokenizer.as_deref(), x.sev_weight.error, x.sev_weight.warn), (Some("chars4"), 9, 2));
        let (mut c2, mut t2) = cx("[context]\nbytes-per-token = 0\n");
        let _ = parse_context(&mut c2, &mut t2);
        assert_eq!(c2.diags.len(), 1);
    }

    #[test]
    fn r_syn_039__baseline_secrets_exceptions_eligibility() {
        let text = "[baseline]\nfile = \"fairlead/gen/baseline.txt\"\n\n[secrets]\nallow = [\"0123456789ab\"]\n\n[exceptions]\nmax-days = 45\nmax-live-per-rule = 3\n\n[eligibility]\nmin-lines = 8\n";
        let (mut c, mut t) = cx(text);
        assert_eq!(parse_baseline(&mut c, &mut t).file, "fairlead/gen/baseline.txt");
        assert_eq!(parse_secrets(&mut c, &mut t).allow, ["0123456789ab"]);
        let e = parse_exceptions(&mut c, &mut t);
        assert_eq!((e.max_days, e.max_live_per_rule), (Some(45), 3));
        let el = parse_eligibility(&mut c, &mut t);
        assert_eq!((el.min_lines, el.min_files), (8, 3));
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let (mut c2, mut t2) = cx("[secrets]\nallow = [\"not-a-fingerprint\"]\n");
        let _ = parse_secrets(&mut c2, &mut t2);
        assert_eq!(c2.diags.len(), 1);
        assert_eq!(ExceptionsConfig::default().max_live_per_rule, 10);
        assert_eq!(codes::HN_C001.0, "HN-C001");
    }
}
```

- [ ] **Step 2: Add `pub mod rules; pub use rules::*;`. Run** `cargo test -p fairlead-core r_chk_005 r_syn_053 r_scope_018 r_scope_056 r_syn_039`. Expected: FAIL (compile error: `cannot find function parse_rules`).

- [ ] **Step 3: Write the implementation** (above the tests).

```rust
//! `[rules.*]`, `[policies]`, `[[allow]]`, `[[req]]`, `[[convention]]` and the small root tables.
use crate::classify::FileClass;
use crate::config::schema::{
    check_glob, clean_pattern, clean_rel_path, join_key, take_bool, take_date_string, take_string, take_string_list,
    take_table, take_table_array, take_u32, BaselineConfig, ContextConfig, Ctx, ConventionAction, ConventionDecl,
    EligibilityConfig, ExceptionsConfig, Policy, SecretsConfig, Stage,
};
use crate::diag::Severity;
use crate::hash::Fingerprint;
use fairlead_syntax::ids::{Id, ScopeSlug};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RuleCheckDecl {
    Leak { tokens: Vec<String> },
    Pattern { forbid: String, msg: String },
    Builtin,
}

/// `[rules.<name>]` (R-CHK-005).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RuleDecl {
    pub name: String,
    pub check: RuleCheckDecl,
    pub sev: Option<Severity>,
    pub stage: Option<Stage>,
    pub glob: Vec<String>,
    pub exclude: Vec<String>,
    pub waivable: bool,
    pub why: Option<String>,
    pub since: Option<jiff::civil::Date>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WaiverPolicy {
    pub max_exp_days: u32,
    pub budget: u32,
    pub max_renewals: u32,
    pub approval_required: bool,
}

impl Default for WaiverPolicy {
    fn default() -> Self {
        Self { max_exp_days: 90, budget: 25, max_renewals: 2, approval_required: true }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldPolicy {
    pub target: Option<String>,
    pub fields: Vec<String>,
    pub policy: Policy,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PolicyTable {
    pub by_pattern: BTreeMap<String, Policy>,
    pub waiver: WaiverPolicy,
    pub fields: BTreeMap<String, FieldPolicy>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AllowDecl {
    pub target: String,
    pub field: Option<String>,
    pub for_scopes: Vec<String>,
    pub why: String,
    pub exp: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReqDecl {
    pub to: String,
    pub target: String,
    pub set: Option<String>,
    pub why: String,
    pub blocking: bool,
}

fn severity_of(cx: &mut Ctx<'_>, key: &str, s: &str) -> Option<Severity> {
    match s {
        "info" => Some(Severity::Info),
        "warn" => Some(Severity::Warn),
        "error" => Some(Severity::Error),
        _ => {
            cx.error(key, format!("`sev` must be info, warn or error, got {s:?}"));
            None
        }
    }
}

fn stage_of(cx: &mut Ctx<'_>, key: &str, s: &str) -> Option<Stage> {
    match s {
        "observe" => Some(Stage::Observe),
        "warn" => Some(Stage::Warn),
        "enforce" => Some(Stage::Enforce),
        _ => {
            cx.error(key, format!("`stage` must be observe, warn or enforce, got {s:?}"));
            None
        }
    }
}

fn policy_of(cx: &mut Ctx<'_>, key: &str, s: &str) -> Option<Policy> {
    match s {
        "open" => Some(Policy::Open),
        "needs-exception" => Some(Policy::NeedsException),
        "sealed" => Some(Policy::Sealed),
        _ => {
            cx.error(key, format!("policy must be open, needs-exception or sealed, got {s:?}"));
            None
        }
    }
}

pub(crate) fn parse_rules(cx: &mut Ctx<'_>, root: &mut toml::Table) -> BTreeMap<String, RuleDecl> {
    let mut out = BTreeMap::new();
    let Some(rules) = take_table(cx, root, "", "rules") else { return out };
    for (name, v) in rules {
        let key = join_key("rules", &name);
        if Id::parse(&format!("rule.{name}")).is_err() {
            cx.error(&key, format!("rule name {name:?} must be lowercase slugs separated by '.'"));
            continue;
        }
        let Some(mut t) = v.as_table().cloned() else {
            cx.error(&key, format!("`{key}` must be a table"));
            continue;
        };
        let check = match take_string(cx, &mut t, &key, "check").as_deref() {
            None => Some(RuleCheckDecl::Builtin),
            Some("leak") => {
                let tokens = take_string_list(cx, &mut t, &key, "tokens").unwrap_or_default();
                if tokens.is_empty() {
                    cx.error(&key, "a `leak` rule needs a non-empty `tokens` list");
                    None
                } else {
                    Some(RuleCheckDecl::Leak { tokens })
                }
            }
            Some("pattern") => {
                let forbid = take_string(cx, &mut t, &key, "forbid");
                let msg = take_string(cx, &mut t, &key, "msg");
                match (forbid, msg) {
                    (Some(forbid), Some(msg)) => match regex::RegexBuilder::new(&forbid).size_limit(1_000_000).build() {
                        Ok(_) => Some(RuleCheckDecl::Pattern { forbid, msg }),
                        Err(e) => {
                            cx.error(&key, format!("`forbid` is not a valid regex: {e}"));
                            None
                        }
                    },
                    _ => {
                        cx.error(&key, "a `pattern` rule needs `forbid` and `msg`");
                        None
                    }
                }
            }
            Some(other) => {
                cx.error(&key, format!("unknown check {other:?}; use \"leak\" or \"pattern\""));
                None
            }
        };
        let sev = take_string(cx, &mut t, &key, "sev").and_then(|s| severity_of(cx, &key, &s));
        let stage = take_string(cx, &mut t, &key, "stage").and_then(|s| stage_of(cx, &key, &s));
        let glob = take_string_list(cx, &mut t, &key, "glob")
            .unwrap_or_default()
            .iter()
            .filter_map(|p| clean_pattern(cx, &key, p))
            .collect();
        let exclude = take_string_list(cx, &mut t, &key, "exclude")
            .unwrap_or_default()
            .iter()
            .filter_map(|p| clean_pattern(cx, &key, p))
            .collect();
        let waivable = take_bool(cx, &mut t, &key, "waivable").unwrap_or(false);
        let why = take_string(cx, &mut t, &key, "why");
        let since = take_date_string(cx, &mut t, &key, "since").and_then(|s| s.parse::<jiff::civil::Date>().ok());
        cx.unknown_in(&key, &t);
        if let Some(check) = check {
            out.insert(name.clone(), RuleDecl { name, check, sev, stage, glob, exclude, waivable, why, since });
        }
    }
    out
}

fn parse_waiver(cx: &mut Ctx<'_>, mut w: toml::Table) -> WaiverPolicy {
    let mut out = WaiverPolicy::default();
    let bad_exp = "`policies.waiver.max-exp` must be a number of days such as 90 or \"90d\"";
    match w.remove("max-exp") {
        None => {}
        Some(toml::Value::Integer(n)) => match u32::try_from(n) {
            Ok(v) => out.max_exp_days = v,
            Err(_) => cx.error("policies.waiver.max-exp", bad_exp),
        },
        Some(toml::Value::String(s)) => match s.strip_suffix('d').and_then(|d| d.parse::<u32>().ok()) {
            Some(v) => out.max_exp_days = v,
            None => cx.error("policies.waiver.max-exp", bad_exp),
        },
        Some(_) => cx.error("policies.waiver.max-exp", bad_exp),
    }
    if let Some(v) = take_u32(cx, &mut w, "policies.waiver", "budget") {
        out.budget = v;
    }
    if let Some(v) = take_u32(cx, &mut w, "policies.waiver", "max-renewals") {
        out.max_renewals = v;
    }
    match w.remove("approval") {
        None => {}
        Some(toml::Value::Boolean(b)) => out.approval_required = b,
        Some(toml::Value::String(s)) if s == "required" => out.approval_required = true,
        Some(toml::Value::String(s)) if s == "none" => out.approval_required = false,
        Some(_) => cx.error("policies.waiver.approval", "approval must be \"required\", \"none\" or a bool"),
    }
    cx.unknown_in("policies.waiver", &w);
    out
}

pub(crate) fn parse_policies(cx: &mut Ctx<'_>, root: &mut toml::Table) -> PolicyTable {
    let mut out = PolicyTable::default();
    let Some(t) = take_table(cx, root, "", "policies") else { return out };
    for (k, v) in t {
        let key = join_key("policies", &k);
        match v {
            toml::Value::Table(w) if k == "waiver" => out.waiver = parse_waiver(cx, w),
            toml::Value::String(s) => {
                if let Some(p) = policy_of(cx, &key, &s) {
                    out.by_pattern.insert(k, p);
                }
            }
            toml::Value::Table(mut ft) => {
                let target = take_string(cx, &mut ft, &key, "target");
                let fields = take_string_list(cx, &mut ft, &key, "fields").unwrap_or_default();
                let policy = take_string(cx, &mut ft, &key, "policy").and_then(|s| policy_of(cx, &key, &s));
                cx.unknown_in(&key, &ft);
                match (policy, fields.is_empty()) {
                    (Some(policy), false) => {
                        out.fields.insert(k, FieldPolicy { target, fields, policy });
                    }
                    _ => cx.error(&key, format!("`{key}` needs a non-empty `fields` list and a valid `policy`")),
                }
            }
            _ => cx.error(&key, format!("`{key}` must be a policy string or a table")),
        }
    }
    out
}

fn scope_ids(cx: &mut Ctx<'_>, key: &str, raw: &[String]) -> Option<Vec<String>> {
    let mut ok = true;
    for s in raw {
        if ScopeSlug::parse(s).is_err() {
            cx.error(key, format!("{s:?} is not a valid scope id"));
            ok = false;
        }
    }
    ok.then(|| raw.to_vec())
}

pub(crate) fn parse_allows(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<AllowDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "allow") {
        let before = cx.diags.len();
        let target = take_string(cx, &mut t, "allow", "target");
        let field = take_string(cx, &mut t, "allow", "field");
        let for_raw = take_string_list(cx, &mut t, "allow", "for");
        let why = take_string(cx, &mut t, "allow", "why");
        let exp = take_date_string(cx, &mut t, "allow", "exp");
        cx.unknown_in("allow", &t);
        let for_scopes = for_raw.and_then(|l| scope_ids(cx, "allow.for", &l));
        match (target, for_scopes, why, exp) {
            (Some(target), Some(for_scopes), Some(why), Some(exp)) if !for_scopes.is_empty() => {
                out.push(AllowDecl { target, field, for_scopes, why, exp });
            }
            _ => {
                if cx.diags.len() == before {
                    cx.error("allow", "an [[allow]] needs `target`, a non-empty `for`, `why` and `exp`");
                }
            }
        }
    }
    out
}

pub(crate) fn parse_reqs(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<ReqDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "req") {
        let before = cx.diags.len();
        let to = take_string(cx, &mut t, "req", "to");
        let target = take_string(cx, &mut t, "req", "target");
        let set = take_string(cx, &mut t, "req", "set");
        let why = take_string(cx, &mut t, "req", "why");
        let blocking = take_bool(cx, &mut t, "req", "blocking").unwrap_or(false);
        cx.unknown_in("req", &t);
        let to_ok = to.as_ref().and_then(|s| scope_ids(cx, "req.to", std::slice::from_ref(s)));
        match (to, to_ok, target, why) {
            (Some(to), Some(_), Some(target), Some(why)) => out.push(ReqDecl { to, target, set, why, blocking }),
            _ => {
                if cx.diags.len() == before {
                    cx.error("req", "a [[req]] needs `to`, `target` and `why`");
                }
            }
        }
    }
    out
}

pub(crate) fn parse_conventions(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<ConventionDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "convention") {
        let before = cx.diags.len();
        let glob = take_string(cx, &mut t, "convention", "glob")
            .and_then(|g| clean_rel_path(cx, "convention.glob", &g))
            .filter(|g| check_glob(cx, "convention.glob", g));
        let transparent = take_bool(cx, &mut t, "convention", "transparent").unwrap_or(false);
        let class = take_string(cx, &mut t, "convention", "class");
        cx.unknown_in("convention", &t);
        let class = match class.as_deref() {
            None => None,
            Some("external-generated") => Some(FileClass::ExternalGenerated),
            Some("vendor-owned") => Some(FileClass::VendorOwned),
            Some(other) => {
                cx.error("convention.class", format!("class {other:?} is not supported; use \"external-generated\" or \"vendor-owned\""));
                None
            }
        };
        let action = match (transparent, class) {
            (true, None) => Some(ConventionAction::Transparent),
            (false, Some(c)) => Some(ConventionAction::Class(c)),
            _ => None,
        };
        match (glob, action) {
            (Some(glob), Some(action)) => out.push(ConventionDecl { glob, action }),
            _ => {
                if cx.diags.len() == before {
                    cx.error("convention", "a [[convention]] needs `glob` and exactly one of `transparent = true` or `class`");
                }
            }
        }
    }
    out
}

pub(crate) fn parse_context(cx: &mut Ctx<'_>, root: &mut toml::Table) -> ContextConfig {
    let mut out = ContextConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "context") {
        if let Some(v) = take_u32(cx, &mut t, "context", "budget-start") {
            out.budget_start = v;
        }
        if let Some(v) = take_u32(cx, &mut t, "context", "budget-path") {
            out.budget_path = v;
        }
        if let Some(v) = take_u32(cx, &mut t, "context", "bytes-per-token") {
            if v == 0 {
                cx.error("context.bytes-per-token", "bytes-per-token must be at least 1");
            } else {
                out.bytes_per_token = v;
            }
        }
        out.tokenizer = take_string(cx, &mut t, "context", "tokenizer");
        if let Some(mut w) = take_table(cx, &mut t, "context", "sev-weight") {
            for (key, slot) in [
                ("error", &mut out.sev_weight.error),
                ("warn", &mut out.sev_weight.warn),
                ("info", &mut out.sev_weight.info),
            ] {
                if let Some(v) = take_u32(cx, &mut w, "context.sev-weight", key) {
                    *slot = v;
                }
            }
            cx.unknown_in("context.sev-weight", &w);
        }
        cx.unknown_in("context", &t);
    }
    out
}

pub(crate) fn parse_baseline(cx: &mut Ctx<'_>, root: &mut toml::Table) -> BaselineConfig {
    let mut out = BaselineConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "baseline") {
        if let Some(f) = take_string(cx, &mut t, "baseline", "file") {
            if let Some(clean) = clean_rel_path(cx, "baseline.file", &f) {
                out.file = clean;
            }
        }
        cx.unknown_in("baseline", &t);
    }
    out
}

pub(crate) fn parse_secrets(cx: &mut Ctx<'_>, root: &mut toml::Table) -> SecretsConfig {
    let mut out = SecretsConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "secrets") {
        for fp in take_string_list(cx, &mut t, "secrets", "allow").unwrap_or_default() {
            match Fingerprint::parse(&fp) {
                Ok(_) => out.allow.push(fp),
                Err(e) => cx.error("secrets.allow", format!("{fp:?} is not a 12-character fingerprint: {e}")),
            }
        }
        cx.unknown_in("secrets", &t);
    }
    out
}

pub(crate) fn parse_exceptions(cx: &mut Ctx<'_>, root: &mut toml::Table) -> ExceptionsConfig {
    let mut out = ExceptionsConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "exceptions") {
        out.max_days = take_u32(cx, &mut t, "exceptions", "max-days");
        if let Some(v) = take_u32(cx, &mut t, "exceptions", "max-live-per-rule") {
            out.max_live_per_rule = v;
        }
        cx.unknown_in("exceptions", &t);
    }
    out
}

pub(crate) fn parse_eligibility(cx: &mut Ctx<'_>, root: &mut toml::Table) -> EligibilityConfig {
    let mut out = EligibilityConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "eligibility") {
        if let Some(v) = take_u32(cx, &mut t, "eligibility", "min-lines") {
            out.min_lines = v;
        }
        if let Some(v) = take_u32(cx, &mut t, "eligibility", "min-files") {
            out.min_files = v;
        }
        cx.unknown_in("eligibility", &t);
    }
    out
}
```

Verify: `r_scope_018__convention_needs_exactly_one_action...` expects the unknown `convention.own` on line 16 of that text; count the lines if the text changes. The test `r_chk_005__rule_errors_and_builtin_default` expects five errors: leak without tokens, pattern without msg, invalid name, unknown check, bad severity; `bad-sev` and `fine` survive because their `check` is absent (Builtin).

- [ ] **Step 4: Run** the Step 2 command. Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): rules, policies, grants, requests, conventions and small root tables`.

---

### Task 13: config/mod.rs: `ControlFile`, `parse_control`, delegation, hygiene

Rules: R-SYN-038/039/040/041/050/051, R-SCOPE-051/052/056/065, R-AGT-013, R-SYN-033 (fail closed), R-FILE-020 (baseline default), R-ARCH-076 (hash).

**Files:**
- Modify: `crates/fairlead-core/src/config/mod.rs` (append `ControlFile`, `parse_control`, `strip_non_delegable`, `ControlFile::diagnostics`; tests in a new `#[cfg(test)] mod tests` at the bottom)

**Interfaces (contract 4.6, fixed):** `ControlFile { path, dir, content_hash, fairlead, settings, scan, comments, agents, kinds, file_kinds, layout, files, vendors, ovrs, rules, policies, allows, reqs, conventions, context, baseline, secrets, exceptions, eligibility, unknown, violations }`, `parse_control(&RepoPath, &[u8], bool) -> Result<ControlFile, ConfigFailure>`. Addition: `ControlFile::diagnostics() -> Vec<Diagnostic>` (the `HN-C002` warnings from `unknown` plus `violations`).

Delegation (R-SCOPE-051/052/056, R-AGT-013, R-SYN-041): a non-root file may hold `[fairlead] scope`, `[settings] purpose`, `[scan] exclude/skip`, `[[file]]`, `[[vendor]]`, `[[ovr]]`, `[[allow]]`, `[[req]]`, `[[convention]]`, `[kinds.K]`, `[file-kinds.K]`, `[rules.R]`. Any known 0.1 key or table outside that set is stripped before typed parsing and reported as `HN-C003` in `violations` with the hint `fairlead new escalation`; the rest of the file still applies ("inert key", not a parse failure). Unknown tables stay unknown (`HN-C002`). The root-dir rules: `edition` is required (C-34) and `root` must be `true` (`HN-C004`).

- [ ] **Step 1: Write the failing tests** (append a test module to `config/mod.rs`).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;
    use crate::discover::Edition;

    const ROOT: &str = r#"[fairlead]
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

[rules.no-raw-hex]
check = "leak"
tokens = ["tok.*"]
sev = "error"
stage = "observe"
glob = ["**/*.css", "**/*.tsx"]
waivable = true
why = "colours come from tokens"

[policies]
"rules.*" = "sealed"
"rules.no-raw-hex" = "needs-exception"
[policies.waiver]
max-exp = "90d"
budget = 25

[[allow]]
for = ["web"]
target = "rules.no-raw-hex.stage"
why = "web is migrating"
exp = "2027-03-31"
"#;

    fn p(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }

    fn root_file() -> ControlFile {
        parse_control(&p("fairlead/control.toml"), ROOT.as_bytes(), true).unwrap()
    }

    #[test]
    fn r_syn_038__root_control_parses_the_spec_example() {
        let c = root_file();
        assert_eq!(c.fairlead.scope.as_str(), "root");
        assert_eq!((c.fairlead.edition, c.fairlead.root, c.fairlead.marker.as_deref()), (Some(Edition(1)), true, Some("fairlead")));
        assert_eq!(c.dir, RepoPath::root());
        assert_eq!(c.settings.owners.len(), 1);
        assert_eq!(c.scan.exclude.len(), 4);
        assert_eq!(c.agents, ["AGENTS.md", "CLAUDE.md"]);
        assert_eq!(c.kinds.len(), 2);
        assert!(matches!(&c.rules["no-raw-hex"].check, RuleCheckDecl::Leak { tokens } if tokens.len() == 1));
        assert_eq!(c.allows[0].for_scopes, ["web"]);
        assert_eq!(c.baseline.file, "fairlead/gen/baseline.txt", "default follows folder and layout (R-FILE-020)");
        assert!(c.unknown.is_empty() && c.violations.is_empty());
        assert!(c.diagnostics().is_empty());
    }

    #[test]
    fn r_syn_038__root_requires_edition_and_root_true() {
        let no_edition = "[fairlead]\nroot = true\nscope = \"root\"\n";
        let f = parse_control(&p("fairlead/control.toml"), no_edition.as_bytes(), true).unwrap_err();
        assert!(f.diags.iter().any(|d| d.code == codes::HN_C001 && d.message.contains("edition")));
        let not_root = "[fairlead]\nedition = 1\nscope = \"root\"\n";
        let f = parse_control(&p("fairlead/control.toml"), not_root.as_bytes(), true).unwrap_err();
        assert!(f.diags.iter().any(|d| d.code == codes::HN_C004));
        assert_eq!(f.path, p("fairlead/control.toml"));
        assert!(f.diags.iter().all(|d| d.location.path == p("fairlead/control.toml")));
    }

    #[test]
    fn r_syn_033__unparseable_or_mistyped_control_fails_closed() {
        for bad in ["[fairlead\n", "[fairlead]\nscope = \"a\"\n[settings]\nowners = 5\n", "[fairlead]\nscope = \"a\"\n[scan]\nmax-file = \"big\"\n"] {
            let f = parse_control(&p("web/fairlead/control.toml"), bad.as_bytes(), false).unwrap_err();
            assert!(f.diags.iter().all(|d| d.code == codes::HN_C001 || d.code == codes::HN_P001), "{bad:?}");
            assert!(!f.diags.is_empty());
        }
    }

    #[test]
    fn r_syn_039__unknown_tables_and_keys_warn_but_parse() {
        let text = format!("{ROOT}\n[future]\nx = 1\n\n[tok.color]\nrole = \"brand\"\n\n[delegate]\n\"rules.*\" = \"tighten\"\n");
        let c = parse_control(&p("fairlead/control.toml"), text.as_bytes(), true).unwrap();
        let paths: Vec<&str> = c.unknown.iter().map(|u| u.path.as_str()).collect();
        assert_eq!(paths, ["delegate", "future", "tok"]);
        let d = c.diagnostics();
        assert_eq!(d.len(), 3);
        assert!(d.iter().all(|x| x.code == codes::HN_C002 && x.severity == Severity::Warn));
    }

    #[test]
    fn r_syn_050__cmd_and_tok_tables_are_unknown_tables_not_declarations() {
        let text = "[fairlead]\nscope = \"web\"\n\n[cmd.sync-now]\nsym = \"x\"\n";
        let c = parse_control(&p("web/fairlead/control.toml"), text.as_bytes(), false).unwrap();
        assert_eq!(c.unknown[0].path, "cmd");
        assert!(c.kinds.is_empty());
    }

    #[test]
    fn r_syn_051__requires_is_not_a_settings_key() {
        let text = format!("{ROOT}\n[scope]\nportable = true\nrequires = [\"x\"]\n");
        let c = parse_control(&p("fairlead/control.toml"), text.as_bytes(), true).unwrap();
        assert!(c.unknown.iter().any(|u| u.path == "scope"), "`[scope]` is a 0.3 table: unknown in 0.1");
        let text2 = ROOT.replace("fairlead-version = \"0.1\"", "requires = \"x\"");
        let c2 = parse_control(&p("fairlead/control.toml"), text2.as_bytes(), true).unwrap();
        assert!(c2.unknown.iter().any(|u| u.path == "settings.requires"));
    }

    const CHILD_BAD: &str = r#"[fairlead]
scope = "web"
marker = "other"
edition = 2

[settings]
purpose = "the web app"
folder = "gov"
fairlead-version = "9.9"
owners = ["@x"]

[scan]
exclude = ["dist/"]
max-file = 1

[comments]
rs = ["//"]

[policies]
"rules.*" = "open"

[agents]
targets = ["X.md"]
"#;

    #[test]
    fn r_syn_041__child_non_delegable_keys_are_hc003_and_inert() {
        let c = parse_control(&p("web/fairlead/control.toml"), CHILD_BAD.as_bytes(), false).unwrap();
        let mut keys: Vec<String> = c.violations.iter().map(|d| d.related.clone().unwrap_or_default()).collect();
        keys.sort();
        assert_eq!(
            keys,
            ["agents", "comments", "fairlead.edition", "fairlead.marker", "policies", "scan.max-file", "settings.folder", "settings.fairlead-version", "settings.owners"]
        );
        assert!(c.violations.iter().all(|d| d.code == codes::HN_C003 && d.hint.contains("fairlead new escalation")));
        assert_eq!(c.fairlead.marker, None, "the stripped marker does not apply");
        assert_eq!(c.settings.folder, None);
        assert!(c.settings.owners.is_empty());
        assert_eq!(c.settings.purpose.as_deref(), Some("the web app"), "delegable keys still apply");
        assert_eq!(c.scan.exclude, ["dist/"]);
        assert_eq!(c.scan.max_file, 2_000_000);
        assert!(c.agents.is_empty() && c.comments.is_empty());
        assert_eq!(c.dir, p("web"));
    }

    #[test]
    fn r_agt_013__vocabulary_keys_change_only_in_the_root_file() {
        let root_ok = parse_control(&p("fairlead/control.toml"), ROOT.as_bytes(), true).unwrap();
        assert!(root_ok.violations.is_empty());
        let c = parse_control(&p("web/fairlead/control.toml"), CHILD_BAD.as_bytes(), false).unwrap();
        assert!(c.violations.iter().any(|d| d.related.as_deref() == Some("fairlead.marker")));
        assert!(c.violations.iter().any(|d| d.related.as_deref() == Some("settings.fairlead-version")));
    }

    #[test]
    fn r_scope_056__child_may_hold_facts_without_violations() {
        let text = r#"[fairlead]
scope = "web"

[settings]
purpose = "web"

[scan]
exclude = ["dist/"]
skip = ["fixtures/**"]

[kinds.wid]
title = "widget"

[file-kinds.sheet]
extractor = "css"
binding = "import"

[[file]]
id = "file.web-styles"
kind = "stylesheet"
path = "app/styles.css"
extends = "root:file.styles"

[[vendor]]
path = "components.json"
tool = "shadcn"

[[ovr]]
target = "rule.no-raw-hex"
why = "legacy"
owner = "@platform"
exp = "2026-12-31"

[[allow]]
for = ["web-admin"]
target = "rules.no-raw-hex.stage"
why = "w"
exp = "2027-01-01"

[[req]]
to = "root"
target = "rule.x"
why = "w"

[[convention]]
glob = "**/*.gen.ts"
class = "external-generated"

[rules.web-only]
check = "pattern"
forbid = "TODO"
msg = "no TODO"
"#;
        let c = parse_control(&p("web/fairlead/control.toml"), text.as_bytes(), false).unwrap();
        assert!(c.violations.is_empty() && c.unknown.is_empty(), "{:?} {:?}", c.violations, c.unknown);
        assert_eq!((c.files.len(), c.vendors.len(), c.ovrs.len(), c.allows.len(), c.reqs.len(), c.conventions.len()), (1, 1, 1, 1, 1, 1));
        assert_eq!((c.kinds.len(), c.file_kinds.len(), c.rules.len()), (1, 1, 1));
    }

    #[test]
    fn r_scope_051_052__root_only_tables_in_a_child_are_violations() {
        for table in ["comments", "agents", "layout", "policies", "context", "baseline", "secrets", "exceptions", "eligibility"] {
            let text = format!("[fairlead]\nscope = \"web\"\n\n[{table}]\n");
            let c = parse_control(&p("web/fairlead/control.toml"), text.as_bytes(), false).unwrap();
            assert_eq!(c.violations.len(), 1, "{table}");
            assert_eq!(c.violations[0].related.as_deref(), Some(table));
        }
    }

    #[test]
    fn r_scope_065__kinds_exc_fails_the_whole_file() {
        let text = format!("{ROOT}\n[kinds.exc]\ntitle = \"x\"\n");
        let f = parse_control(&p("fairlead/control.toml"), text.as_bytes(), true).unwrap_err();
        assert!(f.diags[0].message.contains("exc"));
    }

    #[test]
    fn r_arch_076__control_content_hash_ignores_line_endings() {
        let a = parse_control(&p("fairlead/control.toml"), ROOT.as_bytes(), true).unwrap();
        let crlf = ROOT.replace('\n', "\r\n");
        let b = parse_control(&p("fairlead/control.toml"), crlf.as_bytes(), true).unwrap();
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a, b);
    }

    #[test]
    fn r_file_020__baseline_default_follows_folder_and_layout() {
        let text = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n[settings]\nfolder = \"gov\"\n[layout]\ngen = \"generated\"\n";
        let c = parse_control(&p("gov/control.toml"), text.as_bytes(), true).unwrap();
        assert_eq!(c.baseline.file, "gov/generated/baseline.txt");
        assert_eq!(c.layout.r#gen, "generated");
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-core r_syn_038 r_syn_033 r_syn_041 r_scope_051 r_scope_056 r_agt_013`. Expected: FAIL (compile error: `cannot find type ControlFile`). Note the violation tests read the offending key from `Diagnostic.related`; the implementation below sets `related = Some(dotted key)`.

- [ ] **Step 3: Append the implementation to `config/mod.rs`** (above the test module; also add `use` lines at the top).

```rust
use crate::diag::{codes, Location, Severity};
use crate::discover::DEFAULT_FOLDER;
use crate::hash::{hash_normalised, ContentHash};
use schema::{Ctx, join_key};
use std::collections::BTreeMap;

/// One parsed `control.toml` (contract 4.6). `dir` is the directory that OWNS the managerial folder.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ControlFile {
    pub path: RepoPath,
    pub dir: RepoPath,
    pub content_hash: ContentHash,
    pub fairlead: FairleadHeader,
    pub settings: Settings,
    pub scan: ScanConfig,
    pub comments: BTreeMap<String, Vec<String>>,
    pub agents: Vec<String>,
    pub kinds: BTreeMap<String, KindDecl>,
    pub file_kinds: BTreeMap<String, FileKindDecl>,
    pub layout: Layout,
    pub files: Vec<FileItemDecl>,
    pub vendors: Vec<VendorDecl>,
    pub ovrs: Vec<OvrDecl>,
    pub rules: BTreeMap<String, RuleDecl>,
    pub policies: PolicyTable,
    pub allows: Vec<AllowDecl>,
    pub reqs: Vec<ReqDecl>,
    pub conventions: Vec<ConventionDecl>,
    pub context: ContextConfig,
    pub baseline: BaselineConfig,
    pub secrets: SecretsConfig,
    pub exceptions: ExceptionsConfig,
    pub eligibility: EligibilityConfig,
    /// `HN-C002` (R-SYN-039): unknown keys and tables, kept for tools that rewrite the file.
    pub unknown: Vec<UnknownKey>,
    /// `HN-C003` (R-SYN-041, R-SCOPE-051/052/056): non-delegable keys in a child file; the keys are inert.
    pub violations: Vec<Diagnostic>,
}

impl ControlFile {
    /// The warnings and inert-key errors of this file: `HN-C002` per unknown key, then `violations`.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut out: Vec<Diagnostic> = self
            .unknown
            .iter()
            .map(|u| {
                Diagnostic::new(
                    codes::HN_C002,
                    Severity::Warn,
                    Location::at(self.path.clone(), u.line, 1),
                    format!("unknown control key or table `{}`", u.path),
                )
                .with_hint("remove it; keys of a later milestone warn as unknown in an older build")
            })
            .collect();
        out.extend(self.violations.iter().cloned());
        out
    }
}

/// Top-level tables a child may not hold (known 0.1 root-only tables).
const ROOT_ONLY_TABLES: [&str; 9] =
    ["comments", "agents", "layout", "policies", "context", "baseline", "secrets", "exceptions", "eligibility"];
/// Known keys that only the root may set, per table.
const ROOT_ONLY_KEYS: [(&str, &[&str]); 3] = [
    ("fairlead", &["edition", "marker"]),
    ("settings", &["folder", "fairlead-version", "fairlead-range", "index-values", "owners"]),
    ("scan", &["header-skip", "max-file", "max-directives"]),
];

fn violation(cx: &Ctx<'_>, dotted: &str) -> Diagnostic {
    let line = header::locate_key(cx.text, dotted);
    let mut d = Diagnostic::new(
        codes::HN_C003,
        Severity::Error,
        Location::at(cx.path.clone(), line, 1),
        format!("`{dotted}` is not delegable: only the root control file may set it (inert here)"),
    )
    .with_hint("fairlead new escalation (writes a [[req]] asking the ancestor)");
    d.related = Some(dotted.to_string());
    d
}

fn strip_non_delegable(cx: &Ctx<'_>, table: &mut toml::Table, out: &mut Vec<Diagnostic>) {
    for name in ROOT_ONLY_TABLES {
        if table.remove(name).is_some() {
            out.push(violation(cx, name));
        }
    }
    for (name, keys) in ROOT_ONLY_KEYS {
        if let Some(toml::Value::Table(t)) = table.get_mut(name) {
            for k in keys {
                if t.remove(*k).is_some() {
                    out.push(violation(cx, &join_key(name, k)));
                }
            }
        }
    }
}

fn sorted(mut diags: Vec<Diagnostic>) -> Vec<Diagnostic> {
    crate::diag::sort_diagnostics(&mut diags);
    diags
}

/// Pure whole-file parse. On failure the scope is unresolved (R-SYN-033). `is_root_dir` is true when
/// the file lives in the root directory's managerial folder.
pub fn parse_control(path: &RepoPath, bytes: &[u8], is_root_dir: bool) -> Result<ControlFile, ConfigFailure> {
    let (text, mut table) = header::parse_table(bytes).map_err(|f| f.with_path(path))?;
    let mut cx = Ctx::new(path, &text);
    let mut violations = Vec::new();
    if !is_root_dir {
        strip_non_delegable(&cx, &mut table, &mut violations);
    }
    let fairlead_tbl = match table.remove("fairlead") {
        Some(toml::Value::Table(h)) => h,
        _ => return Err(header::failure(&text, "fairlead", "a control file must contain a [fairlead] header table").with_path(path)),
    };
    let fairlead = header::header_from_table(&text, &fairlead_tbl).map_err(|f| f.with_path(path))?;
    let extra: toml::Table = fairlead_tbl.into_iter().filter(|(k, _)| !header::HEADER_KEYS.contains(&k.as_str())).collect();
    cx.unknown_in("fairlead", &extra);
    if is_root_dir {
        if fairlead.edition.is_none() {
            cx.error("fairlead", "the root control file requires `edition` in [fairlead] (C-34)");
        }
        if !fairlead.root {
            cx.error_code(codes::HN_C004, "fairlead.root", "the root control file must set `root = true` in [fairlead]");
        }
    }
    let settings = schema::parse_settings(&mut cx, &mut table);
    let scan = schema::parse_scan(&mut cx, &mut table);
    let comments = schema::parse_comments(&mut cx, &mut table);
    let agents = schema::parse_agents(&mut cx, &mut table);
    let kinds = schema::parse_kinds(&mut cx, &mut table);
    let file_kinds = schema::parse_file_kinds(&mut cx, &mut table);
    let layout = schema::parse_layout(&mut cx, &mut table);
    let files = schema::parse_files(&mut cx, &mut table);
    let vendors = schema::parse_vendors(&mut cx, &mut table);
    let ovrs = schema::parse_ovrs(&mut cx, &mut table);
    let rules = rules::parse_rules(&mut cx, &mut table);
    let policies = rules::parse_policies(&mut cx, &mut table);
    let allows = rules::parse_allows(&mut cx, &mut table);
    let reqs = rules::parse_reqs(&mut cx, &mut table);
    let conventions = rules::parse_conventions(&mut cx, &mut table);
    let context = rules::parse_context(&mut cx, &mut table);
    let mut baseline = rules::parse_baseline(&mut cx, &mut table);
    let secrets = rules::parse_secrets(&mut cx, &mut table);
    let exceptions = rules::parse_exceptions(&mut cx, &mut table);
    let eligibility = rules::parse_eligibility(&mut cx, &mut table);
    cx.unknown_in("", &table);

    if baseline.file.is_empty() {
        let folder = settings.folder.clone().unwrap_or_else(|| DEFAULT_FOLDER.to_string());
        baseline.file = format!("{folder}/{}/baseline.txt", layout.r#gen);
    }
    if !cx.diags.is_empty() {
        return Err(ConfigFailure { path: path.clone(), diags: sorted(cx.diags) });
    }
    let dir = path.parent().and_then(|p| p.parent()).unwrap_or_else(RepoPath::root);
    let mut unknown = cx.unknown;
    unknown.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(ControlFile {
        path: path.clone(),
        dir,
        content_hash: hash_normalised(bytes),
        fairlead,
        settings,
        scan,
        comments,
        agents,
        kinds,
        file_kinds,
        layout,
        files,
        vendors,
        ovrs,
        rules,
        policies,
        allows,
        reqs,
        conventions,
        context,
        baseline,
        secrets,
        exceptions,
        eligibility,
        unknown,
        violations: sorted(violations),
    })
}
```

Also replace the `use` block at the top of `config/mod.rs` with this complete list of module declarations and re-exports:

```rust
pub mod header;
pub mod rules;
pub mod schema;

pub use header::{parse_header_lenient, parse_pin_lenient, FairleadHeader};
pub use rules::*;
pub use schema::*;
```

Verify: `schema::Ctx` and `join_key` are `pub(crate)`; the glob `pub use schema::*` re-exports only the `pub` items, which is intended. A stripped `[settings]` table can end up empty and still be consumed by `parse_settings`.

- [ ] **Step 4: Run** `cargo test -p fairlead-core config::`. Expected: PASS (all config tests including Tasks 9 to 12).

- [ ] **Step 5: Commit** `feat(core): ControlFile and parse_control with delegation checks`.

---

### Task 14: config/approvals.rs: `approvals.toml`

Rules: R-CLI-037 / D-34 data shape; supports R-CHK-026 (fingerprint binding) and R-ARCH-102 (an unparseable approvals file is an error, never "no approvals").

**Files:**
- Create: `crates/fairlead-core/src/config/approvals.rs`
- Modify: `crates/fairlead-core/src/config/mod.rs` (`pub mod approvals; pub use approvals::{parse_approvals, Approval, ApprovalsFile};`)

**Interfaces (contract 4.6, fixed):** `Approval { fp: Fingerprint, target: Id, scope: ScopeSlug, approver: String, date: jiff::civil::Date, renewal: u32 }`, `ApprovalsFile { approvals: Vec<Approval> }`, `parse_approvals(&[u8]) -> Result<ApprovalsFile, ConfigFailure>`. File order is preserved (the file is append-only, written by `approve`).

- [ ] **Step 1: Write the failing tests.** Create `approvals.rs` with only this module.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;

    const TWO: &str = r#"[[approval]]
fp = "0123456789ab"
target = "rule.no-raw-hex"
scope = "web"
approver = "@dana"
date = 2026-10-09
renewal = 1

[[approval]]
fp = "zzzzzzzzzzzz"
target = "web:exc.x1"
scope = "root"
approver = "@lee"
date = "2026-10-10"
"#;

    #[test]
    fn r_cli_037__approvals_parse_in_file_order() {
        let f = parse_approvals(TWO.as_bytes()).unwrap();
        assert_eq!(f.approvals.len(), 2);
        let a = &f.approvals[0];
        assert_eq!(a.fp.as_str(), "0123456789ab");
        assert_eq!(a.target.to_string(), "rule.no-raw-hex");
        assert_eq!((a.scope.as_str(), a.approver.as_str(), a.renewal), ("web", "@dana", 1));
        assert_eq!(a.date.to_string(), "2026-10-09");
        let b = &f.approvals[1];
        assert_eq!((b.renewal, b.date.to_string().as_str()), (0, "2026-10-10"));
        assert_eq!(b.target.to_string(), "web:exc.x1");
    }

    #[test]
    fn r_cli_037__empty_file_has_no_approvals() {
        assert!(parse_approvals(b"").unwrap().approvals.is_empty());
        assert!(parse_approvals(b"# nothing yet\n").unwrap().approvals.is_empty());
    }

    #[test]
    fn r_cli_037__bad_records_fail_closed_with_hc001() {
        for bad in [
            "[[approval]]\nfp = \"short\"\ntarget = \"rule.x\"\nscope = \"a\"\napprover = \"@a\"\ndate = 2026-01-01\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"nodot\"\nscope = \"a\"\napprover = \"@a\"\ndate = 2026-01-01\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"Bad Scope\"\napprover = \"@a\"\ndate = 2026-01-01\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"a\"\napprover = \"\"\ndate = 2026-01-01\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"a\"\napprover = \"@a\"\ndate = \"soon\"\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"a\"\napprover = \"@a\"\n",
            "approval = 5\n",
            "[[approval]\n",
        ] {
            let f = parse_approvals(bad.as_bytes()).unwrap_err();
            assert!(f.diags.iter().all(|d| d.code == codes::HN_C001), "{bad:?}");
            assert!(!f.diags.is_empty(), "{bad:?}");
        }
    }

    #[test]
    fn r_cli_037__crlf_and_bom_are_accepted() {
        let text = format!("\u{feff}{}", TWO.replace('\n', "\r\n"));
        assert_eq!(parse_approvals(text.as_bytes()).unwrap().approvals.len(), 2);
    }
}
```

- [ ] **Step 2: Add the module line. Run** `cargo test -p fairlead-core r_cli_037`. Expected: FAIL (compile error: `cannot find function parse_approvals`).

- [ ] **Step 3: Write the implementation** (above the tests).

```rust
//! `<folder>/approvals.toml`: `[[approval]]` records written only by `fairlead approve` (D-34, R-CLI-037).
use crate::config::header::parse_table;
use crate::config::schema::{take_date_string, take_string, take_table_array, take_u32, Ctx};
use crate::config::ConfigFailure;
use crate::diag::sort_diagnostics;
use crate::hash::Fingerprint;
use crate::path::RepoPath;
use fairlead_syntax::ids::{Id, ScopeSlug};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Approval {
    pub fp: Fingerprint,
    pub target: Id,
    pub scope: ScopeSlug,
    pub approver: String,
    pub date: jiff::civil::Date,
    pub renewal: u32,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ApprovalsFile {
    pub approvals: Vec<Approval>,
}

/// Parse the whole file. Any bad record fails the file: a malformed approval must never be read as
/// "not approved" silently. The failure path is the root placeholder; callers use `with_path`.
pub fn parse_approvals(bytes: &[u8]) -> Result<ApprovalsFile, ConfigFailure> {
    let (text, mut table) = parse_table(bytes)?;
    let mut cx = Ctx::new(&RepoPath::root(), &text);
    let mut out = Vec::new();
    for mut t in take_table_array(&mut cx, &mut table, "", "approval") {
        let before = cx.diags.len();
        let fp = take_string(&mut cx, &mut t, "approval", "fp");
        let target = take_string(&mut cx, &mut t, "approval", "target");
        let scope = take_string(&mut cx, &mut t, "approval", "scope");
        let approver = take_string(&mut cx, &mut t, "approval", "approver");
        let date = take_date_string(&mut cx, &mut t, "approval", "date");
        let renewal = take_u32(&mut cx, &mut t, "approval", "renewal").unwrap_or(0);
        let (Some(fp), Some(target), Some(scope), Some(approver), Some(date)) = (fp, target, scope, approver, date) else {
            if cx.diags.len() == before {
                cx.error("approval", "an [[approval]] needs fp, target, scope, approver and date");
            }
            continue;
        };
        let fp = Fingerprint::parse(&fp).map_err(|e| cx.error("approval.fp", format!("invalid fingerprint {fp:?}: {e}"))).ok();
        let target = Id::parse(&target).map_err(|e| cx.error("approval.target", format!("invalid target {target:?}: {e:?}"))).ok();
        let scope = ScopeSlug::parse(&scope).map_err(|e| cx.error("approval.scope", format!("invalid scope {scope:?}: {e:?}"))).ok();
        let date = date.parse::<jiff::civil::Date>().ok();
        if approver.trim().is_empty() {
            cx.error("approval.approver", "approver must not be empty");
        }
        if let (Some(fp), Some(target), Some(scope), Some(date)) = (fp, target, scope, date) {
            if !approver.trim().is_empty() {
                out.push(Approval { fp, target, scope, approver, date, renewal });
            }
        }
    }
    if cx.diags.is_empty() {
        Ok(ApprovalsFile { approvals: out })
    } else {
        sort_diagnostics(&mut cx.diags);
        Err(ConfigFailure { path: RepoPath::root(), diags: cx.diags })
    }
}
```

Verify: `.map_err(|e| cx.error(...)).ok()` uses the closure's `()` as the error type; if the borrow checker rejects capturing `cx` inside the closure while `cx` is also borrowed elsewhere, rewrite each as a `match`. The `take_table_array` call turns `approval = 5` into an `HN-C001` ("must be an array of tables"). Unknown keys inside a record are reported by none of these calls; add `cx.unknown_in("approval", &t)` if P4 wants them (they are ignored in 0.1).

- [ ] **Step 4: Run** `cargo test -p fairlead-core r_cli_037`. Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): approvals.toml parser`.

---

### Task 15: discover.rs (part 2): root lookup and the discovery I/O

Rules: R-SCOPE-004, R-CLI-004, R-ARCH-010, R-ARCH-011, R-ARCH-013, R-CLI-005 (enforcement point), D-31, D-33, C-34, C-12.

**Files:**
- Modify: `crates/fairlead-core/src/discover.rs` (append; the `pin_tests` module of Task 6 stays)
- Modify: `crates/fairlead-core/src/testutil.rs` (append `test_root`, `ROOT_CONTROL`, `CHILD_CONTROL`)

**Interfaces (contract 4.4, names fixed):** `RootSource { Flag, Env, Search }`, `Root { dir, folder, source, edition, pin, config_hash }`, `DiscoverRequest { root_flag, env_root, cwd, binary_version, require_root }`, `Discovered { root, root_control, cwd_rel }`, `RootProbe { NoControl, NotRoot, Root }`, `search_upward(start, probe) -> Option<Utf8PathBuf>`, `discover(&DiscoverRequest) -> Result<Discovered, Error>`. Additions: `discover_optional` (returns `Ok(None)` when there is no root; `init` uses it because `Discovered` cannot represent "no root", see Contract issues), `read_env_root()` (the only `std::env` read in the crate).

Behaviour fixed here:
1. Source order: `root_flag`, then `env_root`, then the upward search from `cwd`. The search accepts only `<dir>/fairlead/control.toml` whose `[fairlead]` header has `root = true`; ordinary managerial folders are skipped; the nearest root wins; the version-control top level is never consulted.
2. An unparseable control file met during the search stops it with `Error::Config` (fail closed, R-ARCH-011): a broken scope must never let the search fall through to a farther root.
3. For `--root`/`FAIRLEAD_ROOT` the default folder is tried first; otherwise the immediate sub-folders of the directory are scanned (sorted) for a `control.toml` with `root = true`, so a renamed managerial folder (D-30) is still found when the root is given explicitly.
4. Order of work: lenient header and pin first (`parse_pin_lenient`), pin refusal, only then the full `parse_control` (R-ARCH-013). `edition != 1` is a pin refusal (C-34); a missing `edition` is `HN-C001`.
5. `[settings] folder` that differs from the real folder name is `HN-C005`.
6. `cwd_rel` is `RepoPath::root()` when the working directory is outside the root. `Root.config_hash` is the normalised hash of the root control file's bytes; the hash over all control files is `config::config_hash_of` (Task 17, see Contract issues).

- [ ] **Step 1: Append to `testutil.rs`.**

```rust
use crate::discover::{Edition, Pin, Root, RootSource};

/// A minimal valid root control file.
pub const ROOT_CONTROL: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n";
/// A minimal valid child control file.
pub const CHILD_CONTROL: &str = "[fairlead]\nscope = \"web\"\n";

/// A `Root` for tests that do not run discovery.
pub fn test_root(dir: &Utf8Path) -> Root {
    Root {
        dir: dir.to_path_buf(),
        folder: "fairlead".to_string(),
        source: RootSource::Flag,
        edition: Edition(1),
        pin: Pin { version: None, edition: Edition(1) },
        config_hash: crate::hash::hash_raw(b""),
    }
}
```

- [ ] **Step 2: Write the failing tests** (append a second test module to `discover.rs`).

```rust
#[cfg(test)]
mod root_tests {
    use super::*;
    use crate::error::{Error, ErrorClass};
    use crate::testutil::{utf8_tmp, write_file, CHILD_CONTROL, ROOT_CONTROL};

    fn req(cwd: &Utf8Path) -> DiscoverRequest {
        DiscoverRequest {
            root_flag: None,
            env_root: None,
            cwd: cwd.to_path_buf(),
            binary_version: Version::new(0, 1, 0),
            require_root: true,
        }
    }

    fn canon(p: &Utf8Path) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(dunce::canonicalize(p.as_std_path()).unwrap()).unwrap()
    }

    #[test]
    fn r_scope_004__search_upward_is_pure_and_nearest_first() {
        let mut seen = Vec::new();
        let found = search_upward(Utf8Path::new("/a/b/c"), |d| {
            seen.push(d.to_string());
            match d.as_str() {
                "/a/b" => RootProbe::Root,
                "/a" => RootProbe::Root,
                "/a/b/c" => RootProbe::NotRoot,
                _ => RootProbe::NoControl,
            }
        });
        assert_eq!(found, Some(Utf8PathBuf::from("/a/b")));
        assert_eq!(seen, ["/a/b/c", "/a/b"], "the search stops at the nearest root");
        assert_eq!(search_upward(Utf8Path::new("/x/y"), |_| RootProbe::NotRoot), None);
    }

    #[test]
    fn r_scope_004__flag_then_env_then_search() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "a/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "b/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "a/sub/x.txt", "");
        let mut r = req(&t.join("a/sub"));
        let d = discover(&r).unwrap();
        assert_eq!((d.root.source, d.root.dir.clone()), (RootSource::Search, canon(&t.join("a"))));
        assert_eq!(d.cwd_rel.as_str(), "sub");
        assert_eq!(d.root.folder, "fairlead");
        assert_eq!(d.root.edition, Edition(1));
        r.env_root = Some(t.join("b"));
        let d = discover(&r).unwrap();
        assert_eq!((d.root.source, d.root.dir.clone()), (RootSource::Env, canon(&t.join("b"))));
        assert!(d.cwd_rel.is_root(), "a working directory outside the root maps to the root path");
        r.root_flag = Some(t.join("a"));
        let d = discover(&r).unwrap();
        assert_eq!((d.root.source, d.root.dir), (RootSource::Flag, canon(&t.join("a"))));
    }

    #[test]
    fn r_cli_004__ordinary_managerial_folders_are_skipped() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "repo/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "repo/pkg/fairlead/control.toml", CHILD_CONTROL);
        write_file(&t, "repo/pkg/src/a.rs", "");
        let d = discover(&req(&t.join("repo/pkg/src"))).unwrap();
        assert_eq!(d.root.dir, canon(&t.join("repo")));
        assert_eq!(d.cwd_rel.as_str(), "pkg/src");
    }

    #[test]
    fn r_cli_004__nearest_root_true_wins_so_a_mounted_subroot_stops_the_search() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "vendor/lib/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "vendor/lib/src/x.rs", "");
        let d = discover(&req(&t.join("vendor/lib/src"))).unwrap();
        assert_eq!(d.root.dir, canon(&t.join("vendor/lib")));
    }

    #[test]
    fn r_arch_010__the_vcs_top_level_is_never_a_root() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".git/HEAD", "ref: refs/heads/main\n");
        write_file(&t, "proj/src/a.rs", "");
        assert!(discover_optional(&req(&t.join("proj/src"))).unwrap().is_none());
        write_file(&t, "proj/fairlead/control.toml", ROOT_CONTROL);
        let d = discover(&req(&t.join("proj/src"))).unwrap();
        assert_eq!(d.root.dir, canon(&t.join("proj")));
    }

    #[test]
    fn r_cli_004__no_root_is_exit_2_and_never_invented() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "src/a.rs", "");
        let e = discover(&req(&t.join("src"))).unwrap_err();
        assert!(matches!(e, Error::NoRoot { .. }), "{e}");
        assert_eq!(e.class(), ErrorClass::Usage);
        assert!(e.to_string().contains("fairlead init"));
        let mut r = req(&t);
        r.require_root = false;
        assert!(discover_optional(&r).unwrap().is_none());
        r.root_flag = Some(t.join("src"));
        let e = discover(&r).unwrap_err();
        assert!(matches!(e, Error::NoRoot { .. }), "a flag directory without a root control file is NoRoot");
        r.root_flag = Some(t.join("missing"));
        assert_eq!(discover(&r).unwrap_err().class(), ErrorClass::Usage, "NotFound on a user path is a usage error");
    }

    #[test]
    fn r_arch_011__unparseable_control_on_the_path_fails_closed() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "repo/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "repo/pkg/fairlead/control.toml", "[fairlead\n");
        write_file(&t, "repo/pkg/src/a.rs", "");
        let e = discover(&req(&t.join("repo/pkg/src"))).unwrap_err();
        assert!(matches!(e, Error::Config(_)), "must not fall through to the farther root: {e}");
        assert_eq!(e.class(), ErrorClass::Usage);
    }

    #[test]
    fn r_arch_011__root_without_edition_or_with_wrong_types_is_hc001() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "a/fairlead/control.toml", "[fairlead]\nroot = true\nscope = \"root\"\n");
        let e = discover(&req(&t.join("a"))).unwrap_err();
        assert!(matches!(&e, Error::Config(f) if f.diags[0].code == crate::diag::codes::HN_C001), "{e}");
        write_file(&t, "b/fairlead/control.toml", &format!("{ROOT_CONTROL}[scan]\nmax-file = \"big\"\n"));
        let e = discover(&req(&t.join("b"))).unwrap_err();
        assert!(matches!(e, Error::Config(_)), "{e}");
    }

    #[test]
    fn r_arch_013__pin_is_checked_before_the_full_parse() {
        let (_g, t) = utf8_tmp();
        let bad_rest = "[settings]\nfairlead-version = \"99.0\"\n[scan]\nmax-file = \"not a number\"\n";
        write_file(&t, "a/fairlead/control.toml", &format!("{ROOT_CONTROL}{bad_rest}"));
        let e = discover(&req(&t.join("a"))).unwrap_err();
        match &e {
            Error::Pin(r) => assert_eq!(r.install_line, "cargo install fairlead --version 99.0.0 --locked"),
            other => panic!("expected the pin refusal first, got {other}"),
        }
        assert_eq!(e.class(), ErrorClass::Refused);
        write_file(&t, "b/fairlead/control.toml", &format!("{ROOT_CONTROL}{}", bad_rest.replace("99.0", "0.1")));
        assert!(matches!(discover(&req(&t.join("b"))).unwrap_err(), Error::Config(_)));
    }

    #[test]
    fn r_cli_005__other_edition_is_refused_and_a_newer_binary_proceeds() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "a/fairlead/control.toml", "[fairlead]\nedition = 2\nroot = true\nscope = \"root\"\n[future]\nx = 1\n");
        assert!(matches!(discover(&req(&t.join("a"))).unwrap_err(), Error::Pin(_)));
        write_file(&t, "b/fairlead/control.toml", &format!("{ROOT_CONTROL}[settings]\nfairlead-version = \"0.0.9\"\n"));
        let d = discover(&req(&t.join("b"))).unwrap();
        assert_eq!(d.root.pin.version, Some(Version::new(0, 0, 9)));
        assert!(matches!(pin_check(&Version::new(0, 1, 0), SUPPORTED_EDITION, &d.root.pin), PinVerdict::OkNewerBinary { .. }));
    }

    #[test]
    fn r_scope_004__folder_mismatch_is_hc005_and_renamed_folders_are_found_by_flag() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "a/fairlead/control.toml", &format!("{ROOT_CONTROL}[settings]\nfolder = \"gov\"\n"));
        let e = discover(&req(&t.join("a"))).unwrap_err();
        assert!(matches!(&e, Error::Config(f) if f.diags[0].code == crate::diag::codes::HN_C005), "{e}");
        write_file(&t, "b/gov/control.toml", &format!("{ROOT_CONTROL}[settings]\nfolder = \"gov\"\n"));
        let mut r = req(&t.join("b"));
        r.root_flag = Some(t.join("b"));
        let d = discover(&r).unwrap();
        assert_eq!(d.root.folder, "gov");
        assert_eq!(d.root_control.path.as_str(), "gov/control.toml");
    }

    #[test]
    fn r_arch_077__root_config_hash_is_the_normalised_hash_of_the_root_control_bytes() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "a/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "b/fairlead/control.toml", &ROOT_CONTROL.replace('\n', "\r\n"));
        let a = discover(&req(&t.join("a"))).unwrap();
        let b = discover(&req(&t.join("b"))).unwrap();
        assert_eq!(a.root.config_hash, crate::hash::hash_normalised(ROOT_CONTROL.as_bytes()));
        assert_eq!(a.root.config_hash, b.root.config_hash);
        assert_eq!(a.root_control.fairlead.scope.as_str(), "root");
    }
}
```

- [ ] **Step 3: Run** `cargo test -p fairlead-core r_scope_004 r_cli_004 r_arch_010 r_arch_011 r_arch_013`. Expected: FAIL (compile error: `cannot find struct DiscoverRequest`).

- [ ] **Step 4: Append the implementation** (above the test modules; move the `use` lines to the top of the file).

```rust
use crate::config::{parse_control, parse_header_lenient, parse_pin_lenient, ConfigFailure, ControlFile};
use crate::diag::{codes, Diagnostic, Location, Severity};
use crate::error::Error;
use crate::hash::{hash_normalised, ContentHash};
use crate::path::RepoPath;
use camino::{Utf8Path, Utf8PathBuf};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RootSource {
    Flag,
    Env,
    Search,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Root {
    /// Absolute, `dunce`-cleaned.
    pub dir: Utf8PathBuf,
    /// Managerial folder name (`[settings] folder`, default `fairlead`).
    pub folder: String,
    pub source: RootSource,
    pub edition: Edition,
    pub pin: Pin,
    /// Normalised hash of the root control file; the hash over every control file is
    /// `config::config_hash_of` (cache key).
    pub config_hash: ContentHash,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiscoverRequest {
    pub root_flag: Option<Utf8PathBuf>,
    pub env_root: Option<Utf8PathBuf>,
    pub cwd: Utf8PathBuf,
    pub binary_version: Version,
    /// False only for `init`, which calls `discover_optional`.
    pub require_root: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Discovered {
    pub root: Root,
    pub root_control: ControlFile,
    pub cwd_rel: RepoPath,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RootProbe {
    NoControl,
    NotRoot,
    Root,
}

/// Pure core of the upward search: the nearest ancestor-or-self for which `probe` says `Root`.
pub fn search_upward<F: FnMut(&Utf8Path) -> RootProbe>(start: &Utf8Path, mut probe: F) -> Option<Utf8PathBuf> {
    start.ancestors().find(|dir| probe(*dir) == RootProbe::Root).map(Utf8Path::to_path_buf)
}

/// `FAIRLEAD_ROOT`, if set and non-empty. The ONLY environment read in fairlead-core (the bin passes the
/// value in `DiscoverRequest::env_root`).
pub fn read_env_root() -> Option<Utf8PathBuf> {
    std::env::var("FAIRLEAD_ROOT").ok().filter(|v| !v.is_empty()).map(Utf8PathBuf::from)
}

enum Probed {
    No,
    NotRoot,
    Root,
    Broken(ConfigFailure),
}

fn probe_default(dir: &Utf8Path) -> Probed {
    let control = dir.join(DEFAULT_FOLDER).join("control.toml");
    let Ok(bytes) = std::fs::read(control.as_std_path()) else { return Probed::No };
    match parse_header_lenient(&bytes) {
        Ok(h) if h.root => Probed::Root,
        Ok(_) => Probed::NotRoot,
        Err(f) => Probed::Broken(f),
    }
}

fn control_rel(folder: &str) -> RepoPath {
    RepoPath::new(&format!("{folder}/control.toml")).unwrap_or_else(|_| RepoPath::root())
}

/// Upward search; an unreadable header on the way stops it with an error (R-ARCH-011).
fn search(cwd: &Utf8Path) -> Result<Option<Utf8PathBuf>, Error> {
    let mut broken: Option<(Utf8PathBuf, ConfigFailure)> = None;
    let found = search_upward(cwd, |dir| match probe_default(dir) {
        Probed::No => RootProbe::NoControl,
        Probed::NotRoot => RootProbe::NotRoot,
        Probed::Root => RootProbe::Root,
        Probed::Broken(f) => {
            broken = Some((dir.to_path_buf(), f));
            RootProbe::Root
        }
    });
    if let Some((dir, f)) = broken {
        let mut failure = f.with_path(&control_rel(DEFAULT_FOLDER));
        for d in &mut failure.diags {
            d.message = format!("{} (control file in {dir})", d.message);
        }
        return Err(Error::from(failure));
    }
    Ok(found)
}

fn absolutize(p: &Utf8Path, cwd: &Utf8Path) -> Utf8PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

fn canonical(p: &Utf8Path) -> Result<Utf8PathBuf, Error> {
    let c = dunce::canonicalize(p.as_std_path()).map_err(|e| Error::io(p.as_str(), "resolve", &e, true))?;
    Utf8PathBuf::from_path_buf(c).map_err(|bad| Error::Usage(format!("path is not valid UTF-8: {}", bad.display())))
}

/// The root control file of an explicitly given root directory: the default folder first, then the
/// immediate sub-folders (sorted) whose `control.toml` header says `root = true`.
fn find_root_control(dir: &Utf8Path) -> Result<Option<(String, Vec<u8>)>, Error> {
    let default = dir.join(DEFAULT_FOLDER).join("control.toml");
    if let Ok(bytes) = std::fs::read(default.as_std_path()) {
        return Ok(Some((DEFAULT_FOLDER.to_string(), bytes)));
    }
    let rd = std::fs::read_dir(dir.as_std_path()).map_err(|e| Error::io(dir.as_str(), "read directory", &e, true))?;
    let mut names: Vec<String> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    for name in names {
        let p = dir.join(&name).join("control.toml");
        if let Ok(bytes) = std::fs::read(p.as_std_path()) {
            if parse_header_lenient(&bytes).is_ok_and(|h| h.root) {
                return Ok(Some((name, bytes)));
            }
        }
    }
    Ok(None)
}

/// Discovery that tolerates "no root" (`Ok(None)`), for `init`.
pub fn discover_optional(req: &DiscoverRequest) -> Result<Option<Discovered>, Error> {
    let (dir, source) = if let Some(f) = &req.root_flag {
        (absolutize(f, &req.cwd), RootSource::Flag)
    } else if let Some(e) = &req.env_root {
        (absolutize(e, &req.cwd), RootSource::Env)
    } else {
        match search(&req.cwd)? {
            Some(found) => (found, RootSource::Search),
            None => return Ok(None),
        }
    };
    let dir = canonical(&dir)?;
    let Some((folder, bytes)) = find_root_control(&dir)? else {
        return Err(Error::NoRoot { searched_from: dir.to_string() });
    };
    let rel = control_rel(&folder);
    // 1. Lenient header and pin first; nothing else is interpreted before the pin check (R-ARCH-013).
    let (header, pin) = parse_pin_lenient(&bytes).map_err(|f| Error::from(f.with_path(&rel)))?;
    if !header.root {
        return Err(Error::Usage(format!("{dir} is not a codebase root: {rel} lacks `root = true`")));
    }
    if let PinVerdict::Refused(refusal) = pin_check(&req.binary_version, SUPPORTED_EDITION, &pin) {
        return Err(Error::Pin(refusal));
    }
    // 2. Full parse of the root control file.
    let root_control = parse_control(&rel, &bytes, true).map_err(Error::from)?;
    let declared = root_control.settings.folder.clone().unwrap_or_else(|| DEFAULT_FOLDER.to_string());
    if declared != folder {
        let d = Diagnostic::new(
            codes::HN_C005,
            Severity::Error,
            Location::at(rel.clone(), 1, 1),
            format!("folder name mismatch: [settings] folder is {declared:?} but the managerial folder is {folder:?}"),
        )
        .with_hint("rename the folder or the key; renaming is done by `fairlead upgrade`");
        return Err(Error::from(ConfigFailure { path: rel, diags: vec![d] }));
    }
    let cwd_abs = canonical(&absolutize(&req.cwd, &req.cwd)).unwrap_or_else(|_| req.cwd.clone());
    let cwd_rel = cwd_abs
        .strip_prefix(&dir)
        .ok()
        .and_then(|r| RepoPath::from_native_rel(r).ok())
        .unwrap_or_else(RepoPath::root);
    let root = Root { dir, folder, source, edition: pin.edition, pin, config_hash: hash_normalised(&bytes) };
    Ok(Some(Discovered { root, root_control, cwd_rel }))
}

/// S1: flag, then `FAIRLEAD_ROOT`, then the upward search (R-CLI-004, D-31).
pub fn discover(req: &DiscoverRequest) -> Result<Discovered, Error> {
    discover_optional(req)?.ok_or_else(|| Error::NoRoot { searched_from: req.cwd.to_string() })
}
```

Verify: `Utf8Path::ancestors()` yields `&Utf8Path` from the path itself upward, including itself. `dunce::canonicalize` strips `\\?\` on Windows. On macOS `canonicalize` resolves `/var` to `/private/var`, which is why tests compare against `canon(..)`.

- [ ] **Step 5: Run** `cargo test -p fairlead-core discover::`. Expected: PASS. **Commit** `feat(core): root discovery, pin enforcement and fail-closed control lookup`.

---

### Task 16: walk.rs: sorted parallel listing

Rules: R-ARCH-020, R-ARCH-021, R-ARCH-022, R-ARCH-023 (size half; NUL and UTF-8 halves are Task 22), R-ARCH-024, R-ARCH-025, R-SCOPE-006, R-SCOPE-007, R-FILE-023 (ignored managerial path), R-SYN-040 / R-SYN-027 (`HN-P001`).

**Files:**
- Create: `crates/fairlead-core/src/walk.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod walk;`)

**Interfaces (contract 4.4, names fixed):** `WalkOptions { excludes, max_file, threads }`, `WalkEntry { path, size, mtime_ns }`, `SkipReason { TooLarge, Binary, NotUtf8, SymlinkOrJunction, Excluded }`, `WalkResult { files, skipped, ignored_managerial, diags }`, `walk(&Root, &WalkOptions) -> Result<WalkResult, Error>`. Additions: `Default` for `WalkOptions`, `WalkOptions::from_root_control`, `path_diagnostics`, `SkipReason` serde derives (it is stored in the cached `FileRecord`).

Behaviour fixed here (resolves the contract's open points; see Contract issues):
- `files` lists every regular file that is not ignored, excluded or inside a stop directory, including files larger than `max_file` and binary files (extract turns them into skip records, C-37). Over-large files are additionally listed in `skipped` as `TooLarge` and get one `HN-I001` info diagnostic here. `Binary` (NUL in the first 8 KB) and `NotUtf8` are discovered only when a file is read (Task 22).
- Stop rules: a directory containing a `.git` entry that is a FILE (a submodule checkout or a linked worktree) is not entered and not listed; `.git` and `.fairlead` entries are never listed; the root itself may be a linked worktree.
- Ignore rules: `.gitignore` files and `.fairleadignore` files only (`require_git(false)`, `git_global(false)`, no `.git/info/exclude`, no `.ignore`, no parent-directory ignore files), plus `opts.excludes` in gitignore syntax. Nothing is hard-coded: with no excludes `target/` is listed (R-ARCH-021).
- Symlinks and junctions are never followed; they are recorded as `SymlinkOrJunction`, and inside a managerial folder they are `HN-C001` errors.
- `ignored_managerial` lists `<dir>/<folder>/control.toml` files that exist on disk but were not listed because an ignore rule or exclude covers them (`HN-C001`, C-6).

- [ ] **Step 1: Write the failing tests.** Create `walk.rs` with only this module.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;
    use crate::testutil::{test_root, utf8_tmp, write_bytes, write_file};

    fn paths(r: &WalkResult) -> Vec<&str> {
        r.files.iter().map(|f| f.path.as_str()).collect()
    }

    fn opts(excludes: &[&str]) -> WalkOptions {
        WalkOptions { excludes: excludes.iter().map(|s| s.to_string()).collect(), ..WalkOptions::default() }
    }

    #[test]
    fn r_arch_024__result_is_sorted_bytewise_and_thread_count_independent() {
        let (_g, t) = utf8_tmp();
        for f in ["b.txt", "a/z.txt", "a/b.txt", "B.txt", "A/x.txt", "a.txt"] {
            write_file(&t, f, f);
        }
        let root = test_root(&t);
        let one = walk(&root, &WalkOptions { threads: Some(1), ..WalkOptions::default() }).unwrap();
        let many = walk(&root, &WalkOptions { threads: Some(4), ..WalkOptions::default() }).unwrap();
        assert_eq!(paths(&one), ["A/x.txt", "B.txt", "a.txt", "a/b.txt", "a/z.txt", "b.txt"]);
        assert_eq!(one, many);
        assert_eq!(one.files[0].size, "A/x.txt".len() as u64);
        assert!(one.files[0].mtime_ns > 0);
    }

    #[test]
    fn r_arch_025__walk_works_outside_a_git_repository_and_honours_gitignore() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".gitignore", "ignored.txt\nbuild/\n");
        write_file(&t, "ignored.txt", "x");
        write_file(&t, "build/out.js", "x");
        write_file(&t, "src/keep.rs", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r), [".gitignore", "src/keep.rs"], "hidden files are listed, ignored ones are not");
    }

    #[test]
    fn r_arch_020__fairleadignore_is_honoured_and_git_info_exclude_is_not_read() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".fairleadignore", "scratch/\n");
        write_file(&t, "scratch/a.txt", "x");
        write_file(&t, ".git/info/exclude", "secret.txt\n");
        write_file(&t, "secret.txt", "x");
        write_file(&t, "src/a.rs", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r), [".fairleadignore", "secret.txt", "src/a.rs"]);
    }

    #[test]
    fn r_arch_021__default_excludes_live_in_options_not_in_code() {
        let (_g, t) = utf8_tmp();
        for f in ["target/debug/x.rs", "node_modules/p/i.js", "src/a.gen.ts", "src/b.ts"] {
            write_file(&t, f, "x");
        }
        let root = test_root(&t);
        let none = walk(&root, &WalkOptions::default()).unwrap();
        assert_eq!(none.files.len(), 4, "nothing is excluded unless the control file says so");
        let some = walk(&root, &opts(&["target/", "node_modules/", "**/*.gen.ts"])).unwrap();
        assert_eq!(paths(&some), ["src/b.ts"]);
    }

    #[test]
    fn r_arch_022__git_and_cache_dirs_are_never_listed() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".git/config", "x");
        write_file(&t, ".fairlead/cache/x.json", "{}");
        write_file(&t, "a/.fairlead/y", "x");
        write_file(&t, "a.txt", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r), ["a.txt"]);
    }

    #[test]
    fn r_arch_022__submodule_and_linked_worktree_boundaries_stop_the_walk() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "sub/.git", "gitdir: ../.git/modules/sub\n");
        write_file(&t, "sub/inner.rs", "x");
        write_file(&t, "plain/inner.rs", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r), ["plain/inner.rs"]);
        // The root itself may be a linked worktree: its own `.git` file is not a boundary.
        let (_g2, t2) = utf8_tmp();
        write_file(&t2, ".git", "gitdir: /elsewhere\n");
        write_file(&t2, "src/a.rs", "x");
        let r2 = walk(&test_root(&t2), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r2), ["src/a.rs"]);
    }

    #[cfg(unix)]
    #[test]
    fn r_scope_006__symlinks_are_not_followed_and_are_errors_inside_a_managerial_folder() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "real/x.txt", "x");
        write_file(&t, "fairlead/control.toml", "x");
        std::os::unix::fs::symlink(t.join("real").as_std_path(), t.join("link").as_std_path()).unwrap();
        std::os::unix::fs::symlink(t.join("real/x.txt").as_std_path(), t.join("fairlead/docs-link").as_std_path()).unwrap();
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r), ["fairlead/control.toml", "real/x.txt"]);
        assert!(r.skipped.contains(&(RepoPath::new("link").unwrap(), SkipReason::SymlinkOrJunction)));
        let errs: Vec<&Diagnostic> = r.diags.iter().filter(|d| d.code == codes::HN_C001).collect();
        assert_eq!(errs.len(), 1, "{:?}", r.diags);
        assert_eq!(errs[0].location.path.as_str(), "fairlead/docs-link");
    }

    #[test]
    fn r_scope_007__case_collisions_and_hygiene_are_hn_p001_on_every_platform() {
        let mk = |p: &str| WalkEntry { path: RepoPath::new(p).unwrap(), size: 0, mtime_ns: 0 };
        let files = vec![mk("Ui/a.rs"), mk("ui/b.rs"), mk("src/CON.txt"), mk("docs/ok.md"), mk("cafe\u{301}.txt")];
        let d = path_diagnostics(&files);
        let msgs: Vec<String> = d.iter().map(|x| x.message.clone()).collect();
        assert!(d.iter().all(|x| x.code == codes::HN_P001));
        assert!(msgs.iter().any(|m| m.contains("Ui") && m.contains("ui") && m.contains("case")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("CON.txt") && m.contains("reserved")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("NFC")), "{msgs:?}");
        assert_eq!(d.len(), 3);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn r_scope_007__walk_reports_a_real_case_collision_on_a_case_sensitive_filesystem() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "Ui/a.rs", "x");
        write_file(&t, "ui/b.rs", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert!(r.diags.iter().any(|d| d.code == codes::HN_P001 && d.message.contains("Ui")), "{:?}", r.diags);
    }

    #[test]
    fn r_file_023__managerial_path_matched_by_an_ignore_rule_is_reported() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".gitignore", "fairlead/\n");
        write_file(&t, "fairlead/control.toml", "x");
        write_file(&t, "web/fairlead/control.toml", "x");
        write_file(&t, "web/src/b.rs", "x");
        write_file(&t, "src/a.rs", "x");
        let r = walk(&test_root(&t), &opts(&["web/fairlead/"])).unwrap();
        assert_eq!(
            r.ignored_managerial,
            vec![RepoPath::new("fairlead/control.toml").unwrap(), RepoPath::new("web/fairlead/control.toml").unwrap()]
        );
        assert_eq!(r.diags.iter().filter(|d| d.code == codes::HN_C001).count(), 2);
        assert!(!paths(&r).contains(&"fairlead/control.toml"));
    }

    #[test]
    fn r_file_023__a_listed_managerial_folder_is_not_reported() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert!(r.ignored_managerial.is_empty() && r.diags.is_empty());
    }

    #[test]
    fn r_arch_023__over_large_files_are_listed_skipped_and_noted_without_being_read() {
        let (_g, t) = utf8_tmp();
        write_bytes(&t, "big.bin", &vec![b'a'; 3000]);
        write_file(&t, "small.txt", "ab");
        let r = walk(&test_root(&t), &WalkOptions { max_file: 1000, ..WalkOptions::default() }).unwrap();
        assert_eq!(paths(&r), ["big.bin", "small.txt"]);
        assert_eq!(r.skipped, vec![(RepoPath::new("big.bin").unwrap(), SkipReason::TooLarge)]);
        let note = r.diags.iter().find(|d| d.code == codes::HN_I001).expect("an info diagnostic");
        assert_eq!(note.severity, crate::diag::Severity::Info);
        assert_eq!(note.location.path.as_str(), "big.bin");
    }

    #[test]
    fn r_arch_020__invalid_exclude_pattern_is_a_usage_error() {
        let (_g, t) = utf8_tmp();
        let e = walk(&test_root(&t), &opts(&["[unclosed"])).unwrap_err();
        assert_eq!(e.class(), crate::error::ErrorClass::Usage);
    }
}
```

- [ ] **Step 2: Add `pub mod walk;`. Run** `cargo test -p fairlead-core walk::`. Expected: FAIL (compile error: `cannot find function walk`).

- [ ] **Step 3: Write the implementation** (above the tests).

```rust
//! Sorted, parallel file listing (R-ARCH-020..026, R-SCOPE-006/007, R-FILE-023).
use crate::config::ControlFile;
use crate::diag::{codes, sort_diagnostics, Diagnostic, Location, Severity};
use crate::discover::Root;
use crate::error::{Error, IoFailure};
use crate::path::{find_case_collisions, PathIssue, RepoPath};
use camino::Utf8Path;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{WalkBuilder, WalkState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::UNIX_EPOCH;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WalkOptions {
    /// Gitignore-syntax patterns relative to the root (`[scan] exclude`).
    pub excludes: Vec<String>,
    /// Files larger than this are listed but never read (`[scan] max-file`).
    pub max_file: u64,
    pub threads: Option<usize>,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self { excludes: Vec::new(), max_file: 2_000_000, threads: None }
    }
}

impl WalkOptions {
    /// Options from the root control file's `[scan]` table.
    pub fn from_root_control(c: &ControlFile, threads: Option<usize>) -> Self {
        Self { excludes: c.scan.exclude.clone(), max_file: c.scan.max_file, threads }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WalkEntry {
    pub path: RepoPath,
    pub size: u64,
    pub mtime_ns: i128,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum SkipReason {
    TooLarge,
    Binary,
    NotUtf8,
    SymlinkOrJunction,
    Excluded,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WalkResult {
    pub files: Vec<WalkEntry>,
    pub skipped: Vec<(RepoPath, SkipReason)>,
    pub ignored_managerial: Vec<RepoPath>,
    pub diags: Vec<Diagnostic>,
}

#[derive(Default)]
struct Acc {
    files: Vec<WalkEntry>,
    skipped: Vec<(RepoPath, SkipReason)>,
    diags: Vec<Diagnostic>,
    errors: Vec<String>,
}

struct Visit {
    root_dir: std::path::PathBuf,
    folder: String,
    max_file: u64,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn build_excludes(root: &Utf8Path, patterns: &[String]) -> Result<Gitignore, Error> {
    let mut b = GitignoreBuilder::new(root.as_std_path());
    for p in patterns {
        b.add_line(None, p)
            .map_err(|e| Error::Usage(format!("invalid [scan] exclude pattern {p:?}: {e}")))?;
    }
    b.build().map_err(|e| Error::Usage(format!("invalid [scan] exclude patterns: {e}")))
}

fn mtime_ns(md: &std::fs::Metadata) -> i128 {
    match md.modified() {
        Ok(t) => match t.duration_since(UNIX_EPOCH) {
            Ok(d) => i128::try_from(d.as_nanos()).unwrap_or(i128::MAX),
            Err(e) => -i128::try_from(e.duration().as_nanos()).unwrap_or(i128::MAX),
        },
        Err(_) => 0,
    }
}

/// Pruning filter run by the walker before it descends (keeps `node_modules/` cheap).
fn keep_entry(root_dir: &std::path::Path, excludes: &Gitignore, e: &ignore::DirEntry) -> bool {
    if e.depth() == 0 {
        return true;
    }
    let name = e.file_name();
    if name == ".git" || name == ".fairlead" {
        return false;
    }
    let is_dir = e.file_type().is_some_and(|t| t.is_dir());
    // A `.git` FILE marks a submodule checkout or a linked worktree: stop (R-ARCH-022).
    if is_dir && std::fs::symlink_metadata(e.path().join(".git")).is_ok_and(|m| m.is_file()) {
        return false;
    }
    match e.path().strip_prefix(root_dir) {
        Ok(rel) => !excludes.matched_path_or_any_parents(rel, is_dir).is_ignore(),
        Err(_) => true,
    }
}

fn visit(v: &Visit, acc: &Mutex<Acc>, res: Result<ignore::DirEntry, ignore::Error>) -> WalkState {
    let entry = match res {
        Ok(e) => e,
        Err(err) => {
            lock(acc).errors.push(err.to_string());
            return WalkState::Continue;
        }
    };
    if entry.depth() == 0 {
        return WalkState::Continue;
    }
    let Some(ft) = entry.file_type() else { return WalkState::Continue };
    let Ok(rel_native) = entry.path().strip_prefix(&v.root_dir) else { return WalkState::Continue };
    let Some(rel) = Utf8Path::from_path(rel_native) else {
        let lossy = rel_native.to_string_lossy().replace('\\', "/");
        let path = RepoPath::new(&lossy).unwrap_or_else(|_| RepoPath::root());
        lock(acc).diags.push(
            Diagnostic::new(codes::HN_P001, Severity::Error, Location::file(path), "path is not valid UTF-8; skipped")
                .with_hint("rename the file"),
        );
        return WalkState::Continue;
    };
    let Ok(rp) = RepoPath::from_native_rel(rel) else { return WalkState::Continue };
    if ft.is_symlink() {
        let mut g = lock(acc);
        if rp.as_str().split('/').any(|s| s == v.folder) {
            g.diags.push(
                Diagnostic::new(
                    codes::HN_C001,
                    Severity::Error,
                    Location::file(rp.clone()),
                    "symlink or junction inside a managerial folder (R-SCOPE-006)",
                )
                .with_hint("replace the link with a real file"),
            );
        }
        g.skipped.push((rp, SkipReason::SymlinkOrJunction));
        return WalkState::Continue;
    }
    if !ft.is_file() {
        return WalkState::Continue;
    }
    let md = match entry.metadata() {
        Ok(m) => m,
        Err(err) => {
            lock(acc).errors.push(err.to_string());
            return WalkState::Continue;
        }
    };
    let size = md.len();
    let mut g = lock(acc);
    if size > v.max_file {
        g.skipped.push((rp.clone(), SkipReason::TooLarge));
        g.diags.push(
            Diagnostic::new(
                codes::HN_I001,
                Severity::Info,
                Location::file(rp.clone()),
                format!("file skipped: {size} bytes is over the max-file limit of {}", v.max_file),
            )
            .with_hint("raise [scan] max-file or add the path to [scan] skip"),
        );
    }
    g.files.push(WalkEntry { path: rp, size, mtime_ns: mtime_ns(&md) });
    WalkState::Continue
}

/// `HN-P001` diagnostics for a sorted file list: case-only collisions and path hygiene.
pub fn path_diagnostics(files: &[WalkEntry]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for group in find_case_collisions(files.iter().map(|f| &f.path)) {
        let Some(first) = group.first() else { continue };
        let names: Vec<&str> = group.iter().map(RepoPath::as_str).collect();
        let mut d = Diagnostic::new(
            codes::HN_P001,
            Severity::Error,
            Location::file(first.clone()),
            format!("paths differ only in case: {}", names.join(", ")),
        )
        .with_hint("rename one of them; case-insensitive filesystems cannot hold both");
        d.related = names.get(1).map(|s| s.to_string());
        out.push(d);
    }
    for f in files {
        for issue in f.path.hygiene() {
            let message = match issue {
                PathIssue::NotNfc => "path is not NFC-normalised".to_string(),
                PathIssue::ReservedName { segment } => format!("path segment {segment:?} is a Windows-reserved name"),
                PathIssue::TrailingDotOrSpace { segment } => format!("path segment {segment:?} ends with a dot or space"),
            };
            out.push(
                Diagnostic::new(codes::HN_P001, Severity::Error, Location::file(f.path.clone()), message)
                    .with_hint("rename the file; Windows and NTFS cannot represent it"),
            );
        }
    }
    out
}

fn detect_ignored_managerial(root: &Root, files: &[WalkEntry]) -> (Vec<RepoPath>, Vec<Diagnostic>) {
    let listed: BTreeSet<&RepoPath> = files.iter().map(|f| &f.path).collect();
    let mut dirs: BTreeSet<RepoPath> = BTreeSet::new();
    dirs.insert(RepoPath::root());
    for f in files {
        let mut cur = f.path.parent();
        while let Some(d) = cur {
            if !dirs.insert(d.clone()) {
                break;
            }
            cur = d.parent();
        }
    }
    let mut out = Vec::new();
    let mut diags = Vec::new();
    for d in &dirs {
        let Ok(control) = d.join(&root.folder).and_then(|f| f.join("control.toml")) else { continue };
        if listed.contains(&control) {
            continue;
        }
        let abs = root.dir.join(control.as_str());
        if std::fs::metadata(abs.as_std_path()).is_ok_and(|m| m.is_file()) {
            diags.push(
                Diagnostic::new(
                    codes::HN_C001,
                    Severity::Error,
                    Location::file(control.clone()),
                    "managerial path is matched by an ignore rule (.gitignore, .fairleadignore or [scan] exclude); authored rules would be lost on clone (R-FILE-023)",
                )
                .with_hint("remove the ignore rule that covers the managerial folder"),
            );
            out.push(control);
        }
    }
    out.sort();
    (out, diags)
}

/// S2: parallel listing, gathered and sorted (R-ARCH-024). I/O.
pub fn walk(root: &Root, opts: &WalkOptions) -> Result<WalkResult, Error> {
    let excludes = build_excludes(&root.dir, &opts.excludes)?;
    let root_std = root.dir.as_std_path().to_path_buf();
    let mut b = WalkBuilder::new(&root_std);
    b.hidden(false)
        .parents(false)
        .ignore(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .follow_links(false)
        .add_custom_ignore_filename(".fairleadignore")
        .threads(opts.threads.unwrap_or(0));
    {
        let root_dir = root_std.clone();
        b.filter_entry(move |e| keep_entry(&root_dir, &excludes, e));
    }
    let acc = Arc::new(Mutex::new(Acc::default()));
    let ctx = Arc::new(Visit { root_dir: root_std, folder: root.folder.clone(), max_file: opts.max_file });
    b.build_parallel().run(|| {
        let acc = Arc::clone(&acc);
        let ctx = Arc::clone(&ctx);
        Box::new(move |res| visit(&ctx, &acc, res))
    });
    let mut acc = match Arc::try_unwrap(acc) {
        Ok(m) => m.into_inner().unwrap_or_else(PoisonError::into_inner),
        Err(shared) => std::mem::take(&mut *lock(&shared)),
    };
    if !acc.errors.is_empty() {
        acc.errors.sort();
        let detail = acc.errors.remove(0);
        return Err(Error::Io(IoFailure {
            path: root.dir.to_string(),
            op: "walk",
            kind: std::io::ErrorKind::Other,
            detail,
            user_path: false,
        }));
    }
    acc.files.sort_by(|a, b| a.path.cmp(&b.path));
    acc.skipped.sort();
    let mut diags = acc.diags;
    diags.extend(path_diagnostics(&acc.files));
    let (ignored_managerial, extra) = detect_ignored_managerial(root, &acc.files);
    diags.extend(extra);
    sort_diagnostics(&mut diags);
    Ok(WalkResult { files: acc.files, skipped: acc.skipped, ignored_managerial, diags })
}
```

Verify (ignore 0.4): `WalkBuilder::filter_entry` takes `Fn(&DirEntry) -> bool + Send + Sync + 'static`; `GitignoreBuilder::add_line(None, line)`; `Gitignore::matched_path_or_any_parents(path, is_dir)` requires a relative path (it asserts the path has no root); `DirEntry::file_name()` returns `&OsStr` and compares with `&str` via `PartialEq<str>`. `WalkBuilder::threads(0)` means automatic. On Windows `FileType::is_symlink()` is also true for directory junctions in current Rust std; if a junction is ever listed as a directory, the symlink test above (unix only) has a Windows twin in P9's `platform.rs`.

- [ ] **Step 4: Run** `cargo test -p fairlead-core walk::`. Expected: PASS (Linux runs all tests; Windows skips the two `cfg` tests).

- [ ] **Step 5: Commit** `feat(core): sorted parallel walk with ignore rules, stops and path diagnostics`.

---

### Task 17: load_controls, control diagnostics, config hash, child excludes

Rules: R-SCOPE-004 (second `root = true` is `HN-C004`), R-ARCH-012 (zero declared rules is knowable), R-SYN-033 / R-ARCH-104 (a broken file does not hide others), R-ARCH-077 (config hash), R-SCOPE-056 (child `[scan] exclude`).

**Files:**
- Modify: `crates/fairlead-core/src/config/mod.rs` (append `ControlSet`, `load_controls`, `control_diagnostics`, `config_hash_of`; extend its test module)
- Modify: `crates/fairlead-core/src/walk.rs` (append `apply_child_excludes`; extend its test module)

**Interfaces (contract 4.6, fixed):** `ControlSet { files: BTreeMap<RepoPath, Result<ControlFile, ConfigFailure>> }`, `load_controls(&Root, &WalkResult) -> Result<ControlSet, Error>`. Additions: `ControlSet::{ok_files, root_control, declared_rule_count}`, `control_diagnostics(&ControlSet) -> Vec<Diagnostic>`, `config_hash_of(&ControlSet) -> ContentHash`, `walk::apply_child_excludes(WalkResult, &ControlSet) -> Result<WalkResult, Error>`.

Why `apply_child_excludes`: the walk (S2) runs before the control files are read (S3), so only the root's `[scan] exclude` can prune during the walk; a child file's `exclude` (relative to its scope directory) is applied afterwards. P8's `load` calls it between S3 and S4.

- [ ] **Step 1: Append tests to the `tests` module of `config/mod.rs`.**

```rust
    use crate::testutil::{test_root, utf8_tmp, write_file, CHILD_CONTROL, ROOT_CONTROL};
    use crate::walk::{walk, WalkOptions};

    fn load(t: &camino::Utf8Path) -> ControlSet {
        let root = test_root(t);
        let w = walk(&root, &WalkOptions::default()).unwrap();
        load_controls(&root, &w).unwrap()
    }

    #[test]
    fn r_scope_004__load_controls_finds_every_control_file_and_only_those() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "web/fairlead/control.toml", CHILD_CONTROL);
        write_file(&t, "api/fairlead/control.toml", "[fairlead\n");
        write_file(&t, "docs/control.toml", "decoy");
        write_file(&t, "web/fairlead/docs/control.toml", "decoy");
        let set = load(&t);
        let keys: Vec<&str> = set.files.keys().map(RepoPath::as_str).collect();
        assert_eq!(keys, ["api/fairlead/control.toml", "fairlead/control.toml", "web/fairlead/control.toml"]);
        assert!(set.files[&p("api/fairlead/control.toml")].is_err());
        assert_eq!(set.root_control().unwrap().fairlead.scope.as_str(), "root");
        assert_eq!(set.files[&p("web/fairlead/control.toml")].as_ref().unwrap().dir, p("web"));
    }

    #[test]
    fn r_arch_104__a_broken_file_is_reported_and_does_not_hide_the_others() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "web/fairlead/control.toml", CHILD_CONTROL);
        write_file(&t, "api/fairlead/control.toml", "[fairlead\n");
        let set = load(&t);
        assert_eq!(set.ok_files().count(), 2);
        let d = control_diagnostics(&set);
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].code, d[0].location.path.as_str()), (codes::HN_C001, "api/fairlead/control.toml"));
    }

    #[test]
    fn r_scope_004__second_root_true_below_the_root_is_root_dup() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "vendor/lib/fairlead/control.toml", "[fairlead]\nscope = \"lib\"\nroot = true\n");
        let d = control_diagnostics(&load(&t));
        assert!(d.iter().any(|x| x.code == codes::HN_C004 && x.location.path.as_str() == "vendor/lib/fairlead/control.toml"), "{d:?}");
    }

    #[test]
    fn r_syn_041__warnings_and_violations_flow_into_the_diagnostics() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", &format!("{ROOT_CONTROL}[mystery]\nx = 1\n"));
        write_file(&t, "web/fairlead/control.toml", "[fairlead]\nscope = \"web\"\nmarker = \"x\"\n");
        let d = control_diagnostics(&load(&t));
        let codes_seen: Vec<&str> = d.iter().map(|x| x.code.0).collect();
        assert_eq!(codes_seen, ["HN-C002", "HN-C003"], "sorted by path: fairlead/ before web/");
    }

    #[test]
    fn r_arch_012__declared_rule_count_is_zero_for_a_ruleless_repository() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", ROOT_CONTROL);
        assert_eq!(load(&t).declared_rule_count(), 0, "P5 turns this into the HN-N001 notice, never a bare pass");
        write_file(&t, "fairlead/control.toml", &format!("{ROOT_CONTROL}[rules.a]\nsev = \"warn\"\n[rules.b]\n"));
        write_file(&t, "web/fairlead/control.toml", &format!("{CHILD_CONTROL}[rules.c]\n"));
        assert_eq!(load(&t).declared_rule_count(), 3);
    }

    #[test]
    fn r_arch_077__config_hash_changes_with_any_control_file_and_is_otherwise_stable() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "web/fairlead/control.toml", CHILD_CONTROL);
        let h1 = config_hash_of(&load(&t));
        assert_eq!(h1, config_hash_of(&load(&t)));
        write_file(&t, "web/fairlead/control.toml", &format!("{CHILD_CONTROL}[settings]\npurpose = \"x\"\n"));
        let h2 = config_hash_of(&load(&t));
        assert_ne!(h1, h2);
        write_file(&t, "api/fairlead/control.toml", "[fairlead\n");
        assert_ne!(h2, config_hash_of(&load(&t)), "a new (even broken) control file changes the key");
    }
```

And append to the `tests` module of `walk.rs`:

```rust
    #[test]
    fn r_scope_056__child_scan_exclude_applies_below_its_own_directory_only() {
        use crate::config::load_controls;
        use crate::testutil::{CHILD_CONTROL, ROOT_CONTROL};
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "web/fairlead/control.toml", &format!("{CHILD_CONTROL}[scan]\nexclude = [\"dist/\", \"*.snap\"]\n"));
        for f in ["dist/root.js", "web/dist/a.js", "web/src/b.ts", "web/src/x.snap", "api/dist/c.js"] {
            write_file(&t, f, "x");
        }
        let root = test_root(&t);
        let w = walk(&root, &WalkOptions::default()).unwrap();
        let set = load_controls(&root, &w).unwrap();
        let w2 = apply_child_excludes(w, &set).unwrap();
        let kept: Vec<&str> = w2.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(kept, ["api/dist/c.js", "dist/root.js", "fairlead/control.toml", "web/fairlead/control.toml", "web/src/b.ts"]);
        assert!(w2.skipped.contains(&(RepoPath::new("web/dist/a.js").unwrap(), SkipReason::Excluded)));
        assert!(w2.skipped.contains(&(RepoPath::new("web/src/x.snap").unwrap(), SkipReason::Excluded)));
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-core r_scope_004 r_arch_104 r_syn_041 r_arch_012 r_arch_077 r_scope_056`. Expected: FAIL (compile error: `cannot find type ControlSet`).

- [ ] **Step 3: Append to `config/mod.rs`** (above its test module).

```rust
use crate::discover::Root;
use crate::error::Error;
use crate::walk::WalkResult;

/// Every control file found by the walk; a file that cannot be parsed stays in the map as the
/// failure so its subtree is unresolved (R-SYN-033).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ControlSet {
    pub files: BTreeMap<RepoPath, Result<ControlFile, ConfigFailure>>,
}

impl ControlSet {
    pub fn ok_files(&self) -> impl Iterator<Item = &ControlFile> {
        self.files.values().filter_map(|r| r.as_ref().ok())
    }

    /// The root directory's control file, when it parsed.
    pub fn root_control(&self) -> Option<&ControlFile> {
        self.ok_files().find(|c| c.dir.is_root())
    }

    /// Rules declared in all parsed control files (R-ARCH-012: zero means "nothing verified").
    pub fn declared_rule_count(&self) -> usize {
        self.ok_files().map(|c| c.rules.len()).sum()
    }
}

/// S3: read and parse every `<folder>/control.toml` listed by the walk. Parse failures are data;
/// only I/O failures are errors.
pub fn load_controls(root: &Root, walk: &WalkResult) -> Result<ControlSet, Error> {
    let mut files = BTreeMap::new();
    for e in &walk.files {
        if e.path.file_name() != "control.toml" {
            continue;
        }
        let Some(folder_dir) = e.path.parent() else { continue };
        if folder_dir.file_name() != root.folder {
            continue;
        }
        let abs = root.dir.join(e.path.as_str());
        let bytes = std::fs::read(abs.as_std_path()).map_err(|err| Error::io(e.path.as_str(), "read", &err, false))?;
        let is_root_dir = folder_dir.parent().is_some_and(|d| d.is_root());
        files.insert(e.path.clone(), parse_control(&e.path, &bytes, is_root_dir));
    }
    Ok(ControlSet { files })
}

/// All load-time diagnostics of the set, sorted: failures (`HN-C001`/`HN-P001`), unknown keys
/// (`HN-C002`), non-delegable keys (`HN-C003`) and a second `root = true` below the root
/// (`HN-C004`, ROOT-DUP).
pub fn control_diagnostics(set: &ControlSet) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for r in set.files.values() {
        match r {
            Err(f) => out.extend(f.diags.iter().cloned()),
            Ok(c) => {
                out.extend(c.diagnostics());
                if !c.dir.is_root() && c.fairlead.root {
                    out.push(
                        Diagnostic::new(
                            codes::HN_C004,
                            Severity::Error,
                            Location::at(c.path.clone(), 1, 1),
                            "a second `root = true` control file below the root (ROOT-DUP); mounted sub-roots arrive in 0.3",
                        )
                        .with_hint("remove `root = true`, or run fairlead from the real root"),
                    );
                }
            }
        }
    }
    crate::diag::sort_diagnostics(&mut out);
    out
}

/// Hash over every control file (path and normalised content hash): the config part of the cache key.
pub fn config_hash_of(set: &ControlSet) -> ContentHash {
    let mut h = blake3::Hasher::new();
    for (path, r) in &set.files {
        h.update(path.as_str().as_bytes());
        h.update(&[0]);
        match r {
            Ok(c) => {
                h.update(&c.content_hash.0);
            }
            Err(_) => {
                h.update(b"unparseable");
            }
        }
        h.update(&[1]);
    }
    ContentHash(*h.finalize().as_bytes())
}
```

- [ ] **Step 4: Append to `walk.rs`** (above its test module).

```rust
/// Apply each child control file's `[scan] exclude` (relative to its own directory) to a finished
/// walk. Excluded files move to `skipped` as `SkipReason::Excluded`. The root's excludes were
/// already applied by `walk` through `WalkOptions`.
pub fn apply_child_excludes(walk: WalkResult, set: &crate::config::ControlSet) -> Result<WalkResult, Error> {
    let mut matchers: Vec<(RepoPath, Gitignore)> = Vec::new();
    for c in set.ok_files() {
        if c.dir.is_root() || c.scan.exclude.is_empty() {
            continue;
        }
        matchers.push((c.dir.clone(), build_excludes(Utf8Path::new(""), &c.scan.exclude)?));
    }
    if matchers.is_empty() {
        return Ok(walk);
    }
    let mut kept = Vec::new();
    let mut skipped = walk.skipped;
    for e in walk.files {
        let excluded = matchers.iter().any(|(dir, gi)| {
            if e.path == *dir || !e.path.is_within(dir) {
                return false;
            }
            let rel = e.path.as_str().get(dir.as_str().len() + 1..).unwrap_or("");
            !rel.is_empty() && gi.matched_path_or_any_parents(rel, false).is_ignore()
        });
        if excluded {
            skipped.push((e.path, SkipReason::Excluded));
        } else {
            kept.push(e);
        }
    }
    skipped.sort();
    Ok(WalkResult { files: kept, skipped, ..walk })
}
```

Verify: `GitignoreBuilder::new("")` accepts an empty root and relative candidate paths are matched as given; if the crate rejects the empty root, use `GitignoreBuilder::new(".")`.

- [ ] **Step 5: Run** the Step 2 command plus `cargo test -p fairlead-core walk::`. Expected: PASS. **Commit** `feat(core): load_controls, control diagnostics, config hash and child excludes`.

---

### Task 18: classify.rs: `FileClass` by location and declaration

Rules: R-FILE-001, R-FILE-004, R-FILE-005, R-FILE-009, R-FILE-014, R-FILE-020 (layout names), R-FILE-044, R-FILE-054, R-SCOPE-018 (class conventions), R-AGT-018, R-SYN-030 (`skip`).

**Files:**
- Modify: `crates/fairlead-core/src/classify.rs` (the `FileClass` enum from Task 10 stays at the top; append the rest)

**Interfaces (contract 4.4, names fixed):** `FileClass` (done), `ClassifyRules`, `ClassifyRules::from_controls(&Root, &ControlSet) -> ClassifyRules`, `classify(&RepoPath, &ClassifyRules) -> FileClass`, `scans_directives(FileClass) -> bool`. Additions: `ClassifyRules::{skips, in_managerial_folder, conflicts}`. `ClassifyRules` derives `Clone, Debug` only (a `GlobSet` has no `PartialEq`).

Rules of classification (class comes from location and declaration, never from content, R-FILE-001/014):
1. A path under a managerial folder (`<dir>/<folder>/...` where a parsed control file exists) is classified by the first segment under it, using the ROOT `[layout]` names: `gen/` and `vendored.lock` are Generated, `ledger/` Ledger, `ref/` Reference, everything else (`control.toml`, `approvals.toml`, `docs/`, `contract/`) Authored.
2. Otherwise a path matching a `[[vendor]]` entry or a `[[convention]] class = "vendor-owned"` of a scope that contains it is VendorOwned (patterns are relative to the declaring scope's directory).
3. Otherwise a path matching a `[[convention]] class = "external-generated"` is ExternalGenerated.
4. Everything else is Authored.
`scans_directives` is false exactly for Reference, VendorOwned and ExternalGenerated (R-FILE-004/005/009). `[scan] skip` globs (also scope-relative) are exposed through `skips`, not through the class, because `skip` files stay Authored but are not scanned.

- [ ] **Step 1: Write the failing tests** (append a test module to `classify.rs`).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{parse_control, ControlSet};
    use crate::diag::codes;
    use crate::testutil::{test_root, ROOT_CONTROL};
    use camino::Utf8Path;

    fn set(files: &[(&str, String)]) -> ControlSet {
        let mut s = ControlSet::default();
        for (path, text) in files {
            let p = RepoPath::new(path).unwrap();
            let is_root = p.as_str() == "fairlead/control.toml";
            s.files.insert(p.clone(), parse_control(&p, text.as_bytes(), is_root));
        }
        s
    }

    fn rules(files: &[(&str, String)]) -> ClassifyRules {
        ClassifyRules::from_controls(&test_root(Utf8Path::new("/r")), &set(files))
    }

    fn class(r: &ClassifyRules, p: &str) -> FileClass {
        classify(&RepoPath::new(p).unwrap(), r)
    }

    fn root_only() -> ClassifyRules {
        rules(&[("fairlead/control.toml", ROOT_CONTROL.to_string())])
    }

    #[test]
    fn r_file_001__every_file_under_a_managerial_folder_has_exactly_one_class() {
        let r = root_only();
        for (p, want) in [
            ("fairlead/control.toml", FileClass::Authored),
            ("fairlead/approvals.toml", FileClass::Authored),
            ("fairlead/docs/adr-0001.md", FileClass::Authored),
            ("fairlead/contract/tokens.md", FileClass::Authored),
            ("fairlead/gen/reg/tok.reg", FileClass::Generated),
            ("fairlead/gen/baseline.txt", FileClass::Generated),
            ("fairlead/gen/members", FileClass::Generated),
            ("fairlead/vendored.lock", FileClass::Generated),
            ("fairlead/ledger/hist/2026.hl", FileClass::Ledger),
            ("fairlead/ref/syntax.md", FileClass::Reference),
            ("src/main.rs", FileClass::Authored),
            ("README.md", FileClass::Authored),
        ] {
            assert_eq!(class(&r, p), want, "{p}");
        }
    }

    #[test]
    fn r_file_001__nested_scope_folders_follow_the_root_layout() {
        let child = "[fairlead]\nscope = \"web\"\n".to_string();
        let r = rules(&[("fairlead/control.toml", ROOT_CONTROL.to_string()), ("web/fairlead/control.toml", child)]);
        assert_eq!(class(&r, "web/fairlead/gen/reg/cmp.reg"), FileClass::Generated);
        assert_eq!(class(&r, "web/fairlead/docs/x.md"), FileClass::Authored);
        assert_eq!(class(&r, "web/fairlead/ref/syntax.md"), FileClass::Reference);
        assert_eq!(class(&r, "web/gen/x.ts"), FileClass::Authored, "`gen` outside a managerial folder is just a folder");
        assert!(r.in_managerial_folder(&RepoPath::new("web/fairlead/docs/x.md").unwrap()));
        assert!(!r.in_managerial_folder(&RepoPath::new("web/src/x.ts").unwrap()));
    }

    #[test]
    fn r_file_020__layout_names_come_from_the_root_control_file() {
        let root = format!("{ROOT_CONTROL}[layout]\ngen = \"generated\"\nref = \"reference\"\nledger = \"history\"\n");
        let r = rules(&[("fairlead/control.toml", root)]);
        assert_eq!(class(&r, "fairlead/generated/reg/tok.reg"), FileClass::Generated);
        assert_eq!(class(&r, "fairlead/reference/syntax.md"), FileClass::Reference);
        assert_eq!(class(&r, "fairlead/history/2026.hl"), FileClass::Ledger);
        assert_eq!(class(&r, "fairlead/gen/reg/tok.reg"), FileClass::Authored, "the default names no longer apply");
    }

    #[test]
    fn r_file_004__reference_vendor_owned_and_external_generated_are_never_scanned() {
        assert!(scans_directives(FileClass::Authored));
        assert!(scans_directives(FileClass::Generated), "CLASS-LINE (HN-C011) needs generated files scanned");
        assert!(scans_directives(FileClass::Ledger));
        assert!(!scans_directives(FileClass::Reference));
        assert!(!scans_directives(FileClass::VendorOwned));
        assert!(!scans_directives(FileClass::ExternalGenerated));
    }

    #[test]
    fn r_file_005__vendor_declarations_apply_inside_the_declaring_scope_only() {
        let child = "[fairlead]\nscope = \"web\"\n[[vendor]]\npath = \"components.json\"\ntool = \"shadcn\"\n[[vendor]]\npath = \"src/ui/**\"\ntool = \"shadcn\"\n".to_string();
        let r = rules(&[("fairlead/control.toml", ROOT_CONTROL.to_string()), ("web/fairlead/control.toml", child)]);
        assert_eq!(class(&r, "web/components.json"), FileClass::VendorOwned);
        assert_eq!(class(&r, "web/src/ui/button.tsx"), FileClass::VendorOwned);
        assert_eq!(class(&r, "components.json"), FileClass::Authored);
        assert_eq!(class(&r, "api/components.json"), FileClass::Authored);
        assert_eq!(class(&r, "web/src/app.tsx"), FileClass::Authored);
    }

    #[test]
    fn r_file_044__vendor_owned_files_are_exempt_from_directive_scanning() {
        let child = "[fairlead]\nscope = \"web\"\n[[vendor]]\npath = \"vendor/\"\ntool = \"x\"\n".to_string();
        let r = rules(&[("fairlead/control.toml", ROOT_CONTROL.to_string()), ("web/fairlead/control.toml", child)]);
        let c = class(&r, "web/vendor/deep/lib.js");
        assert_eq!(c, FileClass::VendorOwned, "a trailing slash means the whole directory");
        assert!(!scans_directives(c));
    }

    #[test]
    fn r_scope_018__class_conventions_mark_external_generated_files() {
        let root = format!("{ROOT_CONTROL}[[convention]]\nglob = \"**/*.gen.ts\"\nclass = \"external-generated\"\n");
        let child = "[fairlead]\nscope = \"api\"\n[[convention]]\nglob = \"proto/**\"\nclass = \"external-generated\"\n".to_string();
        let r = rules(&[("fairlead/control.toml", root), ("api/fairlead/control.toml", child)]);
        assert_eq!(class(&r, "src/a.gen.ts"), FileClass::ExternalGenerated);
        assert_eq!(class(&r, "a.gen.ts"), FileClass::ExternalGenerated);
        assert_eq!(class(&r, "api/proto/x.rs"), FileClass::ExternalGenerated);
        assert_eq!(class(&r, "proto/x.rs"), FileClass::Authored);
        assert_eq!(class(&r, "src/a.ts"), FileClass::Authored);
    }

    #[test]
    fn r_file_014__class_follows_the_writer_not_the_content() {
        let r = root_only();
        assert_eq!(class(&r, "src/schema.gen.ts"), FileClass::Authored, "undeclared: fairlead cannot know another tool wrote it");
        assert_eq!(class(&r, "fairlead/gen/reg/tok.reg"), FileClass::Generated, "a file fairlead writes is generated");
    }

    #[test]
    fn r_file_054__a_file_with_two_declared_writers_is_reported() {
        let root = format!("{ROOT_CONTROL}[[vendor]]\npath = \"gen/**\"\ntool = \"x\"\n[[convention]]\nglob = \"gen/**\"\nclass = \"external-generated\"\n");
        let r = rules(&[("fairlead/control.toml", root)]);
        let paths = [RepoPath::new("gen/a.ts").unwrap(), RepoPath::new("src/b.ts").unwrap()];
        let d = r.conflicts(&paths);
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].code, d[0].location.path.as_str()), (codes::HN_C001, "gen/a.ts"));
        assert!(d[0].message.contains("two writers"));
        assert_eq!(class(&r, "gen/a.ts"), FileClass::VendorOwned, "vendor-owned wins until the conflict is fixed");
    }

    #[test]
    fn r_agt_018__skip_globs_are_scope_relative_and_do_not_change_the_class() {
        let root = format!("{ROOT_CONTROL}[scan]\nskip = [\"tests/fixtures/**\"]\n");
        let child = "[fairlead]\nscope = \"web\"\n[scan]\nskip = [\"snapshots/**\"]\n".to_string();
        let r = rules(&[("fairlead/control.toml", root), ("web/fairlead/control.toml", child)]);
        let p = |s: &str| RepoPath::new(s).unwrap();
        assert!(r.skips(&p("tests/fixtures/a.rs")));
        assert!(r.skips(&p("web/snapshots/a.snap")));
        assert!(!r.skips(&p("snapshots/a.snap")), "the child's skip applies below the child only");
        assert!(!r.skips(&p("src/a.rs")));
        assert_eq!(class(&r, "tests/fixtures/a.rs"), FileClass::Authored);
    }

    #[test]
    fn r_file_001__unparseable_control_files_contribute_nothing() {
        let r = rules(&[("fairlead/control.toml", ROOT_CONTROL.to_string()), ("web/fairlead/control.toml", "[fairlead\n".to_string())]);
        assert_eq!(class(&r, "web/fairlead/gen/x.reg"), FileClass::Authored);
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-core r_file_001 r_file_004 r_file_005 r_file_014 r_file_020 r_file_044 r_file_054 r_scope_018 r_agt_018`. Expected: FAIL (compile error: `cannot find struct ClassifyRules`).

- [ ] **Step 3: Append the implementation to `classify.rs`** (above the tests; merge the `use` lines with the existing ones at the top).

```rust
use crate::config::{ControlSet, ConventionAction, Layout};
use crate::diag::{codes, Diagnostic, Location, Severity};
use crate::discover::Root;
use crate::path::RepoPath;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::BTreeSet;

/// Compiled location and declaration rules (contract 4.4).
#[derive(Clone, Debug)]
pub struct ClassifyRules {
    managerial: BTreeSet<RepoPath>,
    layout: Layout,
    vendor: GlobSet,
    external: GlobSet,
    skip: GlobSet,
}

fn add_glob(b: &mut GlobSetBuilder, dir: &RepoPath, pattern: &str) {
    let pat = pattern.trim_start_matches('/');
    let pat = if pat.ends_with('/') { format!("{pat}**") } else { pat.to_string() };
    let full = if dir.is_root() { pat } else { format!("{}/{}", dir.as_str(), pat) };
    // Patterns were validated at load (`check_glob`); a failure here simply adds nothing.
    if let Ok(g) = GlobBuilder::new(&full).literal_separator(true).build() {
        b.add(g);
    }
}

fn finish(b: GlobSetBuilder) -> GlobSet {
    b.build().unwrap_or_else(|_| GlobSet::empty())
}

impl ClassifyRules {
    pub fn from_controls(_root: &Root, controls: &ControlSet) -> Self {
        let layout = controls.root_control().map(|c| c.layout.clone()).unwrap_or_default();
        let mut managerial = BTreeSet::new();
        let (mut vendor, mut external, mut skip) = (GlobSetBuilder::new(), GlobSetBuilder::new(), GlobSetBuilder::new());
        for c in controls.ok_files() {
            if let Some(m) = c.path.parent() {
                managerial.insert(m);
            }
            for v in &c.vendors {
                add_glob(&mut vendor, &c.dir, &v.path);
            }
            for cv in &c.conventions {
                if let ConventionAction::Class(class) = &cv.action {
                    match class {
                        FileClass::VendorOwned => add_glob(&mut vendor, &c.dir, &cv.glob),
                        _ => add_glob(&mut external, &c.dir, &cv.glob),
                    }
                }
            }
            for s in &c.scan.skip {
                add_glob(&mut skip, &c.dir, s);
            }
        }
        Self { managerial, layout, vendor: finish(vendor), external: finish(external), skip: finish(skip) }
    }

    /// The nearest managerial folder containing `path`, if any.
    fn managerial_of(&self, path: &RepoPath) -> Option<RepoPath> {
        let mut cur = path.parent();
        while let Some(d) = cur {
            if self.managerial.contains(&d) {
                return Some(d);
            }
            cur = d.parent();
        }
        None
    }

    pub fn in_managerial_folder(&self, path: &RepoPath) -> bool {
        self.managerial_of(path).is_some()
    }

    /// `[scan] skip` match (scope-relative globs): the file is not scanned for directives.
    pub fn skips(&self, path: &RepoPath) -> bool {
        self.skip.is_match(path.as_str())
    }

    /// `HN-C001` for every path declared both vendor-owned and external-generated (R-FILE-054).
    pub fn conflicts(&self, paths: &[RepoPath]) -> Vec<Diagnostic> {
        paths
            .iter()
            .filter(|p| self.vendor.is_match(p.as_str()) && self.external.is_match(p.as_str()))
            .map(|p| {
                Diagnostic::new(
                    codes::HN_C001,
                    Severity::Error,
                    Location::file(p.clone()),
                    "file has two writers: declared both vendor-owned and external-generated (R-FILE-054)",
                )
                .with_hint("declare it with exactly one writer")
            })
            .collect()
    }
}

/// Pure. Class by location and declaration, never by content (R-FILE-001).
pub fn classify(path: &RepoPath, rules: &ClassifyRules) -> FileClass {
    if let Some(m) = rules.managerial_of(path) {
        let rel = path.as_str().get(m.as_str().len()..).unwrap_or("").trim_start_matches('/');
        let first = rel.split('/').next().unwrap_or("");
        return if first == rules.layout.r#gen || rel == "vendored.lock" {
            FileClass::Generated
        } else if first == rules.layout.ledger {
            FileClass::Ledger
        } else if first == rules.layout.reference {
            FileClass::Reference
        } else {
            FileClass::Authored
        };
    }
    if rules.vendor.is_match(path.as_str()) {
        return FileClass::VendorOwned;
    }
    if rules.external.is_match(path.as_str()) {
        return FileClass::ExternalGenerated;
    }
    FileClass::Authored
}

/// Pure. Reference, vendor-owned and external-generated files are never scanned for directives
/// (R-FILE-004/005/009).
pub fn scans_directives(class: FileClass) -> bool {
    !matches!(class, FileClass::Reference | FileClass::VendorOwned | FileClass::ExternalGenerated)
}
```

Verify: with `m` equal to the root path (never, a managerial folder is at least `fairlead`), `get(len..)` stays valid. For the root managerial folder `fairlead` and path `fairlead/control.toml`, `rel` is `control.toml`.

- [ ] **Step 4: Run** the Step 2 command. Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): file classes by location and declaration`.

---

### Task 19: extract/styles.rs: CSS custom-property sites (plus the extraction record types)

Rules: R-FILE-034 (per-kind regex-level extractor, one item with several value sites, enclosing selector or at-rule), R-FILE-042 (foreign regions are flagged, not policed), R-FILE-036 (no DOM inference), R-ARCH-111 (CRLF identical).

**Files:**
- Create: `crates/fairlead-core/src/extract/mod.rs` (only the record types and `pub mod styles;` now; Tasks 21 and 22 extend it)
- Create: `crates/fairlead-core/src/extract/styles.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod extract;`)

**Interfaces (contract 4.5):** `CssTokenSite { name, value, context, span, in_foreign }`, `LiteralSite { property, value_norm, raw, span, projection_hint }`, `SecretHit { class, span, fp_prefix }` (all `Serialize, Deserialize`: they are cached). Additions: `Extras { css_tokens, literals, secrets }` (what is derived from a file's text besides the `Extraction`), `StyleKinds { extensions, declared }` with `applies` and `from_controls`, `extract_css_tokens(text, foreign_lines)`, `foreign_ranges(&Extraction)`.

Extractor semantics: a statement is the text between `{`, `}` and `;` (outside strings, comments and parentheses). A statement that begins `--name:` is a custom-property site. `context` is the chain of enclosing preludes joined by `" > "` (`":root"`, `".dark"`, `"@theme inline"`, `"@media (min-width: 600px) > :root"`; empty at top level). `name` keeps the leading dashes (`--primary`). `value` has whitespace runs collapsed to one space. `span.start/end` are original byte offsets of `--name`; `line`/`col` are 1-based, columns in Unicode scalar values. `in_foreign` is true when the site's line lies inside a `def foreign.*` region.

- [ ] **Step 1: Create `extract/mod.rs` with the record types.**

```rust
//! Per-file extraction records (contract 4.5). Task 19 creates the site types; Tasks 21 and 22 add
//! `FileRecord`, `ExtractPlan`, `extract_file`, `extract_files` and `Corpus`.
pub mod styles;

pub use styles::{extract_css_tokens, foreign_ranges, StyleKinds};

use fairlead_syntax::diag::Span;
use serde::{Deserialize, Serialize};

/// One value site of a CSS custom property (R-FILE-034).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CssTokenSite {
    pub name: String,
    pub value: String,
    /// Enclosing selector or at-rule chain, outermost first, joined by " > ".
    pub context: String,
    pub span: Span,
    pub in_foreign: bool,
}

/// A literal value in a property context (R-CHK-019); produced by the P5 scanner.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LiteralSite {
    pub property: String,
    pub value_norm: String,
    pub raw: String,
    pub span: Span,
    pub projection_hint: Option<String>,
}

/// A secret-like value; never carries the value itself (R-AGT-023).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SecretHit {
    pub class: String,
    pub span: Span,
    pub fp_prefix: String,
}

/// Everything derived from a file's text besides its `Extraction`; cached by content hash and plan.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Extras {
    pub css_tokens: Vec<CssTokenSite>,
    pub literals: Vec<LiteralSite>,
    pub secrets: Vec<SecretHit>,
}
```

- [ ] **Step 2: Write the failing tests.** Create `styles.rs` with only this module.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sites(text: &str) -> Vec<CssTokenSite> {
        extract_css_tokens(text, &[])
    }

    fn triple(s: &CssTokenSite) -> (&str, &str, &str) {
        (s.name.as_str(), s.context.as_str(), s.value.as_str())
    }

    #[test]
    fn r_file_034__one_token_in_root_dark_and_theme_is_three_value_sites() {
        let css = ":root { --primary: #2563eb; --radius: 0.5rem }\n.dark { --primary: #60a5fa; }\n@theme inline { --color-primary: var(--primary); }\n";
        let s = sites(css);
        let got: Vec<(&str, &str, &str)> = s.iter().map(triple).collect();
        assert_eq!(
            got,
            [
                ("--primary", ":root", "#2563eb"),
                ("--radius", ":root", "0.5rem"),
                ("--primary", ".dark", "#60a5fa"),
                ("--color-primary", "@theme inline", "var(--primary)"),
            ]
        );
    }

    #[test]
    fn r_file_034__nested_at_rules_join_their_preludes() {
        let s = sites("@media (min-width: 600px) {\n  :root {\n    --gap:   4px\n  }\n}\n");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].context, "@media (min-width: 600px) > :root");
        assert_eq!(s[0].value, "4px");
    }

    #[test]
    fn r_file_034__comments_strings_and_urls_do_not_confuse_the_scanner() {
        let css = "/* --fake: 1; */\n:root {\n  content: \"; --nope: 1\";\n  --img: url(data:image/png;base64,AAA);\n  --a: 1 /* x */ 2;\n  --s: 'a;b';\n}\n";
        let s = sites(css);
        let got: Vec<(&str, &str)> = s.iter().map(|x| (x.name.as_str(), x.value.as_str())).collect();
        assert_eq!(got, [("--img", "url(data:image/png;base64,AAA)"), ("--a", "1 2"), ("--s", "'a;b'")]);
    }

    #[test]
    fn r_file_034__span_is_original_bytes_with_one_based_line_and_scalar_column() {
        let css = "/* é */\n:root {\n  --x: 1;\n}\n";
        let s = sites(css);
        assert_eq!(s.len(), 1);
        let sp = s[0].span;
        assert_eq!((sp.line, sp.col), (3, 3));
        assert_eq!(&css[sp.start as usize..sp.end as usize], "--x");
    }

    #[test]
    fn r_arch_111__crlf_gives_the_same_sites_except_for_byte_offsets() {
        let lf = ":root {\n  --a: 1;\n  --b: 2;\n}\n";
        let crlf = lf.replace('\n', "\r\n");
        let a = sites(lf);
        let b = sites(&crlf);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!((&x.name, &x.value, &x.context, x.span.line, x.span.col), (&y.name, &y.value, &y.context, y.span.line, y.span.col));
        }
        assert_ne!(a[1].span.start, b[1].span.start, "offsets refer to the original bytes");
    }

    #[test]
    fn r_file_042__sites_inside_a_foreign_region_are_flagged_not_dropped() {
        let css = ":root {\n  --mine: 1;\n  --shadcn: 2;\n}\n";
        let s = extract_css_tokens(css, &[(3, 3)]);
        assert_eq!(s.iter().map(|x| x.in_foreign).collect::<Vec<_>>(), [false, true]);
    }

    #[test]
    fn r_file_034__garbage_never_panics_and_yields_nothing_wrong() {
        for g in ["", "}}}{{{", "--: 1;", "--a", "--a:", "{ --x: 1", "/* open", "\"open", ":root { --a: url( }", "--é-ü: 1;", "a{--b:1}c{--d:2}"] {
            let _ = extract_css_tokens(g, &[]);
        }
        assert_eq!(sites("--a:").len(), 1);
        assert_eq!(sites("--a:")[0].value, "");
        assert!(sites("--: 1;").is_empty());
        assert!(sites("--a").is_empty());
        let two = sites("a{--b:1}c{--d:2}");
        assert_eq!(two.iter().map(|s| s.context.as_str()).collect::<Vec<_>>(), ["a", "c"]);
    }

    #[test]
    fn r_file_034__style_kinds_apply_by_extension_or_declaration() {
        let mut k = StyleKinds::default();
        let p = |s: &str| RepoPath::new(s).unwrap();
        assert!(k.applies(&p("app/globals.css")) && k.applies(&p("a/b.SCSS")) && k.applies(&p("x.less")));
        assert!(!k.applies(&p("a.ts")));
        k.declared.insert(p("theme/tokens.txt"));
        assert!(k.applies(&p("theme/tokens.txt")));
    }

    #[test]
    fn r_file_034__style_kinds_follow_declared_stylesheet_file_items() {
        use crate::config::{parse_control, ControlSet};
        let root = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n[file-kinds.stylesheet]\nextractor = \"css-custom-properties\"\nbinding = \"import\"\n[file-kinds.other]\nextractor = \"json\"\nbinding = \"none\"\n[[file]]\nid = \"file.styles\"\nkind = \"stylesheet\"\npath = \"app/theme.pcss\"\n[[file]]\nid = \"file.cfg\"\nkind = \"other\"\npath = \"cfg.json\"\n";
        let mut set = ControlSet::default();
        let path = RepoPath::new("fairlead/control.toml").unwrap();
        set.files.insert(path.clone(), parse_control(&path, root.as_bytes(), true));
        let k = StyleKinds::from_controls(&set);
        assert!(k.applies(&RepoPath::new("app/theme.pcss").unwrap()));
        assert!(!k.applies(&RepoPath::new("cfg.json").unwrap()));
    }

    #[test]
    fn r_file_042__foreign_ranges_come_from_def_foreign_regions() {
        use fairlead_syntax::delimiters::CommentTable;
        use fairlead_syntax::extract::ExtractConfig;
        let cfg = ExtractConfig {
            marker: "fairlead".into(),
            comments: CommentTable::builtin(),
            header_skip: vec![],
            max_file: 2_000_000,
            max_directives: 500,
        };
        let css = "/* fairlead: def foreign.shadcn tool=shadcn +end */\n  --chart-1: red;\n/* fairlead: end */\n:root { --mine: 1; }\n";
        let ex = fairlead_syntax::extract::extract(css.as_bytes(), "css", &cfg);
        let ranges = foreign_ranges(&ex);
        assert_eq!(ranges.len(), 1, "{ranges:?}");
        let s = extract_css_tokens(css, &ranges);
        assert_eq!(s.iter().map(|x| (x.name.as_str(), x.in_foreign)).collect::<Vec<_>>(), [("--chart-1", true), ("--mine", false)]);
    }
}
```

Verify (P2 module paths, see "P2 API as consumed" above): `ExtractConfig` and `Extraction` live in `fairlead_syntax::extract`, `CommentTable` in `fairlead_syntax::delimiters`; adjust the `use` paths to P2's real module layout if they differ. The last test depends on P2 giving a `+end` region the extent `Extent::Lines { first, last }` covering the lines between the directive and its `end` (R-SYN-019).

- [ ] **Step 3: Add `pub mod extract;` to `lib.rs`. Run** `cargo test -p fairlead-core r_file_034 r_file_042 r_arch_111`. Expected: FAIL (compile error: `cannot find function extract_css_tokens`).

- [ ] **Step 4: Write the implementation** (above the tests).

```rust
//! Regex-level CSS custom-property extractor (R-FILE-034). No DOM inference (R-FILE-036).
use super::CssTokenSite;
use crate::config::ControlSet;
use crate::path::RepoPath;
use fairlead_syntax::diag::Span;
use fairlead_syntax::directive::{Extent, Verb};
use fairlead_syntax::extract::Extraction;
use std::collections::BTreeSet;

/// Which files get the CSS extractor: by extension, or because a `[[file]]` item whose file kind
/// has a `css*` extractor declares them.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StyleKinds {
    pub extensions: BTreeSet<String>,
    pub declared: BTreeSet<RepoPath>,
}

impl Default for StyleKinds {
    fn default() -> Self {
        Self { extensions: ["css", "scss", "less"].iter().map(|s| s.to_string()).collect(), declared: BTreeSet::new() }
    }
}

impl StyleKinds {
    pub fn applies(&self, path: &RepoPath) -> bool {
        self.declared.contains(path) || self.extensions.contains(&path.extension().to_ascii_lowercase())
    }

    pub fn from_controls(controls: &ControlSet) -> Self {
        let mut out = Self::default();
        let kinds: Vec<(&String, &crate::config::FileKindDecl)> =
            controls.ok_files().flat_map(|c| c.file_kinds.iter()).collect();
        for c in controls.ok_files() {
            for f in &c.files {
                let is_css = kinds.iter().any(|(name, k)| **name == f.kind && k.extractor.starts_with("css"));
                if is_css {
                    let joined = if c.dir.is_root() { f.path.clone() } else { format!("{}/{}", c.dir.as_str(), f.path) };
                    if let Ok(p) = RepoPath::new(&joined) {
                        out.declared.insert(p);
                    }
                }
            }
        }
        out
    }
}

/// Line ranges (1-based, inclusive) of `def foreign.*` regions found by the syntax extraction.
pub fn foreign_ranges(extraction: &Extraction) -> Vec<(u32, u32)> {
    extraction
        .directives
        .iter()
        .filter(|d| d.verb == Verb::Def && d.ids.first().is_some_and(|id| id.kind_name().starts_with("foreign.")))
        .filter_map(|d| match d.extent {
            Extent::Lines { first, last } => Some((first, last)),
            _ => None,
        })
        .collect()
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flush(seg: &str, pos: Option<(usize, u32, u32)>, stack: &[String], foreign: &[(u32, u32)], out: &mut Vec<CssTokenSite>) {
    let Some((start, line, col)) = pos else { return };
    let Some(rest) = seg.trim().strip_prefix("--") else { return };
    let Some((name, value)) = rest.split_once(':') else { return };
    let name = name.trim_end();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return;
    }
    let full = format!("--{name}");
    out.push(CssTokenSite {
        value: collapse(value),
        context: stack.join(" > "),
        span: Span {
            start: u32::try_from(start).unwrap_or(u32::MAX),
            end: u32::try_from(start + full.len()).unwrap_or(u32::MAX),
            line,
            col,
        },
        in_foreign: foreign.iter().any(|(a, b)| *a <= line && line <= *b),
        name: full,
    });
}

/// Scan CSS-like text. `foreign` are the line ranges of foreign regions. Pure, total, never panics.
pub fn extract_css_tokens(text: &str, foreign: &[(u32, u32)]) -> Vec<CssTokenSite> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut seg = String::new();
    let mut pos: Option<(usize, u32, u32)> = None;
    let (mut line, mut col) = (1u32, 1u32);
    let mut paren = 0u32;
    let mut i = 0usize;
    while let Some(&(byte, c)) = chars.get(i) {
        let next = chars.get(i + 1).map(|p| p.1);
        if c == '/' && next == Some('*') {
            let mut j = i + 2;
            while let Some(&(_, d)) = chars.get(j) {
                if d == '*' && chars.get(j + 1).map(|p| p.1) == Some('/') {
                    j += 2;
                    break;
                }
                j += 1;
            }
            for &(_, d) in chars.get(i..j.min(chars.len())).unwrap_or(&[]) {
                if d == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
            }
            seg.push(' ');
            i = j;
            continue;
        }
        if c == '"' || c == '\'' {
            if pos.is_none() {
                pos = Some((byte, line, col));
            }
            seg.push(c);
            col += 1;
            i += 1;
            while let Some(&(_, d)) = chars.get(i) {
                seg.push(d);
                i += 1;
                if d == '\n' {
                    line += 1;
                    col = 1;
                    break;
                }
                col += 1;
                if d == '\\' {
                    if let Some(&(_, e)) = chars.get(i) {
                        seg.push(e);
                        i += 1;
                        if e == '\n' {
                            line += 1;
                            col = 1;
                        } else {
                            col += 1;
                        }
                    }
                    continue;
                }
                if d == c {
                    break;
                }
            }
            continue;
        }
        match c {
            '{' if paren == 0 => {
                stack.push(collapse(&seg));
                seg.clear();
                pos = None;
            }
            '}' if paren == 0 => {
                flush(&seg, pos, &stack, foreign, &mut out);
                stack.pop();
                seg.clear();
                pos = None;
            }
            ';' if paren == 0 => {
                flush(&seg, pos, &stack, foreign, &mut out);
                seg.clear();
                pos = None;
            }
            _ => {
                if c == '(' {
                    paren += 1;
                } else if c == ')' {
                    paren = paren.saturating_sub(1);
                }
                if pos.is_none() && !c.is_whitespace() {
                    pos = Some((byte, line, col));
                }
                if !(seg.is_empty() && c.is_whitespace()) {
                    seg.push(c);
                }
            }
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
        i += 1;
    }
    flush(&seg, pos, &stack, foreign, &mut out);
    out
}
```

Verify: `Directive.ids[0].kind_name()` returns `"foreign.shadcn"` for `def foreign.shadcn` (kind `foreign`, name `shadcn`, contract 4.1). `Extent` and `Verb` are `Copy`/`PartialEq` (contract 4.2). The test `--a:` yields one site with an empty value; `--a` (no colon) yields none.

- [ ] **Step 5: Run** the Step 3 command. Expected: PASS. **Commit** `feat(core): CSS custom-property extractor and extraction record types`.

---

### Task 20: cache: JSON cache keyed by version, grammar, config hash and as-of

Rules: R-ARCH-070, R-ARCH-072, R-ARCH-073, R-ARCH-074, R-ARCH-077, R-FILE-023 (disposable cache), R-CHK-013 (cache cannot change results; proved end to end in Task 22).

**Files:**
- Create: `crates/fairlead-core/src/cache/mod.rs`, `crates/fairlead-core/src/cache/key.rs`, `crates/fairlead-core/src/cache/files.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod cache;`)

**Interfaces (contract 4.7, names fixed):** `CacheKey { fairlead_version, grammar, config_hash, as_of }`, `Cache::{open, get_extraction, put_extraction, file_row, put_file_rows, token_estimate, put_token_estimate}`, `FileRow { size, mtime_ns, hash }`. Additions: `CacheKey::id`, `Cache::{get_extras, put_extras, is_active, dir}`.

Layout (all under `<root>/.fairlead/cache/<key-id>/`, `key-id` = 20 base32 characters of a blake3 over the key tuple): `key.json` (the tuple, human readable), `files.json` (path to `FileRow`), `x/<hh>/<hash>.json` (an `Extraction`), `e/<plan>/<hh>/<hash>.json` (`Extras` for one extraction plan), `t/<hh>/<hash>.json` (token estimate). Every failure (no permission, corrupt JSON, key skew) degrades to uncached operation or a rebuilt entry and is never an error (R-ARCH-072). Writes go to a temporary file in the same directory and are renamed (R-ARCH-074). `mtime_ns` is stored as a decimal string because JSON numbers do not round-trip `i128`.

- [ ] **Step 1: Write the failing tests.** Create `cache/mod.rs` with only this module (the `use` lines of Step 3 go above it later).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{CssTokenSite, Extras};
    use crate::hash::hash_raw;
    use crate::testutil::{test_root, utf8_tmp};
    use crate::time::AsOf;
    use fairlead_syntax::delimiters::CommentTable;
    use fairlead_syntax::diag::Span;
    use fairlead_syntax::extract::ExtractConfig;

    fn key(version: &str, grammar: u32, cfg: &[u8], as_of: Option<&str>) -> CacheKey {
        CacheKey {
            fairlead_version: version.into(),
            grammar,
            config_hash: hash_raw(cfg),
            as_of: as_of.map(|s| AsOf::parse(s).unwrap()),
        }
    }

    fn sample_extraction() -> Extraction {
        let cfg = ExtractConfig {
            marker: "fairlead".into(),
            comments: CommentTable::builtin(),
            header_skip: vec![],
            max_file: 2_000_000,
            max_directives: 500,
        };
        fairlead_syntax::extract::extract(b"// fairlead: def cmp.a\nfn a() {}\n", "rs", &cfg)
    }

    #[test]
    fn r_arch_070__cache_lives_under_dot_fairlead_and_disabled_touches_nothing() {
        let (_g, t) = utf8_tmp();
        let root = test_root(&t);
        let c = Cache::open(&root, true, key("0.1.0", 1, b"c", None));
        assert!(c.is_active());
        assert!(c.dir().unwrap().starts_with(t.join(".fairlead")), "{:?}", c.dir());
        let (_g2, t2) = utf8_tmp();
        let off = Cache::open(&test_root(&t2), false, key("0.1.0", 1, b"c", None));
        assert!(!off.is_active());
        assert!(!t2.join(".fairlead").exists(), "a disabled cache creates nothing");
        let h = hash_raw(b"x");
        off.put_extraction(&h, &sample_extraction());
        assert!(off.get_extraction(&h).is_none());
    }

    #[test]
    fn r_arch_073__extraction_roundtrips_as_readable_json() {
        let (_g, t) = utf8_tmp();
        let c = Cache::open(&test_root(&t), true, key("0.1.0", 1, b"c", None));
        let h = hash_raw(b"file");
        let e = sample_extraction();
        assert!(c.get_extraction(&h).is_none());
        c.put_extraction(&h, &e);
        assert_eq!(*c.get_extraction(&h).unwrap(), e);
        let hex = h.to_hex();
        let path = c.dir().unwrap().join("x").join(&hex[..2]).join(format!("{hex}.json"));
        let text = std::fs::read_to_string(path.as_std_path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(v.get("directives").is_some(), "plain JSON, no binary format: {text}");
        assert!(text.contains('\n') || text.len() < 4000);
    }

    #[test]
    fn r_arch_072__corrupt_entries_are_discarded_not_errors() {
        let (_g, t) = utf8_tmp();
        let c = Cache::open(&test_root(&t), true, key("0.1.0", 1, b"c", None));
        let h = hash_raw(b"file");
        c.put_extraction(&h, &sample_extraction());
        let hex = h.to_hex();
        let path = c.dir().unwrap().join("x").join(&hex[..2]).join(format!("{hex}.json"));
        std::fs::write(path.as_std_path(), b"{ not json").unwrap();
        assert!(c.get_extraction(&h).is_none());
        assert!(!path.exists(), "the corrupt entry is removed so it is rebuilt");
        c.put_extraction(&h, &sample_extraction());
        assert!(c.get_extraction(&h).is_some());
    }

    #[test]
    fn r_arch_072__unwritable_location_degrades_to_uncached_operation() {
        let (_g, t) = utf8_tmp();
        std::fs::write(t.join(".fairlead").as_std_path(), b"i am a file, not a directory").unwrap();
        let c = Cache::open(&test_root(&t), true, key("0.1.0", 1, b"c", None));
        assert!(!c.is_active());
        let h = hash_raw(b"x");
        c.put_extraction(&h, &sample_extraction());
        assert!(c.get_extraction(&h).is_none());
        c.put_file_rows(&[(RepoPath::new("a.rs").unwrap(), FileRow { size: 1, mtime_ns: 2, hash: h })]);
        assert!(c.file_row(&RepoPath::new("a.rs").unwrap()).is_some(), "rows still work in memory for this run");
    }

    #[test]
    fn r_arch_072__corrupt_file_table_is_rebuilt_empty() {
        let (_g, t) = utf8_tmp();
        let k = key("0.1.0", 1, b"c", None);
        let c = Cache::open(&test_root(&t), true, k.clone());
        std::fs::write(c.dir().unwrap().join("files.json").as_std_path(), b"[[[").unwrap();
        let again = Cache::open(&test_root(&t), true, k);
        assert!(again.is_active());
        assert!(again.file_row(&RepoPath::new("a.rs").unwrap()).is_none());
    }

    #[test]
    fn r_arch_074__writes_are_atomic_and_concurrent_writers_are_harmless() {
        let (_g, t) = utf8_tmp();
        let c = std::sync::Arc::new(Cache::open(&test_root(&t), true, key("0.1.0", 1, b"c", None)));
        let h = hash_raw(b"same");
        let e = sample_extraction();
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (c, e) = (std::sync::Arc::clone(&c), e.clone());
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        c.put_extraction(&h, &e);
                        if let Some(got) = c.get_extraction(&h) {
                            assert_eq!(*got, e, "readers only ever see complete files");
                        }
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let hex = h.to_hex();
        let dir = c.dir().unwrap().join("x").join(&hex[..2]);
        let names: Vec<String> = std::fs::read_dir(dir.as_std_path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, [format!("{hex}.json")], "no temporary files are left behind");
    }

    #[test]
    fn r_arch_077__entries_are_valid_only_for_the_exact_key_tuple() {
        let (_g, t) = utf8_tmp();
        let root = test_root(&t);
        let h = hash_raw(b"f");
        let base = key("0.1.0", 1, b"c", None);
        Cache::open(&root, true, base.clone()).put_extraction(&h, &sample_extraction());
        assert!(Cache::open(&root, true, base.clone()).get_extraction(&h).is_some());
        for other in [
            key("0.1.1", 1, b"c", None),
            key("0.1.0", 2, b"c", None),
            key("0.1.0", 1, b"d", None),
            key("0.1.0", 1, b"c", Some("2026-10-09")),
        ] {
            assert_ne!(other.id(), base.id());
            assert!(Cache::open(&root, true, other).get_extraction(&h).is_none());
        }
        assert_ne!(key("0.1.0", 1, b"c", Some("2026-10-09")).id(), key("0.1.0", 1, b"c", Some("2026-10-10")).id());
        assert_eq!(key("0.1.0", 1, b"c", Some("2026-10-09")).id(), key("0.1.0", 1, b"c", Some("2026-10-09")).id());
        assert_eq!(base.id().len(), 20);
    }

    #[test]
    fn r_arch_071__file_rows_roundtrip_through_disk_including_huge_mtimes() {
        let (_g, t) = utf8_tmp();
        let root = test_root(&t);
        let k = key("0.1.0", 1, b"c", None);
        let rows = vec![
            (RepoPath::new("a.rs").unwrap(), FileRow { size: 3, mtime_ns: 1_700_000_000_123_456_789, hash: hash_raw(b"a") }),
            (RepoPath::new("b/c.rs").unwrap(), FileRow { size: 0, mtime_ns: -5, hash: hash_raw(b"b") }),
            (RepoPath::new("d.rs").unwrap(), FileRow { size: 9, mtime_ns: i128::MAX / 2, hash: hash_raw(b"d") }),
        ];
        Cache::open(&root, true, k.clone()).put_file_rows(&rows);
        let again = Cache::open(&root, true, k);
        for (p, row) in &rows {
            assert_eq!(again.file_row(p).as_ref(), Some(row));
        }
        assert!(again.file_row(&RepoPath::new("zzz").unwrap()).is_none());
    }

    #[test]
    fn r_arch_071__token_estimates_and_extras_are_keyed_by_hash_and_plan() {
        let (_g, t) = utf8_tmp();
        let c = Cache::open(&test_root(&t), true, key("0.1.0", 1, b"c", None));
        let h = hash_raw(b"f");
        assert_eq!(c.token_estimate(&h), None);
        c.put_token_estimate(&h, 42);
        assert_eq!(c.token_estimate(&h), Some(42));
        let extras = Extras {
            css_tokens: vec![CssTokenSite { name: "--a".into(), value: "1".into(), context: ":root".into(), span: Span { start: 0, end: 3, line: 1, col: 1 }, in_foreign: false }],
            ..Extras::default()
        };
        c.put_extras(&h, "plan-a", &extras);
        assert_eq!(c.get_extras(&h, "plan-a"), Some(extras));
        assert_eq!(c.get_extras(&h, "plan-b"), None);
    }
}
```

- [ ] **Step 2: Add `pub mod cache;`. Run** `cargo test -p fairlead-core r_arch_070 r_arch_071 r_arch_072 r_arch_073 r_arch_074 r_arch_077`. Expected: FAIL (compile error: `cannot find struct Cache`).

- [ ] **Step 3: Write `cache/key.rs`.**

```rust
//! The tuple every cache entry is valid for (Q-TOOL-1, R-ARCH-077).
use crate::hash::{base32, ContentHash};
use crate::time::AsOf;

/// `as_of` is `Some` only for entries whose result depends on the date (R-ARCH-090).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CacheKey {
    pub fairlead_version: String,
    pub grammar: u32,
    pub config_hash: ContentHash,
    pub as_of: Option<AsOf>,
}

impl CacheKey {
    /// 20 base32 characters of a blake3 over the tuple; the name of the cache directory.
    pub fn id(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(b"fairlead-cache-v1\0");
        h.update(self.fairlead_version.as_bytes());
        h.update(&[0]);
        h.update(&self.grammar.to_le_bytes());
        h.update(&self.config_hash.0);
        match self.as_of {
            Some(a) => {
                h.update(b"as-of\0");
                h.update(a.to_string().as_bytes());
            }
            None => {
                h.update(b"no-as-of\0");
            }
        }
        base32(h.finalize().as_bytes(), 20)
    }

    /// Readable description stored as `key.json` (diagnostic aid; validated on open).
    pub(crate) fn describe(&self) -> serde_json::Value {
        serde_json::json!({
            "fairlead_version": self.fairlead_version,
            "grammar": self.grammar,
            "config_hash": self.config_hash.to_hex(),
            "as_of": self.as_of.map(|a| a.to_string()),
        })
    }
}
```

- [ ] **Step 4: Write `cache/files.rs`.**

```rust
//! The file table: one `FileRow` per path (50-architecture Section 6).
use crate::hash::ContentHash;
use crate::path::RepoPath;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FileRow {
    pub size: u64,
    #[serde(with = "i128_string")]
    pub mtime_ns: i128,
    pub hash: ContentHash,
}

mod i128_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &i128, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i128, D::Error> {
        let s = String::deserialize(d)?;
        s.parse::<i128>().map_err(serde::de::Error::custom)
    }
}

pub(crate) type FileTable = BTreeMap<RepoPath, FileRow>;
```

- [ ] **Step 5: Write the implementation of `cache/mod.rs`** (above the tests).

```rust
//! Disposable JSON cache in `<root>/.fairlead/` (50-architecture Section 6). Never a source of truth.
mod files;
mod key;

pub use files::FileRow;
pub use key::CacheKey;

use crate::discover::Root;
use crate::extract::Extras;
use crate::hash::ContentHash;
use crate::path::RepoPath;
use camino::Utf8PathBuf;
use files::FileTable;
use fairlead_syntax::extract::Extraction;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Debug)]
pub struct Cache {
    dir: Option<Utf8PathBuf>,
    rows: Mutex<FileTable>,
}

fn read_json<T: DeserializeOwned>(path: &Utf8PathBuf) -> Option<T> {
    let bytes = std::fs::read(path.as_std_path()).ok()?;
    match serde_json::from_slice::<T>(&bytes) {
        Ok(v) => Some(v),
        Err(_) => {
            let _ = std::fs::remove_file(path.as_std_path()); // corrupt: discard and rebuild
            None
        }
    }
}

/// Temporary file in the same directory, then rename (R-ARCH-074). Every failure is ignored.
fn write_atomic<T: Serialize>(path: &Utf8PathBuf, value: &T) {
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent.as_std_path()).is_err() {
        return;
    }
    let Ok(bytes) = serde_json::to_vec(value) else { return };
    let Ok(mut tmp) = tempfile::NamedTempFile::new_in(parent.as_std_path()) else { return };
    if std::io::Write::write_all(&mut tmp, &bytes).is_err() {
        return;
    }
    let _ = tmp.persist(path.as_std_path());
}

impl Cache {
    /// Infallible: an unusable location yields an inactive cache.
    pub fn open(root: &Root, enabled: bool, key: CacheKey) -> Cache {
        let none = || Cache { dir: None, rows: Mutex::new(FileTable::new()) };
        if !enabled {
            return none();
        }
        let dir = root.dir.join(".fairlead").join("cache").join(key.id());
        let described = key.describe();
        let key_file = dir.join("key.json");
        if let Some(existing) = read_json::<serde_json::Value>(&key_file) {
            if existing != described {
                let _ = std::fs::remove_dir_all(dir.as_std_path()); // version skew: discard
            }
        }
        if std::fs::create_dir_all(dir.as_std_path()).is_err() {
            return none();
        }
        write_atomic(&key_file, &described);
        let rows = read_json::<FileTable>(&dir.join("files.json")).unwrap_or_default();
        Cache { dir: Some(dir), rows: Mutex::new(rows) }
    }

    pub fn is_active(&self) -> bool {
        self.dir.is_some()
    }

    pub fn dir(&self) -> Option<&Utf8PathBuf> {
        self.dir.as_ref()
    }

    fn shard(&self, kind: &str, h: &ContentHash) -> Option<Utf8PathBuf> {
        let hex = h.to_hex();
        let first = hex.get(..2)?.to_string();
        Some(self.dir.as_ref()?.join(kind).join(first).join(format!("{hex}.json")))
    }

    pub fn get_extraction(&self, h: &ContentHash) -> Option<Arc<Extraction>> {
        read_json::<Extraction>(&self.shard("x", h)?).map(Arc::new)
    }

    pub fn put_extraction(&self, h: &ContentHash, e: &Extraction) {
        if let Some(p) = self.shard("x", h) {
            write_atomic(&p, e);
        }
    }

    pub fn get_extras(&self, h: &ContentHash, plan: &str) -> Option<Extras> {
        read_json::<Extras>(&self.shard(&format!("e/{plan}"), h)?)
    }

    pub fn put_extras(&self, h: &ContentHash, plan: &str, extras: &Extras) {
        if let Some(p) = self.shard(&format!("e/{plan}"), h) {
            write_atomic(&p, extras);
        }
    }

    pub fn file_row(&self, p: &RepoPath) -> Option<FileRow> {
        self.rows.lock().unwrap_or_else(PoisonError::into_inner).get(p).cloned()
    }

    /// Merge rows into the in-memory table (always) and rewrite `files.json` (when active).
    pub fn put_file_rows(&self, rows: &[(RepoPath, FileRow)]) {
        let mut table = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
        for (p, r) in rows {
            table.insert(p.clone(), r.clone());
        }
        if let Some(dir) = &self.dir {
            write_atomic(&dir.join("files.json"), &*table);
        }
    }

    pub fn token_estimate(&self, h: &ContentHash) -> Option<u32> {
        read_json::<u32>(&self.shard("t", h)?)
    }

    pub fn put_token_estimate(&self, h: &ContentHash, tokens: u32) {
        if let Some(p) = self.shard("t", h) {
            write_atomic(&p, &tokens);
        }
    }
}
```

Verify: `Extraction` must derive `Serialize, Deserialize, PartialEq, Debug, Clone` (contract: "cached"); `tempfile::NamedTempFile::persist` replaces an existing target atomically on all three platforms (on Windows it can fail while a reader holds the target: the failure is ignored and the next run rewrites). `PathBuf`s from `camino` are `Utf8PathBuf`; `starts_with` in the first test uses `Utf8Path::starts_with`.

- [ ] **Step 6: Run** the Step 2 command. Expected: PASS. **Commit** `feat(core): disposable JSON cache keyed by version, grammar, config hash and as-of`.

---

### Task 21: extract/mod.rs: `FileRecord`, `ExtractPlan`, pure `extract_file`

Rules: R-ARCH-030, R-ARCH-023 (NUL, size and UTF-8 halves, pure side), R-ARCH-076, R-FILE-004, R-FILE-005, R-FILE-009, R-AGT-018, R-PORT-016, R-FILE-034 (wiring), C-37.

**Files:**
- Modify: `crates/fairlead-core/src/extract/mod.rs` (append)
- Create: `crates/fairlead-core/src/extract/literals.rs` (STUB, replaced wholesale by P5)
- Create: `crates/fairlead-core/src/secrets.rs` (STUB, replaced wholesale by P5)
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod secrets;`)

**Interfaces (contract 4.5, names fixed):** `FileRecord { path, class, size, hash, extraction, css_tokens, literals, secrets, skip }`, `ExtractPlan { syntax, classify, styles, leak_properties }`, `extract_file(&WalkEntry, &[u8], &ExtractPlan) -> FileRecord`, `scan_literals(text, ext, properties) -> Vec<LiteralSite>` and `scan_secrets(text) -> Vec<SecretHit>` (signatures only; P5 implements). Additions: `ExtractPlan::{from_controls, fingerprint}`, `extras_wanted`, `skip_of`.

Behaviour fixed here:
- `extract_file` is pure of `(bytes, path, plan)`: it reads no other file (R-ARCH-030) and its `size` is `bytes.len()`.
- Directive scanning happens only when `scans_directives(class)` and the path is not in a `[scan] skip` glob. Otherwise `extraction` is `None`.
- `skip` is derived from the syntax status: `SkippedTooLarge`, `SkippedBinary`, `SkippedNotUtf8` map to `SkipReason::{TooLarge, Binary, NotUtf8}` (C-37). The hash is `hash_content`: normalised for UTF-8 text, raw bytes otherwise (R-PORT-016).
- `css_tokens`: only for Authored, scanned files for which `StyleKinds::applies` holds, with foreign regions from the syntax extraction. `literals`: only for Authored, scanned files and only when `leak_properties` is non-empty. `secrets`: for every UTF-8 file under a managerial folder, regardless of `skip` (R-AGT-032, fail closed).
- The P5 stubs return empty vectors. P5 replaces `extract/literals.rs` and `secrets.rs` in full; nothing else in P3 changes (see Execution handoff).

- [ ] **Step 1: Write the two stubs.**

`extract/literals.rs`:

```rust
//! P5 REPLACES THIS FILE. P3 ships only the contract signature (4.5) so `extract_file` compiles.
use super::LiteralSite;

pub fn scan_literals(_text: &str, _ext: &str, _properties: &[String]) -> Vec<LiteralSite> {
    Vec::new()
}
```

`secrets.rs`:

```rust
//! P5 REPLACES THIS FILE. P3 ships only the contract signature (4.5) so `extract_file` compiles.
use crate::extract::SecretHit;

pub fn scan_secrets(_text: &str) -> Vec<SecretHit> {
    Vec::new()
}
```

Add `pub mod secrets;` to `lib.rs` (the `pub mod literals;` line is part of the Step 4 block for `extract/mod.rs`).

- [ ] **Step 2: Write the failing tests** (append a test module to `extract/mod.rs`).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{parse_control, ControlSet};
    use crate::diag::codes;
    use crate::hash::{hash_normalised, hash_raw};
    use crate::testutil::{test_root, ROOT_CONTROL};
    use camino::Utf8Path;

    pub(super) fn plan(extra_root: &str, leak: &[&str]) -> ExtractPlan {
        let mut set = ControlSet::default();
        let p = RepoPath::new("fairlead/control.toml").unwrap();
        set.files.insert(p.clone(), parse_control(&p, format!("{ROOT_CONTROL}{extra_root}").as_bytes(), true));
        ExtractPlan::from_controls(
            &test_root(Utf8Path::new("/r")),
            &set,
            StyleKinds::default(),
            leak.iter().map(|s| s.to_string()).collect(),
        )
        .unwrap()
    }

    fn entry(path: &str, bytes: &[u8]) -> WalkEntry {
        WalkEntry { path: RepoPath::new(path).unwrap(), size: bytes.len() as u64, mtime_ns: 0 }
    }

    fn rec(path: &str, bytes: &[u8], plan: &ExtractPlan) -> FileRecord {
        extract_file(&entry(path, bytes), bytes, plan)
    }

    const RS: &[u8] = b"// fairlead: def cmp.a\nfn a() {}\n";

    #[test]
    fn r_arch_030__extraction_is_a_pure_function_of_bytes_path_and_plan() {
        let p = plan("", &[]);
        let a = rec("src/does-not-exist-on-disk.rs", RS, &p);
        let b = rec("src/does-not-exist-on-disk.rs", RS, &p);
        assert_eq!(a, b);
        let ex = a.extraction.as_ref().expect("authored source is scanned");
        assert_eq!(ex.directives.len(), 1);
        assert_eq!((a.class, a.size, a.skip), (FileClass::Authored, RS.len() as u64, None));
        assert_eq!(a.hash, hash_normalised(RS));
    }

    #[test]
    fn r_arch_076__crlf_and_lf_checkouts_give_the_same_hash_and_directives() {
        let p = plan("", &[]);
        let lf = rec("src/a.rs", RS, &p);
        let crlf_bytes = String::from_utf8(RS.to_vec()).unwrap().replace('\n', "\r\n");
        let crlf = rec("src/a.rs", crlf_bytes.as_bytes(), &p);
        assert_eq!(lf.hash, crlf.hash);
        assert_eq!(lf.extraction.as_ref().unwrap().directives.len(), crlf.extraction.as_ref().unwrap().directives.len());
    }

    #[test]
    fn r_port_016__non_utf8_is_hashed_raw_and_skipped_for_directives() {
        let p = plan("", &[]);
        let bytes = b"// fairlead: def cmp.l\n// caf\xE9\n";
        let r = rec("src/legacy.rs", bytes, &p);
        assert_eq!(r.hash, hash_raw(bytes));
        assert_eq!(r.skip, Some(SkipReason::NotUtf8));
        assert!(r.extraction.as_ref().unwrap().directives.is_empty());
        assert!(r.extraction.as_ref().unwrap().diags.iter().any(|d| d.code == codes::HN_I003));
    }

    #[test]
    fn r_arch_023__nul_in_the_first_8k_and_oversize_are_skip_records() {
        let p = plan("[scan]\nmax-file = 100\n", &[]);
        let mut bin = vec![b'a'; 50];
        bin[10] = 0;
        let r = rec("assets/x.bin", &bin, &p);
        assert_eq!(r.skip, Some(SkipReason::Binary));
        let big = vec![b'a'; 101];
        let r = rec("assets/big.txt", &big, &p);
        assert_eq!(r.skip, Some(SkipReason::TooLarge));
        assert_eq!(r.hash, hash_normalised(&big), "C-37: the skip record still carries its hash");
    }

    #[test]
    fn r_file_004__reference_files_are_never_scanned() {
        let p = plan("", &[]);
        let r = rec("fairlead/ref/syntax.md", b"<!-- fairlead: def cmp.x -->\n", &p);
        assert_eq!(r.class, FileClass::Reference);
        assert!(r.extraction.is_none());
        assert_eq!(r.hash, hash_normalised(b"<!-- fairlead: def cmp.x -->\n"));
    }

    #[test]
    fn r_file_005_and_009__vendor_owned_and_external_generated_text_is_ignored() {
        let p = plan("[[vendor]]\npath = \"ui/**\"\ntool = \"shadcn\"\n[[convention]]\nglob = \"**/*.gen.ts\"\nclass = \"external-generated\"\n", &[]);
        let a = rec("ui/button.tsx", b"// fairlead: def cmp.evil\n", &p);
        assert_eq!((a.class, a.extraction.is_none()), (FileClass::VendorOwned, true));
        let b = rec("src/api.gen.ts", b"// fairlead: def cmp.evil\n", &p);
        assert_eq!((b.class, b.extraction.is_none()), (FileClass::ExternalGenerated, true));
    }

    #[test]
    fn r_file_009__external_generated_folders_declared_by_convention_are_ignored() {
        let p = plan("[[convention]]\nglob = \"proto/**\"\nclass = \"external-generated\"\n", &[]);
        let r = rec("proto/a.rs", b"// fairlead: def cmp.evil\n", &p);
        assert_eq!((r.class, r.extraction.is_none()), (FileClass::ExternalGenerated, true));
        assert_eq!(r.hash, hash_normalised(b"// fairlead: def cmp.evil\n"));
    }

    #[test]
    fn r_agt_018__skip_globs_stop_scanning_but_not_hashing() {
        let p = plan("[scan]\nskip = [\"tests/fixtures/**\"]\n", &[]);
        let r = rec("tests/fixtures/a.rs", RS, &p);
        assert!(r.extraction.is_none());
        assert_eq!(r.class, FileClass::Authored);
        assert_eq!(r.hash, hash_normalised(RS));
    }

    #[test]
    fn r_file_034__css_tokens_only_for_authored_stylesheets_with_foreign_flags() {
        let p = plan("[[vendor]]\npath = \"vendor/**\"\ntool = \"x\"\n", &[]);
        let css = b"/* fairlead: def foreign.shadcn tool=shadcn +end */\n  --chart-1: red;\n/* fairlead: end */\n:root { --mine: 1; }\n";
        let r = rec("app/globals.css", css, &p);
        let got: Vec<(&str, bool)> = r.css_tokens.iter().map(|t| (t.name.as_str(), t.in_foreign)).collect();
        assert_eq!(got, [("--chart-1", true), ("--mine", false)]);
        assert!(rec("vendor/theme.css", css, &p).css_tokens.is_empty(), "vendor-owned CSS is not indexed");
        assert!(rec("src/a.ts", css, &p).css_tokens.is_empty());
    }

    #[test]
    fn r_agt_018__p5_scanners_are_called_only_where_the_contract_says() {
        // With the P3 stubs both stay empty; the gating is observable through `extras_wanted`.
        let p = plan("", &["background-color"]);
        let path = |s: &str| RepoPath::new(s).unwrap();
        assert!(extras_wanted(&p, &path("src/a.tsx"), FileClass::Authored), "literal scan needs leak properties");
        assert!(extras_wanted(&p, &path("fairlead/docs/x.md"), FileClass::Authored), "secrets: managerial folder");
        assert!(!extras_wanted(&p, &path("src/a.tsx"), FileClass::VendorOwned));
        let none = plan("", &[]);
        assert!(!extras_wanted(&none, &path("src/a.tsx"), FileClass::Authored));
        assert!(extras_wanted(&none, &path("src/a.css"), FileClass::Authored), "css tokens");
    }

    #[test]
    fn r_syn_039__plan_uses_marker_comments_and_limits_from_the_root_control_file() {
        let p = plan("[scan]\nmax-file = 77\nmax-directives = 3\n", &[]);
        assert_eq!((p.syntax.max_file, p.syntax.max_directives), (77, 3));
        assert_eq!(p.syntax.marker, "fairlead");
        assert_eq!(plan("", &[]).fingerprint(), plan("", &[]).fingerprint());
        assert_ne!(plan("", &[]).fingerprint(), plan("", &["color"]).fingerprint());
        assert_ne!(plan("", &[]).fingerprint(), plan("[scan]\nmax-file = 5\n", &[]).fingerprint());
    }
}
```

- [ ] **Step 3: Run** `cargo test -p fairlead-core r_arch_030 r_arch_076 r_port_016 r_arch_023 r_file_004 r_file_005 r_agt_018 r_syn_039 r_file_034`. Expected: FAIL (compile error: `cannot find struct ExtractPlan`).

- [ ] **Step 4: Append the implementation to `extract/mod.rs`** (above the test module; merge the `use` lines at the top of the file).

```rust
use crate::classify::{classify, scans_directives, ClassifyRules, FileClass};
use crate::config::ControlSet;
use crate::discover::Root;
use crate::error::Error;
use crate::hash::{base32, hash_content, ContentHash};
use crate::path::RepoPath;
use crate::walk::{SkipReason, WalkEntry};
use fairlead_syntax::delimiters::{CommentTable, Leader};
use fairlead_syntax::extract::{extract as syntax_extract, ExtractConfig, Extraction, ScanStatus};
use std::sync::Arc;

pub mod literals;

pub use literals::scan_literals;

/// Per-file record (contract 4.5). Equal for equal bytes, path and plan, whatever the cache state.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileRecord {
    pub path: RepoPath,
    pub class: FileClass,
    pub size: u64,
    /// Normalised for UTF-8 text, raw bytes otherwise (R-ARCH-076, R-PORT-016).
    pub hash: ContentHash,
    /// `None` when the class forbids scanning, a `[scan] skip` glob matches, or the file was not read.
    pub extraction: Option<Arc<Extraction>>,
    pub css_tokens: Vec<CssTokenSite>,
    pub literals: Vec<LiteralSite>,
    pub secrets: Vec<SecretHit>,
    pub skip: Option<SkipReason>,
}

/// Everything extraction needs from the control files, computed once per run.
#[derive(Clone, Debug)]
pub struct ExtractPlan {
    pub syntax: ExtractConfig,
    pub classify: ClassifyRules,
    pub styles: StyleKinds,
    /// CSS properties named by `applies=` of declared tokens (sorted, deduplicated); empty = no literal scan.
    pub leak_properties: Vec<String>,
}

impl ExtractPlan {
    /// Build the plan from the root control file: marker, `[comments]`, `[scan]`, plus the class rules.
    pub fn from_controls(
        root: &Root,
        controls: &ControlSet,
        styles: StyleKinds,
        mut leak_properties: Vec<String>,
    ) -> Result<ExtractPlan, Error> {
        let Some(rc) = controls.root_control() else {
            return Err(Error::Usage("the root control file did not parse; extraction needs its [scan] and [comments]".into()));
        };
        let mut comments = CommentTable::builtin();
        for (ext, specs) in &rc.comments {
            let mut leaders = Vec::new();
            for s in specs {
                // Validated at load (parse_comments); a failure here would be an internal inconsistency.
                match Leader::parse(s) {
                    Ok(l) => leaders.push(l),
                    Err(e) => return Err(Error::Internal(format!("comment leader {s:?} for .{ext}: {e:?}"))),
                }
            }
            comments = comments.with_overrides(ext, leaders);
        }
        leak_properties.sort();
        leak_properties.dedup();
        Ok(ExtractPlan {
            syntax: ExtractConfig {
                marker: rc.fairlead.marker.clone().unwrap_or_else(|| "fairlead".to_string()),
                comments,
                header_skip: rc.scan.header_skip.clone(),
                max_file: rc.scan.max_file,
                max_directives: rc.scan.max_directives,
            },
            classify: ClassifyRules::from_controls(root, controls),
            styles,
            leak_properties,
        })
    }

    /// 16 base32 characters over everything that changes extraction output; part of the cache key
    /// of cached `Extras`.
    pub fn fingerprint(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(&self.syntax.config_fingerprint());
        for p in &self.leak_properties {
            h.update(p.as_bytes());
            h.update(&[0]);
        }
        h.update(b"styles\0");
        for e in &self.styles.extensions {
            h.update(e.as_bytes());
            h.update(&[0]);
        }
        for d in &self.styles.declared {
            h.update(d.as_str().as_bytes());
            h.update(&[0]);
        }
        base32(h.finalize().as_bytes(), 16)
    }
}

/// Which text-derived extras exist for this path and class (the cache reads them only if true).
pub fn extras_wanted(plan: &ExtractPlan, path: &RepoPath, class: FileClass) -> bool {
    let scanned_authored = class == FileClass::Authored && !plan.classify.skips(path);
    (scanned_authored && (plan.styles.applies(path) || !plan.leak_properties.is_empty()))
        || plan.classify.in_managerial_folder(path)
}

/// `SkipReason` for a syntax status (C-37).
pub fn skip_of(ex: &Extraction) -> Option<SkipReason> {
    match ex.status {
        ScanStatus::SkippedTooLarge => Some(SkipReason::TooLarge),
        ScanStatus::SkippedBinary => Some(SkipReason::Binary),
        ScanStatus::SkippedNotUtf8 => Some(SkipReason::NotUtf8),
        ScanStatus::Scanned | ScanStatus::NoMarker => None,
    }
}

fn compute_extras(path: &RepoPath, bytes: &[u8], class: FileClass, scanned: bool, plan: &ExtractPlan, ex: Option<&Extraction>) -> Extras {
    let Ok(text) = std::str::from_utf8(bytes) else { return Extras::default() };
    let ext = path.extension().to_ascii_lowercase();
    let mut out = Extras::default();
    if scanned && class == FileClass::Authored && plan.styles.applies(path) {
        let foreign = ex.map(foreign_ranges).unwrap_or_default();
        out.css_tokens = extract_css_tokens(text, &foreign);
    }
    if scanned && class == FileClass::Authored && !plan.leak_properties.is_empty() {
        out.literals = scan_literals(text, &ext, &plan.leak_properties);
    }
    if plan.classify.in_managerial_folder(path) {
        out.secrets = crate::secrets::scan_secrets(text);
    }
    out
}

/// Pure per file (R-ARCH-030). `bytes` are already read.
pub fn extract_file(entry: &WalkEntry, bytes: &[u8], plan: &ExtractPlan) -> FileRecord {
    let class = classify(&entry.path, &plan.classify);
    let ext = entry.path.extension().to_ascii_lowercase();
    let scanned = scans_directives(class) && !plan.classify.skips(&entry.path);
    let extraction = scanned.then(|| Arc::new(syntax_extract(bytes, &ext, &plan.syntax)));
    let skip = extraction.as_deref().and_then(skip_of);
    let extras = if skip.is_none() {
        compute_extras(&entry.path, bytes, class, scanned, plan, extraction.as_deref())
    } else {
        Extras::default()
    };
    FileRecord {
        path: entry.path.clone(),
        class,
        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        hash: hash_content(bytes),
        extraction,
        css_tokens: extras.css_tokens,
        literals: extras.literals,
        secrets: extras.secrets,
        skip,
    }
}
```

Verify: `ScanStatus` has exactly the five variants of contract 4.2, so the `match` is exhaustive; `Leader::parse(&str)` is the P2 addition named in Contract issues item 5.

- [ ] **Step 5: Run** the Step 3 command. Expected: PASS. **Commit** `feat(core): FileRecord, ExtractPlan and pure extract_file (P5 scanner stubs)`.

---

### Task 22: extract_files: rayon extraction with the cache, `Corpus`, skip diagnostics

Rules: R-ARCH-023 (NUL probe, over-size and non-UTF-8 handled without a full read where the spec says so), R-ARCH-071 (cold, warm and deleted cache give identical output), R-CHK-013, R-ARCH-080 (any thread count), R-ARCH-024 (sorted), R-PORT-016, R-FILE-004 (reference files carried without extraction), C-37.

**Files:**
- Modify: `crates/fairlead-core/src/extract/mod.rs` (append `Corpus`, `extract_files`, helpers; extend the `tests` module)

**Interfaces (contract 4.5, fixed):** `extract_files(&WalkResult, &Root, &ExtractPlan, &Cache, Option<&BTreeSet<RepoPath>>) -> Result<Corpus, Error>`, `Corpus { records: BTreeMap<RepoPath, FileRecord> }`. Additions: `Corpus::diagnostics`.

Flow per file (inside `rayon` `par_iter`, results gathered in walk order and then inserted into the `BTreeMap`):
1. A file over `plan.syntax.max_file` is never read in full (R-ARCH-023). Its record is a skip record (`TooLarge`) whose hash is `hash_raw` over the text `fairlead:too-large:<size>:` followed by the file's first 8 KB. C-37 asks for "its hash over raw bytes", but ARCH-023 ("never read fully") wins; see Contract issues. Real hashes of large vendor-owned files are computed by `collect_facts` (Task 25), which streams them.
2. A cached file row with equal `(size, mtime_ns)` and a cached extraction (and cached extras when `extras_wanted`) rebuilds the record without opening the file.
3. Otherwise the file is opened, the first 8 KB probed for NUL (`Binary` skip record, not read further), the rest read, `extract_file` run, and the extraction and extras written to the cache; the new row is collected and written once at the end.
4. A file that vanished between walk and read is dropped (not an error); any other I/O error aborts with the first error in path order.

- [ ] **Step 1: Append tests to the `tests` module of `extract/mod.rs`.**

```rust
    use crate::cache::{Cache, CacheKey};
    use crate::testutil::{utf8_tmp, write_bytes, write_file};
    use crate::walk::{walk, WalkOptions};
    use std::collections::BTreeSet;

    struct Fixture {
        _guard: tempfile::TempDir,
        dir: camino::Utf8PathBuf,
        root: crate::discover::Root,
        walked: crate::walk::WalkResult,
        plan: ExtractPlan,
    }

    fn fixture() -> Fixture {
        let (guard, dir) = utf8_tmp();
        write_file(&dir, "fairlead/control.toml", &format!("{ROOT_CONTROL}[scan]\nmax-file = 1000\nskip = [\"tests/fixtures/**\"]\n"));
        write_file(&dir, "fairlead/ref/syntax.md", "<!-- fairlead: def cmp.x -->\n");
        write_file(&dir, "src/a.rs", "// fairlead: def cmp.a\nfn a() {}\n");
        write_file(&dir, "src/b.rs", "fn b() {}\n");
        write_bytes(&dir, "src/legacy.rs", b"// fairlead: def cmp.l\n// caf\xE9\n");
        write_bytes(&dir, "assets/logo.bin", &[0u8, 1, 2, 3, 0, 0, 7]);
        write_bytes(&dir, "assets/big.txt", &vec![b'a'; 3000]);
        write_file(&dir, "web/app.css", ":root { --a: 1; }\n.dark { --a: 2; }\n");
        write_file(&dir, "tests/fixtures/f.rs", "// fairlead: def cmp.f\n");
        let root = test_root(&dir);
        let walked = walk(&root, &WalkOptions { max_file: 1000, ..WalkOptions::default() }).unwrap();
        let set = crate::config::load_controls(&root, &walked).unwrap();
        let plan = ExtractPlan::from_controls(&root, &set, StyleKinds::default(), vec![]).unwrap();
        Fixture { _guard: guard, dir, root, walked, plan }
    }

    fn cache_for(f: &Fixture, enabled: bool) -> Cache {
        Cache::open(&f.root, enabled, CacheKey { fairlead_version: "0.1.0".into(), grammar: 1, config_hash: hash_raw(b"cfg"), as_of: None })
    }

    #[test]
    fn r_arch_071__cold_warm_and_deleted_cache_give_identical_corpora() {
        let f = fixture();
        let cold = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let warm = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        std::fs::remove_dir_all(f.dir.join(".fairlead").as_std_path()).unwrap();
        let rebuilt = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let off = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        assert_eq!(cold, warm);
        assert_eq!(cold, rebuilt);
        assert_eq!(cold, off, "R-CHK-013: --no-cache and a warm cache give identical results");
        assert_eq!(cold.records.len(), f.walked.files.len());
    }

    #[test]
    fn r_chk_013__the_cache_never_changes_results() {
        let f = fixture();
        let on = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let on_again = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let off = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        assert_eq!(on_again, off);
        assert_eq!(on, off);
        assert_eq!(on.diagnostics(), off.diagnostics());
    }

    #[test]
    fn r_arch_071__a_warm_run_does_not_open_unchanged_files() {
        let f = fixture();
        let cold = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        std::fs::remove_file(f.dir.join("src/a.rs").as_std_path()).unwrap();
        let warm = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        assert_eq!(warm.records.get(&RepoPath::new("src/a.rs").unwrap()), cold.records.get(&RepoPath::new("src/a.rs").unwrap()));
        let uncached = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        assert!(!uncached.records.contains_key(&RepoPath::new("src/a.rs").unwrap()), "a vanished file is dropped, not an error");
    }

    #[test]
    fn r_arch_023__skip_records_for_binary_oversize_and_non_utf8() {
        let f = fixture();
        let c = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        let get = |p: &str| &c.records[&RepoPath::new(p).unwrap()];
        assert_eq!(get("assets/logo.bin").skip, Some(SkipReason::Binary));
        assert!(get("assets/logo.bin").extraction.is_none());
        assert_eq!(get("assets/big.txt").skip, Some(SkipReason::TooLarge));
        assert_eq!(get("src/legacy.rs").skip, Some(SkipReason::NotUtf8));
        assert_eq!(get("src/legacy.rs").hash, hash_raw(b"// fairlead: def cmp.l\n// caf\xE9\n"), "R-PORT-016");
        assert_eq!(get("src/a.rs").skip, None);
        assert_eq!(get("src/a.rs").extraction.as_ref().unwrap().directives.len(), 1);
        assert!(get("src/b.rs").extraction.is_some());
        assert!(get("tests/fixtures/f.rs").extraction.is_none(), "[scan] skip");
        assert!(get("fairlead/ref/syntax.md").extraction.is_none(), "R-FILE-004");
        assert_eq!(get("web/app.css").css_tokens.len(), 2);
    }

    #[test]
    fn r_arch_023__diagnostics_report_every_skip_exactly_once() {
        let f = fixture();
        let c = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        let d = c.diagnostics();
        let line = |code: crate::diag::DiagCode, path: &str| d.iter().filter(|x| x.code == code && x.location.path.as_str() == path).count();
        assert_eq!(line(codes::HN_I001, "assets/logo.bin"), 1, "binary: info");
        assert_eq!(line(codes::HN_I003, "src/legacy.rs"), 1, "non-UTF-8: warning, from the syntax extraction");
        assert_eq!(line(codes::HN_I001, "assets/big.txt"), 0, "the over-size note already came from the walk");
        assert!(d.windows(2).all(|w| (&w[0].location.path, w[0].location.line) <= (&w[1].location.path, w[1].location.line)));
    }

    #[test]
    fn r_arch_080__thread_count_does_not_change_the_corpus() {
        let f = fixture();
        let run = |n: usize| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build()
                .unwrap()
                .install(|| extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap())
        };
        assert_eq!(run(1), run(4));
    }

    #[test]
    fn r_arch_075__only_limits_extraction_to_the_given_paths() {
        let f = fixture();
        let only: BTreeSet<RepoPath> = [RepoPath::new("src/a.rs").unwrap(), RepoPath::new("web/app.css").unwrap()].into_iter().collect();
        let c = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), Some(&only)).unwrap();
        assert_eq!(c.records.keys().map(RepoPath::as_str).collect::<Vec<_>>(), ["src/a.rs", "web/app.css"]);
    }

    #[test]
    fn r_arch_071__changed_content_with_a_new_mtime_is_re_extracted() {
        let f = fixture();
        let cache = cache_for(&f, true);
        let first = extract_files(&f.walked, &f.root, &f.plan, &cache, None).unwrap();
        write_file(&f.dir, "src/b.rs", "// fairlead: def cmp.b\nfn b() {}\n");
        let walked2 = walk(&f.root, &WalkOptions { max_file: 1000, ..WalkOptions::default() }).unwrap();
        let second = extract_files(&walked2, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let b = RepoPath::new("src/b.rs").unwrap();
        assert_eq!(first.records[&b].extraction.as_ref().unwrap().directives.len(), 0);
        assert_eq!(second.records[&b].extraction.as_ref().unwrap().directives.len(), 1);
        assert_ne!(first.records[&b].hash, second.records[&b].hash);
    }
```

- [ ] **Step 2: Run** `cargo test -p fairlead-core r_arch_071 r_arch_023 r_arch_080 r_arch_075`. Expected: FAIL (compile error: `cannot find function extract_files`).

- [ ] **Step 3: Append the implementation** (above the test module).

```rust
use crate::cache::{Cache, FileRow};
use crate::diag::{codes, sort_diagnostics, Diagnostic, Location, Severity};
use crate::walk::WalkResult;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;

const PROBE_BYTES: u64 = 8 * 1024;

/// Records of one run, keyed and iterated by path (R-ARCH-060).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Corpus {
    pub records: BTreeMap<RepoPath, FileRecord>,
}

impl Corpus {
    /// Syntax diagnostics of every scanned file plus the `HN-I001` note for binary files, sorted.
    /// Over-size notes come from the walk; non-UTF-8 notes (`HN-I003`) from the syntax extraction.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for rec in self.records.values() {
            if let Some(ex) = &rec.extraction {
                out.extend(ex.diags.iter().cloned().map(|d| Diagnostic::from_syntax(&rec.path, d)));
            }
            if rec.skip == Some(SkipReason::Binary) {
                out.push(
                    Diagnostic::new(
                        codes::HN_I001,
                        Severity::Info,
                        Location::file(rec.path.clone()),
                        "file skipped: binary (NUL byte in the first 8 KB)",
                    )
                    .with_hint("add the path to [scan] skip or [scan] exclude to silence this note"),
                );
            }
        }
        sort_diagnostics(&mut out);
        out
    }
}

enum Read {
    Binary,
    Bytes(Vec<u8>),
}

/// Open once; read the first 8 KB; stop at a NUL byte; otherwise read the rest.
fn read_probing(abs: &camino::Utf8Path, rel: &RepoPath) -> Result<Option<Read>, Error> {
    let mut f = match std::fs::File::open(abs.as_std_path()) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(rel.as_str(), "open", &e, false)),
    };
    let mut head = Vec::new();
    if let Err(e) = f.by_ref().take(PROBE_BYTES).read_to_end(&mut head) {
        return Err(Error::io(rel.as_str(), "read", &e, false));
    }
    if head.contains(&0) {
        return Ok(Some(Read::Binary));
    }
    if let Err(e) = f.read_to_end(&mut head) {
        return Err(Error::io(rel.as_str(), "read", &e, false));
    }
    Ok(Some(Read::Bytes(head)))
}

fn extras_key(fp: &str, plan: &ExtractPlan, path: &RepoPath) -> String {
    format!("{fp}-m{}-s{}", u8::from(plan.classify.in_managerial_folder(path)), u8::from(plan.styles.applies(path)))
}

/// A record for a file that is not read: no extraction, `skip` set, stand-in hash (see Task 22 note).
fn unread_record(e: &WalkEntry, class: FileClass, skip: SkipReason, head: &[u8]) -> FileRecord {
    let mut stand_in = format!("fairlead:{}:{}:", if skip == SkipReason::TooLarge { "too-large" } else { "binary" }, e.size).into_bytes();
    stand_in.extend_from_slice(head);
    FileRecord {
        path: e.path.clone(),
        class,
        size: e.size,
        hash: crate::hash::hash_raw(&stand_in),
        extraction: None,
        css_tokens: Vec::new(),
        literals: Vec::new(),
        secrets: Vec::new(),
        skip: Some(skip),
    }
}

fn from_cache(e: &WalkEntry, class: FileClass, row: &FileRow, plan: &ExtractPlan, cache: &Cache, fp: &str) -> Option<FileRecord> {
    let scanned = scans_directives(class) && !plan.classify.skips(&e.path);
    let extraction = if scanned { Some(cache.get_extraction(&row.hash)?) } else { None };
    let extras = if extras_wanted(plan, &e.path, class) {
        cache.get_extras(&row.hash, &extras_key(fp, plan, &e.path))?
    } else {
        Extras::default()
    };
    let skip = extraction.as_deref().and_then(skip_of);
    Some(FileRecord {
        path: e.path.clone(),
        class,
        size: e.size,
        hash: row.hash,
        extraction,
        css_tokens: extras.css_tokens,
        literals: extras.literals,
        secrets: extras.secrets,
        skip,
    })
}

type Processed = Option<(FileRecord, Option<(RepoPath, FileRow)>)>;

fn process(e: &WalkEntry, root: &Root, plan: &ExtractPlan, cache: &Cache, fp: &str) -> Result<Processed, Error> {
    let class = classify(&e.path, &plan.classify);
    if e.size > plan.syntax.max_file {
        let head = read_head_best_effort(&root.dir.join(e.path.as_str()));
        return Ok(Some((unread_record(e, class, SkipReason::TooLarge, &head), None)));
    }
    if let Some(row) = cache.file_row(&e.path) {
        if row.size == e.size && row.mtime_ns == e.mtime_ns {
            if let Some(rec) = from_cache(e, class, &row, plan, cache, fp) {
                return Ok(Some((rec, None)));
            }
        }
    }
    let abs = root.dir.join(e.path.as_str());
    match read_probing(&abs, &e.path)? {
        None => Ok(None),
        Some(Read::Binary) => Ok(Some((unread_record(e, class, SkipReason::Binary, &[]), None))),
        Some(Read::Bytes(bytes)) => {
            let rec = extract_file(e, &bytes, plan);
            if let Some(ex) = &rec.extraction {
                cache.put_extraction(&rec.hash, ex);
            }
            if extras_wanted(plan, &e.path, class) && rec.skip.is_none() {
                let extras = Extras { css_tokens: rec.css_tokens.clone(), literals: rec.literals.clone(), secrets: rec.secrets.clone() };
                cache.put_extras(&rec.hash, &extras_key(fp, plan, &e.path), &extras);
            }
            let row = FileRow { size: e.size, mtime_ns: e.mtime_ns, hash: rec.hash };
            Ok(Some((rec, Some((e.path.clone(), row)))))
        }
    }
}

/// First 8 KB of an over-large file (never the whole file); empty on any error.
fn read_head_best_effort(abs: &camino::Utf8Path) -> Vec<u8> {
    let mut head = Vec::new();
    if let Ok(f) = std::fs::File::open(abs.as_std_path()) {
        let _ = f.take(PROBE_BYTES).read_to_end(&mut head);
    }
    head
}

/// S4: I/O plus rayon; the result is a `BTreeMap`, so output order never depends on scheduling.
pub fn extract_files(
    walk: &WalkResult,
    root: &Root,
    plan: &ExtractPlan,
    cache: &Cache,
    only: Option<&BTreeSet<RepoPath>>,
) -> Result<Corpus, Error> {
    let fp = plan.fingerprint();
    let entries: Vec<&WalkEntry> = walk.files.iter().filter(|e| only.is_none_or(|o| o.contains(&e.path))).collect();
    let results: Vec<Result<Processed, Error>> = entries.par_iter().map(|e| process(e, root, plan, cache, &fp)).collect();
    let mut records = BTreeMap::new();
    let mut rows = Vec::new();
    for r in results {
        if let Some((rec, row)) = r? {
            if let Some(row) = row {
                rows.push(row);
            }
            records.insert(rec.path.clone(), rec);
        }
    }
    if !rows.is_empty() {
        cache.put_file_rows(&rows);
    }
    Ok(Corpus { records })
}
```

Verify: `Read::by_ref().take(n)` needs `std::io::Read` in scope (imported as `_`); the `Read` enum above does not clash because the trait is imported anonymously. `Option::is_none_or` is stable since Rust 1.82. The `Extraction` JSON round trip must be lossless (including `toml::Table` inside `Frontmatter`): `r_arch_071__cold_warm...` is the test that proves it; if it fails, the bug is in P2's serde derives, not here.

- [ ] **Step 4: Run** the Step 2 command. Expected: PASS (the `HN_I003` assertion depends on P2 emitting the warning for non-UTF-8 input, R-FILE-015).

- [ ] **Step 5: Commit** `feat(core): extract_files with rayon, cache reuse, skip records and corpus diagnostics`.

---

### Task 23: manifest.rs: Cargo workspace hints

Rules: R-ARCH-006, R-SCOPE-016 (manifests are an oracle for hints, never edited, declarations win).

**Files:**
- Create: `crates/fairlead-core/src/manifest.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod manifest;`)

**Interfaces (contract 4.4, fixed):** `ManifestHints { workspace_members: BTreeSet<RepoPath>, default_excludes: Vec<String> }`, `read_manifest_hints(&Root, &WalkResult) -> ManifestHints`. `ManifestHints` also derives `Default`.

Behaviour: every `Cargo.toml` listed by the walk is read with `toml` (read-only). A `[workspace]` table contributes its `members` globs (relative to the manifest's directory, matched against the directories that hold a listed `Cargo.toml`) minus its `exclude` globs. Any parsed manifest adds the hint `target/` to `default_excludes`. A manifest that cannot be read or parsed contributes nothing and never fails the run. `package.json`, `pnpm-workspace.yaml` and `cargo metadata` are not used.

- [ ] **Step 1: Write the failing tests.** Create `manifest.rs` with only this module.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{test_root, utf8_tmp, write_file};
    use crate::walk::{walk, WalkOptions};

    fn hints(files: &[(&str, &str)]) -> (ManifestHints, camino::Utf8PathBuf, tempfile::TempDir) {
        let (g, t) = utf8_tmp();
        for (p, c) in files {
            write_file(&t, p, c);
        }
        let root = test_root(&t);
        let w = walk(&root, &WalkOptions::default()).unwrap();
        (read_manifest_hints(&root, &w), t, g)
    }

    fn members(h: &ManifestHints) -> Vec<&str> {
        h.workspace_members.iter().map(RepoPath::as_str).collect()
    }

    const PKG: &str = "[package]\nname = \"x\"\nversion = \"0.1.0\"\n";

    #[test]
    fn r_arch_006__workspace_members_expand_globs_and_honour_exclude() {
        let (h, _t, _g) = hints(&[
            ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\", \"tools/x\"]\nexclude = [\"crates/skip\"]\n"),
            ("crates/a/Cargo.toml", PKG),
            ("crates/b/Cargo.toml", PKG),
            ("crates/skip/Cargo.toml", PKG),
            ("crates/notacrate/readme.md", "x"),
            ("tools/x/Cargo.toml", PKG),
            ("other/Cargo.toml", PKG),
        ]);
        assert_eq!(members(&h), ["crates/a", "crates/b", "tools/x"]);
        assert_eq!(h.default_excludes, ["target/"]);
    }

    #[test]
    fn r_scope_016__a_nested_workspace_is_relative_to_its_own_directory() {
        let (h, _t, _g) = hints(&[
            ("engine/Cargo.toml", "[workspace]\nmembers = [\"parts/*\"]\n"),
            ("engine/parts/p1/Cargo.toml", PKG),
            ("parts/p2/Cargo.toml", PKG),
        ]);
        assert_eq!(members(&h), ["engine/parts/p1"]);
    }

    #[test]
    fn r_arch_006__unparseable_manifests_yield_no_hints_and_never_fail() {
        let (h, _t, _g) = hints(&[("Cargo.toml", "[workspace\nmembers = "), ("a/Cargo.toml", PKG)]);
        assert!(h.workspace_members.is_empty());
        assert_eq!(h.default_excludes, ["target/"], "the parsable manifest still contributes its hint");
        let (none, _t2, _g2) = hints(&[("Cargo.toml", "= broken")]);
        assert_eq!(none, ManifestHints::default());
    }

    #[test]
    fn r_arch_006__no_manifest_no_hints_and_only_cargo_toml_is_read() {
        let (h, _t, _g) = hints(&[("package.json", "{\"workspaces\": [\"packages/*\"]}"), ("pnpm-workspace.yaml", "packages: ['a']"), ("packages/a/package.json", "{}")]);
        assert_eq!(h, ManifestHints::default());
    }

    #[test]
    fn r_scope_016__a_plain_package_adds_only_the_default_exclude() {
        let (h, _t, _g) = hints(&[("Cargo.toml", PKG)]);
        assert!(h.workspace_members.is_empty());
        assert_eq!(h.default_excludes, ["target/"]);
    }

    #[test]
    fn r_scope_016__manifests_are_never_modified() {
        let ws = "[workspace]\nmembers = [\"crates/*\"]\n";
        let (_h, t, _g) = hints(&[("Cargo.toml", ws), ("crates/a/Cargo.toml", PKG)]);
        assert_eq!(std::fs::read_to_string(t.join("Cargo.toml").as_std_path()).unwrap(), ws);
        assert_eq!(std::fs::read_to_string(t.join("crates/a/Cargo.toml").as_std_path()).unwrap(), PKG);
    }

    #[test]
    fn r_scope_016__a_member_without_a_listed_manifest_is_not_a_hint() {
        let (h, _t, _g) = hints(&[("Cargo.toml", "[workspace]\nmembers = [\"gone\", \"here\"]\n"), ("here/Cargo.toml", PKG)]);
        assert_eq!(members(&h), ["here"]);
    }
}
```

- [ ] **Step 2: Add `pub mod manifest;`. Run** `cargo test -p fairlead-core r_arch_006 r_scope_016`. Expected: FAIL (compile error: `cannot find type ManifestHints`).

- [ ] **Step 3: Write the implementation** (above the tests).

```rust
//! Cargo workspace hints (R-ARCH-006, R-SCOPE-016). Read-only; failure means "no hints".
use crate::discover::Root;
use crate::path::RepoPath;
use crate::walk::WalkResult;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::BTreeSet;

/// Oracle for eligibility hints and the default excludes `init` proposes; never authoritative.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ManifestHints {
    pub workspace_members: BTreeSet<RepoPath>,
    pub default_excludes: Vec<String>,
}

fn strings(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(toml::Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn globs(base: &RepoPath, patterns: &[String]) -> GlobSet {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        let p = p.trim_start_matches("./").trim_end_matches('/');
        let full = if base.is_root() { p.to_string() } else { format!("{}/{}", base.as_str(), p) };
        if let Ok(g) = GlobBuilder::new(&full).literal_separator(true).build() {
            b.add(g);
        }
    }
    b.build().unwrap_or_else(|_| GlobSet::empty())
}

pub fn read_manifest_hints(root: &Root, walk: &WalkResult) -> ManifestHints {
    let manifests: BTreeSet<RepoPath> =
        walk.files.iter().filter(|f| f.path.file_name() == "Cargo.toml").map(|f| f.path.clone()).collect();
    let manifest_dirs: BTreeSet<RepoPath> = manifests.iter().filter_map(RepoPath::parent).collect();
    let mut hints = ManifestHints::default();
    let mut parsed_any = false;
    for m in &manifests {
        let abs = root.dir.join(m.as_str());
        let Ok(text) = std::fs::read_to_string(abs.as_std_path()) else { continue };
        let Ok(table) = text.parse::<toml::Table>() else { continue };
        parsed_any = true;
        let Some(base) = m.parent() else { continue };
        let Some(ws) = table.get("workspace").and_then(toml::Value::as_table) else { continue };
        let include = globs(&base, &strings(ws.get("members")));
        let exclude = globs(&base, &strings(ws.get("exclude")));
        for d in &manifest_dirs {
            if *d != base && d.is_within(&base) && include.is_match(d.as_str()) && !exclude.is_match(d.as_str()) {
                hints.workspace_members.insert(d.clone());
            }
        }
    }
    if parsed_any {
        hints.default_excludes.push("target/".to_string());
    }
    hints
}
```

- [ ] **Step 4: Run** the Step 2 command. Expected: PASS. **Commit** `feat(core): read-only Cargo manifest hints`.

---

### Task 24: git.rs: read-only repository facts (changed set, sparse checkout, shallow)

Rules: R-ARCH-005 (all git access in one module), R-ARCH-075 (changed set), R-ARCH-081 (one handle per worker), R-ARCH-102 (cannot determine is never a pass), R-ARCH-025 and R-ARCH-026 (no repository, sparse checkout), R-GIT-007, R-GIT-071 (open from the root), R-GIT-074 (shallow flag), R-CLI-010 (read-only: no commit, stage or push API), C-3, C-36.

**Files:**
- Create: `crates/fairlead-core/src/git.rs`
- Create: `crates/fairlead-core/src/facts.rs` (only `ChangedSet` and `GitFacts` now; Task 25 completes it)
- Modify: `crates/fairlead-core/src/lib.rs` (`pub mod git; pub mod facts;`)

**Interfaces (contract 4.4, names fixed):** `GitRepo::open(&Root) -> Option<GitRepo>`, `GitRepo::changed_files() -> Result<ChangedSet, Error>`, `GitRepo::sparse_absent() -> Result<Vec<RepoPath>, Error>`, `ChangedSet(pub BTreeSet<RepoPath>)`, `GitFacts { available, changed, sparse_absent, unreadable }`. Additions: `GitRepo::is_shallow() -> bool` (a `shallow` field on `GitFacts` is a 0.2 amendment, see Contract issues), `is_skip_worktree(u16)`, a `#[cfg(test)] test_support` module so tests elsewhere can build repositories without naming the git library.

Behaviour fixed here:
- `open` starts at `Root.dir` (never the working directory) with the standard upward search from there, so a root inside a larger repository works; a bare repository or no repository is `None`. Paths are made relative to the root and paths outside it are dropped. `GitRepo` is not `Sync` (the underlying handle is not): every worker opens its own.
- `changed_files` = working tree plus index against HEAD, untracked non-ignored files included, ignored files excluded, submodules excluded; a deleted or renamed-away path is included under its old name. Any libgit2 failure is `Error::CannotDetermine` (class Internal, exit 5), never an empty set.
- `sparse_absent` is empty unless `core.sparseCheckout` is true; then every index entry with the skip-worktree flag whose file is missing from the work tree, sorted.

- [ ] **Step 1: Create `facts.rs` with the git-facing types.**

```rust
//! Everything a pure check, context or sync-plan needs from disk or git, gathered once per run
//! (contract 4.4). Task 24 creates the git-facing types; Task 25 adds `Facts` and `collect_facts`.
use crate::path::RepoPath;
use std::collections::BTreeSet;

/// Working tree plus index against HEAD, untracked included (C-36).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ChangedSet(pub BTreeSet<RepoPath>);

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct GitFacts {
    pub available: bool,
    pub changed: Option<ChangedSet>,
    pub sparse_absent: Vec<RepoPath>,
    /// Set when git state could not be read; under `--changed` this means exit 5 (R-ARCH-075).
    pub unreadable: Option<String>,
}
```

- [ ] **Step 2: Write the failing tests.** Create `git.rs` with only this module.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{test_root, utf8_tmp, write_file};

    fn names(c: &ChangedSet) -> Vec<&str> {
        c.0.iter().map(RepoPath::as_str).collect()
    }

    #[test]
    fn r_arch_025__no_repository_means_unavailable() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "a.rs", "x");
        assert!(GitRepo::open(&test_root(&t)).is_none());
    }

    #[test]
    fn r_arch_075__changed_set_is_worktree_plus_index_with_untracked_and_without_ignored() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, ".gitignore", "*.log\n");
        for f in ["a.rs", "b.rs", "keep.rs"] {
            write_file(&t, f, "1");
        }
        test_support::commit_all(&t);
        write_file(&t, "a.rs", "22");
        std::fs::remove_file(t.join("b.rs").as_std_path()).unwrap();
        write_file(&t, "c.rs", "x");
        write_file(&t, "dir/deep/d.rs", "x");
        write_file(&t, "noise.log", "x");
        write_file(&t, "staged.rs", "x");
        test_support::stage(&t, "staged.rs");
        let g = GitRepo::open(&test_root(&t)).expect("repository");
        let changed = g.changed_files().unwrap();
        assert_eq!(names(&changed), ["a.rs", "b.rs", "c.rs", "dir/deep/d.rs", "staged.rs"]);
    }

    #[test]
    fn r_git_007__a_clean_tree_has_an_empty_changed_set_and_a_fresh_repo_works_without_commits() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        let g = GitRepo::open(&test_root(&t)).unwrap();
        assert!(g.changed_files().unwrap().0.is_empty());
        write_file(&t, "new.rs", "x");
        assert_eq!(names(&g.changed_files().unwrap()), ["new.rs"], "an unborn HEAD still yields the untracked file");
    }

    #[test]
    fn r_git_007__a_root_inside_a_larger_repository_sees_only_its_own_subtree() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "proj/a.rs", "1");
        write_file(&t, "outside.txt", "1");
        test_support::commit_all(&t);
        write_file(&t, "proj/a.rs", "22");
        write_file(&t, "proj/new.rs", "x");
        write_file(&t, "outside.txt", "22");
        let g = GitRepo::open(&test_root(&t.join("proj"))).expect("found by searching up from the root");
        assert_eq!(names(&g.changed_files().unwrap()), ["a.rs", "new.rs"]);
    }

    #[test]
    fn r_arch_102__unreadable_git_state_is_cannot_determine_never_an_empty_pass() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "a.rs", "1");
        test_support::commit_all(&t);
        test_support::corrupt_index(&t);
        let g = GitRepo::open(&test_root(&t)).expect("the repository itself still opens");
        let err = g.changed_files().err().expect("a corrupt index must not read as 'nothing changed'");
        assert!(matches!(err, Error::CannotDetermine(_)), "{err}");
        assert_eq!(err.class(), crate::error::ErrorClass::Internal);
    }

    #[test]
    fn r_git_007__skip_worktree_flag_is_bit_14_of_the_extended_flags() {
        assert!(is_skip_worktree(1 << 14));
        assert!(is_skip_worktree((1 << 14) | 1));
        assert!(!is_skip_worktree(1 << 13));
        assert!(!is_skip_worktree(0));
    }

    #[test]
    fn r_arch_026__sparse_checkout_reports_absent_files_and_is_silent_otherwise() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "web/fairlead/control.toml", "x");
        write_file(&t, "src/a.rs", "x");
        test_support::commit_all(&t);
        assert!(GitRepo::open(&test_root(&t)).unwrap().sparse_absent().unwrap().is_empty());
        // Verify: if libgit2 drops the extended flag on write on some build, delete THIS test only;
        // the pure `is_skip_worktree` test above and the empty case remain.
        test_support::mark_skip_worktree(&t, "web/fairlead/control.toml");
        std::fs::remove_dir_all(t.join("web").as_std_path()).unwrap();
        let absent = GitRepo::open(&test_root(&t)).unwrap().sparse_absent().unwrap();
        assert_eq!(absent, vec![RepoPath::new("web/fairlead/control.toml").unwrap()]);
    }

    #[test]
    fn r_git_007__a_skip_worktree_file_that_is_present_is_not_absent() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "web/x.rs", "x");
        test_support::commit_all(&t);
        test_support::mark_skip_worktree(&t, "web/x.rs");
        assert!(GitRepo::open(&test_root(&t)).unwrap().sparse_absent().unwrap().is_empty());
    }

    #[test]
    fn r_git_074__shallow_flag_is_read_from_the_repository() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "a.rs", "1");
        test_support::commit_all(&t);
        let g = GitRepo::open(&test_root(&t)).unwrap();
        assert!(!g.is_shallow());
        test_support::make_shallow(&t);
        assert!(GitRepo::open(&test_root(&t)).unwrap().is_shallow());
    }

    #[test]
    fn r_cli_010__reading_git_state_leaves_the_repository_byte_identical() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "a.rs", "1");
        test_support::commit_all(&t);
        write_file(&t, "a.rs", "22");
        let before = test_support::snapshot_git_dir(&t);
        let g = GitRepo::open(&test_root(&t)).unwrap();
        let _ = g.changed_files().unwrap();
        let _ = g.sparse_absent().unwrap();
        let _ = g.is_shallow();
        assert_eq!(before, test_support::snapshot_git_dir(&t), "no commit, stage, ref or index write happened");
    }

    #[test]
    fn r_arch_081__each_worker_opens_its_own_handle() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "a.rs", "1");
        test_support::commit_all(&t);
        write_file(&t, "a.rs", "22");
        write_file(&t, "b.rs", "x");
        let root = test_root(&t);
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let root = root.clone();
                std::thread::spawn(move || GitRepo::open(&root).expect("open").changed_files().expect("status"))
            })
            .collect();
        let results: Vec<ChangedSet> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(results.windows(2).all(|w| w[0] == w[1]));
        assert_eq!(names(&results[0]), ["a.rs", "b.rs"]);
    }

    #[test]
    fn r_arch_005__the_git_library_is_named_only_in_git_rs() {
        // The needles are built from pieces so this file does not contain them.
        let needles = [concat!("git", "2::"), concat!("use git", "2")];
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        let mut checked = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let p = entry.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "rs") && p.file_name().is_some_and(|n| n != "git.rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    for n in needles {
                        assert!(!text.contains(n), "{} names the git library outside git.rs", p.display());
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 10);
    }
}
```

- [ ] **Step 3: Add `pub mod git; pub mod facts;`. Run** `cargo test -p fairlead-core r_arch_025 r_arch_075 r_git_007 r_git_074 r_arch_102 r_arch_026 r_cli_010 r_arch_081 r_arch_005`. Expected: FAIL (compile error: `cannot find struct GitRepo`).

- [ ] **Step 4: Write the implementation** (above the tests).

```rust
//! The ONLY module that names the git library (R-ARCH-005). Read-only in 0.1: no commit, stage or
//! push API exists (R-CLI-010). Actions that must run hooks or sign are 0.2 and use the user's `git`
//! executable from this same module.
use crate::discover::Root;
use crate::error::Error;
use crate::facts::ChangedSet;
use crate::path::RepoPath;
use std::collections::BTreeSet;

/// `GIT_INDEX_ENTRY_SKIP_WORKTREE` in the extended index flags.
const SKIP_WORKTREE: u16 = 1 << 14;

pub fn is_skip_worktree(flags_extended: u16) -> bool {
    flags_extended & SKIP_WORKTREE != 0
}

/// One repository handle; not `Sync`. Open one per worker (R-ARCH-081).
pub struct GitRepo {
    repo: git2::Repository,
    /// The root relative to the work tree, as a `dir/` prefix; empty when they coincide.
    prefix: String,
}

fn cannot_determine(what: &str, e: &git2::Error) -> Error {
    Error::CannotDetermine(format!("{what}: {e}"))
}

impl GitRepo {
    /// Opened from the root directory, never from the working directory (R-GIT-071).
    pub fn open(root: &Root) -> Option<GitRepo> {
        let repo = git2::Repository::open_ext(
            root.dir.as_std_path(),
            git2::RepositoryOpenFlags::empty(),
            std::iter::empty::<&std::ffi::OsStr>(),
        )
        .ok()?;
        let workdir = dunce::canonicalize(repo.workdir()?).ok()?;
        let base = dunce::canonicalize(root.dir.as_std_path()).ok()?;
        let rel = base.strip_prefix(&workdir).ok()?.to_str()?.replace('\\', "/");
        let prefix = if rel.is_empty() { String::new() } else { format!("{rel}/") };
        Some(GitRepo { repo, prefix })
    }

    fn add(&self, set: &mut BTreeSet<RepoPath>, git_path: &str) {
        if let Some(rest) = git_path.strip_prefix(&self.prefix) {
            if let Ok(p) = RepoPath::new(rest) {
                set.insert(p);
            }
        }
    }

    /// Working tree plus index against HEAD, untracked non-ignored files included (C-36).
    pub fn changed_files(&self) -> Result<ChangedSet, Error> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .include_unmodified(false)
            .exclude_submodules(true)
            .renames_head_to_index(false)
            .renames_index_to_workdir(false);
        let statuses = self.repo.statuses(Some(&mut opts)).map_err(|e| cannot_determine("cannot read git status", &e))?;
        let mut set = BTreeSet::new();
        for entry in statuses.iter() {
            if let Some(p) = entry.path() {
                self.add(&mut set, p);
            }
            for delta in [entry.head_to_index(), entry.index_to_workdir()].into_iter().flatten() {
                if let Some(old) = delta.old_file().path().and_then(|p| p.to_str()) {
                    self.add(&mut set, &old.replace('\\', "/"));
                }
            }
        }
        Ok(ChangedSet(set))
    }

    /// Files whose index entry has skip-worktree set and that are missing from the work tree,
    /// sorted; empty unless `core.sparseCheckout` is true (R-ARCH-026, R-GIT-007).
    pub fn sparse_absent(&self) -> Result<Vec<RepoPath>, Error> {
        let sparse = self
            .repo
            .config()
            .ok()
            .and_then(|c| c.get_bool("core.sparseCheckout").ok())
            .unwrap_or(false);
        if !sparse {
            return Ok(Vec::new());
        }
        let index = self.repo.index().map_err(|e| cannot_determine("cannot read the git index", &e))?;
        let workdir = self.repo.workdir().map(std::path::Path::to_path_buf);
        let mut out = BTreeSet::new();
        for entry in index.iter() {
            if !is_skip_worktree(entry.flags_extended) {
                continue;
            }
            let Ok(path) = String::from_utf8(entry.path.clone()) else { continue };
            if let Some(wd) = &workdir {
                if wd.join(&path).exists() {
                    continue;
                }
            }
            self.add(&mut out, &path);
        }
        Ok(out.into_iter().collect())
    }

    /// The repository's shallow flag (R-GIT-074).
    pub fn is_shallow(&self) -> bool {
        self.repo.is_shallow()
    }
}

/// Test-only repository builders, so tests in other modules never name the git library.
#[cfg(test)]
pub(crate) mod test_support {
    use camino::Utf8Path;
    use std::collections::BTreeMap;

    pub fn init(dir: &Utf8Path) {
        git2::Repository::init(dir.as_std_path()).expect("init");
    }

    /// Stage everything (respecting ignore rules) and commit with a fixed identity and time.
    pub fn commit_all(dir: &Utf8Path) {
        let repo = git2::Repository::open(dir.as_std_path()).expect("open");
        let mut index = repo.index().expect("index");
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None).expect("add_all");
        index.write().expect("write index");
        let tree = repo.find_tree(index.write_tree().expect("write tree")).expect("tree");
        let sig = git2::Signature::new("t", "t@example.com", &git2::Time::new(0, 0)).expect("signature");
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, "c", &tree, &parents).expect("commit");
    }

    pub fn stage(dir: &Utf8Path, rel: &str) {
        let repo = git2::Repository::open(dir.as_std_path()).expect("open");
        let mut index = repo.index().expect("index");
        index.add_path(std::path::Path::new(rel)).expect("add_path");
        index.write().expect("write index");
    }

    pub fn corrupt_index(dir: &Utf8Path) {
        std::fs::write(dir.join(".git/index").as_std_path(), b"garbage, not an index").expect("overwrite index");
    }

    /// Verify: needs index version 3 for the extended flag; see the Verify note on the sparse test.
    pub fn mark_skip_worktree(dir: &Utf8Path, rel: &str) {
        let repo = git2::Repository::open(dir.as_std_path()).expect("open");
        repo.config().expect("config").set_bool("core.sparseCheckout", true).expect("set sparse");
        let mut index = repo.index().expect("index");
        index.set_version(3).expect("index v3");
        let mut entry = index.get_path(std::path::Path::new(rel), 0).expect("entry");
        entry.flags |= 1 << 14; // GIT_INDEX_ENTRY_EXTENDED
        entry.flags_extended |= super::SKIP_WORKTREE;
        index.add(&entry).expect("add entry");
        index.write().expect("write index");
    }

    pub fn make_shallow(dir: &Utf8Path) {
        let repo = git2::Repository::open(dir.as_std_path()).expect("open");
        let head = repo.head().expect("head").peel_to_commit().expect("commit").id();
        std::fs::write(dir.join(".git/shallow").as_std_path(), format!("{head}\n")).expect("shallow file");
    }

    /// Path to bytes of every file under `.git`.
    pub fn snapshot_git_dir(dir: &Utf8Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.join(".git").into_std_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("read_dir") {
                let p = e.expect("entry").path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.insert(p.to_string_lossy().into_owned(), std::fs::read(&p).expect("read"));
                }
            }
        }
        out
    }
}
```

Verify (git2 0.21): `Repository::open_ext` generic arguments, `StatusOptions::{exclude_submodules, renames_*}`, `Config::get_bool`, `Index::{iter, get_path, add, set_version}`, `IndexEntry::{flags, flags_extended, path}` and `Repository::is_shallow` exist as used; `index.add_all(["*"], ..)` takes any `IntoIterator` of `IntoCString`. The `mark_skip_worktree` helper and the shallow helper are the two places most likely to need adjusting; neither is used by production code. `Utf8PathBuf::into_std_path_buf` exists in camino.

- [ ] **Step 5: Run** the Step 3 command. Expected: PASS (the corrupt-index test depends on libgit2 reporting an error for an unparseable index; if it silently treats it as empty, keep the assertion on `err()` and instead truncate `.git/HEAD` to garbage, which `statuses` cannot resolve). If libgit2 refreshes stat data in `.git/index` during `statuses` on some build, the `r_cli_010` snapshot test must skip `.git/index` and keep comparing refs, objects and config. **Commit** `feat(core): read-only git facts (changed set, sparse checkout, shallow flag)`.

---

### Task 25: facts.rs: `Facts` and `collect_facts`

Rules: R-ARCH-025, R-ARCH-026, R-ARCH-102 (unreadable git is data, not a pass), R-FILE-005 / R-FILE-040 (vendor hashes and the lock), R-FILE-007 (generated bytes on disk), R-AGT-002 inputs (agent files), R-CHK-053 input (CODEOWNERS), R-CLI-037 input (approvals), R-ARCH-006, R-GIT-007, R-PORT-016.

**Files:**
- Modify: `crates/fairlead-core/src/facts.rs` (append `Facts`, `collect_facts`, `parse_vendor_lock`; add a test module)

**Interfaces (contract 4.4, fixed):** `Facts { generated_on_disk, agent_files, codeowners, vendor_hashes, vendor_lock, approvals, git, manifest_hints }` (also `Default`), `collect_facts(&Root, &WalkResult, &ControlSet, bool) -> Result<Facts, Error>`. Addition: `parse_vendor_lock(&str) -> BTreeMap<RepoPath, ContentHash>`.

Behaviour fixed here:
- `generated_on_disk`: the raw bytes of every file of class Generated (shards, `members`, `baseline.txt`, `vendored.lock`), not normalised (P5 compares and normalises).
- `agent_files`: the `[agents] targets` of the root control file that exist as regular files (symlinks are not followed), raw text (invalid UTF-8 replaced). A missing target is simply absent: P5 reports `HN-A001`.
- `codeowners`: first existing of `.github/CODEOWNERS`, `CODEOWNERS`, `docs/CODEOWNERS` (GitHub's order), text only, offline.
- `vendor_hashes`: for each vendor-owned file the normalised hash for UTF-8 text and the raw hash otherwise (`hash_content`); files over 16 MB are streamed and hashed raw. `vendor_lock`: parsed from every `vendored.lock`. Lock line grammar (defined here, P5/P7 write exactly this): `<repo-relative path> tool=<tool> h=<64 lowercase hex>`; lines starting `#` and malformed lines are ignored.
- `approvals`: `<folder>/approvals.toml`; absent means empty, unparseable is `Error::Config` (exit 2, fail closed).
- `git`: `available` is whether a repository was found; `changed` only when `want_changed`; `sparse_absent` when available; `unreadable` is set when a repository exists but its state cannot be read, and, when `want_changed` is true and there is no repository at all (so `--changed` can never pass).

- [ ] **Step 1: Write the failing tests** (append to `facts.rs`).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_controls;
    use crate::hash::{hash_normalised, hash_raw};
    use crate::testutil::{test_root, utf8_tmp, write_bytes, write_file, ROOT_CONTROL};
    use crate::walk::{walk, WalkOptions};

    struct F {
        _g: tempfile::TempDir,
        dir: camino::Utf8PathBuf,
        root: Root,
        walked: WalkResult,
        set: ControlSet,
    }

    fn build(extra_root: &str, files: &[(&str, &str)]) -> F {
        let (g, dir) = utf8_tmp();
        write_file(&dir, "fairlead/control.toml", &format!("{ROOT_CONTROL}{extra_root}"));
        for (p, c) in files {
            write_file(&dir, p, c);
        }
        let root = test_root(&dir);
        let walked = walk(&root, &WalkOptions::default()).unwrap();
        let set = load_controls(&root, &walked).unwrap();
        F { _g: g, dir, root, walked, set }
    }

    fn facts(f: &F, want_changed: bool) -> Facts {
        collect_facts(&f.root, &f.walked, &f.set, want_changed).unwrap()
    }

    fn p(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }

    #[test]
    fn r_file_007__generated_files_are_read_raw_and_include_the_baseline_and_lock() {
        let f = build("", &[
            ("fairlead/gen/reg/tok.reg", "# scope root edition=1\r\ntok.a css=--a\r\n"),
            ("fairlead/gen/baseline.txt", "# baseline\n"),
            ("fairlead/vendored.lock", "# lock\n"),
            ("fairlead/docs/adr.md", "authored"),
            ("src/a.rs", "authored"),
        ]);
        let x = facts(&f, false);
        let keys: Vec<&str> = x.generated_on_disk.keys().map(RepoPath::as_str).collect();
        assert_eq!(keys, ["fairlead/gen/baseline.txt", "fairlead/gen/reg/tok.reg", "fairlead/vendored.lock"]);
        assert_eq!(&*x.generated_on_disk[&p("fairlead/gen/reg/tok.reg")], b"# scope root edition=1\r\ntok.a css=--a\r\n");
    }

    #[test]
    fn r_agt_002__agent_files_are_the_existing_targets_only() {
        let f = build("[agents]\ntargets = [\"AGENTS.md\", \"CLAUDE.md\", \"docs/AI.md\"]\n", &[("AGENTS.md", "hello\r\n"), ("docs/AI.md", "x")]);
        let x = facts(&f, false);
        assert_eq!(x.agent_files.keys().map(RepoPath::as_str).collect::<Vec<_>>(), ["AGENTS.md", "docs/AI.md"]);
        assert_eq!(&*x.agent_files[&p("AGENTS.md")], "hello\r\n");
    }

    #[cfg(unix)]
    #[test]
    fn r_agt_002__a_symlinked_agent_file_is_not_followed() {
        let f = build("[agents]\ntargets = [\"AGENTS.md\"]\n", &[("real.md", "secret")]);
        std::os::unix::fs::symlink(f.dir.join("real.md").as_std_path(), f.dir.join("AGENTS.md").as_std_path()).unwrap();
        assert!(facts(&f, false).agent_files.is_empty());
    }

    #[test]
    fn r_chk_053__codeowners_uses_githubs_search_order() {
        let f = build("", &[("CODEOWNERS", "root"), ("docs/CODEOWNERS", "docs")]);
        assert_eq!(facts(&f, false).codeowners.as_deref(), Some("root"));
        let g = build("", &[(".github/CODEOWNERS", "gh"), ("CODEOWNERS", "root")]);
        assert_eq!(facts(&g, false).codeowners.as_deref(), Some("gh"));
        assert_eq!(facts(&build("", &[]), false).codeowners, None);
    }

    #[test]
    fn r_file_005__vendor_files_are_hashed_normalised_for_text_and_raw_for_binary() {
        let f = build("[[vendor]]\npath = \"ui/**\"\ntool = \"shadcn\"\n", &[("ui/a.tsx", "line\r\nline\r\n"), ("ui/b.tsx", "line\nline\n"), ("src/own.rs", "x")]);
        write_bytes(&f.dir, "ui/font.woff", b"\x00\r\n\xff");
        let walked = walk(&f.root, &WalkOptions::default()).unwrap();
        let x = collect_facts(&f.root, &walked, &f.set, false).unwrap();
        assert_eq!(x.vendor_hashes[&p("ui/a.tsx")], hash_normalised(b"line\nline\n"));
        assert_eq!(x.vendor_hashes[&p("ui/a.tsx")], x.vendor_hashes[&p("ui/b.tsx")], "autocrlf cannot cause drift (R-FILE-024)");
        assert_eq!(x.vendor_hashes[&p("ui/font.woff")], hash_raw(b"\x00\r\n\xff"));
        assert!(!x.vendor_hashes.contains_key(&p("src/own.rs")));
    }

    #[test]
    fn r_file_005__vendored_lock_lines_are_parsed_tolerantly() {
        let h = hash_raw(b"a");
        let text = format!("# scope root edition=1 fairlead=0.1.0 body=00000000000000000000\nui/a.tsx tool=shadcn h={}\n\nbroken line\n../escape tool=x h={}\nui/b.tsx tool=x h=nothex\nui/c.tsx h={}\n", h.to_hex(), h.to_hex(), h.to_hex());
        let m = parse_vendor_lock(&text);
        assert_eq!(m.keys().map(RepoPath::as_str).collect::<Vec<_>>(), ["ui/a.tsx", "ui/c.tsx"]);
        assert_eq!(m[&p("ui/a.tsx")], h);
        let f = build("[[vendor]]\npath = \"ui/**\"\ntool = \"x\"\n", &[("fairlead/vendored.lock", text.as_str()), ("ui/a.tsx", "x")]);
        assert_eq!(facts(&f, false).vendor_lock.len(), 2);
    }

    #[test]
    fn r_cli_037__approvals_load_absent_means_empty_and_broken_fails_closed() {
        let good = "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"root\"\napprover = \"@a\"\ndate = 2026-10-09\n";
        let f = build("", &[("fairlead/approvals.toml", good)]);
        assert_eq!(facts(&f, false).approvals.approvals.len(), 1);
        assert!(facts(&build("", &[]), false).approvals.approvals.is_empty());
        let bad = build("", &[("fairlead/approvals.toml", "[[approval]\n")]);
        let e = collect_facts(&bad.root, &bad.walked, &bad.set, false).unwrap_err();
        assert!(matches!(&e, Error::Config(c) if c.path == p("fairlead/approvals.toml")), "{e}");
        assert_eq!(e.class(), crate::error::ErrorClass::Usage);
    }

    #[test]
    fn r_arch_025__outside_a_repository_git_is_unavailable_and_changed_can_never_pass() {
        let f = build("", &[("src/a.rs", "x")]);
        let quiet = facts(&f, false);
        assert!(!quiet.git.available && quiet.git.changed.is_none() && quiet.git.unreadable.is_none());
        let asked = facts(&f, true);
        assert!(!asked.git.available);
        assert!(asked.git.changed.is_none());
        assert!(asked.git.unreadable.as_deref().is_some_and(|m| m.contains("not inside a git repository")), "{:?}", asked.git);
    }

    #[test]
    fn r_arch_075__changed_is_computed_only_when_asked() {
        let f = build("", &[("src/a.rs", "1")]);
        crate::git::test_support::init(&f.dir);
        crate::git::test_support::commit_all(&f.dir);
        write_file(&f.dir, "src/a.rs", "22");
        write_file(&f.dir, "src/new.rs", "x");
        let without = facts(&f, false);
        assert!(without.git.available && without.git.changed.is_none() && without.git.unreadable.is_none());
        let walked = walk(&f.root, &WalkOptions::default()).unwrap();
        let with = collect_facts(&f.root, &walked, &f.set, true).unwrap();
        let changed: Vec<&str> = with.git.changed.as_ref().unwrap().0.iter().map(RepoPath::as_str).collect();
        assert_eq!(changed, ["src/a.rs", "src/new.rs"]);
        assert!(with.git.sparse_absent.is_empty());
    }

    #[test]
    fn r_arch_102__unreadable_git_state_is_reported_not_swallowed() {
        let f = build("", &[("src/a.rs", "1")]);
        crate::git::test_support::init(&f.dir);
        crate::git::test_support::commit_all(&f.dir);
        crate::git::test_support::corrupt_index(&f.dir);
        let x = facts(&f, true);
        assert!(x.git.available);
        assert!(x.git.changed.is_none());
        assert!(x.git.unreadable.is_some(), "P5 turns this into exit 5 under --changed");
    }

    #[test]
    fn r_arch_006__manifest_hints_are_part_of_the_facts() {
        let f = build("", &[("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"), ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n")]);
        let x = facts(&f, false);
        assert_eq!(x.manifest_hints.workspace_members.iter().map(RepoPath::as_str).collect::<Vec<_>>(), ["crates/a"]);
    }

    #[test]
    fn r_arch_060__facts_default_is_empty_for_pure_tests_in_other_phases() {
        let d = Facts::default();
        assert!(d.generated_on_disk.is_empty() && d.agent_files.is_empty() && d.vendor_hashes.is_empty());
        assert!(!d.git.available && d.codeowners.is_none());
    }
}
```

- [ ] **Step 2: Run** `cargo test -p fairlead-core facts::`. Expected: FAIL (compile error: `cannot find type Facts`).

- [ ] **Step 3: Append the implementation to `facts.rs`** (above the test module; merge the `use` lines at the top).

```rust
use crate::classify::{classify, ClassifyRules, FileClass};
use crate::config::{parse_approvals, ApprovalsFile, ControlSet};
use crate::discover::Root;
use crate::error::Error;
use crate::git::GitRepo;
use crate::hash::{hash_content, ContentHash};
use crate::manifest::{read_manifest_hints, ManifestHints};
use crate::walk::WalkResult;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Files above this size are streamed and hashed raw instead of read into memory.
const LARGE_FILE: u64 = 16 * 1024 * 1024;

/// Everything a pure check, context or sync plan needs from disk or git (contract 4.4).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Facts {
    pub generated_on_disk: BTreeMap<RepoPath, Arc<[u8]>>,
    pub agent_files: BTreeMap<RepoPath, Arc<str>>,
    pub codeowners: Option<Arc<str>>,
    pub vendor_hashes: BTreeMap<RepoPath, ContentHash>,
    pub vendor_lock: BTreeMap<RepoPath, ContentHash>,
    pub approvals: ApprovalsFile,
    pub git: GitFacts,
    pub manifest_hints: ManifestHints,
}

/// Bytes of a regular file; `None` for a missing file, a symlink or a directory (never followed).
fn read_regular(root: &Root, rel: &RepoPath) -> Result<Option<Vec<u8>>, Error> {
    let abs = root.dir.join(rel.as_str());
    match std::fs::symlink_metadata(abs.as_std_path()) {
        Ok(m) if m.is_file() => std::fs::read(abs.as_std_path())
            .map(Some)
            .map_err(|e| Error::io(rel.as_str(), "read", &e, false)),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(rel.as_str(), "stat", &e, false)),
    }
}

fn hash_file(root: &Root, rel: &RepoPath, size: u64) -> Result<Option<ContentHash>, Error> {
    if size <= LARGE_FILE {
        return Ok(read_regular(root, rel)?.map(|b| hash_content(&b)));
    }
    let abs = root.dir.join(rel.as_str());
    let file = match std::fs::File::open(abs.as_std_path()) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(rel.as_str(), "open", &e, false)),
    };
    let mut h = blake3::Hasher::new();
    h.update_reader(file).map_err(|e| Error::io(rel.as_str(), "read", &e, false))?;
    Ok(Some(ContentHash(*h.finalize().as_bytes())))
}

/// `vendored.lock` lines: `<repo-relative path> tool=<tool> h=<64 hex>`; `#` lines and malformed
/// lines are ignored (the file is generated, so a malformed line surfaces as drift in P5).
pub fn parse_vendor_lock(text: &str) -> BTreeMap<RepoPath, ContentHash> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(path) = parts.next() else { continue };
        let Ok(path) = RepoPath::new(path) else { continue };
        if let Some(hex) = parts.find_map(|t| t.strip_prefix("h=")) {
            if let Ok(h) = ContentHash::from_hex(hex) {
                out.insert(path, h);
            }
        }
    }
    out
}

fn git_facts(root: &Root, want_changed: bool) -> GitFacts {
    let Some(repo) = GitRepo::open(root) else {
        return GitFacts {
            unreadable: want_changed.then(|| "not inside a git repository: git state is unavailable".to_string()),
            ..GitFacts::default()
        };
    };
    let mut git = GitFacts { available: true, ..GitFacts::default() };
    match repo.sparse_absent() {
        Ok(v) => git.sparse_absent = v,
        Err(e) => git.unreadable = Some(e.to_string()),
    }
    if want_changed {
        match repo.changed_files() {
            Ok(c) => git.changed = Some(c),
            Err(e) => git.unreadable = Some(e.to_string()),
        }
    }
    git
}

/// S5: gather everything once so checks stay pure. I/O (disk, git).
pub fn collect_facts(root: &Root, walk: &WalkResult, controls: &ControlSet, want_changed: bool) -> Result<Facts, Error> {
    let rules = ClassifyRules::from_controls(root, controls);
    let mut facts = Facts::default();
    for e in &walk.files {
        match classify(&e.path, &rules) {
            FileClass::Generated => {
                if let Some(bytes) = read_regular(root, &e.path)? {
                    if e.path.file_name() == "vendored.lock" {
                        facts.vendor_lock.extend(parse_vendor_lock(&String::from_utf8_lossy(&bytes)));
                    }
                    facts.generated_on_disk.insert(e.path.clone(), Arc::<[u8]>::from(bytes));
                }
            }
            FileClass::VendorOwned => {
                if let Some(h) = hash_file(root, &e.path, e.size)? {
                    facts.vendor_hashes.insert(e.path.clone(), h);
                }
            }
            _ => {}
        }
    }
    if let Some(rc) = controls.root_control() {
        for target in &rc.agents {
            let Ok(p) = RepoPath::new(target) else { continue };
            if let Some(bytes) = read_regular(root, &p)? {
                facts.agent_files.insert(p, Arc::<str>::from(String::from_utf8_lossy(&bytes).into_owned()));
            }
        }
    }
    for candidate in [".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"] {
        let Ok(p) = RepoPath::new(candidate) else { continue };
        if let Some(bytes) = read_regular(root, &p)? {
            facts.codeowners = Some(Arc::<str>::from(String::from_utf8_lossy(&bytes).into_owned()));
            break;
        }
    }
    let approvals_rel = RepoPath::new(&format!("{}/approvals.toml", root.folder))
        .map_err(|e| Error::Internal(format!("approvals path: {e}")))?;
    if let Some(bytes) = read_regular(root, &approvals_rel)? {
        facts.approvals = parse_approvals(&bytes).map_err(|f| Error::from(f.with_path(&approvals_rel)))?;
    }
    facts.git = git_facts(root, want_changed);
    facts.manifest_hints = read_manifest_hints(root, walk);
    Ok(facts)
}
```

Move the `use` lines to the top of the file above `ChangedSet`. Verify: `blake3::Hasher::update_reader` needs blake3's default `std` feature (do not disable it); `Arc::<[u8]>::from(Vec<u8>)` and `Arc::<str>::from(String)` exist.

- [ ] **Step 4: Run** `cargo test -p fairlead-core facts::`. Expected: PASS.

- [ ] **Step 5: Commit** `feat(core): collect_facts with generated bytes, vendor hashes, agent files, approvals and git facts`.

---

### Task 26: end-to-end foundation test (public API only)

Rules: supports R-ARCH-071, R-ARCH-080, R-ARCH-111, R-ARCH-043, R-ARCH-021, R-CLI-010 at the P3 boundary (P9's golden and determinism suites repeat them through the binary).

**Files:**
- Create: `crates/fairlead-core/tests/foundation_pipeline.rs`

This test uses only `pub` items and the order of the data-flow diagram: `discover`, `walk`, `load_controls`, `apply_child_excludes`, `ExtractPlan::from_controls`, `extract_files`, `collect_facts`. It is the executable form of the S1 to S5 handoff to P4 to P8.

- [ ] **Step 1: Write the test file.**

```rust
//! The S1..S5 pipeline over a real directory, via the public API only (contract Section 5).
use camino::{Utf8Path, Utf8PathBuf};
use fairlead_core::cache::{Cache, CacheKey};
use fairlead_core::config::{config_hash_of, control_diagnostics, load_controls};
use fairlead_core::discover::{discover, DiscoverRequest};
use fairlead_core::extract::{extract_files, ExtractPlan, StyleKinds};
use fairlead_core::facts::collect_facts;
use fairlead_core::walk::{apply_child_excludes, walk, WalkOptions};
use semver::Version;
use std::collections::BTreeMap;

const ROOT: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n[settings]\nfairlead-version = \"0.1\"\n[scan]\nexclude = [\"dist/\"]\n[agents]\ntargets = [\"AGENTS.md\"]\n[rules.no-raw-hex]\ncheck = \"leak\"\ntokens = [\"tok.*\"]\n";
const WEB: &str = "[fairlead]\nscope = \"web\"\n[scan]\nexclude = [\"gen-out/\"]\n";

/// (path, bytes, is_text): text files are converted to CRLF for the CRLF checkout.
fn tree() -> Vec<(&'static str, Vec<u8>, bool)> {
    let t = |p: &'static str, s: &str| (p, s.as_bytes().to_vec(), true);
    vec![
        t("fairlead/control.toml", ROOT),
        t("fairlead/gen/reg/tok.reg", "# scope root edition=1\ntok.primary css=--primary\n"),
        t("web/fairlead/control.toml", WEB),
        t("web/src/app.css", ":root {\n  --primary: #2563eb;\n}\n.dark {\n  --primary: #60a5fa;\n}\n"),
        t("web/src/App.tsx", "// fairlead: def cmp.app sym=App\nexport function App() { return null; }\n"),
        t("web/gen-out/skipped.js", "// fairlead: def cmp.never\n"),
        t("dist/skipped.js", "// fairlead: def cmp.never\n"),
        t("src/lib.rs", "// fairlead: use tok.primary\npub fn f() {}\n"),
        t("AGENTS.md", "pointer block\n"),
        ("src/legacy.rs", b"// fairlead: def cmp.l\n// caf\xE9\n".to_vec(), false),
        ("assets/logo.bin", vec![0u8, 1, 2, 3], false),
    ]
}

fn materialise(dir: &Utf8Path, crlf: bool) {
    for (p, bytes, is_text) in tree() {
        let full = dir.join(p);
        std::fs::create_dir_all(full.parent().unwrap().as_std_path()).unwrap();
        let out = if crlf && is_text {
            String::from_utf8(bytes).unwrap().replace('\n', "\r\n").into_bytes()
        } else {
            bytes
        };
        std::fs::write(full.as_std_path(), out).unwrap();
    }
}

fn tmp() -> (tempfile::TempDir, Utf8PathBuf) {
    let td = tempfile::tempdir().unwrap();
    let p = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();
    (td, p)
}

/// Everything that must not depend on cache state, thread count, line endings or cwd.
fn run(dir: &Utf8Path, cwd: &str, threads: usize, use_cache: bool) -> Vec<String> {
    let d = discover(&DiscoverRequest {
        root_flag: None,
        env_root: None,
        cwd: dir.join(cwd),
        binary_version: Version::new(0, 1, 0),
        require_root: true,
    })
    .unwrap();
    let w = walk(&d.root, &WalkOptions::from_root_control(&d.root_control, Some(threads))).unwrap();
    let set = load_controls(&d.root, &w).unwrap();
    let w = apply_child_excludes(w, &set).unwrap();
    let plan = ExtractPlan::from_controls(&d.root, &set, StyleKinds::from_controls(&set), vec![]).unwrap();
    let key = CacheKey { fairlead_version: "0.1.0".into(), grammar: 1, config_hash: config_hash_of(&set), as_of: None };
    let cache = Cache::open(&d.root, use_cache, key);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    let corpus = pool.install(|| extract_files(&w, &d.root, &plan, &cache, None)).unwrap();
    let facts = collect_facts(&d.root, &w, &set, false).unwrap();

    let mut out = Vec::new();
    out.push(format!("root scope={} rules={}", d.root_control.fairlead.scope.as_str(), set.declared_rule_count()));
    for (path, rec) in &corpus.records {
        let directives = rec.extraction.as_ref().map(|e| {
            e.directives.iter().map(|x| format!("{:?} {}", x.verb, x.ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(";")
        });
        let css: Vec<String> = rec.css_tokens.iter().map(|t| format!("{}@{}={}", t.name, t.context, t.value)).collect();
        out.push(format!("{path} {:?} {} skip={:?} dirs={:?} css={:?}", rec.class, rec.hash, rec.skip, directives, css));
    }
    for (path, bytes) in &facts.generated_on_disk {
        out.push(format!("generated {path} {}", fairlead_core::hash::hash_normalised(bytes)));
    }
    out.push(format!("agents {:?}", facts.agent_files.keys().map(ToString::to_string).collect::<Vec<_>>()));
    let mut diags: Vec<String> = control_diagnostics(&set).iter().chain(corpus.diagnostics().iter()).map(|x| format!("{} {}", x.code.0, x.location.path)).collect();
    diags.sort();
    out.extend(diags);
    out
}

fn snapshot(dir: &Utf8Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.as_std_path().to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == ".fairlead") {
                    continue;
                }
                stack.push(p);
            } else {
                out.insert(p.to_string_lossy().into_owned(), std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

#[test]
fn r_arch_071__pipeline_is_identical_for_cold_warm_deleted_cache_and_thread_counts() {
    let (_g, dir) = tmp();
    materialise(&dir, false);
    let baseline = run(&dir, ".", 1, false);
    assert!(baseline.len() > 10, "{baseline:#?}");
    assert_eq!(baseline, run(&dir, ".", 4, true), "cold cache, 4 threads");
    assert_eq!(baseline, run(&dir, ".", 4, true), "warm cache");
    std::fs::remove_dir_all(dir.join(".fairlead").as_std_path()).unwrap();
    assert_eq!(baseline, run(&dir, ".", 2, true), "deleted cache");
}

#[test]
fn r_arch_111__pipeline_is_identical_for_lf_and_crlf_checkouts() {
    let (_g1, lf) = tmp();
    let (_g2, crlf) = tmp();
    materialise(&lf, false);
    materialise(&crlf, true);
    assert_eq!(run(&lf, ".", 2, false), run(&crlf, ".", 2, false));
}

#[test]
fn r_arch_043__pipeline_is_independent_of_the_invocation_directory() {
    let (_g, dir) = tmp();
    materialise(&dir, false);
    let from_root = run(&dir, ".", 2, false);
    assert_eq!(from_root, run(&dir, "web/src", 2, false));
    assert_eq!(from_root, run(&dir, "src", 2, false));
}

#[test]
fn r_arch_021__excludes_and_skips_are_applied_by_the_pipeline() {
    let (_g, dir) = tmp();
    materialise(&dir, false);
    let out = run(&dir, ".", 2, false).join("\n");
    assert!(!out.contains("dist/skipped.js"), "root [scan] exclude");
    assert!(!out.contains("web/gen-out/skipped.js"), "child [scan] exclude");
    assert!(out.contains("web/src/app.css") && out.contains("--primary@:root=#2563eb") && out.contains("--primary@.dark=#60a5fa"));
    assert!(out.contains("src/legacy.rs") && out.contains("NotUtf8"));
    assert!(out.contains("assets/logo.bin") && out.contains("Binary"));
    assert!(out.contains("rules=1"));
}

#[test]
fn r_cli_010__the_pipeline_changes_nothing_outside_dot_fairlead() {
    let (_g, dir) = tmp();
    materialise(&dir, false);
    let before = snapshot(&dir);
    let _ = run(&dir, ".", 2, true);
    let _ = run(&dir, ".", 2, true);
    assert_eq!(before, snapshot(&dir), "readers write only under .fairlead/ (R-ARCH-140)");
    assert!(dir.join(".fairlead").exists(), "the cache itself was written");
}
```

`rayon` and `semver` are normal dependencies of `fairlead-core`, so the integration test can name them.

- [ ] **Step 2: Run** `cargo test -p fairlead-core --test foundation_pipeline`. Expected: PASS once Tasks 1 to 25 are complete (before that: compile errors for missing items). If a line-ending test fails, the projection shows which record differs; the likely cause is a hash computed over raw bytes instead of `hash_content` (Task 2) or a CRLF-sensitive extractor (Task 19).

- [ ] **Step 3: Run the phase gate.**

```text
cargo fmt --all --check
cargo clippy -p fairlead-core --all-targets --locked -- -D warnings
cargo test -p fairlead-core
```

Expected: all PASS. Fix lints at the source (no new `#[allow]` without a reason comment). Separately confirm the MSRV job (`cargo +1.88 check -p fairlead-core --all-targets`) in CI.

- [ ] **Step 4: Commit** `test(core): foundation pipeline determinism and read-only checks`.

---

## Traceability

Every rule of the P3 row of contract Section 6 (60 rules) plus the two adjacent rules assigned to P3 (R-PORT-016, R-GIT-007), mapped to the task that implements it and the tests that prove it. A test named `r_x_nnn__...` starts with its rule ID (contract Section 7). Where a rule is a documentation or absence-of-API rule, the test that checks the absence is named.

| Rule | Task | Test functions |
|---|---|---|
| R-SCOPE-004 | 15, 17 | `r_scope_004__search_upward_is_pure_and_nearest_first`, `r_scope_004__flag_then_env_then_search`, `r_scope_004__folder_mismatch_is_hc005_and_renamed_folders_are_found_by_flag`, `r_scope_004__load_controls_finds_every_control_file_and_only_those`, `r_scope_004__second_root_true_below_the_root_is_root_dup` |
| R-SCOPE-006 | 16 | `r_scope_006__symlinks_are_not_followed_and_are_errors_inside_a_managerial_folder` (unix; the Windows junction twin is P9 `platform.rs`) |
| R-SCOPE-007 | 3, 16 | `r_scope_007__case_only_directories_collide`, `r_scope_007__case_only_files_collide`, `r_scope_007__fold_key_is_nfc_and_case_insensitive`, `r_scope_007__case_collisions_and_hygiene_are_hn_p001_on_every_platform`, `r_scope_007__walk_reports_a_real_case_collision_on_a_case_sensitive_filesystem` |
| R-SCOPE-016 | 23 | `r_scope_016__a_nested_workspace_is_relative_to_its_own_directory`, `r_scope_016__a_plain_package_adds_only_the_default_exclude`, `r_scope_016__manifests_are_never_modified`, `r_scope_016__a_member_without_a_listed_manifest_is_not_a_hint` |
| R-SCOPE-018 | 12, 18 | `r_scope_018__convention_actions`, `r_scope_018__convention_needs_exactly_one_action_and_own_is_unknown_in_0_1`, `r_scope_018__class_conventions_mark_external_generated_files` |
| R-SCOPE-051 | 13 | `r_scope_051_052__root_only_tables_in_a_child_are_violations` |
| R-SCOPE-052 | 13 | `r_scope_051_052__root_only_tables_in_a_child_are_violations`, `r_syn_041__child_non_delegable_keys_are_hc003_and_inert` |
| R-SCOPE-056 | 12, 13, 16 | `r_scope_056__child_may_hold_facts_without_violations`, `r_scope_056__allow_and_req_tables`, `r_scope_056__allow_and_req_need_their_keys_and_valid_scope_ids`, `r_scope_056__child_scan_exclude_applies_below_its_own_directory_only` |
| R-SCOPE-065 | 11, 13 | `r_scope_065__kinds_exc_is_rejected_at_load`, `r_scope_065__kinds_exc_fails_the_whole_file` |
| R-FILE-001 | 18 | `r_file_001__every_file_under_a_managerial_folder_has_exactly_one_class`, `r_file_001__nested_scope_folders_follow_the_root_layout`, `r_file_001__unparseable_control_files_contribute_nothing` |
| R-FILE-004 | 18, 21 | `r_file_004__reference_vendor_owned_and_external_generated_are_never_scanned`, `r_file_004__reference_files_are_never_scanned` |
| R-FILE-005 | 11, 18, 21, 25 | `r_file_005__vendor_decl_needs_path_and_tool`, `r_file_005__vendor_declarations_apply_inside_the_declaring_scope_only`, `r_file_005_and_009__vendor_owned_and_external_generated_text_is_ignored`, `r_file_005__vendor_files_are_hashed_normalised_for_text_and_raw_for_binary`, `r_file_005__vendored_lock_lines_are_parsed_tolerantly` |
| R-FILE-009 | 21 | `r_file_009__external_generated_folders_declared_by_convention_are_ignored`, `r_file_005_and_009__vendor_owned_and_external_generated_text_is_ignored` |
| R-FILE-014 | 18 | `r_file_014__class_follows_the_writer_not_the_content` |
| R-FILE-020 | 11, 13, 18 | `r_file_020__file_kinds_need_extractor_and_binding`, `r_file_020__baseline_default_follows_folder_and_layout`, `r_file_020__layout_names_come_from_the_root_control_file` |
| R-FILE-023 | 16, 20 | `r_file_023__managerial_path_matched_by_an_ignore_rule_is_reported`, `r_file_023__a_listed_managerial_folder_is_not_reported`; cache disposable: `r_arch_070__cache_lives_under_dot_fairlead_and_disabled_touches_nothing` |
| R-FILE-024 | 2 | `r_file_024__known_blake3_vector_and_hex_roundtrip`, `r_arch_076__crlf_lf_and_bom_hash_equal` |
| R-FILE-031 | 11 | `r_file_031__source_generate_is_rejected_in_0_1` |
| R-FILE-034 | 19, 21 | `r_file_034__one_token_in_root_dark_and_theme_is_three_value_sites`, `r_file_034__nested_at_rules_join_their_preludes`, `r_file_034__comments_strings_and_urls_do_not_confuse_the_scanner`, `r_file_034__span_is_original_bytes_with_one_based_line_and_scalar_column`, `r_file_034__garbage_never_panics_and_yields_nothing_wrong`, `r_file_034__style_kinds_apply_by_extension_or_declaration`, `r_file_034__style_kinds_follow_declared_stylesheet_file_items`, `r_file_034__css_tokens_only_for_authored_stylesheets_with_foreign_flags` |
| R-FILE-044 | 18 | `r_file_044__vendor_owned_files_are_exempt_from_directive_scanning` |
| R-FILE-050 | 11 | `r_file_050__file_items_parse_with_extends`, `r_file_050__file_item_requires_id_kind_path_and_a_bare_id` |
| R-FILE-054 | 18 | `r_file_054__a_file_with_two_declared_writers_is_reported` |
| R-SYN-038 | 9, 13 | `r_syn_038__header_reads_all_four_frozen_keys`, `r_syn_038__non_root_header_carries_only_scope`, `r_syn_038__header_requires_a_valid_scope`, `r_syn_038__wrong_types_in_header_are_hc001`, `r_syn_038__lenient_pass_ignores_later_tables_and_unknown_header_keys`, `r_syn_038__bom_crlf_and_non_utf8`, `r_syn_038__pin_needs_edition_and_pads_the_version`, `r_syn_038__root_control_parses_the_spec_example`, `r_syn_038__root_requires_edition_and_root_true`, `r_syn_038__settings_owners_accept_handles_and_inline_tables`, `r_syn_038__settings_purpose_is_at_most_160_chars_and_index_values_is_checked` |
| R-SYN-039 | 9, 10, 11, 12, 13 | `r_syn_039__duplicate_keys_are_a_parse_error_with_a_line`, `r_syn_039__invalid_toml_is_hc001`, `r_syn_039__scan_values_parse`, `r_syn_039__unknown_keys_are_collected_not_fatal`, `r_syn_039__wrong_type_is_hc001`, `r_syn_039__comments_and_agents_tables`, `r_syn_039__ovr_table_parses_known_keys_and_field_sets`, `r_syn_039__context_table_parses_and_defaults`, `r_syn_039__unknown_tables_and_keys_warn_but_parse` |
| R-SYN-040 | 3, 10 | `r_syn_040__new_rejects_structural_garbage`, `r_syn_040__hygiene_reports_nfc_reserved_and_trailing`, `r_syn_040__layout_names_and_paths_get_hygiene`, `r_syn_040__scan_patterns_are_nfc_and_forward_slashes` |
| R-SYN-041 | 4, 13, 17 | `r_syn_041__child_non_delegable_keys_are_hc003_and_inert`, `r_syn_041__warnings_and_violations_flow_into_the_diagnostics`, `r_syn_041__control_family_codes_are_the_frozen_ones`, `r_syn_041__policy_and_stage_orders` |
| R-SYN-050 | 11, 13 | `r_syn_050__kind_names_are_two_to_eight_lowercase_letters`, `r_syn_050__kind_fields_parse`, `r_syn_050__cmd_and_tok_tables_are_unknown_tables_not_declarations` |
| R-SYN-051 | 10, 13 | `r_syn_051__settings_use_fairlead_range_not_requires`, `r_syn_051__requires_is_not_a_settings_key` |
| R-SYN-053 | 12 | `r_syn_053__field_level_policy_form`, `r_syn_053__policy_errors` |
| R-CHK-005 | 12 | `r_chk_005__leak_rule_parses_with_all_keys`, `r_chk_005__pattern_rule_compiles_its_regex_at_load`, `r_chk_005__rule_errors_and_builtin_default`, `r_chk_005__waiver_defaults_match_the_contract` |
| R-CHK-013 | 22 | `r_chk_013__the_cache_never_changes_results` |
| R-CHK-014 | 8 | `r_chk_014__parse_accepts_iso_dates_only`, `r_chk_014__as_of_orders_by_date` |
| R-CLI-004 | 15 | `r_cli_004__ordinary_managerial_folders_are_skipped`, `r_cli_004__nearest_root_true_wins_so_a_mounted_subroot_stops_the_search`, `r_cli_004__no_root_is_exit_2_and_never_invented` |
| R-CLI-005 | 6, 15 | `r_cli_005__older_binary_is_refused_with_exact_install_line`, `r_cli_005__newer_binary_same_edition_proceeds_with_info_line`, `r_cli_005__equal_version_and_missing_pin_are_ok`, `r_cli_005__other_edition_is_refused_whichever_side_is_newer`, `r_cli_005__prerelease_binary_satisfies_its_release_pin`, `r_cli_005__pin_version_is_padded_to_three_parts`, `r_cli_005__other_edition_is_refused_and_a_newer_binary_proceeds` |
| R-CLI-010 | 24, 26 | `r_cli_010__reading_git_state_leaves_the_repository_byte_identical`, `r_cli_010__the_pipeline_changes_nothing_outside_dot_fairlead`; the absence of any commit, stage or push function is checked by `r_arch_005__the_git_library_is_named_only_in_git_rs` plus review of the `GitRepo` impl |
| R-AGT-013 | 13 | `r_agt_013__vocabulary_keys_change_only_in_the_root_file` |
| R-AGT-018 | 18, 21 | `r_agt_018__skip_globs_are_scope_relative_and_do_not_change_the_class`, `r_agt_018__skip_globs_stop_scanning_but_not_hashing`, `r_agt_018__p5_scanners_are_called_only_where_the_contract_says` |
| R-ARCH-005 | 24 | `r_arch_005__the_git_library_is_named_only_in_git_rs` |
| R-ARCH-006 | 23, 25 | `r_arch_006__workspace_members_expand_globs_and_honour_exclude`, `r_arch_006__unparseable_manifests_yield_no_hints_and_never_fail`, `r_arch_006__no_manifest_no_hints_and_only_cargo_toml_is_read`, `r_arch_006__manifest_hints_are_part_of_the_facts` |
| R-ARCH-010 | 15 | `r_arch_010__the_vcs_top_level_is_never_a_root` |
| R-ARCH-011 | 15 | `r_arch_011__unparseable_control_on_the_path_fails_closed`, `r_arch_011__root_without_edition_or_with_wrong_types_is_hc001` |
| R-ARCH-012 | 17 | `r_arch_012__declared_rule_count_is_zero_for_a_ruleless_repository` (P5 prints the notice) |
| R-ARCH-013 | 6, 15 | `r_arch_013__supported_edition_is_one`, `r_arch_013__pin_is_checked_before_the_full_parse` |
| R-ARCH-020 | 16 | `r_arch_020__fairleadignore_is_honoured_and_git_info_exclude_is_not_read`, `r_arch_020__invalid_exclude_pattern_is_a_usage_error` |
| R-ARCH-021 | 10, 16, 26 | `r_arch_021__defaults_for_scan_live_in_config_not_in_code`, `r_arch_021__default_excludes_live_in_options_not_in_code`, `r_arch_021__excludes_and_skips_are_applied_by_the_pipeline` |
| R-ARCH-022 | 16 | `r_arch_022__git_and_cache_dirs_are_never_listed`, `r_arch_022__submodule_and_linked_worktree_boundaries_stop_the_walk` |
| R-ARCH-023 | 16, 21, 22 | `r_arch_023__over_large_files_are_listed_skipped_and_noted_without_being_read`, `r_arch_023__nul_in_the_first_8k_and_oversize_are_skip_records`, `r_arch_023__skip_records_for_binary_oversize_and_non_utf8`, `r_arch_023__diagnostics_report_every_skip_exactly_once` |
| R-ARCH-024 | 3, 16 | `r_arch_024__repo_path_orders_bytewise`, `r_arch_024__parent_join_extension_within`, `r_arch_024__from_native_rel_converts_separators`, `r_arch_024__serde_rejects_invalid_paths`, `r_arch_024__result_is_sorted_bytewise_and_thread_count_independent` |
| R-ARCH-025 | 16, 24, 25 | `r_arch_025__walk_works_outside_a_git_repository_and_honours_gitignore`, `r_arch_025__no_repository_means_unavailable`, `r_arch_025__outside_a_repository_git_is_unavailable_and_changed_can_never_pass` |
| R-ARCH-026 | 24 | `r_arch_026__sparse_checkout_reports_absent_files_and_is_silent_otherwise` (and `r_git_007__*`) |
| R-ARCH-030 | 21 | `r_arch_030__extraction_is_a_pure_function_of_bytes_path_and_plan` |
| R-ARCH-070 | 20 | `r_arch_070__cache_lives_under_dot_fairlead_and_disabled_touches_nothing` |
| R-ARCH-072 | 20 | `r_arch_072__corrupt_entries_are_discarded_not_errors`, `r_arch_072__unwritable_location_degrades_to_uncached_operation`, `r_arch_072__corrupt_file_table_is_rebuilt_empty` |
| R-ARCH-073 | 20 | `r_arch_073__extraction_roundtrips_as_readable_json` |
| R-ARCH-074 | 20 | `r_arch_074__writes_are_atomic_and_concurrent_writers_are_harmless` |
| R-ARCH-076 | 2, 13, 21 | `r_arch_076__crlf_lf_and_bom_hash_equal`, `r_arch_076__control_content_hash_ignores_line_endings`, `r_arch_076__crlf_and_lf_checkouts_give_the_same_hash_and_directives` |
| R-ARCH-077 | 15, 17, 20 | `r_arch_077__entries_are_valid_only_for_the_exact_key_tuple`, `r_arch_077__root_config_hash_is_the_normalised_hash_of_the_root_control_bytes`, `r_arch_077__config_hash_changes_with_any_control_file_and_is_otherwise_stable` |
| R-ARCH-081 | 24 | `r_arch_081__each_worker_opens_its_own_handle` |
| R-ARCH-100 | 7 | `r_arch_100__class_mapping`, `r_arch_100__io_not_found_on_user_path_is_usage_else_internal`, `r_arch_100__config_error_exposes_diagnostics`, `r_arch_100__reserved_verb_message_names_the_milestone`, `r_arch_100__core_never_calls_process_exit` |
| R-ARCH-102 | 7, 24, 25 | `r_arch_102__cannot_determine_is_internal_and_says_so`, `r_arch_102__unreadable_git_state_is_cannot_determine_never_an_empty_pass`, `r_arch_102__unreadable_git_state_is_reported_not_swallowed` |
| R-PORT-016 | 2, 21 | `r_port_016__non_utf8_is_hashed_over_raw_bytes`, `r_port_016__non_utf8_is_hashed_raw_and_skipped_for_directives` |
| R-GIT-007 | 24, 25 | `r_git_007__a_clean_tree_has_an_empty_changed_set_and_a_fresh_repo_works_without_commits`, `r_git_007__a_root_inside_a_larger_repository_sees_only_its_own_subtree`, `r_git_007__skip_worktree_flag_is_bit_14_of_the_extended_flags`, `r_git_007__a_skip_worktree_file_that_is_present_is_not_absent` |

Supporting tests that do not carry a P3 rule of their own (they protect names the contract fixes for other phases): `r_cli_003__*` (Task 5, `ExitCode::resolve` precedence 5, 2, 4, 1, 3, 0, proved for the CLI by P8), `r_arch_055__core_codes_are_unique_and_well_formed` and `r_arch_103__*` (Task 4), `r_arch_071__*` and `r_arch_080__*` (Tasks 20, 22, 26; proved end to end by P9), `r_arch_075__*` (Tasks 22, 24, 25; consumed by P5), `r_arch_111__*`, `r_arch_043__*` (Tasks 19, 26), `r_cli_037__*` (Tasks 14, 25; consumed by P7), `r_chk_031__*` (Task 2, fingerprint types; the algorithm is P5), `r_file_042__*` (Task 19), `r_agt_002__*`, `r_chk_053__*`, `r_file_007__*` (Task 25 facts consumed by P5), `r_git_074__*` (Task 24).

## Contract issues

Everything here is a gap or a conflict found while writing the plan. Each was closed by the most consistent choice and needs the owner's confirmation, or a contract amendment in `01-contract.md` Section 9, before the dependent phase starts.

1. **`impl From<SyntaxDiag> for Diagnostic` cannot be lossless.** `Location` needs a `RepoPath`; `SyntaxDiag` carries only a `Span`. Chosen: `Diagnostic::from_syntax(&RepoPath, SyntaxDiag)` is the real conversion; the `From` impl exists (contract name kept) with the root path as placeholder. Callers that know the file must use `from_syntax`.
2. **`Layout.gen` is a reserved word in edition 2024.** The field is written `r#gen`; the name and meaning are unchanged. Callers write `layout.r#gen`.
3. **`OvrOp` lives in the registry module in contract 4.8 but `OvrDecl` (4.6, P3) needs it.** Chosen: defined in `config/schema.rs`; P4's `registry/ovr.rs` does `pub use crate::config::OvrOp;`. The contract name `OvrOp` is unchanged.
4. **`Root.config_hash` is documented as "hash of all control files", but discovery runs before the walk and sees only the root control file.** Chosen: `Root.config_hash` is the normalised hash of the root control file; `config::config_hash_of(&ControlSet)` is the hash over every control file and is what `CacheKey.config_hash` must use (P8 builds the key after `load_controls`).
5. **P2 additions and paths.** (a) `CommentTable::with_overrides` needs `Leader` values but the contract gives `Leader` no constructor: P3 consumes `Leader::parse(&str)` accepting the `[comments]` string forms (`//`, `#`, `--`, `/* */`, `<!-- -->`); P2 must add it. (b) The "P2 API as consumed" table at the top lists the exact module paths used; (c) `Extraction` must round-trip through `serde_json` (the cold/warm test relies on it) and `extract` must emit `HN-I003` for non-UTF-8 input and `Extent::Lines` for `+end` regions.
6. **Discovery cannot express "no root" or carry the info line.** `Discovered` always holds a `Root`, but `init` runs with no root: P3 adds `discover_optional -> Result<Option<Discovered>, Error>`; `discover` is `discover_optional` plus `NoRoot`. `PinVerdict::OkNewerBinary { info }` has no field in `Discovered`; the bin recomputes `pin_check(&binary, SUPPORTED_EDITION, &root.pin)` to print the info line. `DiscoverRequest.env_root` is filled by the bin from `discover::read_env_root()` (the only `std::env` read).
7. **Skip handling is split between walk and extract, and C-37 conflicts with R-ARCH-023.** Chosen: walk lists every regular file (including over-large and binary ones), records `TooLarge` in `WalkResult.skipped` and emits the `HN-I001` note; `Binary` (NUL in the first 8 KB) and `NotUtf8` are found when a file is read, so `SkipReason::{Binary, NotUtf8}` come from `extract`, and `Corpus::diagnostics` emits the binary note. C-37 asks for a hash over raw bytes for every skipped file, R-ARCH-023 says over-large and binary files are never read fully: the spec rule wins, so over-large and binary records carry a deterministic stand-in hash (size plus first 8 KB). A symlink inside a managerial folder uses `HN-C001` (the contract writes "HN-C..."). A file that vanishes between walk and read is dropped, and any other read failure aborts with `Error::Io` because `SkipReason` has no "unreadable" variant.
8. **Control files are read after the walk.** The root's `[scan] exclude` can prune during the walk; a child's `exclude` (scope-relative) cannot. Chosen: `walk::apply_child_excludes(WalkResult, &ControlSet)`, to be called by P8's `load` between S3 and S4. `[scan] skip` globs are not part of `ExtractPlan`'s contract fields; they are carried by `ClassifyRules::skips`. `ExtractPlan::from_controls` is added as the only constructor that reads the control files.
9. **Git facts.** `GitFacts` has no `shallow` field and adding one would break struct literals in P5 tests, so shallow detection is `GitRepo::is_shallow()` only (HN-V010 is 0.2). `GitFacts.unreadable` is also set when sparse detection fails, not only for the changed set; P5 decides what exits 5. `GitRepo::open` searches upward from the root directory (so a root inside a larger repository works and paths are re-based), which is the reading of R-GIT-071 used here.
10. **`vendored.lock` line grammar is not specified anywhere.** Chosen and implemented by `parse_vendor_lock`: `<repo-relative path> tool=<tool> h=<64 lowercase hex>`, `#` lines ignored. P5's `GenKind::VendorLock` renderer and P7's `--accept-vendor` must emit exactly this.
11. **Dependencies beyond contract C-2.** `fairlead-core` also needs `unicode-normalization` (RepoPath folding, control-file path hygiene) and uses `globset`, `tempfile` and `semver` as listed. They are UNVERIFIED until the engineer checks maturity and licence (R-ARCH-120).
12. **Derives.** `ClassifyRules`, `ExtractPlan` and `GitRepo` cannot derive `PartialEq`/`Eq` (compiled globs, a git handle); `FileRecord` derives them but not `Serialize` (it holds an `Arc<Extraction>`; the cache stores `Extraction` and `Extras` separately, so `Cache` gains `get_extras`/`put_extras`). `Diagnostic` has no `Ord` in the contract; `sort_diagnostics` provides the (path, line, col, code, message) order.
13. **`[[ovr]]` in TOML.** TOML cannot write `k+=a,b`. Chosen: a key with a trailing `+` (`"tags+" = [...]`) means append; `OvrDecl.set` keeps the raw key and P4 interprets it. `OvrDecl.exp` is the validated `YYYY-MM-DD` text, not a parsed date. `ConventionAction::Own` exists (0.2) but a 0.1 parse never produces it.
14. **`RepoPath`.** The empty string is the repository root (`RepoPath::root()`), needed for `ControlFile.dir`. `RepoPath::new` checks structure only; non-NFC, Windows-reserved and trailing-dot names are `hygiene()` findings (`HN-P001`) so that a Linux checkout containing them is reported, not refused.
15. **P5 files created by P3.** To compile independently, P3 ships stubs of `extract/literals.rs` (`scan_literals`) and `secrets.rs` (`scan_secrets`) that return nothing. P5 replaces both files wholesale. Until then `FileRecord.literals` and `.secrets` are always empty: no secret is detected. This must not ship; P5's exit check includes "the stubs are gone".
16. **Spec gaps surfaced.** (a) The upward search can only find the default folder name `fairlead` (the folder name is stored inside the file it names); a renamed folder (D-30) is found only through `--root` or `FAIRLEAD_ROOT`. (b) R-ARCH-021 says default excludes live in the control file, while `ManifestHints.default_excludes` proposes `target/` from a manifest: it is a hint for `init` to write into the control file, never applied implicitly. (c) `HN-C002` lines come from a best-effort key search because `toml::Table` carries no spans. (d) Pre-release binaries compare as their release for the pin check, so `0.1.0-alpha.N` satisfies the pin `0.1`.

## Execution handoff

**Recommended execution:** `superpowers:subagent-driven-development`, one fresh subagent per task, in the numeric order above (the order is a topological sort: each task compiles and passes without later tasks). The controller reviews each task's diff against its Interfaces block before the next task starts; a reviewer checks that no contract name changed and that no new `HashMap`, `unwrap`, `println!` or environment/clock read crept in. `superpowers:executing-plans` works the same way in a single session with a checkpoint after Tasks 8, 14, 17, 22 and 25.

**Parallelism inside P3.** Tasks 2 to 8 are independent of the config tasks except for the shared types; Tasks 9 to 14 (config) can run in one worker while Tasks 2 to 8 and 23 run in another, and Task 19 (styles) and Task 20 (cache) are independent of Tasks 15 to 18 once Task 10 exists. P4 may start after Task 14 (it needs `ControlFile`, `ControlSet` types and `parse_control`) with Tasks 15 to 26 still in flight; P5 needs Task 22 and Task 25; P6 needs Task 20 and Task 22; P7 needs Task 14 and Task 25; P8 needs all.

**Handoffs P3 provides** (contract Section 8): to P4: `Root`, `ControlSet`, `ControlFile`, `ConfigFailure`, `Corpus`, `FileRecord`, `FileClass`, `ApprovalsFile`, `RepoPath`, `AsOf`, `ContentHash`, `Diagnostic`, `OvrOp`, and the load-time diagnostics via `control_diagnostics` and `Corpus::diagnostics`. To P5, P6, P7: `Facts`, `GitFacts`, `WalkResult`, `Cache`, `Error`, `ExitCode`, `VerbOutcome`. To P8: `discover`, `discover_optional`, `read_env_root`, `walk`, `WalkOptions::from_root_control`, `load_controls`, `apply_child_excludes`, `config_hash_of`, `ExtractPlan::from_controls`, `StyleKinds::from_controls`, `extract_files`, `collect_facts`, and the call order in `tests/foundation_pipeline.rs`, which P8's `pipeline::load` must reproduce.

**What P3 does not do:** resolution, checks, rendering, generated-file writing, the writer lock, CLI argument handling and the process exit mapping. `ExitCode` and `Error::class` are data; the bin maps them.

**Final gate for P3:** `cargo fmt --all --check`, `cargo clippy -p fairlead-core --all-targets --locked -- -D warnings`, `cargo test -p fairlead-core` on Linux, macOS and Windows, and the MSRV 1.88 check. Then record in `50-architecture.md` Section 11 the verified versions of `regex`, `thiserror`, `unicode-normalization` and the confirmed feature names of `toml`, `jiff` and `git2`, and report the Contract issues above to the maintainer.
