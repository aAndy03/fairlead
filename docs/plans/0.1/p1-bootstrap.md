# P1 Bootstrap and CI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the empty repository into a compiling, linted and CI-gated Cargo workspace: three product crates and one tooling crate, every module of contract section 3 present as a documented stub, a working forbidden-crate scan, the dogfooding folder `fairlead/`, and the first commits. After this phase, phases P2 to P9 only fill files that already exist.

**Architecture:** One Cargo workspace at the repository root (edition 2024, resolver 3, MSRV 1.88). Product crates: `fairlead-syntax` (lib, `#![forbid(unsafe_code)]`), `fairlead-core` (lib, `#![forbid(unsafe_code)]`, one feature `vendored-libgit2`, on by default) and `fairlead` (bin; `unsafe_code` is denied by the workspace lint). `xtask` is tooling and is never shipped. Lints come from `[workspace.lints]`, inherited with `[lints] workspace = true`. `clippy.toml` carries the disallowed types and methods that enforce ordering, clock and environment rules. The GitHub Actions workflows run fmt, clippy, the forbidden-crate scan, cargo-deny, the test suite on Linux, Windows and macOS, the MSRV job and the audit. The fuzz, benchmark and dogfood workflows exist now and run only once the files they test exist. Nothing in P1 reads the network, runs a background process, reads the clock or the environment, or parses YAML.

**Tech Stack:** Rust 1.88.0 as MSRV (`rust-version = "1.88"`, edition 2024); the stable toolchain for development (`rust-toolchain.toml`); cargo-deny; cargo-audit (RustSec); `b3sum` (BLAKE3 checksums of release binaries); actionlint (workflow lint); GitHub Actions on `ubuntu-latest`, `windows-latest` and `macos-latest`. Actions: `actions/checkout@v4`, `actions/upload-artifact@v4`, `actions/download-artifact@v4`, `Swatinem/rust-cache@v2`, `dtolnay/rust-toolchain` (refs `1.88.0` and `nightly`), `taiki-e/install-action@v2`, `softprops/action-gh-release@v2`, `raven-actions/actionlint@v2`. Local shells: PowerShell for Windows-only commands, Git Bash for the file-creation script in Task 3.

**Spec:** Read these before executing. Where a chapter disagrees with this plan about a NAME, the contract wins; about BEHAVIOUR, the chapter and `decisions.md` win; a disagreement is recorded in Contract issues.

- [01-contract.md](./01-contract.md): sections 1 (layout, lints), 2 (dependencies), 3 (module trees), 6 (phase ownership, P1 row), 7 (test conventions), 9 (decisions C-1 to C-40).
- [50-architecture.md](../../50-architecture.md): section 3 (crates, R-ARCH-001 to 007); section 5 (R-ARCH-060); section 9 (R-ARCH-100, 101); section 10 (R-ARCH-110, 111, 113); section 11 (dependency table, R-ARCH-120 to 124); section 13 (R-ARCH-140 to 146, the read-only guarantee the stubs must respect); section 14 (R-ARCH-150 to 153); section 15 (R-ARCH-160 to 162, dogfooding).
- [15-cli.md](../../15-cli.md): R-CLI-009 (no network verb), R-CLI-003 (exit codes, used by the bin and xtask stubs).
- [40-agent-integration-and-trust.md](../../40-agent-integration-and-trust.md): R-AGT-007 (no background process, no watchers).
- [60-roadmap.md](../../60-roadmap.md): section 9, R-REL-006, 007, 020, 023, 033, 046.
- [decisions.md](../../decisions.md): D-05 (no YAML), D-26 (no background processes), D-27 (blake3, git2, MSRV 1.88), D-33 (control-file header), D-50 (blake3 everywhere, no sha256, no YAML).

**Global Constraints:**

- Edition `2024`, MSRV `1.88` (`rust-version = "1.88"`, R-ARCH-121), resolver `3`. The MSRV job runs `1.88.0` on Linux, Windows and macOS (R-ARCH-113).
- Lints. `unsafe_code`: `forbid` in `fairlead-syntax` and `fairlead-core` (crate attribute `#![forbid(unsafe_code)]`), `deny` in `fairlead` (workspace lint). Denied outside test code, through `[workspace.lints.clippy]`: `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `print_stdout`, `print_stderr`. `todo` stays allowed until P9 (Contract issues I-3 and I-4).
- CI commands, verbatim from contract section 1: `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo fmt --all --check`. Every cargo command in CI and in this plan carries `--locked`.
- Forbidden everywhere (`deny.toml` and `cargo xtask check-deps`): any YAML crate (`serde_yaml`, `serde_yml`, `serde_yaml_ng`), `bincode`, `sled`, `jwalk`, `serde-sarif`, and every HTTP/TLS client (`reqwest`, `ureq`, `hyper`, `openssl`, `rustls`, `native-tls`). This plan adds `openssl-sys`, `curl`, `curl-sys`, `libssh2-sys`, `notify` and `daemonize`; see Contract issues I-7.
- No network stack in the 0.1 binary (R-ARCH-123, R-CLI-009). No YAML crate and no YAML parsing anywhere (D-05, D-50(3)).
- `Cargo.lock` is committed (R-ARCH-122). Releases are built with `--locked` (R-REL-046).
- Line endings are LF. `.gitattributes`: `* text=auto eol=lf`, `fixtures/** -text`, `tests/golden/**/crlf/** -text`. No file this plan creates contains CRLF (R-ARCH-052; contract section 8, invariant 2).
- Hashes are BLAKE3 everywhere (D-27, D-50(2)). Release checksums are BLAKE3 too (Contract issues I-8).
- Stub discipline. Each module of contract section 3 exists as a file whose content is one `//!` line naming the owning phase and its rules. No stub panics, and none silently passes: the bin and the unimplemented xtask subcommands print to stderr and exit 2 (R-CLI-003 usage class).
- Rust formatting. After each Rust step run `cargo fmt --all` and keep rustfmt's layout. The code in this plan is complete and authoritative in content; rustfmt decides the line breaks.
- No code in P1 reads the clock, reads an environment variable, or calls `std::process::exit` (contract section 1 `disallowed-methods`; R-ARCH-090, R-ARCH-100).
- Build tools (`cargo`, `rustc`, `cargo-deny`, `cargo-audit`, `actionlint`) are run only by the executing agent in the verification steps below. Bash is used for git and for the file-creation script in Task 3.

**Repository root and paths.** Every path is relative to ROOT = `C:\Users\andym\Documente\Proiecte\Developer\harness` (Git Bash form `/c/Users/andym/Documente/Proiecte/Developer/harness`). The state was checked on 2026-10-09: no `.git`, no Cargo files, no `crates/`, `xtask/`, `fairlead/`, `tests/` or `.github/`; `docs/` holds the specification and `plans/0.1/`; `.remember/` is plugin state at the root and must never be committed.

**File map** (everything this plan creates; `docs/` already exists):

```text
ROOT/
  Cargo.toml  Cargo.lock  rust-toolchain.toml  rustfmt.toml  clippy.toml  deny.toml
  .cargo/config.toml  .editorconfig  .gitattributes  .gitignore  LICENSE  README.md
  .github/pull_request_template.md
  .github/workflows/{ci,msrv,audit,fuzz,bench,dogfood,release}.yml
  fairlead/control.toml
  crates/fairlead-syntax/Cargo.toml   src/{lib.rs, 9 stub modules}
  crates/fairlead-core/Cargo.toml     src/{lib.rs, 24 top-level modules, 10 directory modules, stubs}
  crates/fairlead/Cargo.toml          src/{main.rs, cli.rs, exit.rs, output.rs, commands/{mod.rs, 8 stubs}}
  tests/{golden,determinism,readonly,sarif,secrets,replay,platform,cli}.rs   (stubs)
  xtask/Cargo.toml   src/{main.rs, check_deps.rs}
```

---

### Task 1: Git repository, attribute files and the specification commit

**Files:**
- Create: `ROOT/.gitattributes`, `ROOT/.gitignore`
- Commit: `ROOT/docs/` (existing specification, including `docs/plans/0.1/`)
- Test: git state checks only (no code in this task)

**Interfaces:**
- Consumes: the existing `docs/` tree.
- Produces: a repository on branch `main`. Its first commit holds `docs/` and the two attribute files, so line endings are normalised from the start. Every later task commits an explicit file list.

- [ ] **Step 1: Confirm that no repository exists (red)**

Run from ROOT: `git rev-parse --show-toplevel`
Expected: FAIL with `fatal: not a git repository` (exit 128). If it prints a path other than ROOT, stop and report it: a nested repository must not be created inside another one.

- [ ] **Step 2: Initialise the repository (green for the init)**

Run from ROOT: `git init -b main`, then `git rev-parse --show-toplevel`.
Expected: PASS, the second command prints ROOT.

- [ ] **Step 3: Write the attribute and ignore files**

Create `ROOT/.gitattributes`:

```gitattributes
* text=auto eol=lf
fixtures/** -text
tests/golden/**/crlf/** -text
```

Create `ROOT/.gitignore`:

```gitignore
# Cargo build output and the disposable fairlead cache (contract section 1, R-ARCH-077).
target/
.fairlead/
# Plugin state written at the repository root; never part of the product (Contract issues I-6).
.remember/
```

- [ ] **Step 4: Verify the ignore and attribute rules (green)**

Run: `git check-ignore -v .remember/tmp/last-save.json target/debug/probe`
Expected: PASS. Both paths are reported as matched by `.gitignore`.

Run: `git check-attr eol text -- docs/README.md`
Expected: PASS. Output shows `eol: lf` and `text: auto`.

- [ ] **Step 5: Commit the specification and the attribute files**

```bash
git add .gitattributes .gitignore docs
git commit -m "docs: import the 0.1 specification and implementation contract" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

Expected: the commit succeeds, and `git status --short` prints nothing.

---

### Task 2: Workspace manifest, toolchain and lint configuration, package manifests

**Files:**
- Create: `ROOT/Cargo.toml`, `ROOT/rust-toolchain.toml`, `ROOT/rustfmt.toml`, `ROOT/clippy.toml`, `ROOT/.cargo/config.toml`
- Create: `ROOT/crates/fairlead-syntax/Cargo.toml`, `ROOT/crates/fairlead-syntax/src/lib.rs`
- Create: `ROOT/crates/fairlead-core/Cargo.toml`, `ROOT/crates/fairlead-core/src/lib.rs`
- Create: `ROOT/crates/fairlead/Cargo.toml`, `ROOT/crates/fairlead/src/main.rs`
- Create: `ROOT/xtask/Cargo.toml`, `ROOT/xtask/src/main.rs` (minimal; Task 5 replaces `main.rs`)
- Create by cargo: `ROOT/Cargo.lock`

**Interfaces:**
- Consumes: the Task 1 repository.
- Produces: workspace members `fairlead-syntax` (library `fairlead_syntax`), `fairlead-core` (library `fairlead_core`, feature `vendored-libgit2` on by default), `fairlead` (binary `fairlead`) and `xtask` (binary). `[workspace.dependencies]` holds the path entries `fairlead-syntax` and `fairlead-core` and the versions of contract section 2. Constants `fairlead_syntax::SYNTAX_VERSION: u32` and `fairlead_syntax::GRAMMAR_EDITION: u32`.

- [ ] **Step 1: Red check**

Run from ROOT: `cargo metadata --no-deps --format-version 1`
Expected: FAIL with `could not find Cargo.toml` (no workspace manifest exists yet).

- [ ] **Step 2: Create the workspace manifest**

Create `ROOT/Cargo.toml`:

```toml
# Cargo workspace of fairlead (contract section 1). A virtual manifest: no package at the root.
[workspace]
members = ["crates/*", "xtask"]
exclude = ["fuzz", "tests/golden", "fixtures"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.88"          # MSRV floor, R-ARCH-121
license-file = "LICENSE"       # placeholder until the owner selects a licence (Contract issues I-1)
publish = false                # no crates.io publish before the owner decides (R-REL-021, R-REL-042)

# Inherited by every member through `[lints] workspace = true` (contract section 1).
[workspace.lints.rust]
unsafe_code = "deny"           # fairlead-syntax and fairlead-core upgrade this to forbid in lib.rs

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
indexing_slicing = "deny"
print_stdout = "deny"          # the fairlead binary allows this, with a reason, in main.rs (Contract issues I-3)
print_stderr = "deny"
todo = "allow"                 # P9 flips this to "deny" once every phase has landed (Contract issues I-4)

[workspace.dependencies]
# Internal path dependencies
fairlead-syntax = { path = "crates/fairlead-syntax" }
fairlead-core = { path = "crates/fairlead-core" }
# fairlead-syntax (contract section 2). Feature names marked UNVERIFIED are checked at first use by P2.
winnow = "1.0.4"
toml = { version = "1.1.7", default-features = false, features = ["std", "parse", "serde"] }
serde = { version = "1.0.229", features = ["derive"] }
# fairlead-core (contract section 2)
ignore = "0.4.33"
globset = "0.4.20"
blake3 = "1.8.7"
rayon = "1.12.0"
git2 = { version = "0.21.0", default-features = false, features = ["vendored-libgit2"] }
toml_edit = "0.25.16"
serde_json = "1.0.151"
jiff = { version = "0.2.38", default-features = false, features = ["std"] }
similar = "3.2.0"
camino = "1.2.6"
dunce = "1.0.5"
tempfile = "3.27.0"
semver = "1.0.28"
# fairlead (contract section 2)
clap = { version = "4.6.7", features = ["derive", "std", "help", "usage", "error-context"] }
# dev dependencies, used by the P9 test targets
insta = { version = "1.49.0", features = ["json", "glob"] }
trycmd = "1.2.1"
# UNVERIFIED crates (unicode-normalization, memchr, regex, thiserror, proptest, criterion,
# libfuzzer-sys, cargo-fuzz, a JSON-schema validator, sysinfo) are NOT declared here. Each is added,
# with its verified Section 11 row, by the phase that needs it (R-ARCH-120; Contract issues I-10).
```

- [ ] **Step 3: Create the toolchain and tool configuration**

Create `ROOT/rust-toolchain.toml`:

```toml
# Development toolchain. CI tests the MSRV separately on 1.88.0 (msrv.yml, R-ARCH-113).
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

Create `ROOT/rustfmt.toml`:

```toml
edition = "2024"
max_width = 100
newline_style = "Unix"
use_field_init_shorthand = true
```

Create `ROOT/clippy.toml`:

```toml
# Clippy configuration (contract section 1). Applies workspace-wide (Contract issues I-2).
msrv = "1.88"

# Test code may unwrap, expect, index, print and use vec!; library code stays denied.
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
allow-print-in-tests = true
allow-dbg-in-tests = true
allow-useless-vec-in-tests = true

# R-ARCH-060: output order comes from ordered collections only.
disallowed-types = [
    { path = "std::collections::HashMap", reason = "R-ARCH-060: output order must not depend on hash iteration; use BTreeMap" },
    { path = "std::collections::HashSet", reason = "R-ARCH-060: output order must not depend on hash iteration; use BTreeSet" },
]

# Exits, environment and clock are confined to designated modules, which carry a local #[allow] with a reason.
disallowed-methods = [
    { path = "std::process::exit", reason = "R-ARCH-100: library code returns typed errors; only the fairlead binary maps exit codes" },
    { path = "std::env::var", reason = "contract section 1: the environment is read only in discover.rs and the fairlead binary" },
    { path = "std::env::var_os", reason = "contract section 1: the environment is read only in discover.rs and the fairlead binary" },
    { path = "std::env::vars", reason = "contract section 1: the environment is read only in discover.rs and the fairlead binary" },
    { path = "std::env::vars_os", reason = "contract section 1: the environment is read only in discover.rs and the fairlead binary" },
    { path = "std::time::SystemTime::now", reason = "R-ARCH-090: the clock is read only in time.rs and check/options.rs" },
    { path = "std::time::Instant::now", reason = "R-ARCH-090: the clock is read only in time.rs and check/options.rs" },
]
```

Create `ROOT/.cargo/config.toml`:

```toml
# `cargo xtask <task>` runs the tooling crate (contract section 1).
[alias]
xtask = "run -p xtask --"
```

- [ ] **Step 4: Create the four package manifests**

Create `ROOT/crates/fairlead-syntax/Cargo.toml`:

```toml
[package]
name = "fairlead-syntax"
description = "Pure directive grammar and extraction for fairlead: no I/O, no git, no threads."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[lints]
workspace = true
```

Create `ROOT/crates/fairlead-core/Cargo.toml`:

```toml
[package]
name = "fairlead-core"
description = "The fairlead engine: discovery, walk, extraction, registry, checks, context and writes."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[features]
# git2 is built from vendored sources by default (R-GIT-077). The git2 dependency arrives with P3.
default = ["vendored-libgit2"]
vendored-libgit2 = []

[lints]
workspace = true

[dependencies]
fairlead-syntax.workspace = true
```

Create `ROOT/crates/fairlead/Cargo.toml`:

```toml
[package]
name = "fairlead"
description = "Command-line interface of fairlead: declare, check and brief."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[[bin]]
name = "fairlead"
path = "src/main.rs"

# End-to-end test targets live in the repository root tests/ (contract section 7). The files are
# stubs from Task 3; P9 fills them. Explicit paths make CARGO_BIN_EXE_fairlead available to them.
[[test]]
name = "golden"
path = "../../tests/golden.rs"

[[test]]
name = "determinism"
path = "../../tests/determinism.rs"

[[test]]
name = "readonly"
path = "../../tests/readonly.rs"

[[test]]
name = "sarif"
path = "../../tests/sarif.rs"

[[test]]
name = "secrets"
path = "../../tests/secrets.rs"

[[test]]
name = "replay"
path = "../../tests/replay.rs"

[[test]]
name = "platform"
path = "../../tests/platform.rs"

[[test]]
name = "cli"
path = "../../tests/cli.rs"

[lints]
workspace = true

[dependencies]
fairlead-core.workspace = true
```

Create `ROOT/xtask/Cargo.toml`:

```toml
[package]
name = "xtask"
description = "Developer tasks for fairlead: the forbidden-crate scan and the subcommands P9 fills."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[lints]
workspace = true
```

- [ ] **Step 5: Create the minimal sources**

Create `ROOT/crates/fairlead-syntax/src/lib.rs`:

```rust
//! `fairlead-syntax`: the pure directive grammar of fairlead (contract sections 3 and 4.1 to 4.2).
//!
//! No I/O, no git, no threads, and no dependency on `fairlead-core` (R-ARCH-002).
#![forbid(unsafe_code)]

/// Version of the syntax implementation (contract section 3, `lib`). Owned by P2 (Contract issues I-5).
pub const SYNTAX_VERSION: u32 = 1;

/// Grammar edition of this build (D-17, D-45): an integer, 1 in 0.1.
pub const GRAMMAR_EDITION: u32 = 1;
```

Create `ROOT/crates/fairlead-core/src/lib.rs`:

```rust
//! `fairlead-core`: the engine behind the `fairlead` binary (contract sections 3 and 6).
//!
//! Library code returns typed errors, never exits the process, and never prints (R-ARCH-100).
#![forbid(unsafe_code)]
```

Create `ROOT/crates/fairlead/src/main.rs`:

```rust
//! The `fairlead` binary. The 0.1 verb set arrives with P8; this bootstrap build compiles and says so.
#![allow(clippy::print_stdout, clippy::print_stderr)] // reason: only the fairlead binary prints (contract section 1)

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("fairlead: bootstrap build, no verb is implemented yet (the verb set arrives with P8)");
    ExitCode::from(2)
}
```

Create `ROOT/xtask/src/main.rs`:

```rust
//! Developer tasks for the fairlead workspace (contract section 1). Tooling, never shipped.
#![forbid(unsafe_code)]
#![allow(clippy::print_stdout, clippy::print_stderr)] // reason: a developer command line prints

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("xtask: no task is implemented in this build; check-deps arrives in Task 5");
    ExitCode::from(2)
}
```

- [ ] **Step 6: Green checks**

Run from ROOT: `cargo check --workspace`
Expected: PASS. Cargo writes `Cargo.lock` with four workspace packages and no external crates.

Run: `cargo check --workspace --locked`
Expected: PASS (the lock file is up to date).

Run: `cargo metadata --no-deps --locked --format-version 1`
Expected: PASS. Exactly four packages: `fairlead-syntax`, `fairlead-core`, `fairlead`, `xtask` (R-ARCH-007). Each reports `rust_version` 1.88.

Run: `cargo tree -p fairlead-syntax --edges normal`
Expected: PASS. Only `fairlead-syntax` is printed, with no `fairlead-core` edge (R-ARCH-002).

Run: `cargo fmt --all --check`
Expected: PASS. If it fails, run `cargo fmt --all` and repeat.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml rustfmt.toml clippy.toml .cargo/config.toml crates xtask
git commit -m "build: cargo workspace with pinned toolchain and lint configuration" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Module tree and end-to-end test targets as documented stubs

**Files:**
- Replace: `ROOT/crates/fairlead-syntax/src/lib.rs`, `ROOT/crates/fairlead-core/src/lib.rs`, `ROOT/crates/fairlead/src/main.rs`
- Create: the stub files listed in the script of Step 3 (every module of contract section 3 except the ones in Step 4)
- Create: `ROOT/crates/fairlead-core/src/{extract,config,cache,registry,check,generate,render,context,write}/mod.rs`, `ROOT/crates/fairlead-core/src/check/builtin/mod.rs`, `ROOT/crates/fairlead-core/src/explain.rs`, `ROOT/crates/fairlead/src/commands/mod.rs`
- Create: `ROOT/tests/{golden,determinism,readonly,sarif,secrets,replay,platform,cli}.rs`

**Interfaces:**
- Consumes: the manifests of Task 2 and the `[[test]]` paths declared in `crates/fairlead/Cargo.toml`.
- Produces: every module path of contract section 3 as a compiling module. Later phases fill files; they never add a module path that this task did not create. The bin declares `commands::{init, check, sync, new, context, explain, approve, reserved}`; the `pub fn run` in `commands/check.rs`, added by P8, is the gate used by `dogfood.yml` in Task 10.

- [ ] **Step 1: Red check**

Run from ROOT: `cargo check --workspace --all-targets --locked`
Expected: FAIL. Cargo reports that the file of the `golden` test target does not exist, because `tests/` is still empty.

- [ ] **Step 2: Create the test-target stubs**

The stub script of Step 3 creates them. Each file holds one `//!` line; no test exists before P9.

- [ ] **Step 3: Create the stub files with one script**

Run this script in Git Bash. It creates each file only if it does not exist, so re-running is safe. Each file holds one `//!` line that names the owning phase.

```bash
cd /c/Users/andym/Documente/Proiecte/Developer/harness
stub() {
  mkdir -p "$(dirname "$1")"
  [ -e "$1" ] || printf '%s\n' "$2" > "$1"
}
stub crates/fairlead-syntax/src/ids.rs '//! P2: ScopeSlug, Kind, Name, Id, slug and reserved-name validation (R-SYN-024..027).'
stub crates/fairlead-syntax/src/normalize.rs '//! P2: BOM strip, CRLF and CR to LF with an offset map, NFC, tab handling (R-SYN-007, R-ARCH-033).'
stub crates/fairlead-syntax/src/delimiters.rs '//! P2: per-extension comment-leader table CommentTable (R-SYN-006).'
stub crates/fairlead-syntax/src/lexer.rs '//! P2: comment-token spans honouring string and raw-string literals, marker prefilter (R-FILE-013, R-ARCH-030).'
stub crates/fairlead-syntax/src/directive.rs '//! P2: winnow grammar, Directive, Arg, known-key tables per verb (R-SYN-001..015, 028, 029, 048).'
stub crates/fairlead-syntax/src/frontmatter.rs '//! P2: the +++ splitter and the directives array (R-SYN-036, R-SYN-037).'
stub crates/fairlead-syntax/src/attach.rs '//! P2: file, trailing, standalone and explicit-range attachment, regions (R-SYN-016..023, 045, 046).'
stub crates/fairlead-syntax/src/diag.rs '//! P2: DiagCode, Severity, Span, SyntaxDiag and the HN-S and HN-I constants (contract section 4.1).'
stub crates/fairlead-syntax/src/extract.rs '//! P2: the single entry point extract(), tying the syntax modules together (R-ARCH-030..033).'
stub crates/fairlead-core/src/error.rs '//! P3: Error, ErrorClass and the sub-errors; typed, never process::exit (R-ARCH-100).'
stub crates/fairlead-core/src/outcome.rs '//! P3: ExitCode and VerbOutcome; exit codes are defined only in 15-cli R-CLI-003.'
stub crates/fairlead-core/src/diag.rs '//! P3: Diagnostic, Location, and the DiagCode constants of every HN code.'
stub crates/fairlead-core/src/time.rs '//! P3: AsOf, the only clock access (R-CHK-014, R-ARCH-090).'
stub crates/fairlead-core/src/hash.rs '//! P3: ContentHash, Digest, Fingerprint, base32 and normalised hashing (C-7, D-50).'
stub crates/fairlead-core/src/path.rs '//! P3: RepoPath, the case-fold key and reserved names (R-SCOPE-007).'
stub crates/fairlead-core/src/discover.rs '//! P3: root lookup and pin check; the environment is read only here (R-ARCH-010..013, R-CLI-004, R-CLI-005).'
stub crates/fairlead-core/src/walk.rs '//! P3: sorted parallel listing with ignore rules (R-ARCH-020..026).'
stub crates/fairlead-core/src/classify.rs '//! P3: FileClass from location and declaration (R-FILE-001..014).'
stub crates/fairlead-core/src/manifest.rs '//! P3: read-only Cargo TOML hints; a failure yields empty hints (R-ARCH-006).'
stub crates/fairlead-core/src/git.rs '//! P3: the only module that touches git, read-only through git2 (R-ARCH-005, C-3).'
stub crates/fairlead-core/src/secrets.rs '//! P5: floor secret scan and redaction (R-AGT-022..024).'
stub crates/fairlead-core/src/pipeline.rs '//! P8: sequences the stages; the only module that chains several of them (R-ARCH-001).'
stub crates/fairlead-core/src/facts.rs '//! P3: Facts, collected once per run so that checks stay pure.'
stub crates/fairlead-core/src/extract/styles.rs '//! P3: CSS token sites and CssTokenSite (R-FILE-034).'
stub crates/fairlead-core/src/extract/literals.rs '//! P5 implements scan_literals; P3 calls it from extract_file (R-CHK-019).'
stub crates/fairlead-core/src/config/header.rs '//! P3: the frozen [fairlead] header, parsed in a lenient first pass (D-33, R-SYN-038).'
stub crates/fairlead-core/src/config/schema.rs '//! P3: ControlFile, the typed schema of a whole-file parse (R-SYN-038).'
stub crates/fairlead-core/src/config/rules.rs '//! P3: the [rules.*], [policies], [[allow]] and [[req]] tables (R-CHK-005).'
stub crates/fairlead-core/src/config/approvals.rs '//! P3: parsing of fairlead/approvals.toml (D-34, R-CLI-037).'
stub crates/fairlead-core/src/cache/files.rs '//! P3: file rows of the disposable cache: size, mtime and hash (R-ARCH-071..074).'
stub crates/fairlead-core/src/cache/key.rs '//! P3: CacheKey, the tuple a cache entry is valid for (Q-TOOL-1, R-ARCH-077).'
stub crates/fairlead-core/src/registry/scope.rs '//! P4: Scope, ScopeId and the scope tree (R-SCOPE-001..003).'
stub crates/fairlead-core/src/registry/item.rs '//! P4: Item, ItemId, Origin, Site and Edge (R-SCOPE-060).'
stub crates/fairlead-core/src/registry/ovr.rs '//! P4: Override, OvrOp and OvrState, override resolution (R-SCOPE-040..047).'
stub crates/fairlead-core/src/registry/grant.rs '//! P4: Allow grants and Req requests (R-SCOPE-036).'
stub crates/fairlead-core/src/registry/view.rs '//! P4: EffectiveView and provenance (R-ARCH-042).'
stub crates/fairlead-core/src/registry/ledger.rs '//! P4: LedgerEntry and LedgerView, computed on read (R-SCOPE-043, R-SCOPE-044).'
stub crates/fairlead-core/src/registry/resolve.rs '//! P4: pure resolution into an immutable Registry (R-ARCH-040, R-ARCH-061).'
stub crates/fairlead-core/src/check/codes.rs '//! P5: the DiagnosticSpec table, one row per code (R-CHK-042, R-ARCH-055).'
stub crates/fairlead-core/src/check/options.rs '//! P5: CheckOptions and Ramp; the second clock exception (R-CHK-001, R-CLI-021..023).'
stub crates/fairlead-core/src/check/finding.rs '//! P5: Finding and its total order (R-CHK-035, R-ARCH-051).'
stub crates/fairlead-core/src/check/fingerprint.rs '//! P5: fingerprint over whitespace-collapsed text (R-CHK-031, R-ARCH-053).'
stub crates/fairlead-core/src/check/floor.rs '//! P5: is_floor, compiled in and not configurable (R-AGT-009, R-CHK-008).'
stub crates/fairlead-core/src/check/rules.rs '//! P5: DeclaredRule, RuleCheck and RuleSet (R-CHK-005..007).'
stub crates/fairlead-core/src/check/baseline.rs '//! P5: Baseline parse, render, prune and accept_new (R-CHK-030..032, R-CHK-052).'
stub crates/fairlead-core/src/check/summary.rs '//! P5: Summary counts and waiver reporting (R-CHK-028, R-CHK-029).'
for family in agents controls exceptions generated leak notices paths refs scopes syntax tokens vendor; do
  stub "crates/fairlead-core/src/check/builtin/$family.rs" "//! P5: built-in check, family $family: one implementation of the Check trait (contract section 4.9)."
done
stub crates/fairlead-core/src/generate/shard.rs '//! P5: sharded generated registry files above 500 lines (C-8, R-FILE-020).'
stub crates/fairlead-core/src/generate/members.rs '//! P5: the gen/members shard renderer (C-8, C-35).'
stub crates/fairlead-core/src/generate/header.rs '//! P5: generated-file header, verify_header and hand-edit detection (C-17, R-CHK-024).'
stub crates/fairlead-core/src/render/human.rs '//! P5: the human output format (R-CHK-037, R-CHK-038).'
stub crates/fairlead-core/src/render/terse.rs '//! P5: the terse ID k=v output format (D-23, R-CTX-030).'
stub crates/fairlead-core/src/render/json.rs '//! P5: JSON output with the top-level keys of C-38 (R-CHK-036).'
stub crates/fairlead-core/src/render/sanitize.rs '//! P5: sanitize_data, stripping control, ANSI and bidi characters (R-CHK-037, R-AGT-017).'
stub crates/fairlead-core/src/render/sarif.rs '//! P5: hand-built SARIF 2.1.0 structs for the subset of R-CHK-036 (R-CHK-036).'
stub crates/fairlead-core/src/render/line.rs '//! P5: TerseLine, the shared ID k=v writer for generate and context (R-CTX-030, R-CTX-031).'
stub crates/fairlead-core/src/context/request.rs '//! P6: ContextRequest and ContextTarget (R-CTX-001..004).'
stub crates/fairlead-core/src/context/tiers.rs '//! P6: tiers L0 to L3 (R-CTX-010..013).'
stub crates/fairlead-core/src/context/priority.rs '//! P6: the priority order P0 to P7 (R-CTX-040, table in 14).'
stub crates/fairlead-core/src/context/budget.rs '//! P6: Budget and priority packing (R-CTX-040..046).'
stub crates/fairlead-core/src/context/estimate.rs '//! P6: estimate_bytes, always labelled as an estimate (R-CTX-043).'
stub crates/fairlead-core/src/context/digest.rs '//! P6: the effective_digest Merkle chain (R-CTX-050, C-7).'
stub crates/fairlead-core/src/context/brief.rs '//! P6: ContextBrief, the one structure both renderings use (C-15, R-CTX-020..022).'
stub crates/fairlead-core/src/write/writeset.rs '//! P7: WriteSet, the path allow-list per verb (R-ARCH-140, R-ARCH-141).'
stub crates/fairlead-core/src/write/lock.rs '//! P7: WriterLock, the pid lock file .fairlead/lock (R-ARCH-146, R-CLI-008).'
stub crates/fairlead-core/src/write/journal.rs '//! P7: the sync journal used by recovery (R-ARCH-145, R-CLI-007).'
stub crates/fairlead-core/src/write/tx.rs '//! P7: Transaction, staged temporary files and atomic renames (R-ARCH-143, R-ARCH-145).'
stub crates/fairlead-core/src/write/init.rs '//! P7: init_plan, a pure planner (R-CLI-013..016).'
stub crates/fairlead-core/src/write/new.rs '//! P7: new_plan, a pure planner that never overwrites (R-CLI-028).'
stub crates/fairlead-core/src/write/sync.rs '//! P7: sync_plan, a pure planner (R-CLI-046, C-8).'
stub crates/fairlead-core/src/write/approve.rs '//! P7: approve_preview and approve_plan (R-CLI-037, C-20).'
stub crates/fairlead-core/src/explain/topics.rs '//! P7: the embedded explain topics (R-CLI-035, C-21).'
stub crates/fairlead/src/cli.rs '//! P8: clap derive definitions for the fixed verb set (R-CLI-001, R-CLI-006).'
stub crates/fairlead/src/exit.rs '//! P8: maps ExitCode and Error to the process exit code (R-CLI-003).'
stub crates/fairlead/src/output.rs '//! P8: writes CommandResult; TTY and NO_COLOR detection; LF only (R-CLI-006).'
stub crates/fairlead/src/commands/init.rs '//! P8: run for init, a thin call into fairlead-core (R-CLI-013).'
stub crates/fairlead/src/commands/check.rs '//! P8: run for check, a thin call into fairlead-core (R-CHK-040). Its pub fn run, added by P8, gates dogfood.yml.'
stub crates/fairlead/src/commands/sync.rs '//! P8: run for sync, a thin call into fairlead-core (R-CLI-025).'
stub crates/fairlead/src/commands/new.rs '//! P8: run for new, a thin call into fairlead-core (R-CLI-028).'
stub crates/fairlead/src/commands/context.rs '//! P8: run for context, a thin call into fairlead-core (R-CTX-001).'
stub crates/fairlead/src/commands/explain.rs '//! P8: run for explain, a thin call into fairlead-core (R-CLI-035).'
stub crates/fairlead/src/commands/approve.rs '//! P8: run for approve, a thin call into fairlead-core (R-CLI-037).'
stub crates/fairlead/src/commands/reserved.rs '//! P8: reserved verbs exit 2 with "<verb> arrives in <milestone>" (R-CLI-001).'
stub tests/golden.rs '//! P9: end-to-end golden repositories (contract section 7). Empty until P9 fills it.'
stub tests/determinism.rs '//! P9: differential determinism runs (R-ARCH-071, R-ARCH-080, R-ARCH-111).'
stub tests/readonly.rs '//! P9: the read-only guarantee, git status clean after reader verbs (R-ARCH-140).'
stub tests/sarif.rs '//! P9: SARIF output validated against the 2.1.0 schema (R-CHK-036).'
stub tests/secrets.rs '//! P9: seeded secret-like values, all rejected (R-AGT-015, R-ARCH-144).'
stub tests/replay.rs '//! P9: history replay for recall and false positives (60-roadmap section 7.3).'
stub tests/platform.rs '//! P9: Windows paths over 260 characters, locked files, antivirus latency (R-ARCH-110, R-PORT-014).'
stub tests/cli.rs '//! P9: trycmd CLI contract cases from tests/cli/*.trycmd (50-architecture section 14).'
```

Then create the files that are not one-line stubs, with the content of Step 4.

- [ ] **Step 4: Create the explicit module files**

Replace `ROOT/crates/fairlead-syntax/src/lib.rs` with:

```rust
//! `fairlead-syntax`: the pure directive grammar of fairlead (contract sections 3 and 4.1 to 4.2).
//!
//! No I/O, no git, no threads, and no dependency on `fairlead-core` (R-ARCH-002).
#![forbid(unsafe_code)]

pub mod attach;
pub mod delimiters;
pub mod diag;
pub mod directive;
pub mod extract;
pub mod frontmatter;
pub mod ids;
pub mod lexer;
pub mod normalize;

/// Version of the syntax implementation (contract section 3, `lib`). Owned by P2 (Contract issues I-5).
pub const SYNTAX_VERSION: u32 = 1;

/// Grammar edition of this build (D-17, D-45): an integer, 1 in 0.1.
pub const GRAMMAR_EDITION: u32 = 1;
```

Replace `ROOT/crates/fairlead-core/src/lib.rs` with:

```rust
//! `fairlead-core`: the engine behind the `fairlead` binary (contract sections 3 and 6).
//!
//! Library code returns typed errors, never exits the process, and never prints (R-ARCH-100).
#![forbid(unsafe_code)]

pub mod cache;
pub mod check;
pub mod classify;
pub mod config;
pub mod context;
pub mod diag;
pub mod discover;
pub mod error;
pub mod explain;
pub mod extract;
pub mod facts;
pub mod generate;
pub mod git;
pub mod hash;
pub mod manifest;
pub mod outcome;
pub mod path;
pub mod pipeline;
pub mod registry;
pub mod render;
pub mod secrets;
pub mod time;
pub mod walk;
pub mod write;
```

Create `ROOT/crates/fairlead-core/src/extract/mod.rs`:

```rust
//! Per-file extraction records (contract section 4.5). P3 owns this tree; P5 owns `literals`.

pub mod literals;
pub mod styles;
```

Create `ROOT/crates/fairlead-core/src/config/mod.rs`:

```rust
//! Control files (contract section 4.6). P3 owns every submodule.

pub mod approvals;
pub mod header;
pub mod rules;
pub mod schema;
```

Create `ROOT/crates/fairlead-core/src/cache/mod.rs`:

```rust
//! The disposable cache under `.fairlead/` (contract section 4.7). P3 owns every submodule.

pub mod files;
pub mod key;
```

Create `ROOT/crates/fairlead-core/src/registry/mod.rs`:

```rust
//! Scopes, items, overrides, grants, views and the ledger (contract section 4.8). P4 owns this tree.

pub mod grant;
pub mod item;
pub mod ledger;
pub mod ovr;
pub mod resolve;
pub mod scope;
pub mod view;
```

Create `ROOT/crates/fairlead-core/src/check/mod.rs`:

```rust
//! Built-in checks, declared rules, baseline, fingerprints and the report (contract section 4.9). P5 owns this tree.

pub mod baseline;
pub mod builtin;
pub mod codes;
pub mod finding;
pub mod fingerprint;
pub mod floor;
pub mod options;
pub mod rules;
pub mod summary;
```

Create `ROOT/crates/fairlead-core/src/check/builtin/mod.rs`:

```rust
//! One built-in check per family (contract section 3). P5 owns this tree.

pub mod agents;
pub mod controls;
pub mod exceptions;
pub mod generated;
pub mod leak;
pub mod notices;
pub mod paths;
pub mod refs;
pub mod scopes;
pub mod syntax;
pub mod tokens;
pub mod vendor;
```

Create `ROOT/crates/fairlead-core/src/generate/mod.rs`:

```rust
//! Pure renderers of generated files (contract section 4.10). P5 owns this tree.

pub mod header;
pub mod members;
pub mod shard;
```

Create `ROOT/crates/fairlead-core/src/render/mod.rs`:

```rust
//! Output formats (contract section 4.10). P5 owns this tree.

pub mod human;
pub mod json;
pub mod line;
pub mod sanitize;
pub mod sarif;
pub mod terse;
```

Create `ROOT/crates/fairlead-core/src/context/mod.rs`:

```rust
//! Context briefs, tiers, priority packing and digests (contract section 4.11). P6 owns this tree.

pub mod brief;
pub mod budget;
pub mod digest;
pub mod estimate;
pub mod priority;
pub mod request;
pub mod tiers;
```

Create `ROOT/crates/fairlead-core/src/write/mod.rs`:

```rust
//! The write layer and the verb planners (contract section 4.12). P7 owns this tree.

pub mod approve;
pub mod init;
pub mod journal;
pub mod lock;
pub mod new;
pub mod sync;
pub mod tx;
pub mod writeset;
```

Create `ROOT/crates/fairlead-core/src/explain.rs`:

```rust
//! The explain verb (contract section 4.12, R-CLI-035). P7 owns this module and `topics`.

pub mod topics;
```

Replace `ROOT/crates/fairlead/src/main.rs` with:

```rust
//! The `fairlead` binary. The 0.1 verb set arrives with P8; this bootstrap build compiles and says so.
#![allow(clippy::print_stdout, clippy::print_stderr)] // reason: only the fairlead binary prints (contract section 1)

mod cli;
mod commands;
mod exit;
mod output;

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("fairlead: bootstrap build, no verb is implemented yet (the verb set arrives with P8)");
    ExitCode::from(2)
}
```

Create `ROOT/crates/fairlead/src/commands/mod.rs`:

```rust
//! One thin module per verb. Each calls into fairlead-core and holds no logic (contract section 4.13).

mod approve;
mod check;
mod context;
mod explain;
mod init;
mod new;
mod reserved;
mod sync;
```

- [ ] **Step 5: Green checks**

Run from ROOT: `cargo fmt --all` and then `cargo fmt --all --check`
Expected: PASS.

Run: `cargo check --workspace --all-targets --locked`
Expected: PASS, with no warnings.

Run: `cargo tree -p fairlead-core --edges normal --locked`
Expected: PASS. The output lists `fairlead-syntax` as the only workspace dependency of `fairlead-core`.

- [ ] **Step 6: Commit**

```bash
git add crates tests
git commit -m "build: module tree and end-to-end test targets as documented stubs" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Prove the lint configuration with probes

**Files:**
- Temporarily modify, then restore: `ROOT/crates/fairlead-syntax/src/lib.rs`, `ROOT/crates/fairlead-core/src/lib.rs`
- Commit: nothing (both files are restored to their Task 3 committed state)

**Interfaces:**
- Consumes: the `[workspace.lints]`, `clippy.toml` and crate attributes of Tasks 2 and 3.
- Produces: evidence that each denied lint and disallowed item fires in library code, and that the test exemption works. The configuration is not changed by this task.

- [ ] **Step 1: Red probe (the lints must fire)**

Append to `ROOT/crates/fairlead-syntax/src/lib.rs`:

```rust
// PROBE, reverted in Step 2.
pub type ProbeMap = std::collections::HashMap<u8, u8>;
```

Append to `ROOT/crates/fairlead-core/src/lib.rs`:

```rust
// PROBE, reverted in Step 2.
pub fn probe_unwrap() -> u8 {
    Some(1_u8).unwrap()
}

pub fn probe_clock() -> std::time::SystemTime {
    std::time::SystemTime::now()
}

pub fn probe_exit() {
    std::process::exit(0);
}

pub fn probe_print() {
    println!("probe");
}

#[cfg(test)]
mod probe_tests {
    #[test]
    fn unwrap_is_allowed_in_tests() {
        assert_eq!(Some(1_u8).unwrap(), 1);
    }
}
```

Run from ROOT: `cargo clippy --workspace --all-targets --locked -- -D warnings`
Expected: FAIL with these errors and no others: `disallowed_types` for `ProbeMap` (R-ARCH-060), `unwrap_used` for `probe_unwrap`, `disallowed_methods` for `SystemTime::now` (R-ARCH-090) and for `process::exit` (R-ARCH-100), and `print_stdout` for `probe_print`. The test `unwrap_is_allowed_in_tests` must produce no error.

- [ ] **Step 2: Revert the probes**

Run from ROOT: `git checkout -- crates/fairlead-syntax/src/lib.rs crates/fairlead-core/src/lib.rs`
Then run `git status --short`.
Expected: no output, so the working tree matches the Task 3 commit.

- [ ] **Step 3: Green check**

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings`
Expected: PASS, no warnings.

Nothing is committed by this task.

---

### Task 5: The `xtask` tool with the `check-deps` forbidden-crate scan

**Files:**
- Replace: `ROOT/xtask/src/main.rs`
- Create: `ROOT/xtask/src/check_deps.rs`
- Test: unit tests inside `check_deps.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: the root `Cargo.lock` (required) and `fuzz/Cargo.lock` (optional), read as text.
- Produces: `check_deps::FORBIDDEN: &[&str]`, `check_deps::LOCK_FILES: &[&str]`, `check_deps::Finding { lock: String, krate: String }`, `check_deps::package_names(&str) -> Vec<String>`, `check_deps::forbidden_in(&str) -> Vec<String>`, `check_deps::run(&Path) -> ExitCode`. The binary answers `cargo xtask check-deps` with 0 (clean), 1 (forbidden crate found) or 5 (a required lock cannot be read). The subcommands `sarif-validate`, `bench-corpus` and `golden-bless` exit 2 with a message until P9 (Contract issues I-11).

- [ ] **Step 1: Red check (the tests must fail first)**

Create `ROOT/xtask/src/check_deps.rs` with the constants, the test module and the signatures below. The three function bodies are `todo!()` for this step only.

```rust
//! `check-deps`: the forbidden-crate scan over the committed lock files (R-ARCH-123, R-AGT-007).
//!
//! The scan never runs cargo and never parses TOML. It reads `name = "..."` lines inside
//! `[[package]]` tables, so it works without a toolchain and cannot be fooled by dependency lists.

use std::io::ErrorKind;
use std::path::Path;
use std::process::ExitCode;

/// Crate names that must not appear in any scanned lock file (contract section 2, R-ARCH-123).
pub const FORBIDDEN: &[&str] = &[
    "serde_yaml",
    "serde_yml",
    "serde_yaml_ng",
    "bincode",
    "sled",
    "jwalk",
    "serde-sarif",
    "reqwest",
    "ureq",
    "hyper",
    "openssl",
    "rustls",
    "native-tls",
    "openssl-sys",
    "curl",
    "curl-sys",
    "libssh2-sys",
    "notify",
    "daemonize",
];

/// Lock files scanned, relative to the repository root. `fuzz/Cargo.lock` is optional: the fuzz
/// project is not a workspace member and may not have a committed lock.
pub const LOCK_FILES: &[&str] = &["Cargo.lock", "fuzz/Cargo.lock"];

/// One forbidden crate found in one lock file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub lock: String,
    pub krate: String,
}

/// Package names declared by `name = "..."` lines inside `[[package]]` tables, in file order.
pub fn package_names(lock_text: &str) -> Vec<String> {
    todo!()
}

/// Forbidden crate names present in one lock text, sorted and de-duplicated.
pub fn forbidden_in(lock_text: &str) -> Vec<String> {
    todo!()
}

/// Runs the scan under `root`.
pub fn run(root: &Path) -> ExitCode {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::{FORBIDDEN, forbidden_in, package_names};

    const REQWEST_LOCK: &str = "version = 4\n\n[[package]]\nname = \"reqwest\"\nversion = \"0.12.0\"\n";

    #[test]
    fn r_arch_123__detects_an_http_client_in_the_lock() {
        assert_eq!(forbidden_in(REQWEST_LOCK), vec!["reqwest".to_owned()]);
    }

    #[test]
    fn r_arch_123__dependency_lists_do_not_count() {
        let text = "[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\n \"reqwest\",\n]\n";
        assert!(forbidden_in(text).is_empty());
    }

    #[test]
    fn r_arch_123__contract_forbidden_names_are_all_listed() {
        for name in [
            "serde_yaml", "serde_yml", "serde_yaml_ng", "bincode", "sled", "jwalk", "serde-sarif",
            "reqwest", "ureq", "hyper", "openssl", "rustls", "native-tls",
        ] {
            assert!(FORBIDDEN.contains(&name), "{name} must be forbidden");
        }
    }

    #[test]
    fn r_arch_123__clean_lock_has_no_hits_and_names_are_read() {
        let text = "[[package]]\nname = \"serde\"\nversion = \"1.0.229\"\n";
        assert!(forbidden_in(text).is_empty());
        assert_eq!(package_names(text), vec!["serde".to_owned()]);
    }

    #[test]
    fn r_arch_123__each_hit_is_reported_once() {
        let text = "[[package]]\nname = \"hyper\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"hyper\"\nversion = \"0.14.0\"\n";
        assert_eq!(forbidden_in(text), vec!["hyper".to_owned()]);
    }
}
```

Create `ROOT/xtask/src/main.rs` with the final content of Step 3 (it already declares `mod check_deps;`).

Run from ROOT: `cargo test -p xtask --locked`
Expected: FAIL. Four tests panic with `not yet implemented`: `r_arch_123__detects_an_http_client_in_the_lock`, `r_arch_123__dependency_lists_do_not_count`, `r_arch_123__clean_lock_has_no_hits_and_names_are_read` and `r_arch_123__each_hit_is_reported_once`. The test `r_arch_123__contract_forbidden_names_are_all_listed` passes, because it reads only the constant.

- [ ] **Step 2: Implement the scan (green)**

Replace the three `todo!()` functions and keep everything else. The complete file is the following; the `use` line, the constants and the test module are unchanged from Step 1.

```rust
//! `check-deps`: the forbidden-crate scan over the committed lock files (R-ARCH-123, R-AGT-007).
//!
//! The scan never runs cargo and never parses TOML. It reads `name = "..."` lines inside
//! `[[package]]` tables, so it works without a toolchain and cannot be fooled by dependency lists.

use std::io::ErrorKind;
use std::path::Path;
use std::process::ExitCode;

/// Crate names that must not appear in any scanned lock file (contract section 2, R-ARCH-123).
pub const FORBIDDEN: &[&str] = &[
    "serde_yaml",
    "serde_yml",
    "serde_yaml_ng",
    "bincode",
    "sled",
    "jwalk",
    "serde-sarif",
    "reqwest",
    "ureq",
    "hyper",
    "openssl",
    "rustls",
    "native-tls",
    "openssl-sys",
    "curl",
    "curl-sys",
    "libssh2-sys",
    "notify",
    "daemonize",
];

/// Lock files scanned, relative to the repository root. `fuzz/Cargo.lock` is optional: the fuzz
/// project is not a workspace member and may not have a committed lock.
pub const LOCK_FILES: &[&str] = &["Cargo.lock", "fuzz/Cargo.lock"];

/// One forbidden crate found in one lock file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub lock: String,
    pub krate: String,
}

/// Package names declared by `name = "..."` lines inside `[[package]]` tables, in file order.
pub fn package_names(lock_text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_package = false;
    for raw in lock_text.lines() {
        let line = raw.trim();
        if line == "[[package]]" {
            in_package = true;
            continue;
        }
        if line.starts_with('[') {
            in_package = false;
            continue;
        }
        if !in_package {
            continue;
        }
        let name = line
            .strip_prefix("name = \"")
            .and_then(|rest| rest.strip_suffix('"'));
        if let Some(name) = name {
            names.push(name.to_owned());
        }
    }
    names
}

/// Forbidden crate names present in one lock text, sorted and de-duplicated.
pub fn forbidden_in(lock_text: &str) -> Vec<String> {
    let mut hits: Vec<String> = package_names(lock_text)
        .into_iter()
        .filter(|name| FORBIDDEN.contains(&name.as_str()))
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

/// Runs the scan under `root`. Exit 0 when clean, 1 when a forbidden crate is present, and 5 when a
/// required lock file cannot be read: the result cannot be determined (contract section 8, invariant 5).
pub fn run(root: &Path) -> ExitCode {
    let mut findings: Vec<Finding> = Vec::new();
    let mut scanned: usize = 0;
    for lock in LOCK_FILES {
        let path = root.join(lock);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                scanned += 1;
                for krate in forbidden_in(&text) {
                    findings.push(Finding {
                        lock: (*lock).to_owned(),
                        krate,
                    });
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound && *lock != "Cargo.lock" => {}
            Err(err) => {
                eprintln!("check-deps: cannot read {}: {err}", path.display());
                return ExitCode::from(5);
            }
        }
    }
    if findings.is_empty() {
        println!("check-deps: ok, {scanned} lock file(s) scanned, no forbidden crate");
        return ExitCode::SUCCESS;
    }
    findings.sort();
    for finding in &findings {
        eprintln!("check-deps: forbidden crate `{}` in {}", finding.krate, finding.lock);
    }
    ExitCode::from(1)
}

#[cfg(test)]
mod tests {
    use super::{FORBIDDEN, forbidden_in, package_names};

    const REQWEST_LOCK: &str = "version = 4\n\n[[package]]\nname = \"reqwest\"\nversion = \"0.12.0\"\n";

    #[test]
    fn r_arch_123__detects_an_http_client_in_the_lock() {
        assert_eq!(forbidden_in(REQWEST_LOCK), vec!["reqwest".to_owned()]);
    }

    #[test]
    fn r_arch_123__dependency_lists_do_not_count() {
        let text = "[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\n \"reqwest\",\n]\n";
        assert!(forbidden_in(text).is_empty());
    }

    #[test]
    fn r_arch_123__contract_forbidden_names_are_all_listed() {
        for name in [
            "serde_yaml", "serde_yml", "serde_yaml_ng", "bincode", "sled", "jwalk", "serde-sarif",
            "reqwest", "ureq", "hyper", "openssl", "rustls", "native-tls",
        ] {
            assert!(FORBIDDEN.contains(&name), "{name} must be forbidden");
        }
    }

    #[test]
    fn r_arch_123__clean_lock_has_no_hits_and_names_are_read() {
        let text = "[[package]]\nname = \"serde\"\nversion = \"1.0.229\"\n";
        assert!(forbidden_in(text).is_empty());
        assert_eq!(package_names(text), vec!["serde".to_owned()]);
    }

    #[test]
    fn r_arch_123__each_hit_is_reported_once() {
        let text = "[[package]]\nname = \"hyper\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"hyper\"\nversion = \"0.14.0\"\n";
        assert_eq!(forbidden_in(text), vec!["hyper".to_owned()]);
    }
}
```

Run from ROOT: `cargo test -p xtask --locked`
Expected: PASS, five tests.

- [ ] **Step 3: The binary, end to end**

Replace the file `ROOT/xtask/src/main.rs` with the final content below if Step 1 has not already created it.

```rust
//! Developer tasks for the fairlead workspace (contract section 1). Tooling, never shipped.
#![forbid(unsafe_code)]
// reason: a developer command line; printing its reports is its job (contract section 1)
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod check_deps;

use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <check-deps|sarif-validate|bench-corpus|golden-bless>";

/// The repository root: the parent of this crate's directory, independent of the working directory.
fn repo_root() -> Option<&'static Path> {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(task) = args.first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match task.as_str() {
        "check-deps" => match repo_root() {
            Some(root) => check_deps::run(root),
            None => {
                eprintln!("xtask: cannot locate the repository root");
                ExitCode::from(5)
            }
        },
        "sarif-validate" | "bench-corpus" | "golden-bless" => {
            eprintln!("xtask: `{task}` is not implemented yet; it arrives with P9 (contract section 6)");
            ExitCode::from(2)
        }
        other => {
            eprintln!("xtask: unknown task `{other}`");
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
```

Run from ROOT: `cargo xtask check-deps`
Expected: PASS. The output is `check-deps: ok, 1 lock file(s) scanned, no forbidden crate` and the exit code is 0.

Probe the failure path. Create `ROOT/fuzz/Cargo.lock` with this content:

```toml
version = 4

[[package]]
name = "ureq"
version = "3.0.0"
```

Run: `cargo xtask check-deps`
Expected: FAIL with exit code 1. Stderr contains ``forbidden crate `ureq` in fuzz/Cargo.lock``.

Then remove the probe directory: `Remove-Item -Recurse -Force fuzz` (PowerShell) and run `cargo xtask check-deps` again.
Expected: PASS, exit code 0.

Run: `cargo xtask bench-corpus`
Expected: FAIL with exit code 2 and the message that the task arrives with P9. This is the intended stub behaviour.

- [ ] **Step 4: Format and lint**

Run from ROOT: `cargo fmt --all` and then `cargo clippy -p xtask --all-targets --locked -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add xtask
git commit -m "feat(xtask): check-deps forbidden-crate scan over lock files as text" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Editor defaults, licence placeholder and repository readme

**Files:**
- Create: `ROOT/.editorconfig`, `ROOT/LICENSE`, `ROOT/README.md`

**Interfaces:**
- Consumes: the `.gitattributes` of Task 1 (line endings are enforced by git, not by the editor).
- Produces: the licence placeholder that `Cargo.toml` names (`license-file = "LICENSE"`), and the readme that points to the specification and states the stability promise of R-ARCH-003.

- [ ] **Step 1: Red check**

Run from ROOT: `Test-Path .editorconfig, LICENSE, README.md` (PowerShell)
Expected: FAIL. Each of the three prints `False`.

- [ ] **Step 2: Create the three files**

Create `ROOT/.editorconfig`:

```ini
# Editor defaults (contract section 1). Git enforces line endings through .gitattributes.
root = true

[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true
indent_style = space
indent_size = 4

[*.{toml,yml,yaml,json}]
indent_size = 2

[*.md]
trim_trailing_whitespace = false

# Fixtures and the CRLF golden trees keep their exact bytes (contract section 1, .gitattributes).
[fixtures/**]
end_of_line = unset
insert_final_newline = unset
trim_trailing_whitespace = unset

[tests/golden/**/crlf/**]
end_of_line = unset
insert_final_newline = unset
trim_trailing_whitespace = unset
```

Create `ROOT/LICENSE`:

```text
All rights reserved until the owner selects a licence.

This file is a placeholder. R-REL-007 (docs/60-roadmap.md) requires MIT OR Apache-2.0, with both
licence texts in the repository before the first publish. The owner makes that choice and replaces
this file. Until then every crate is publish = false (Contract issues I-1).
```

Create `ROOT/README.md`:

```markdown
# fairlead

fairlead keeps a codebase consistent for AI agents and human developers. Intent (rules, structure,
tokens, hierarchy) lives in the repository as documentation. Small comment directives tie code to
it, `fairlead check` reports drift, and `fairlead context` gives an agent a compact, budgeted brief.

`fairlead` is a placeholder name (D-30, D-47). The target name is applied before the first publish
(R-REL-022).

**Status: 0.1 bootstrap.** The workspace, the lints and the continuous integration exist. No verb is
implemented yet.

- Specification and decisions: [docs/README.md](docs/README.md). Start with
  [docs/00-vision.md](docs/00-vision.md) and [docs/decisions.md](docs/decisions.md).
- Implementation contract for 0.1: [docs/plans/0.1/01-contract.md](docs/plans/0.1/01-contract.md).
- Before 1.0 the library APIs of `fairlead-core` and `fairlead-syntax` are not a stability promise. The
  stability surface is the CLI, the directive grammar per edition and the output formats (R-ARCH-003).

Licence: not yet selected (see [LICENSE](LICENSE)). Nothing is published to crates.io.
```

- [ ] **Step 3: Green checks**

Run from ROOT: `Select-String -Path README.md -Pattern 'docs/README.md'`
Expected: PASS, one match.

Run: `Select-String -Path README.md -Pattern 'stability promise'`
Expected: PASS, one match (R-ARCH-003 is stated).

Run: `Select-String -Path .editorconfig -Pattern 'end_of_line = lf'`
Expected: PASS, one match.

- [ ] **Step 4: Commit**

```bash
git add .editorconfig LICENSE README.md
git commit -m "docs: editor defaults, licence placeholder and repository readme" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The cargo-deny policy (bans, licences, sources, advisories)

**Files:**
- Create: `ROOT/deny.toml`
- Probe, then restore: `ROOT/crates/fairlead-core/Cargo.toml` and `ROOT/Cargo.lock`

**Interfaces:**
- Consumes: the workspace from Task 2 (all members are `publish = false`, so the private-crate rule applies to them).
- Produces: a `deny.toml` that the CI job `lint` runs as `cargo deny check` (Task 8). It bans the crates of contract section 2 and the additions of Contract issues I-7, and it allows only the licences listed below.

- [ ] **Step 1: Gap probe (the policy is absent, so a banned crate passes)**

Add the line `reqwest = "0.12"` under `[dependencies]` in `ROOT/crates/fairlead-core/Cargo.toml`, below `fairlead-syntax.workspace = true`. This needs registry access, so run it online.

Run from ROOT: `cargo deny check bans`
Expected: PASS (exit 0). Without a policy, cargo-deny accepts `reqwest`. This is the gap that this task closes.

- [ ] **Step 2: Create the policy**

Create `ROOT/deny.toml`:

```toml
# cargo-deny policy (contract section 2; R-ARCH-122, R-ARCH-123, R-REL-033). Run: cargo deny check
# Verify: the keys below follow the cargo-deny 0.16 configuration format (`crate =` in bans entries,
# `version = 2` in advisories and licenses). The executing engineer confirms them against the pinned
# cargo-deny version before the first CI run.

[graph]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-apple-darwin",
]

[advisories]
version = 2
ignore = []

[licenses]
version = 2
allow = [
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "CC0-1.0",
    "ISC",
    "MIT",
    "MPL-2.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Zlib",
]

[licenses.private]
# Workspace crates are publish = false and carry the placeholder licence (Contract issues I-1).
ignore = true

[bans]
multiple-versions = "warn"
highlight = "all"
deny = [
    # YAML is not needed at all (D-05, D-50(3)).
    { crate = "serde_yaml", reason = "D-05: fairlead reads no YAML; the crate is archived" },
    { crate = "serde_yml", reason = "D-05: RUSTSEC-2025-0068" },
    { crate = "serde_yaml_ng", reason = "D-05: fairlead reads no YAML" },
    # Discontinued or stale formats and stores (D-27, contract section 2).
    { crate = "bincode", reason = "D-27: discontinued, RUSTSEC-2025-0141" },
    { crate = "sled", reason = "contract section 2: pre-1.0 storage, avoided" },
    { crate = "jwalk", reason = "contract section 2: avoided, the crate description points elsewhere" },
    { crate = "serde-sarif", reason = "contract section 2: stale; SARIF is hand-written (R-CHK-036)" },
    # HTTP and TLS clients and the stacks they pull in (R-ARCH-123, R-CLI-009).
    { crate = "reqwest", reason = "R-ARCH-123: the 0.1 binary links no network stack" },
    { crate = "ureq", reason = "R-ARCH-123: the 0.1 binary links no network stack" },
    { crate = "hyper", reason = "R-ARCH-123: the 0.1 binary links no network stack" },
    { crate = "openssl", reason = "R-ARCH-123: the 0.1 binary links no network stack" },
    { crate = "openssl-sys", reason = "R-ARCH-123: the stack a git2 https feature would link (Contract issues I-7)" },
    { crate = "rustls", reason = "R-ARCH-123: the 0.1 binary links no network stack" },
    { crate = "native-tls", reason = "R-ARCH-123: the 0.1 binary links no network stack" },
    { crate = "curl", reason = "R-ARCH-123: no network client (Contract issues I-7)" },
    { crate = "curl-sys", reason = "R-ARCH-123: no network client (Contract issues I-7)" },
    { crate = "libssh2-sys", reason = "R-ARCH-123: the stack a git2 ssh feature would link (Contract issues I-7)" },
    # Background watchers and daemons (R-AGT-007, D-26; Contract issues I-7).
    { crate = "notify", reason = "R-AGT-007: fairlead watches no files" },
    { crate = "daemonize", reason = "R-AGT-007, D-26: no background process" },
]

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

- [ ] **Step 3: Red check against the policy**

Run from ROOT: `cargo deny check bans`
Expected: FAIL. The output reports that `reqwest` is banned (R-ARCH-123), with the reason from `deny.toml`.

- [ ] **Step 4: Remove the probe and run the full check**

Remove the `reqwest` line from `ROOT/crates/fairlead-core/Cargo.toml`, then run `git checkout -- crates/fairlead-core/Cargo.toml Cargo.lock` (both are committed in Task 2). Run `git status --short`.
Expected: no output, so the working tree matches the Task 2 commit and the probe is gone.

Run from ROOT: `cargo deny check`
Expected: PASS for `bans`, `licences`, `sources` and `advisories`. Licence entries that match nothing produce warnings only. Record the cargo-deny version used.

- [ ] **Step 5: Commit**

```bash
git add deny.toml
git commit -m "build(deny): cargo-deny policy for bans, licences, sources and advisories" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Continuous integration (`ci.yml`) and the MSRV job (`msrv.yml`)

**Files:**
- Create: `ROOT/.github/workflows/ci.yml`, `ROOT/.github/workflows/msrv.yml`

**Interfaces:**
- Consumes: the commands of contract section 1 (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`), `cargo xtask check-deps` (Task 5), `cargo deny check` (Task 7).
- Produces: the jobs `lint`, `test` (matrix on three operating systems), `workflows` in `ci.yml`, and the job `msrv` in `msrv.yml` on Rust 1.88.0 on three operating systems (R-ARCH-110, R-ARCH-113).

- [ ] **Step 1: Red check**

Run from ROOT: `Test-Path .github/workflows/ci.yml, .github/workflows/msrv.yml` (PowerShell)
Expected: FAIL. Both print `False`.

- [ ] **Step 2: Create `ROOT/.github/workflows/ci.yml`**

```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: "0"
  RUST_BACKTRACE: "1"
  INSTA_UPDATE: "no"

jobs:
  lint:
    name: fmt, clippy, forbidden crates, cargo-deny
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Swatinem/rust-cache@v2
      - name: Format (R-ARCH-060 ordering is enforced by clippy.toml)
        run: cargo fmt --all --check
      - name: Clippy with warnings as errors
        run: cargo clippy --workspace --all-targets --locked -- -D warnings
      - name: Forbidden crates (R-ARCH-123, R-AGT-007)
        run: cargo xtask check-deps
      - name: Install cargo-deny
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-deny
      - name: Licences, bans, sources and advisories (R-REL-033)
        run: cargo deny check

  test:
    name: test on ${{ matrix.os }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - name: Windows long paths (R-ARCH-110, R-PORT-014)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -Value 1 -Type DWord
          git config --global core.longpaths true
      - uses: Swatinem/rust-cache@v2
      - name: Test the whole workspace (locked)
        run: cargo test --workspace --locked

  workflows:
    name: lint the workflow files
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: actionlint
        uses: raven-actions/actionlint@v2
```

- [ ] **Step 3: Create `ROOT/.github/workflows/msrv.yml`**

```yaml
name: msrv

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always
  INSTA_UPDATE: "no"

jobs:
  msrv:
    name: MSRV 1.88.0 on ${{ matrix.os }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    env:
      # rust-toolchain.toml pins "stable", and rustup prefers that file over the default toolchain.
      # RUSTUP_TOOLCHAIN takes precedence over the file, so this job really runs 1.88.0 (R-ARCH-113,
      # R-ARCH-121; Contract issues I-14).
      RUSTUP_TOOLCHAIN: "1.88.0"
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust 1.88.0
        uses: dtolnay/rust-toolchain@1.88.0
      - name: Windows long paths (R-ARCH-110)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -Value 1 -Type DWord
          git config --global core.longpaths true
      - uses: Swatinem/rust-cache@v2
      - name: Test the whole workspace on the MSRV
        run: cargo test --workspace --locked
```

- [ ] **Step 4: Green checks**

Run from ROOT: `actionlint .github/workflows/ci.yml .github/workflows/msrv.yml`
Expected: PASS, no output. Verify: install actionlint at the version the `workflows` job uses, or rely on that job for the first run if the tool is not available locally.

Run: `Select-String -Path .github/workflows/ci.yml, .github/workflows/msrv.yml -Pattern 'ubuntu-latest|windows-latest|macos-latest'`
Expected: PASS. Both files name all three operating systems (R-ARCH-110, R-ARCH-113).

Run: `Select-String -Path .github/workflows/*.yml -Pattern '--locked'`
Expected: PASS, at least one match per workflow that runs cargo.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml .github/workflows/msrv.yml
git commit -m "ci: fmt, clippy, forbidden crates, cargo-deny, tests on three systems, MSRV 1.88" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Audit (`audit.yml`) and the gated fuzz workflow (`fuzz.yml`)

**Files:**
- Create: `ROOT/.github/workflows/audit.yml`, `ROOT/.github/workflows/fuzz.yml`

**Interfaces:**
- Consumes: `Cargo.lock` (Task 2) and the fuzz project, which does not exist until P9. The fuzz job tests `fuzz/Cargo.toml`.
- Produces: `audit.yml` runs `cargo audit` on a weekly schedule and on changes to a manifest or the lock file (R-ARCH-122, R-REL-033). `fuzz.yml` runs each target of contract section 7 for a bounded time (R-ARCH-150). Until `fuzz/Cargo.toml` exists the fuzz job is skipped by its gate step, so it is green.

- [ ] **Step 1: Red check**

Run from ROOT: `Test-Path .github/workflows/audit.yml, .github/workflows/fuzz.yml` (PowerShell)
Expected: FAIL. Both print `False`.

- [ ] **Step 2: Create `ROOT/.github/workflows/audit.yml`**

```yaml
name: audit

on:
  schedule:
    - cron: "17 5 * * 1"
  push:
    branches: [main]
    paths: ["Cargo.lock", "**/Cargo.toml", ".github/workflows/audit.yml"]
  pull_request:
    paths: ["Cargo.lock", "**/Cargo.toml"]
  workflow_dispatch:

permissions:
  contents: read

jobs:
  audit:
    name: cargo audit against the RustSec database
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install cargo-audit
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-audit
      - name: Audit the committed lock file (R-ARCH-122, R-REL-046)
        run: cargo audit --file Cargo.lock --deny warnings
```

- [ ] **Step 3: Create `ROOT/.github/workflows/fuzz.yml`**

```yaml
name: fuzz

on:
  pull_request:
    paths:
      - "fuzz/**"
      - "crates/fairlead-syntax/**"
      - "crates/fairlead-core/src/config/**"
      - ".github/workflows/fuzz.yml"
  workflow_dispatch:

permissions:
  contents: read

env:
  FUZZ_SECONDS: "60"

jobs:
  fuzz:
    name: fuzz ${{ matrix.target }} for a bounded time (R-ARCH-150)
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        target: [directive, control_file, frontmatter, lexer]
    steps:
      - uses: actions/checkout@v4
      - name: Gate, the fuzz project exists
        id: gate
        shell: bash
        run: |
          if [ -f fuzz/Cargo.toml ]; then
            echo "present=true" >> "$GITHUB_OUTPUT"
          else
            echo "present=false" >> "$GITHUB_OUTPUT"
          fi
      - name: Nightly toolchain (cargo-fuzz requires it)
        if: steps.gate.outputs.present == 'true'
        uses: dtolnay/rust-toolchain@nightly
      - name: Install cargo-fuzz
        if: steps.gate.outputs.present == 'true'
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-fuzz
      - name: Run the target for FUZZ_SECONDS
        if: steps.gate.outputs.present == 'true'
        run: cargo +nightly fuzz run ${{ matrix.target }} -- -max_total_time=${{ env.FUZZ_SECONDS }}
```

- [ ] **Step 4: Green checks**

Run from ROOT: `actionlint .github/workflows/audit.yml .github/workflows/fuzz.yml`
Expected: PASS, no output.

Run: `Select-String -Path .github/workflows/fuzz.yml -Pattern 'fuzz/Cargo.toml'`
Expected: PASS, one match. The gate is present, so the fuzz steps are skipped in this bootstrap state.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/audit.yml .github/workflows/fuzz.yml
git commit -m "ci: scheduled cargo-audit and a fuzz workflow gated on the fuzz project" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Benchmark (`bench.yml`) and dogfood (`dogfood.yml`) workflows

**Files:**
- Create: `ROOT/.github/workflows/bench.yml`, `ROOT/.github/workflows/dogfood.yml`

**Interfaces:**
- Consumes: `crates/fairlead-core/benches/*.rs` (P9); `crates/fairlead/src/commands/check.rs` (a stub from Task 3 that gains `pub fn run` in P8). `cargo xtask bench-corpus` (stub until P9).
- Produces: `bench.yml` runs the corpus and the core benchmarks, gated on the benchmark sources (R-ARCH-130). `dogfood.yml` runs `fairlead check` on this repository, gated on the `check` verb file (R-ARCH-160). Both are green in this bootstrap state because their gates are closed.

- [ ] **Step 1: Red check**

Run from ROOT: `Test-Path .github/workflows/bench.yml, .github/workflows/dogfood.yml` (PowerShell)
Expected: FAIL. Both print `False`.

- [ ] **Step 2: Create `ROOT/.github/workflows/bench.yml`**

```yaml
name: bench

on:
  pull_request:
    paths:
      - "crates/fairlead-core/**"
      - "xtask/**"
      - "Cargo.lock"
      - ".github/workflows/bench.yml"
  workflow_dispatch:

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always

jobs:
  bench:
    name: benchmarks against the R-ARCH-130 targets
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Gate, benchmark sources exist
        id: gate
        shell: bash
        run: |
          if compgen -G "crates/fairlead-core/benches/*.rs" > /dev/null; then
            echo "present=true" >> "$GITHUB_OUTPUT"
          else
            echo "present=false" >> "$GITHUB_OUTPUT"
          fi
      - uses: Swatinem/rust-cache@v2
        if: steps.gate.outputs.present == 'true'
      - name: Generate the benchmark corpus (R-ARCH-130)
        if: steps.gate.outputs.present == 'true'
        run: cargo xtask bench-corpus
      - name: Run the core benchmarks (locked)
        if: steps.gate.outputs.present == 'true'
        run: cargo bench -p fairlead-core --locked
      # P9 adds the threshold comparison that fails a run on a regression above the stated limit.
```

- [ ] **Step 3: Create `ROOT/.github/workflows/dogfood.yml`**

```yaml
name: dogfood

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always

jobs:
  dogfood:
    name: fairlead check on this repository (R-ARCH-160)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Gate, the check verb is implemented
        id: gate
        shell: bash
        run: |
          if grep -q '^pub fn run' crates/fairlead/src/commands/check.rs; then
            echo "present=true" >> "$GITHUB_OUTPUT"
          else
            echo "present=false" >> "$GITHUB_OUTPUT"
          fi
      - uses: Swatinem/rust-cache@v2
        if: steps.gate.outputs.present == 'true'
      - name: In-tree build checks this repository
        if: steps.gate.outputs.present == 'true'
        run: cargo run -p fairlead --locked -- check --format terse
      # P9 adds a second run with the last released binary as the pinned reference (R-ARCH-160).
```

- [ ] **Step 4: Green checks**

Run from ROOT: `actionlint .github/workflows/bench.yml .github/workflows/dogfood.yml`
Expected: PASS, no output.

Run: `Select-String -Path .github/workflows/dogfood.yml -Pattern 'pub fn run'`
Expected: PASS, one match in the gate. Run also: `Select-String -Path crates/fairlead/src/commands/check.rs -Pattern '^pub fn run'`
Expected: FAIL with no match, because the Task 3 stub has no `pub fn run`. The gate therefore reports `present=false` and the dogfood steps are skipped. P8 adds the function and opens the gate.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/bench.yml .github/workflows/dogfood.yml
git commit -m "ci: gated benchmark and dogfood workflows" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Release workflow (`release.yml`) and the pull-request checklist

**Files:**
- Create: `ROOT/.github/workflows/release.yml`
- Create: `ROOT/.github/pull_request_template.md` (Contract issues I-9: a file the contract does not list, kept for R-ARCH-152)

**Interfaces:**
- Consumes: the `fairlead` binary target (Task 2) and the tag convention `vMAJOR.MINOR.PATCH`.
- Produces: on a tag, three binaries built with `--locked` on Rust 1.88.0 and a `blake3sums.txt` attached to a GitHub release (R-REL-023). The workflow publishes no crate: `publish = false` stays until the owner decides (R-REL-020, R-REL-042; Contract issues I-15).

- [ ] **Step 1: Red check**

Run from ROOT: `Test-Path .github/workflows/release.yml, .github/pull_request_template.md` (PowerShell)
Expected: FAIL. Both print `False`.

- [ ] **Step 2: Create `ROOT/.github/workflows/release.yml`**

```yaml
name: release

on:
  push:
    tags: ["v*.*.*"]
  workflow_dispatch:

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    name: build ${{ matrix.asset }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: true
      matrix:
        include:
          - os: ubuntu-latest
            exe: fairlead
            asset: fairlead-linux-x86_64
          - os: windows-latest
            exe: fairlead.exe
            asset: fairlead-windows-x86_64.exe
          - os: macos-latest
            exe: fairlead
            asset: fairlead-macos
    env:
      # The release is built with the MSRV toolchain (R-REL-006). RUSTUP_TOOLCHAIN overrides
      # rust-toolchain.toml, which pins "stable" (Contract issues I-14).
      RUSTUP_TOOLCHAIN: "1.88.0"
    steps:
      - uses: actions/checkout@v4
      - name: Toolchain 1.88.0
        uses: dtolnay/rust-toolchain@1.88.0
      - uses: Swatinem/rust-cache@v2
      - name: Release build, locked (R-REL-046)
        run: cargo build --release --locked -p fairlead
      - name: Stage the binary under its asset name
        shell: bash
        run: |
          mkdir -p dist
          cp "target/release/${{ matrix.exe }}" "dist/${{ matrix.asset }}"
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.asset }}
          path: dist/${{ matrix.asset }}
          if-no-files-found: error

  publish:
    name: GitHub release with BLAKE3 checksums
    needs: build
    if: startsWith(github.ref, 'refs/tags/')
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v4
        with:
          pattern: fairlead-*
          path: release
          merge-multiple: true
      - name: Install b3sum
        uses: taiki-e/install-action@v2
        with:
          tool: b3sum
      - name: BLAKE3 checksums (R-REL-023, D-50; Contract issues I-8)
        shell: bash
        run: |
          cd release
          b3sum fairlead-* > blake3sums.txt
          cat blake3sums.txt
      - name: Create the release with its assets
        uses: softprops/action-gh-release@v2
        with:
          files: release/*
          fail_on_unmatched_files: true
```

- [ ] **Step 3: Create `ROOT/.github/pull_request_template.md`**

```markdown
## Summary

<!-- What changes and why. Cite the rule IDs (R-xxx-nnn) and decisions (D-nn) that the change implements. -->

## Checks

- [ ] `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` pass.
- [ ] `cargo test --workspace --locked` passes on Linux, Windows and macOS (R-ARCH-110).
- [ ] A bug fix adds its regression fixture in an earlier commit than the fix (R-ARCH-152).
- [ ] A new diagnostic code has at least one snapshot test (R-ARCH-151).
- [ ] A new dependency has its row in 50-architecture Section 11 before it is added (R-ARCH-120).
```

- [ ] **Step 4: Green checks**

Run from ROOT: `actionlint .github/workflows/release.yml`
Expected: PASS, no output.

Run: `Select-String -Path .github/workflows/release.yml -Pattern 'cargo build --release --locked'`
Expected: PASS, one match (R-REL-046).

Run: `Select-String -Path .github/workflows/release.yml -Pattern 'publish'`
Expected: PASS, only the job name `publish` and the `needs`/`if` lines. No `cargo publish` command appears (Contract issues I-15).

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml .github/pull_request_template.md
git commit -m "ci(release): tagged builds with locked dependencies, BLAKE3 checksums and a PR checklist" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 12: The root managerial folder `fairlead/` for dogfooding

**Files:**
- Create: `ROOT/fairlead/control.toml`

**Interfaces:**
- Consumes: the `[fairlead]` header and `[settings]` table of decisions D-33 and of the schema index of chapter 12 (`edition`, `root`, `scope`, `marker`; `folder`, `fairlead-version`, `purpose`).
- Produces: the root scope of this repository (`scope = "root"`, `root = true`). Later phases add `approvals.toml` (P7, through `approve`), `gen/` (P7, through `sync`) and `docs/` and the rules (P9).

- [ ] **Step 1: Red check**

Run from ROOT: `Test-Path fairlead/control.toml` (PowerShell)
Expected: FAIL, prints `False`.

- [ ] **Step 2: Create the root control file**

Create `ROOT/fairlead/control.toml`:

```toml
# Root managerial folder of this repository (R-ARCH-160; decisions D-31 and D-33).
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
purpose       = "The repository's own rules, dogfooded with fairlead."
```

- [ ] **Step 3: Green checks**

Run: `Select-String -Path fairlead/control.toml -Pattern '^root\s*=\s*true'`
Expected: PASS, one match. The root flag is set (R-CLI-004).

Run: `Select-String -Path fairlead/control.toml -Pattern '^fairlead-version = "0.1.0"'`
Expected: PASS, one match. It equals the workspace version, so the pin check of R-CLI-005 passes (C-12).

Run from ROOT: `cargo xtask check-deps`
Expected: PASS. The new file does not change the lock files.

- [ ] **Step 4: Commit**

```bash
git add fairlead/control.toml
git commit -m "chore(fairlead): root managerial folder for dogfooding (R-ARCH-160)" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Verification sweep of the whole phase

**Files:** none created. This task runs the checks that the P1 rules name and reads the results.

**Interfaces:**
- Consumes: every earlier task.
- Produces: a clean working tree with eleven commits on `main`, and the evidence that Traceability rows point to.

- [ ] **Step 1: Build, lint and test on the development toolchain**

Run from ROOT, in this order:
- `cargo fmt --all --check`. Expected: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`. Expected: PASS.
- `cargo test --workspace --locked`. Expected: PASS. Five xtask tests run, and the fairlead test targets run zero tests.
- `cargo xtask check-deps`. Expected: PASS, exit code 0.
- `cargo deny check`. Expected: PASS (Task 7 records the cargo-deny version).

- [ ] **Step 2: Structural rules**

- `cargo metadata --no-deps --locked --format-version 1`. Expected: four packages, `rust_version` 1.88, edition 2024 (R-ARCH-007, R-ARCH-121).
- `cargo tree --workspace --edges normal --locked`. Expected: only workspace members appear; no external crate and no network crate (R-ARCH-120, R-ARCH-123, R-CLI-009).
- `cargo tree -p fairlead-syntax --edges normal`. Expected: no `fairlead-core` (R-ARCH-002).
- `git ls-files Cargo.lock`. Expected: `Cargo.lock` is tracked (R-ARCH-122).
- `git ls-files --eol`. Expected: every text file reports `i/lf` (LF in the index). No `i/crlf` line appears.

- [ ] **Step 3: Workflow files**

- `actionlint .github/workflows/*.yml`. Expected: PASS, no output.
- `Select-String -Path .github/workflows/msrv.yml -Pattern 'RUSTUP_TOOLCHAIN'`. Expected: PASS, one match (R-ARCH-113).

- [ ] **Step 4: The repository is clean**

Run: `git status --short`
Expected: no output. `.remember/` is ignored and therefore absent from the list.

Run: `git log --oneline`
Expected: eleven commits, from `docs: import the 0.1 specification` to `chore(fairlead): root managerial folder`, in the order of Tasks 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12. Task 4 and Task 13 create no commit.

- [ ] **Step 5: Report**

Record the versions used: Rust stable (`rustc --version`), cargo-deny, cargo-audit, actionlint and b3sum. Record each Verify note that was checked during execution, with its result, in the commit message body of the commit where it applies.

---

## Traceability

Every P1 rule of contract section 6 (ARCH 002 003 007 060 110 113 120 121 122 123 152; CLI 009; AGT 007) and the release rules that P1 covers partly (60-roadmap section 9). Task numbers refer to this plan.

| Rule | Implemented in | Verifying command or check |
|---|---|---|
| R-ARCH-002 | Task 2: `fairlead-syntax` manifest has no `fairlead-core` edge | `cargo tree -p fairlead-syntax --edges normal` (Task 2 Step 6, Task 13 Step 2) |
| R-ARCH-003 | Task 6: README states that library APIs are not a stability promise | `Select-String README.md -Pattern 'stability promise'` (Task 6 Step 3) |
| R-ARCH-007 | Task 2: three product crates and `xtask`, nothing else | `cargo metadata --no-deps --locked --format-version 1` (Task 2 Step 6) |
| R-ARCH-060 | Task 2: `clippy.toml` disallows `HashMap` and `HashSet` | Task 4 probe: `disallowed_types` fires, then is reverted |
| R-ARCH-090 | Task 2: `clippy.toml` disallows the clock outside `time.rs` and `check/options.rs` | Task 4 probe: `disallowed_methods` for `SystemTime::now` |
| R-ARCH-100 | Task 2: `clippy.toml` disallows `process::exit`; no library exits | Task 4 probe: `disallowed_methods` for `process::exit` |
| R-ARCH-110 | Task 8: `ci.yml` test matrix on three systems; Windows long paths enabled | `Select-String` on `ci.yml` (Task 8 Step 4); the first CI run on all three systems. Windows test content is P9 (Contract issues I-12) |
| R-ARCH-113 | Task 8: `msrv.yml` runs 1.88.0 on three systems | Task 8 Step 4; Task 13 Step 3 |
| R-ARCH-120 | Task 2: dependencies come only from the section 11 table; PR checklist | `cargo tree --workspace --edges normal --locked` (Task 13 Step 2) |
| R-ARCH-121 | Task 2: `rust-version = "1.88"`; Tasks 8 and 11 build on 1.88.0 | `cargo metadata` (Task 2 Step 6) |
| R-ARCH-122 | Task 2: `Cargo.lock` committed; `--locked` in every cargo workflow step; Task 9 `audit.yml` | `git ls-files Cargo.lock` (Task 13 Step 2); the audit job |
| R-ARCH-123 | Task 5: `check-deps` scan; Task 7: `deny.toml` bans | Task 5 Step 3 (probe `ureq` in `fuzz/Cargo.lock`); Task 7 Step 3 (probe `reqwest`) |
| R-ARCH-152 | Task 11: `.github/pull_request_template.md` checklist (Contract issues I-9) | file present; enforcement is review |
| R-CLI-009 | Task 5 (`check-deps`); Task 7 (bans); `fairlead-core` declares no `net` feature (Task 2) | `cargo xtask check-deps`; `cargo tree` (Task 13) |
| R-AGT-007 | Task 5: `notify` and `daemonize` are forbidden; Task 7: bans | Task 7 Step 3 probe for the bans; Task 5 Step 3 for `check-deps` |
| R-ARCH-160 (P1 part) | Task 12: `fairlead/control.toml` root; Task 10 dogfood gate | `Select-String fairlead/control.toml` (Task 12 Step 3); the dogfood job is gated until P8 |
| R-ARCH-150, R-ARCH-130 (workflow part) | Task 9 `fuzz.yml`; Task 10 `bench.yml`, both gated | file content; the targets and thresholds are P9 |
| R-REL-006 | Task 8 `msrv.yml`; Task 11 release on 1.88.0 | Task 8 Step 4; Task 11 Step 4 |
| R-REL-007 | not satisfied in P1: `LICENSE` is a placeholder | owner decision before the first publish (Contract issues I-1) |
| R-REL-020, R-REL-021, R-REL-042 | `publish = false` in `workspace.package`, inherited by every member | `cargo metadata` shows `publish` empty (Task 2 Step 6); the publish path stays closed (Task 11) |
| R-REL-023 | Task 11 `release.yml`: three binaries and BLAKE3 checksums | Task 11 Step 4; the first tagged release |
| R-REL-033 | Task 7 `deny.toml` advisories; Task 9 `audit.yml` | Task 7 Step 4; the audit job |
| R-REL-046 | `ci.yml`, `msrv.yml`, `audit.yml`, `release.yml` all use `--locked` | Task 13 Steps 1 to 3 |

## Contract issues

Each item is a point where the contract could not be followed exactly, or where this plan adds to it. None is silent. Each names the choice made and what the owner must decide.

- **I-1 (R-REL-007, section 1 `LICENSE`).** The contract makes `LICENSE` a placeholder. R-REL-007 requires MIT OR Apache-2.0 with both texts before the first publish. P1 writes the placeholder and keeps `publish = false`. The owner selects the licence before any publish.
- **I-2 (section 1, `clippy.toml`).** The contract places the HashMap and HashSet ban under "syntax and core". `clippy.toml` is workspace-wide and cannot be scoped per crate without per-crate files, which the contract does not list. The ban therefore also applies to the bin, `xtask` and tests, which is stricter. The environment and clock exemptions (`discover.rs`, `time.rs`, `check/options.rs`, the bin) are implemented as local `#[allow(clippy::disallowed_methods)]` with a reason at the call site, as section 1 permits.
- **I-3 (section 1, print lints).** The contract denies `print_stdout` and `print_stderr` for library code, and only `fairlead` prints. P1 allows both with a crate-level attribute and a reason in `crates/fairlead/src/main.rs` and `xtask/src/main.rs`. Library crates stay denied.
- **I-4 (section 1, `todo`).** The contract denies `todo` "once the owning phase has landed" and names no phase. P1 sets `todo = "allow"`. This plan assigns the flip to `deny` to P9, the last phase.
- **I-5 (section 3, `SYNTAX_VERSION`).** The contract names the constant without a value. P1 sets `1`, equal to the grammar edition. P2 owns the constant and may amend it through the contract.
- **I-6 (section 1, `.gitignore`).** The contract lists `target/` and `.fairlead/`. P1 also ignores `.remember/`, the remember plugin's state at the repository root, so plugin logs are never committed.
- **I-7 (section 2, forbidden list).** The contract names the YAML crates, `bincode`, `sled`, `jwalk`, `serde-sarif` and the HTTP/TLS clients. P1 also forbids `openssl-sys`, `curl`, `curl-sys` and `libssh2-sys`, which are the stacks a `git2` https or ssh feature would link (R-ARCH-123), and `notify` and `daemonize`, which are background watchers (R-AGT-007). The owner confirms these additions.
- **I-8 (R-REL-023 against D-50(2)).** R-REL-023 requires published checksums; D-50(2) states that sha256 appears nowhere. P1 publishes BLAKE3 checksums with `b3sum`. The owner confirms this reading.
- **I-9 (R-ARCH-152).** The rule has no executable form in P1, because no bug fix exists yet. P1 adds `.github/pull_request_template.md`, a file the contract does not list, as the enforcement point. The owner may strike it, and the rule then remains a review rule.
- **I-10 (section 2, UNVERIFIED entries).** `unicode-normalization`, `memchr`, `regex`, `thiserror`, `proptest`, `criterion`, `libfuzzer-sys`, `cargo-fuzz`, a JSON-schema validator and `sysinfo` are not declared in P1. Each is added, with its verified section 11 row, by the phase that needs it (R-ARCH-120). The UNVERIFIED feature names in `[workspace.dependencies]` (`toml`, `jiff`, `git2`, `clap`) are carried as the contract gives them and checked at first use.
- **I-11 (section 6, phase ownership).** Section 6 assigns `fuzz.yml`, `bench.yml` and `dogfood.yml` to P9, while section 6's build order and section 7 start the harnesses in P1. This plan creates them as real workflows whose gates are closed until the files they test exist, and P9 completes them. Section 6 assigns no phase to the `xtask` subcommands `sarif-validate`, `bench-corpus` and `golden-bless`; this plan assigns them to P9.
- **I-12 (R-ARCH-110).** The Windows job must exercise paths over 260 characters, locked files and antivirus latency. Those checks are tests that P9 writes in `tests/platform.rs`. P1 wires the job and enables long paths on the runner. The rule is fully met only when P9 lands.
- **I-13 (section 7, test targets).** Section 7 declares the test targets of `crates/fairlead/Cargo.toml` with paths under `tests/`, and no phase is named for creating those files. This plan creates them as one-line stubs, so the declarations resolve in P1.
- **I-14 (R-ARCH-113, `rust-toolchain.toml`).** `rust-toolchain.toml` pins `stable`, and rustup prefers that file over the default toolchain. Without a fix, the MSRV job would silently test stable. This plan sets `RUSTUP_TOOLCHAIN` to `1.88.0` in `msrv.yml` and `release.yml`. The contract does not state this mechanism.
- **I-15 (R-REL-020, R-REL-046).** The release workflow builds the binaries but publishes no crate, because `publish = false` stays. The contract does not say when that changes. The owner decides, and then `cargo publish --dry-run` is added to the release workflow.
- **I-16 (60-roadmap section 9).** The R-REL rules are not in the section 6 phase table. P1 covers R-REL-006, 023, 033 and 046 in part, and R-REL-007 remains open (I-1). R-REL-020, 021 and 042 are covered by `publish = false`.
- **I-17 (repository root).** The plan takes the repository root to be `harness\`, the parent of `docs\`, because the contract makes the root the workspace root and the specification sits in `docs\`. If the owner wants the workspace in a different folder, Task 1 changes.
- **I-18 (section 1, `fairlead/` contents).** P1 creates `fairlead/control.toml` only. `approvals.toml` and `gen/` arrive with P7 (`approve` and `sync`), and `docs/` and the rules arrive with P9.

## Execution handoff

Recommended: **superpowers:subagent-driven-development**, one implementer per task in the order of this plan (Tasks 1 to 13). Use a review pass after Tasks 3, 5, 8 and 13. Tasks 4 and 13 create no commit. Alternative: **superpowers:executing-plans** in one session, with checkpoints at the same points.

Before the first task, the executor reads sections 1, 2, 3 and 6 of the contract. The executor runs `cargo fmt --all` before every Rust commit. Every cargo command keeps `--locked`. Each Verify note names a point where an API, a configuration key or an action version is checked at execution time; the result is recorded in the body of the commit that depends on it.
