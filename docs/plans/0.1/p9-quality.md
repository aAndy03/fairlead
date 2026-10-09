# P9 Quality and Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the validation layer that proves fairlead 0.1 meets exit criteria E1-E10: the golden repositories, the determinism, read-only, SARIF, secrets, replay and platform tests, the fuzz targets, the benchmark corpus and CI gates, the dogfood declarations, and the pre-registered controlled ablation with its scoring tools.

**Architecture:** All end-to-end tests are targets of the `fairlead` package (`crates/fairlead/Cargo.toml`, explicit `[[test]]` paths into `tests/`). They spawn only the built `fairlead` binary (`env!("CARGO_BIN_EXE_fairlead")`) and the `git` executable, never `cargo` or `rustc`. Golden repositories live under `tests/golden/<name>/` and are materialised into temp directories per run; the golden runner and the `golden-bless` xtask share one pure module, `tests/testkit/golden_spec.rs`, included by path. Fuzz targets live in the excluded `fuzz/` crate; benchmarks in `crates/fairlead-core/benches/`; tooling (SARIF validation, benchmark corpus, golden bless, docs check, ablation scoring, syntax-card scoring, precision sampling) in `xtask/`. The ablation protocol is committed before any harness code (R-ROAD-014).

**Tech Stack:** Rust edition 2024, MSRV 1.88, resolver 3. Test crates: `insta` 1.49.0 (`json`, `glob`), `trycmd` 1.2.1, `tempfile` 3.27.0, plus `toml` 1.1.7, `serde` 1.0.229 (`derive`) and `serde_json` 1.0.151 as dev-dependencies of `fairlead`. Fuzzing: `libfuzzer-sys` and `cargo-fuzz` (UNVERIFIED, see Contract issues). Benchmarks: `criterion` (UNVERIFIED). SARIF schema validation in xtask: a JSON-schema validator crate (UNVERIFIED). GitHub Actions workflows: `fuzz.yml`, `bench.yml`, `dogfood.yml`.

**Spec:** `docs/plans/0.1/01-contract.md` (law: sections 1, 6 P9 row, 7, 8, 9); `docs/decisions.md`; `docs/conventions.md`; `docs/60-roadmap.md` 4.1 (E1-E10), 5 (R-ROAD-001..009, 014..017), 7 (validation plan); `docs/50-architecture.md` 12, 14, 15 and R-ARCH-071, 080, 111, 130, 131, 140, 150, 151, 153, 160, 161, 162; `docs/13-checks-and-diagnostics.md` R-CHK-012, 015, 016, 051; `docs/14-context.md` R-CTX-072; `docs/40-agent-integration-and-trust.md` R-AGT-005, 006; `docs/23-git-integration.md` R-GIT-001; `docs/32-portability-and-restructure.md` R-PORT-014; `docs/examples/ts-react.md`, `docs/examples/rust-workspace.md`, `docs/examples/fullstack-five-years.md` (basis for the three large golden repositories).

**Phase boundary:** P9 owns `tests/*`, `tests/golden/*`, `fixtures/*`, `fuzz/*`, `crates/fairlead-core/benches/*`, the P9 xtask commands, `.github/workflows/{fuzz,bench,dogfood}.yml` and the root `fairlead/` dogfood declarations. P9 consumes the binary built by P8 and the public API of P2 to P7. Where P9 needs an API the contract does not name, the requirement is listed in Contract issues and must be agreed before the owning phase starts.

---

## Global Constraints

These hold for every task in this plan. They are copied from the phase brief and are not optional.

- Golden outputs are byte-compared and re-blessed only by `cargo xtask golden-bless <name>`. No test writes an expected file. `golden-bless` refuses to run when the `CI` environment variable is set.
- Tests never spawn `cargo` or `rustc`. They spawn only the built `fairlead` binary (`env!("CARGO_BIN_EXE_fairlead")`) and `git` (for repository fixtures only). Only `xtask` and the workflows call `cargo`.
- CI runs with `INSTA_UPDATE=no`. Snapshots are reviewed and never auto-accepted.
- The thresholds of E1-E10 are fixed before the controlled test is built. They are committed in `docs/plans/0.1/ablation-protocol.md` (Task 1) and asserted against the code constants by a test. A later change is a dated amendment (R-ROAD-014) and invalidates earlier runs.
- Time is never read in tests. Every test passes `--as-of 2026-10-09` (or a fixed date). `Instant::now` and `SystemTime::now` are allowed only in `xtask` timing code, behind a local `#[allow(clippy::disallowed_methods)]` with a reason comment.
- Test files carry `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)]` with the reason "integration test code". Library code is unaffected.
- Output compared in tests is LF-only. Expected files end with exactly one newline.
- No `HashMap` or `HashSet` in any P9 code whose output reaches an expected file (lint-forbidden in `fairlead-syntax` and `fairlead-core`; P9 uses `BTreeMap` throughout).
- Secret-like fixture values are generated at test time from seeds. No provider-prefixed key literal is committed.
- Each rule assigned to P9 has at least one test whose name starts with the snake-case rule ID (contract section 7).

## File map (all paths relative to the repository root `fairlead/`)

Create: `tests/testkit/mod.rs`, `tests/testkit/golden_spec.rs`, `tests/golden.rs`, `tests/determinism.rs`, `tests/readonly.rs`, `tests/sarif.rs`, `tests/secrets.rs`, `tests/replay.rs`, `tests/platform.rs`, `tests/codes.rs` (extension, see Contract issues), `tests/golden/<11 names>/golden.toml`, `tests/golden/<11 names>/repo/**`, `tests/golden/<11 names>/expected/*`, `fixtures/sarif/schema/SOURCE.toml`, `fixtures/sarif/schema/sarif-schema-2.1.0.json` (fetched, Task 15), `fixtures/secrets/seeds.txt`, `fixtures/corpus/{directive,control_file,frontmatter,lexer}/*`, `fixtures/syntax/ts/*`, `fuzz/Cargo.toml`, `fuzz/fuzz_targets/{directive,control_file,frontmatter,lexer}.rs`, `crates/fairlead-core/benches/{extract,digest}.rs`, `xtask/src/{golden,sarif,bench,docs,ablation,syntax_card,precision}.rs`, `ablation/declared.toml`, `ablation/tasks.toml`, `ablation/arms/b-agents.md`, `fairlead/control.toml`, `fairlead/approvals.toml`, `crates/fairlead-core/fairlead/control.toml`, `crates/fairlead-syntax/fairlead/control.toml`, `.github/workflows/fuzz.yml`, `.github/workflows/bench.yml`, `.github/workflows/dogfood.yml`, `docs/plans/0.1/ablation-protocol.md`, `bench/baseline.toml`.

Modify: `crates/fairlead/Cargo.toml` (add `[[test]]` entries and dev-dependencies), `crates/fairlead-core/Cargo.toml` (`[[bench]]` entries with `harness = false`), `xtask/Cargo.toml` (dependencies), `xtask/src/main.rs` (subcommand dispatch), `.gitattributes` (golden CRLF rule), `.gitignore` (`fuzz/corpus/`, `fuzz/artifacts/`, `target/bench/`).

Human-only steps (never executed by an agent): `fairlead approve` (writes `fairlead/approvals.toml`, TTY gate, R-AGT-026), the owner's sign-off of Task 1, first `golden-bless` review, the pre-registered results log.

---

## Task 1: Pre-registration of thresholds, designs and the protocol document

This task comes first. R-ROAD-014 requires the 0.1 exit thresholds, the ablation design and the exception-hygiene design to be committed before the controlled-test fixture is built. The test in this task binds the constants in `xtask` to the committed protocol text, so the code cannot drift from the registered numbers.

**Files:**
- Create: `docs/plans/0.1/ablation-protocol.md`
- Create: `xtask/src/ablation.rs`
- Modify: `xtask/src/main.rs` (add `mod ablation;`)

**Interfaces:**

```rust
// xtask/src/ablation.rs
pub const UI_TASKS: usize = 10;
pub const BACKEND_TASKS: usize = 10;
pub const RUNS_PER_TASK: usize = 3;
pub const PASS_REDUCTION_PCT: u32 = 50;    // E2 gate, arm C against arm A
pub const WEAK_BELOW_PCT: u32 = 20;        // kill assumption 2
pub const HYGIENE_MIN_PCT: u32 = 80;       // E3 and R-ROAD-009
pub const HYGIENE_TASKS: usize = 12;       // roadmap 7.2
pub const PRECISION_SAMPLE: usize = 200;   // R-CHK-051, R-ROAD-003
pub const PRECISION_MAX_FALSE_PCT: u32 = 2;
pub const REPLAY_INTRODUCING: usize = 50;  // R-ROAD-004
pub const REPLAY_RECALL_MIN_PCT: u32 = 70;
pub const REPLAY_UNRELATED: usize = 20;
pub const REPLAY_FP_MAX_PCT: u32 = 5;
pub const SYNTAX_CARD_SAMPLE: usize = 100; // E8
pub const SYNTAX_CARD_MAX_INVALID_PCT: u32 = 5;
pub const ADOPTION_MAX_MINUTES: u32 = 15;  // R-ROAD-006
pub const E6_COLD_CHECK_MAX_S: u64 = 60;
pub const E6_WARM_CHECK_MAX_S: u64 = 5;
pub const E6_CHANGED_MAX_S: u64 = 1;
pub const E6_PEAK_RSS_MAX_MB: u64 = 500;
```

- [ ] **Step 1: Write the failing test**

Create `xtask/src/ablation.rs` with the constants above and this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn protocol() -> String {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/plans/0.1/ablation-protocol.md");
        std::fs::read_to_string(path).unwrap_or_default()
    }

    fn value_of(doc: &str, key: &str) -> Option<u64> {
        let prefix = format!("threshold.{key} = ");
        doc.lines()
            .find_map(|l| l.strip_prefix(&prefix))
            .and_then(|v| v.trim().parse().ok())
    }

    #[test]
    fn r_road_014__thresholds_match_committed_protocol() {
        let doc = protocol();
        let pairs: [(&str, u64); 19] = [
            ("pass_reduction_pct", PASS_REDUCTION_PCT as u64),
            ("weak_below_pct", WEAK_BELOW_PCT as u64),
            ("hygiene_min_pct", HYGIENE_MIN_PCT as u64),
            ("precision_max_false_pct", PRECISION_MAX_FALSE_PCT as u64),
            ("precision_sample", PRECISION_SAMPLE as u64),
            ("replay_recall_min_pct", REPLAY_RECALL_MIN_PCT as u64),
            ("replay_fp_max_pct", REPLAY_FP_MAX_PCT as u64),
            ("replay_introducing", REPLAY_INTRODUCING as u64),
            ("replay_unrelated", REPLAY_UNRELATED as u64),
            ("syntax_card_max_invalid_pct", SYNTAX_CARD_MAX_INVALID_PCT as u64),
            ("syntax_card_sample", SYNTAX_CARD_SAMPLE as u64),
            ("adoption_max_minutes", ADOPTION_MAX_MINUTES as u64),
            ("e6_cold_check_max_s", E6_COLD_CHECK_MAX_S),
            ("e6_warm_check_max_s", E6_WARM_CHECK_MAX_S),
            ("e6_changed_max_s", E6_CHANGED_MAX_S),
            ("e6_peak_rss_max_mb", E6_PEAK_RSS_MAX_MB),
            ("ui_tasks", UI_TASKS as u64),
            ("backend_tasks", BACKEND_TASKS as u64),
            ("runs_per_task", RUNS_PER_TASK as u64),
        ];
        for (key, want) in pairs {
            assert_eq!(value_of(&doc, key), Some(want), "threshold.{key} not committed");
        }
    }
}
```

- [ ] **Step 2: Run the test and expect FAIL**

Run: `cargo test -p xtask r_road_014 -- --nocapture`
Expected: FAIL with `threshold.pass_reduction_pct not committed`, because the protocol file does not exist yet.

- [ ] **Step 3: Write the protocol document**

Create `docs/plans/0.1/ablation-protocol.md` with exactly this content:

```markdown
# 0.1 controlled-test protocol (pre-registered)

> Status: pre-registered · Committed before the fixture is built (R-ROAD-014, Q-PROD-4) · Thresholds: 60-roadmap 4.1, 5, 7.1, 7.2 · Owner: maintainer · Amendments: dated, appended at the end, invalidate earlier runs

## Thresholds (machine-read by xtask; do not reword the keys)

threshold.pass_reduction_pct = 50
threshold.weak_below_pct = 20
threshold.hygiene_min_pct = 80
threshold.precision_sample = 200
threshold.precision_max_false_pct = 2
threshold.replay_introducing = 50
threshold.replay_recall_min_pct = 70
threshold.replay_unrelated = 20
threshold.replay_fp_max_pct = 5
threshold.syntax_card_sample = 100
threshold.syntax_card_max_invalid_pct = 5
threshold.adoption_max_minutes = 15
threshold.e6_cold_check_max_s = 60
threshold.e6_warm_check_max_s = 5
threshold.e6_changed_max_s = 1
threshold.e6_peak_rss_max_mb = 500
threshold.ui_tasks = 10
threshold.backend_tasks = 10
threshold.runs_per_task = 3

## E2: wedge effect (ablation, R-ROAD-005, R-ROAD-018)

- Tasks: 20 (10 UI on the `ts-react` fixture, 10 backend on the `fullstack-tauri` and `rust-workspace` fixtures). Each task has a natural path to a hardcoded declared value (`ablation/tasks.toml`).
- Arms: A, no fairlead artifact. B, a hand-written static AGENTS.md with the same declarations in prose (`ablation/arms/b-agents.md`). C, the output of `fairlead context <path>` inserted at session start.
- Runs: 3 per task per arm, fresh session each, model and settings recorded per run, run order randomised with a recorded seed.
- Primary metric: count of hardcoded declared values in the added lines of the final diff, found by `cargo xtask ablation-score`. The scorer does not call `fairlead check` (R-ROAD-016).
- Decision: reduction of C against A. At least 50% passes. Below 20% is weak and fires kill assumption 2. Arm B is reported, never gated (R-ROAD-018).
- Blinding: the scorer receives diffs under opaque file names. The mapping to arm and task is written after scoring.
- Literal-leak check and `use` lines are enabled in the fixture (R-ROAD-016).
- Threats recorded with each result: model drift between runs, fixture ease, scorer bias, agents reading `fairlead/` directly.

## E3: exception hygiene (R-ROAD-009, roadmap 7.2)

- Fixture: three nested scopes, one sealed key, one waivable rule, one open key. Twelve tasks, each requiring a deviation.
- Per task, one of: (a) avoided by an allowed route, (b) declared `ovr` with `why`, `owner` and expiry, (c) silent deviation, (d) attempt against a sealed key, with the result inert and an escalation produced.
- Pass: at least 80% of the conflicting tasks end in (b), a refusal, or an escalation, in arm C, against arm A.
- A reviewer given only the diff and the ledger lists every deviation. Target: all.

## R-ROAD-003 and R-CHK-051: precision

- Blind review of 200 findings drawn by seeded random sample from the terse output of three real repositories (TypeScript/React, Rust, mixed). The reviewer sees the finding text without its code.
- At most 2% judged false. Zero false dangling-reference reports by automated test (`r_chk_051__no_false_dangling_references`).

## R-ROAD-004: recall on history (replay)

- 50 introducing commits and 20 unrelated commits, from at least three repositories, one TypeScript/React.
- Recall: a literal-leak finding at the introducing commit that did not exist in the parent. At least 70% of the 50.
- False positives: an unrelated commit counts as a false positive when it adds a new literal-leak finding. At most 5% of the 20 (one commit).

## E8: humans and agents write the syntax

- Adoption trial: a developer who has not seen the specification installs the binary, runs `init`, writes a baseline and reaches the first green `check`. Elapsed minutes and the number of documents consulted are recorded. Limit: 15 minutes.
- Syntax-card trial: fresh agent sessions given only the syntax card (`fairlead explain syntax`) write 100 directives. Scored by `cargo xtask syntax-card-score`. Limit: at most 5% invalid.

## E6 and R-ROAD-017: benchmarks

- 100,000-file generated tree (`cargo xtask bench-corpus generate`). Cold check at most 60 s, warm at most 5 s, `--changed` at most 1 s, peak RSS at most 500 MB. `--files-from` is reported against the R-CHK-016 goal of about one second, not gated.
- The bytes-per-token ratio is measured with the target model's tokenizer on sample `context` slices. The result is recorded in 60-roadmap section 11.

## Results log

Runs made under this text are recorded in `docs/plans/0.1/results/` by the maintainer. An amendment is appended below with a date, the changed key and the reason. Runs made before an amendment are invalid for the gate.
```

- [ ] **Step 4: Run the test and expect PASS**

Run: `cargo test -p xtask r_road_014 -- --nocapture`
Expected: `test ablation::tests::r_road_014__thresholds_match_committed_protocol ... ok`.

- [ ] **Step 5: Commit**

```bash
git add docs/plans/0.1/ablation-protocol.md xtask/src/ablation.rs xtask/src/main.rs
git commit -m "docs(p9): pre-register 0.1 thresholds and controlled-test protocol" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

Verify: `cargo test -p xtask r_road_014` is the verifying command for the E2 and E3 design freeze. The owner signs off the protocol before Task 2 starts.

## Task 2: Test targets and the testkit

Every end-to-end test is a target of the `fairlead` package. The testkit is a module, not a crate. It builds temporary repositories, runs the built binary, and snapshots trees for the read-only and determinism tests.

**Files:**
- Modify: `crates/fairlead/Cargo.toml` (dev-dependencies and `[[test]]` entries)
- Create: `tests/testkit/mod.rs`
- Create: `tests/golden.rs` (placeholder test target; its real body is Task 4)

**Interfaces:**

```rust
// tests/testkit/mod.rs
pub const NOW: &str = "2026-10-09";
pub fn fairlead_bin() -> &'static str;                                   // env!("CARGO_BIN_EXE_fairlead")
pub struct Run { pub code: i32, pub stdout: String, pub stderr: String }
pub struct RepoBuilder;                                               // new, file, bytes, crlf, git_init, build
pub struct TempRepo;                                                  // path, run, run_in, git, git_commit, git_status, tree_snapshot
```

- [ ] **Step 1: Declare the test targets (exact content for `crates/fairlead/Cargo.toml`)**

Append to `crates/fairlead/Cargo.toml`:

```toml
[dev-dependencies]
insta = { version = "1.49.0", features = ["json", "glob"] }
trycmd = "1.2.1"
tempfile = "3.27.0"
toml = { version = "1.1.7", default-features = false, features = ["std", "parse", "serde"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"

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
name = "codes"
path = "../../tests/codes.rs"

[[test]]
name = "cli"
path = "../../tests/cli.rs"   # owned by P8 (trycmd), declared here so the binary lookup works
```

- [ ] **Step 2: Write the failing test**

Create `tests/golden.rs` with exactly this content (the module file does not exist yet):

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)]
#[path = "testkit/mod.rs"]
mod testkit;

#[test]
fn testkit_is_linked() {
    let repo = testkit::RepoBuilder::new().file("a.txt", "x\n").build();
    assert!(repo.path().join("a.txt").exists());
}
```

- [ ] **Step 3: Run the test and expect FAIL**

Run: `cargo test -p fairlead --test golden testkit_is_linked`
Expected: FAIL to compile with `file not found for module testkit` or `unresolved module`, because `tests/testkit/mod.rs` does not exist yet.

- [ ] **Step 4: Write the testkit**

Create `tests/testkit/mod.rs` with exactly this content:

```rust
//! Shared helpers for every fairlead test target. Included with
//! `#[path = "testkit/mod.rs"] mod testkit;`. Each target uses a subset,
//! so dead-code warnings are allowed here.
#![allow(dead_code)]

pub mod golden_spec;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The only as-of date used by tests (time is never read in tests).
pub const NOW: &str = "2026-10-09";

/// The binary built for this package. Tests never invoke cargo.
pub fn fairlead_bin() -> &'static str {
    env!("CARGO_BIN_EXE_fairlead")
}

/// The terse check invocation used by most golden cases.
pub const CHECK_TERSE: &[&str] = &["check", "--as-of", NOW, "--format", "terse"];

pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Default)]
pub struct RepoBuilder {
    files: BTreeMap<String, Vec<u8>>,
    crlf: bool,
    git: bool,
}

impl RepoBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a UTF-8 text file at a repository-relative, forward-slash path.
    pub fn file(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.to_string(), text.as_bytes().to_vec());
        self
    }

    /// Adds a file with raw bytes (non-UTF-8 fixtures).
    pub fn bytes(mut self, path: &str, bytes: &[u8]) -> Self {
        self.files.insert(path.to_string(), bytes.to_vec());
        self
    }

    /// Converts every text file to CRLF when built (binary files are left alone).
    pub fn crlf(mut self, on: bool) -> Self {
        self.crlf = on;
        self
    }

    /// Runs `git init`, stages everything and commits once, so `git status` starts clean.
    pub fn git_init(mut self, on: bool) -> Self {
        self.git = on;
        self
    }

    pub fn build(self) -> TempRepo {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel, bytes) in &self.files {
            let out = dir.path().join(rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            let data = if self.crlf { to_crlf(bytes) } else { bytes.clone() };
            std::fs::write(&out, data).expect("write fixture file");
        }
        let repo = TempRepo { dir };
        if self.git {
            repo.git(&["init", "-q"]);
            repo.git(&["add", "-A"]);
            repo.git_commit("init");
        }
        repo
    }
}

/// CRLF-converts text. Files with a NUL byte are binary and returned unchanged.
pub fn to_crlf(bytes: &[u8]) -> Vec<u8> {
    if bytes.contains(&0) {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 20);
    let mut prev = 0u8;
    for &b in bytes {
        if b == b'\n' && prev != b'\r' {
            out.push(b'\r');
        }
        out.push(b);
        prev = b;
    }
    out
}

pub struct TempRepo {
    pub dir: tempfile::TempDir,
}

impl TempRepo {
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Runs fairlead in the repository root with NO_COLOR set and FAIRLEAD_ROOT removed.
    pub fn run(&self, args: &[&str]) -> Run {
        self.run_in(self.path(), args, &[])
    }

    pub fn run_in(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
        let mut cmd = Command::new(fairlead_bin());
        cmd.current_dir(cwd)
            .args(args)
            .env("NO_COLOR", "1")
            .env_remove("FAIRLEAD_ROOT");
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = cmd.output().expect("spawn fairlead");
        Run {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    /// Runs git (fixtures only) with a fixed identity. Returns stdout.
    pub fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(self.path())
            .args(["-c", "user.name=fairlead-test", "-c", "user.email=fairlead-test@invalid"])
            .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    pub fn git_commit(&self, msg: &str) {
        self.git(&["commit", "-q", "--allow-empty", "-m", msg]);
    }

    /// Empty string means a clean working tree.
    pub fn git_status(&self) -> String {
        self.git(&["status", "--porcelain"])
    }

    /// Every file under the repository as forward-slash path to bytes,
    /// excluding `.fairlead/` (the disposable cache) and `.git/`.
    pub fn tree_snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        collect(self.path(), self.path(), &mut out);
        out
    }
}

fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("entry").path())
        .collect();
    entries.sort();
    for p in entries {
        let rel = p.strip_prefix(root).expect("under root");
        let key = rel.to_string_lossy().replace('\\', "/");
        if key == ".fairlead" || key == ".git" {
            continue;
        }
        if p.is_dir() {
            collect(root, &p, out);
        } else {
            out.insert(key, std::fs::read(&p).expect("read"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testkit__crlf_rewrites_text_only() {
        assert_eq!(to_crlf(b"a\nb\n"), b"a\r\nb\r\n".to_vec());
        assert_eq!(to_crlf(b"a\r\nb\n"), b"a\r\nb\r\n".to_vec());
        assert_eq!(to_crlf(b"\x00\x01\n"), b"\x00\x01\n".to_vec());
    }

    #[test]
    fn testkit__snapshot_excludes_cache_and_git() {
        let repo = RepoBuilder::new()
            .file("fairlead/control.toml", "x")
            .file(".fairlead/files.json", "{}")
            .build();
        let snap = repo.tree_snapshot();
        assert!(snap.contains_key("fairlead/control.toml"));
        assert!(!snap.keys().any(|k| k.starts_with(".fairlead/")));
    }
}
```

Create `tests/golden/.gitkeep` only if the directory does not yet exist (it is created by Task 4 anyway).

- [ ] **Step 5: Run the tests and expect PASS**

Run: `cargo test -p fairlead --test golden testkit`
Expected: `testkit_is_linked`, `testkit__crlf_rewrites_text_only`, `testkit__snapshot_excludes_cache_and_git` pass. The testkit module compiles because `golden_spec` is added in Task 3, so this step requires `tests/testkit/golden_spec.rs` to exist as an empty module (`//! Filled in by Task 3.` followed by nothing). Create that one-line file now.

- [ ] **Step 6: Commit**

```bash
git add crates/fairlead/Cargo.toml tests/golden.rs tests/testkit/mod.rs tests/testkit/golden_spec.rs
git commit -m "test(p9): testkit and fairlead test target declarations" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

Verify: `cargo test -p fairlead --test golden testkit` runs the three testkit tests.

## Task 3: Golden model, golden runner and golden-bless

One pure module describes a golden repository (`golden.toml`, copy, CRLF, git initialisation, edits, case execution). The test runner and the `golden-bless` xtask both include it by path, so a bless produces exactly the bytes the test compares.

**Files:**
- Create: `tests/testkit/golden_spec.rs` (replaces the one-line stub from Task 2)
- Modify: `tests/golden.rs` (runner body; the mandatory-set and dangling tests are added here, the bodies of the golden repositories come in Tasks 4 to 14)
- Create: `xtask/src/golden.rs`
- Modify: `xtask/Cargo.toml`, `xtask/src/main.rs` (add `golden-bless` dispatch)

**Interfaces:**

```rust
// tests/testkit/golden_spec.rs
pub struct GoldenFile { pub golden: Meta, pub cases: Vec<Case> }
pub struct Meta { pub name: String, pub platforms: Vec<String>, pub needs_git: bool, pub line_endings: String }
pub struct Case { pub args: Vec<String>, pub cwd: String, pub exit: i32, pub stdout: Option<String>,
                  pub git_status_clean: bool, pub edits: Vec<Edit>, pub env: BTreeMap<String, String> }
pub struct Edit { pub path: String, pub append: String }
pub fn load(dir: &Path) -> Result<GoldenFile, String>;
pub fn materialise(dir: &Path, meta: &Meta, dest: &Path) -> io::Result<()>;
pub fn apply_edit(root: &Path, e: &Edit) -> io::Result<()>;
pub fn run_case(bin: &Path, root: &Path, case: &Case) -> io::Result<CaseRun>;
pub struct CaseRun { pub code: i32, pub stdout: Vec<u8>, pub stderr: String }
pub fn git_status(root: &Path) -> io::Result<String>;
pub fn first_difference(want: &[u8], got: &[u8]) -> String;
pub fn crlf_bytes(bytes: &[u8]) -> Vec<u8>;
```

`golden.toml` extensions are exactly those of contract section 7: `[golden]` with `name`, `platforms`, `needs_git`, `line_endings`; `[[case]]` with `args`, `cwd` (default `"."`), `exit`, `stdout` (path relative to the golden directory), `git_status_clean` (default false); `[[case.edit]]` with `path` and `append` (the file is created when absent). `env` on a case is optional and defaults to empty.

- [ ] **Step 1: Write the failing unit tests for the model**

Create `tests/testkit/golden_spec.rs` with exactly this content (the functions are not defined yet):

```rust
//! Pure golden-repository model. Shared by tests/golden.rs (through testkit) and
//! by xtask golden-bless (through #[path]). Keep it free of other testkit items.
#![allow(dead_code)]

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN_TOML: &str = r#"
[golden]
name = "unit"
platforms = ["linux", "windows", "macos"]
needs_git = true
line_endings = "crlf"

[[case]]
args = ["check"]
exit = 0
git_status_clean = true

[[case.edit]]
path = "fairlead/x.txt"
append = "y\n"
"#;

    fn golden_dir(root: &Path) -> PathBuf {
        let dir = root.join("unit");
        std::fs::create_dir_all(dir.join("repo/fairlead")).expect("mkdir");
        std::fs::write(dir.join("golden.toml"), GOLDEN_TOML).expect("golden.toml");
        std::fs::write(dir.join("repo/a.txt"), "a\nb\n").expect("a.txt");
        std::fs::write(dir.join("repo/fairlead/control.toml"), "x\n").expect("control");
        dir
    }

    #[test]
    fn golden_spec__load_reads_cases_and_edits() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let g = load(&golden_dir(tmp.path())).expect("parses");
        assert_eq!(g.golden.name, "unit");
        assert_eq!(g.cases.len(), 1);
        assert_eq!(g.cases[0].cwd, ".");
        assert_eq!(g.cases[0].edits[0].path, "fairlead/x.txt");
    }

    #[test]
    fn golden_spec__materialise_applies_crlf_and_initialises_git() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = golden_dir(tmp.path());
        let g = load(&dir).expect("parses");
        let dest = tmp.path().join("copy");
        materialise(&dir, &g.golden, &dest).expect("materialise");
        assert_eq!(std::fs::read(dest.join("a.txt")).expect("read"), b"a\r\nb\r\n".to_vec());
        assert!(dest.join(".git").exists(), "needs_git must initialise a repository");
        assert_eq!(git_status(&dest).expect("status"), "", "fresh copy must be clean");
    }
}
```

- [ ] **Step 2: Run the tests and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_spec`
Expected: FAIL to compile with `cannot find function load in this scope` (and the same for `materialise` and `git_status`).

- [ ] **Step 3: Write the model implementation**

Insert the following above the `#[cfg(test)]` module in `tests/testkit/golden_spec.rs`:

```rust
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
pub struct GoldenFile {
    pub golden: Meta,
    #[serde(default, rename = "case")]
    pub cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub name: String,
    pub platforms: Vec<String>,
    pub needs_git: bool,
    pub line_endings: String,
}

#[derive(Debug, Deserialize)]
pub struct Case {
    pub args: Vec<String>,
    #[serde(default = "dot")]
    pub cwd: String,
    pub exit: i32,
    pub stdout: Option<String>,
    #[serde(default)]
    pub git_status_clean: bool,
    #[serde(default, rename = "edit")]
    pub edits: Vec<Edit>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct Edit {
    pub path: String,
    pub append: String,
}

pub struct CaseRun {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

fn dot() -> String {
    ".".to_string()
}

pub fn load(dir: &Path) -> Result<GoldenFile, String> {
    let text = std::fs::read_to_string(dir.join("golden.toml"))
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", dir.display()))
}

/// Copies `<dir>/repo` to `dest`. Text files become CRLF when `line_endings = "crlf"`.
/// `dot-git` is renamed to `.git`. When `needs_git` is set and no repository exists,
/// one is initialised and the tree is committed once.
pub fn materialise(dir: &Path, meta: &Meta, dest: &Path) -> io::Result<()> {
    copy_tree(&dir.join("repo"), dest, meta.line_endings == "crlf")?;
    let dot_git = dest.join("dot-git");
    if dot_git.is_dir() {
        std::fs::rename(&dot_git, dest.join(".git"))?;
    }
    if meta.needs_git && !dest.join(".git").exists() {
        git(dest, &["init", "-q"])?;
        git(dest, &["add", "-A"])?;
        git(dest, &["commit", "-q", "--allow-empty", "-m", "golden"])?;
    }
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path, crlf: bool) -> io::Result<()> {
    std::fs::create_dir_all(dst)?;
    let mut entries: Vec<std::fs::DirEntry> =
        std::fs::read_dir(src)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &to, crlf)?;
        } else {
            let bytes = std::fs::read(e.path())?;
            let data = if crlf { crlf_bytes(&bytes) } else { bytes };
            std::fs::write(&to, data)?;
        }
    }
    Ok(())
}

/// CRLF-converts text. Files with a NUL byte are binary and returned unchanged.
pub fn crlf_bytes(bytes: &[u8]) -> Vec<u8> {
    if bytes.contains(&0) {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 20);
    let mut prev = 0u8;
    for &b in bytes {
        if b == b'\n' && prev != b'\r' {
            out.push(b'\r');
        }
        out.push(b);
        prev = b;
    }
    out
}

fn git(dir: &Path, args: &[&str]) -> io::Result<()> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=fairlead-golden", "-c", "user.email=golden@invalid"])
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()))
    }
}

pub fn git_status(root: &Path) -> io::Result<String> {
    let out = Command::new("git").current_dir(root).args(["status", "--porcelain"]).output()?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Appends the edit's text, creating the file (and its directories) when absent.
pub fn apply_edit(root: &Path, e: &Edit) -> io::Result<()> {
    let p = root.join(&e.path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p)?;
    f.write_all(e.append.as_bytes())
}

/// Applies the case's edits, then runs `bin` in `root/cwd` with NO_COLOR set.
pub fn run_case(bin: &Path, root: &Path, case: &Case) -> io::Result<CaseRun> {
    for e in &case.edits {
        apply_edit(root, e)?;
    }
    let out = Command::new(bin)
        .current_dir(root.join(&case.cwd))
        .args(&case.args)
        .env("NO_COLOR", "1")
        .env_remove("FAIRLEAD_ROOT")
        .envs(&case.env)
        .output()?;
    Ok(CaseRun {
        code: out.status.code().unwrap_or(-1),
        stdout: out.stdout,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// The first line that differs, for a readable failure message.
pub fn first_difference(want: &[u8], got: &[u8]) -> String {
    let (w, g) = (String::from_utf8_lossy(want), String::from_utf8_lossy(got));
    for (i, (a, b)) in w.lines().zip(g.lines()).enumerate() {
        if a != b {
            return format!("line {}: expected {a:?}, got {b:?}", i + 1);
        }
    }
    format!("line count differs: expected {}, got {}", w.lines().count(), g.lines().count())
}
```

- [ ] **Step 4: Run the tests and expect PASS**

Run: `cargo test -p fairlead --test golden golden_spec`
Expected: `golden_spec__load_reads_cases_and_edits` and `golden_spec__materialise_applies_crlf_and_initialises_git` pass.

- [ ] **Step 5: Write the runner body (failing until goldens exist)**

Replace the body of `tests/golden.rs` (keep the two `#![allow]` and `#[path]` lines at the top) with:

```rust
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use testkit::golden_spec as gs;

/// R-ARCH-153 mandatory set (contract section 7). The directory name is the fixture name.
const MANDATORY: [&str; 11] = [
    "ts-react", "rust-workspace", "fullstack-tauri", "legacy-baseline",
    "crlf-and-windows-names", "nested-scopes-ovr", "unparseable-control", "zero-rules",
    "sparse-checkout", "legacy-encoding", "two-writers",
];

fn golden_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

fn golden_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(golden_root())
        .expect("tests/golden exists")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

#[test]
fn r_arch_153__mandatory_golden_set_is_complete_and_includes_typescript() {
    let names: BTreeSet<String> = golden_dirs()
        .iter()
        .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    for m in MANDATORY {
        assert!(names.contains(m), "missing mandatory golden repository {m}");
    }
    let ts = golden_root().join("ts-react/repo");
    let has_tsx = std::fs::read_dir(&ts)
        .expect("ts-react/repo exists")
        .any(|_| true);
    assert!(has_tsx, "ts-react must contain a TypeScript/React tree");
}

#[test]
fn r_chk_051__no_false_dangling_references() {
    for name in ["ts-react", "rust-workspace", "fullstack-tauri"] {
        let dir = golden_root().join(name).join("expected");
        for entry in std::fs::read_dir(&dir).expect("expected dir") {
            let path = entry.expect("entry").path();
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(
                !text.lines().any(|l| l.contains("HN-R001")),
                "{}: false dangling reference in a real-repository golden output",
                path.display()
            );
        }
    }
}

#[test]
fn golden_repositories_match_expected_bytes() {
    let bin = PathBuf::from(testkit::fairlead_bin());
    let mut failures: Vec<String> = Vec::new();
    for dir in golden_dirs() {
        let g = gs::load(&dir).expect("golden.toml parses");
        if !g.golden.platforms.iter().any(|p| p == std::env::consts::OS) {
            continue;
        }
        for (i, case) in g.cases.iter().enumerate() {
            let scratch = tempfile::tempdir().expect("tempdir");
            let root = scratch.path().join("repo");
            gs::materialise(&dir, &g.golden, &root).expect("materialise");
            let run = gs::run_case(&bin, &root, case).expect("run fairlead");
            let label = format!("{} case {} ({})", g.golden.name, i + 1, case.args.join(" "));
            if run.code != case.exit {
                failures.push(format!("{label}: exit {} expected {}\n{}", run.code, case.exit, run.stderr));
            }
            if let Some(rel) = &case.stdout {
                let path = dir.join(rel);
                match std::fs::read(&path) {
                    Err(_) => failures.push(format!("{label}: expected file {rel} is missing")),
                    Ok(want) if want != run.stdout => failures.push(format!(
                        "{label}: stdout differs at {}",
                        gs::first_difference(&want, &run.stdout)
                    )),
                    Ok(_) => {}
                }
            }
            if case.git_status_clean {
                let status = gs::git_status(&root).expect("git status");
                if !status.is_empty() {
                    failures.push(format!("{label}: git status not clean:\n{status}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "golden failures. Re-bless only after a human review: cargo xtask golden-bless <name>\n{}",
        failures.join("\n---\n")
    );
}
```

Note: `r_arch_153__mandatory_golden_set_is_complete_and_includes_typescript` fails until `tests/golden/*/` exists (Tasks 4 to 14). `r_chk_051__no_false_dangling_references` fails until the three real-repository expected directories exist. `golden_repositories_match_expected_bytes` fails until the cases in Tasks 4 to 14 are complete.

- [ ] **Step 6: Run and expect FAIL (goldens absent)**

Run: `cargo test -p fairlead --test golden`
Expected: FAIL with `tests/golden exists` or `missing mandatory golden repository ts-react`. This is the intended red state. Tasks 4 to 14 turn it green.

- [ ] **Step 7: Write the xtask golden-bless command**

Create `xtask/src/golden.rs`:

```rust
//! `cargo xtask golden-bless <name>`: regenerates the expected files of one golden
//! repository by running the same model the test uses. Human review is required before
//! committing the diff. Refuses to run when CI is set.

#[path = "../../tests/testkit/golden_spec.rs"]
mod golden_spec;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Pure refusal rule, unit-tested with a plain boolean.
pub fn refuse_if_ci(ci: bool) -> Result<(), String> {
    if ci {
        Err("golden-bless refuses to run in CI (CI is set); blessing is a local, reviewed act".into())
    } else {
        Ok(())
    }
}

pub fn bless(root: &Path, name: &str) -> Result<(), String> {
    refuse_if_ci(std::env::var_os("CI").is_some())?;
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["build", "-p", "fairlead", "--locked"])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("cargo build -p fairlead failed".into());
    }
    let exe = if cfg!(windows) { "fairlead.exe" } else { "fairlead" };
    let bin: PathBuf = root.join("target/debug").join(exe);
    let dir = root.join("tests/golden").join(name);
    let g = golden_spec::load(&dir)?;
    for case in &g.cases {
        let Some(rel) = &case.stdout else { continue };
        let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
        let repo = scratch.path().join("repo");
        golden_spec::materialise(&dir, &g.golden, &repo).map_err(|e| e.to_string())?;
        let run = golden_spec::run_case(&bin, &repo, case).map_err(|e| e.to_string())?;
        if run.code != case.exit {
            return Err(format!("{name}: exit {} expected {}; not blessed", run.code, case.exit));
        }
        std::fs::write(dir.join(rel), &run.stdout).map_err(|e| e.to_string())?;
        println!("blessed {name}/{rel}");
    }
    println!("review with: git diff tests/golden/{name}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xtask__golden_bless_refuses_in_ci() {
        assert!(refuse_if_ci(true).is_err());
        assert!(refuse_if_ci(false).is_ok());
    }
}
```

Add to `xtask/Cargo.toml` under `[dependencies]`:

```toml
serde = { version = "1.0.229", features = ["derive"] }
toml = { version = "1.1.7", default-features = false, features = ["std", "parse", "serde"] }
tempfile = "3.27.0"
```

In `xtask/src/main.rs`, add `mod golden;` and the dispatch arm `Some("golden-bless") => golden::bless(&root, args.next().as_deref().unwrap_or(""))`, where `root` is the workspace root (`Path::new(env!("CARGO_MANIFEST_DIR")).parent()`).

- [ ] **Step 8: Run the xtask test and expect PASS**

Run: `cargo test -p xtask xtask__golden_bless_refuses_in_ci`
Expected: pass.

- [ ] **Step 9: Commit**

```bash
git add tests/golden.rs tests/testkit/golden_spec.rs xtask/src/golden.rs xtask/src/main.rs xtask/Cargo.toml
git commit -m "test(p9): golden runner, shared golden model and golden-bless" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

Verify: `cargo test -p fairlead --test golden golden_spec` is green now. The golden-set tests stay red until Task 14.

## Golden repositories (Tasks 4 to 14)

Each task creates one directory under `tests/golden/<name>/` with `golden.toml`, `repo/` (the tree that is copied per run) and `expected/` (byte-compared stdout). Expected files are the spec-derived targets, taken from chapters 13 and 14 and the worked examples. The first green run of P8 is reviewed line by line before `cargo xtask golden-bless <name>` is run, and the review is recorded in the commit message. An expectation that the spec contradicts is a spec bug to fix in the contract, not a reason to change the binary silently (contract section 8, rule 2). Expected output avoids digests and fingerprints, which an author cannot compute by hand; those are covered by unit tests in P5 and P6.

Each golden task runs the same loop: write the case and its tree, run the golden test and expect FAIL with `expected file ... is missing`, write the expected file, run again and expect PASS, then commit.

## Task 4: Golden repository `ts-react` (R-ARCH-153, R-ROAD-015, E9)

**Files:**
- Create: `tests/golden/ts-react/golden.toml`
- Create: `tests/golden/ts-react/repo/` (eight files, below)
- Create: `tests/golden/ts-react/expected/check.terse.txt`

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/ts-react/golden.toml`:

```toml
[golden]
name = "ts-react"
platforms = ["linux", "windows", "macos"]
needs_git = true
line_endings = "lf"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 1
stdout = "expected/check.terse.txt"
git_status_clean = true
```

`tests/golden/ts-react/repo/.gitignore`:

```text
.fairlead/
```

`tests/golden/ts-react/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
purpose       = "Console and shared UI: shadcn primitives, Tailwind v4 tokens."
owners        = ["@ui-platform", "@console-ui", "@design"]

[kinds.tok]
title = "design token"
shape = "doc"

[kinds.cmp]
title = "component"
shape = "either"
impl  = 1

[policies]
"tok.color.primary" = "sealed"

[policies.waiver]
budget   = 25
approval = "required"

[rules.no-raw-color]
check    = "leak"
tokens   = ["tok.*"]
sev      = "error"
stage    = "enforce"
glob     = ["**/*.css", "**/*.tsx"]
waivable = true
why      = "colours come from tokens"
```

`tests/golden/ts-react/repo/fairlead/contract/tokens.md`:

```markdown
+++
title = "Token contract"
directives = [
  'def tok.color.primary value="#2563eb" applies=color,background-color sum="brand colour"',
]
+++

Values are declared here. Authored CSS stays the source of truth for styles (D-04).
```

`tests/golden/ts-react/repo/packages/ui/fairlead/control.toml`:

```toml
[fairlead]
scope = "ui"

[settings]
purpose = "Shared UI primitives."
```

`tests/golden/ts-react/repo/packages/ui/fairlead/docs/components.md`:

```markdown
+++
title = "Component catalogue"
directives = [
  'def cmp.button sum="primitive button wrapper"',
]
+++
```

`tests/golden/ts-react/repo/packages/ui/src/atoms/button.tsx`:

```tsx
// fairlead: def cmp.button
// fairlead: use tok.color.primary
export function Button(props: { label: string }) {
  return <button className="rounded-md">{props.label}</button>;
}
```

`tests/golden/ts-react/repo/apps/console/fairlead/control.toml`:

```toml
[fairlead]
scope = "console"

[settings]
purpose = "Admin console: app shell and navigation."
```

`tests/golden/ts-react/repo/apps/console/fairlead/docs/components.md`:

```markdown
+++
title = "Console components"
directives = [
  'def cmp.nav-group sum="navigation group"',
]
+++
```

`tests/golden/ts-react/repo/apps/console/src/molecules/nav-group.tsx` (line 4 column 32 is the `#` of the literal):

```tsx
// fairlead: def cmp.nav-group
// fairlead: ovr rule.no-raw-color why="legacy active tone" owner=@console-ui exp=2026-12-31
export function NavGroup() {
  return <div style={{ color: "#2563eb" }}>Nav</div>;
}
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `ts-react case 1 (...): expected file expected/check.terse.txt is missing`.

- [ ] **Step 3: Write the expected output**

`tests/golden/ts-react/expected/check.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=3 files=8 rules=1
errors 2 warn 0 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0
HN-X002 error apps/console/src/molecules/nav-group.tsx:2:1 ovr rule.no-raw-color has no approval; state=waiver-ineffective | fix: fairlead approve rule.no-raw-color
HN-T001 error apps/console/src/molecules/nav-group.tsx:4:32 literal #2563eb is tok.color.primary (color) | why: "colours come from tokens" | fix: use tok.color.primary
waivers: console 0 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`. The ts-react case passes; the failure list no longer mentions `ts-react`.

```bash
git add tests/golden/ts-react
git commit -m "test(p9): golden repository ts-react" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 5: Golden repository `rust-workspace` (R-SYN-025, R-CHK-060, R-CHK-025)

**Files:**
- Create: `tests/golden/rust-workspace/golden.toml`, `repo/**`, `expected/check.terse.txt` (same loop as Task 4)

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/rust-workspace/golden.toml`:

```toml
[golden]
name = "rust-workspace"
platforms = ["linux", "windows", "macos"]
needs_git = true
line_endings = "lf"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 1
stdout = "expected/check.terse.txt"
git_status_clean = true
```

`tests/golden/rust-workspace/repo/.gitignore`:

```text
.fairlead/
target/
```

`tests/golden/rust-workspace/repo/Cargo.toml` (read by fairlead for membership hints only):

```toml
[workspace]
members = ["crates/sync-core", "crates/sync-engines"]
resolver = "3"
```

`tests/golden/rust-workspace/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
purpose       = "sync-stack: Rust engines behind a Tauri shell."
owners        = ["@platform", "@core"]

[kinds.port]
title = "port"
shape = "code"

[kinds.fn]
title = "function"
shape = "code"

[policies]
"rules.*" = "sealed"

[policies.waiver]
budget   = 25
approval = "required"

[rules.primitive-io]
check    = "pattern"
forbid   = "std::(fs|io|net)::"
msg      = "I/O belongs in an adapter (rank 1)"
glob     = ["crates/sync-core/src/**"]
sev      = "error"
stage    = "enforce"
waivable = true
why      = "primitives stay free of I/O"
```

`tests/golden/rust-workspace/repo/crates/sync-core/fairlead/control.toml`:

```toml
[fairlead]
scope = "core"

[settings]
purpose = "Primitives: no I/O."
```

`tests/golden/rust-workspace/repo/crates/sync-core/fairlead/docs/ports.md`:

```markdown
+++
title = "Core ports"
directives = [
  'def port.progress-sink sum="progress reporting port"',
  'def fn.digest sum="stream digest over bytes"',
]
+++
```

`tests/golden/rust-workspace/repo/crates/sync-core/src/digest.rs` (the `std::io::` match is at line 3 column 35, the `std::fs::` match at line 4 column 5):

```rust
// fairlead: def fn.digest
// fairlead: ovr rule.primitive-io why="streams file bytes; adapter split deferred" owner=@core exp=2026-12-31
pub fn digest_file(path: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}
```

`tests/golden/rust-workspace/repo/crates/sync-engines/src/lib.rs` (a sibling use must be qualified, R-SYN-025):

```rust
// fairlead: use core:fn.digest
pub fn verify() {}
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `rust-workspace case 1 (...): expected file expected/check.terse.txt is missing`.

- [ ] **Step 3: Write the expected output**

`tests/golden/rust-workspace/expected/check.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=2 files=6 rules=1
errors 3 warn 0 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0
HN-X002 error crates/sync-core/src/digest.rs:2:1 ovr rule.primitive-io has no approval; state=waiver-ineffective | fix: fairlead approve rule.primitive-io
HN-U001 error crates/sync-core/src/digest.rs:3:35 rule.primitive-io: "I/O belongs in an adapter (rank 1)" | why: "primitives stay free of I/O" | fix: move the I/O into an adapter, or approve the ovr
HN-U001 error crates/sync-core/src/digest.rs:4:5 rule.primitive-io: "I/O belongs in an adapter (rank 1)" | why: "primitives stay free of I/O" | fix: move the I/O into an adapter, or approve the ovr
waivers: core 0 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/rust-workspace
git commit -m "test(p9): golden repository rust-workspace" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 6: Golden repository `fullstack-tauri` (R-CHK-057 negative control, R-SYN-045)

This is a green tree. Its purpose is the zero-finding baseline for a Tauri shape: a bridge declared in a doc, a command bound through an attribute line (R-SYN-045), and a web consumer.

**Files:**
- Create: `tests/golden/fullstack-tauri/golden.toml`, `repo/**`, `expected/check.terse.txt`

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/fullstack-tauri/golden.toml`:

```toml
[golden]
name = "fullstack-tauri"
platforms = ["linux", "windows", "macos"]
needs_git = true
line_endings = "lf"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 0
stdout = "expected/check.terse.txt"
git_status_clean = true
```

`tests/golden/fullstack-tauri/repo/.gitignore`:

```text
.fairlead/
target/
```

`tests/golden/fullstack-tauri/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
purpose       = "Notes app: React frontend, Tauri v2 backend."
owners        = ["@andy"]

[kinds.cmd]
title = "tauri command"
shape = "code"
```

`tests/golden/fullstack-tauri/repo/fairlead/decl/bridge.md`:

```markdown
+++
title = "Bridge declarations"
directives = [
  'def cmd.note-search sum="search notes by query"',
]
+++
```

`tests/golden/fullstack-tauri/repo/src/fairlead/control.toml`:

```toml
[fairlead]
scope = "web"

[settings]
purpose = "React frontend."
```

`tests/golden/fullstack-tauri/repo/src/api/notes.ts`:

```ts
// fairlead: use cmd.note-search
export async function noteSearch(query: string) {
  return invoke("note_search", { query });
}
```

`tests/golden/fullstack-tauri/repo/src-tauri/fairlead/control.toml`:

```toml
[fairlead]
scope = "tauri"

[settings]
purpose = "Tauri backend."
```

`tests/golden/fullstack-tauri/repo/src-tauri/Cargo.toml`:

```toml
[package]
name = "notes-app"
version = "0.1.0"
edition = "2024"
```

`tests/golden/fullstack-tauri/repo/src-tauri/src/lib.rs` (the attribute line is skipped when binding, R-SYN-045):

```rust
// fairlead: def cmd.note-search
#[tauri::command]
pub fn note_search(query: String) -> Vec<String> {
    vec![query]
}
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `fullstack-tauri case 1 (...): expected file expected/check.terse.txt is missing`.

- [ ] **Step 3: Write the expected output**

`tests/golden/fullstack-tauri/expected/check.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=3 files=7 rules=0
errors 0 warn 0 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0
waivers: root 0 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/fullstack-tauri
git commit -m "test(p9): golden repository fullstack-tauri" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 7: Golden repository `legacy-baseline` (R-CHK-030, R-CHK-032)

A legacy repository is adopted with a baseline and reaches green. The second case must run in the same copy as the first, so this golden uses the `reuse` flag. The runner extension below comes first in this task.

**Runner extension (done first in this task).** `golden.toml` gains an optional boolean `reuse` per case. A case with `reuse = true` runs in the copy that the previous case of the same golden repository left behind. Any other case gets a fresh copy. Three edits:

In `tests/testkit/golden_spec.rs`, add this field to `Case`, after `env`:

```rust
    #[serde(default)]
    pub reuse: bool,
```

In `tests/golden.rs`, the loop in `golden_repositories_match_expected_bytes` begins with `for (i, case) in g.cases.iter().enumerate() {`. Replace the three lines that create `scratch`, call `materialise`, and set `root` with:

```rust
        let mut scratch: Option<tempfile::TempDir> = None;
        for (i, case) in g.cases.iter().enumerate() {
            if !case.reuse || scratch.is_none() {
                let s = tempfile::tempdir().expect("tempdir");
                gs::materialise(&dir, &g.golden, &s.path().join("repo")).expect("materialise");
                scratch = Some(s);
            }
            let root = scratch.as_ref().expect("copy exists").path().join("repo");
```

Delete the original `for` line, since the replacement above contains it. The rest of the loop body is unchanged.

In `xtask/src/golden.rs`, apply the same rule to `bless`: keep one copy while `case.reuse` is set and materialise a fresh copy otherwise.

**Files:**
- Create: `tests/golden/legacy-baseline/golden.toml`, `repo/**`, `expected/check-after-baseline.terse.txt`

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/legacy-baseline/golden.toml`:

```toml
[golden]
name = "legacy-baseline"
platforms = ["linux", "windows", "macos"]
needs_git = false
line_endings = "lf"

[[case]]
args = ["sync", "--as-of", "2026-10-09", "--baseline", "--accept-new", "--why", "legacy adoption"]
exit = 0

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 0
stdout = "expected/check-after-baseline.terse.txt"
reuse = true
```

`tests/golden/legacy-baseline/repo/.gitignore`:

```text
.fairlead/
```

`tests/golden/legacy-baseline/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
owners        = ["@ui-platform"]

[kinds.tok]
title = "design token"
shape = "doc"

[rules.no-raw-color]
check    = "leak"
tokens   = ["tok.*"]
sev      = "error"
stage    = "enforce"
glob     = ["**/*.tsx"]
waivable = true
why      = "colours come from tokens"
```

`tests/golden/legacy-baseline/repo/fairlead/contract/tokens.md`:

```markdown
+++
title = "Token contract"
directives = [
  'def tok.color.primary value="#2563eb" applies=color,background-color sum="brand colour"',
]
+++
```

`tests/golden/legacy-baseline/repo/src/legacy.tsx`:

```tsx
// fairlead: use tok.color.primary
export const Badge = () => <span style={{ color: "#2563eb" }}>new</span>;
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `legacy-baseline case 2 (...): expected file expected/check-after-baseline.terse.txt is missing`.

- [ ] **Step 3: Write the expected output**

`tests/golden/legacy-baseline/expected/check-after-baseline.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=1 files=4 rules=1
errors 0 warn 0 info 0 | baselined 1 | waived 0 | observe 0 | accepted 1
waivers: root 0 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/legacy-baseline
git commit -m "test(p9): golden repository legacy-baseline" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 8: Golden repository `crlf-and-windows-names` (R-ARCH-111, R-CHK-012, R-FILE-024)

The tree is checked out with CRLF. The expected output is the LF golden, so the test proves that line endings do not change the output. The file names stay portable (Windows reserved names are tested through an ID, because a file cannot be created on Windows).

**Files:**
- Create: `tests/golden/crlf-and-windows-names/golden.toml`, `repo/**`, `expected/check.terse.txt`

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/crlf-and-windows-names/golden.toml`:

```toml
[golden]
name = "crlf-and-windows-names"
platforms = ["linux", "windows", "macos"]
needs_git = false
line_endings = "crlf"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 1
stdout = "expected/check.terse.txt"
```

`tests/golden/crlf-and-windows-names/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
owners        = ["@andy"]

[kinds.cmp]
title = "component"
shape = "either"
```

`tests/golden/crlf-and-windows-names/repo/fairlead/docs/names.md` (the ID `cmp.aux` uses a Windows reserved name, R-SYN-027):

```markdown
+++
title = "Names"
directives = [
  'def cmp.aux sum="reserved name probe"',
]
+++
```

`tests/golden/crlf-and-windows-names/repo/src/main.ts`:

```ts
// fairlead: use cmp.aux
export const x = 1;
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `crlf-and-windows-names case 1 (...): expected file expected/check.terse.txt is missing`.

- [ ] **Step 3: Write the expected output**

`tests/golden/crlf-and-windows-names/expected/check.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=1 files=3 rules=0
errors 1 warn 0 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0
HN-P001 error fairlead/docs/names.md:2:1 ID "cmp.aux" is a Windows reserved name | fix: rename the ID
waivers: root 0 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/crlf-and-windows-names
git commit -m "test(p9): golden repository crlf-and-windows-names" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 9: Golden repository `nested-scopes-ovr` (R-SCOPE-040, R-SYN-011, R-CHK-025)

Three behaviours in one tree: an override of an open key is effective with `why` alone, an override of a sealed key is inert (warning, and the escalation hint), and both are read from the nested scope.

**Files:**
- Create: `tests/golden/nested-scopes-ovr/golden.toml`, `repo/**`, `expected/check.terse.txt`

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/nested-scopes-ovr/golden.toml`:

```toml
[golden]
name = "nested-scopes-ovr"
platforms = ["linux", "windows", "macos"]
needs_git = false
line_endings = "lf"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 0
stdout = "expected/check.terse.txt"
```

`tests/golden/nested-scopes-ovr/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
owners        = ["@root", "@web"]

[kinds.tok]
title = "design token"
shape = "doc"

[policies]
"tok.color.primary"    = "sealed"
"tok.color.background" = "open"

[policies.waiver]
budget   = 25
approval = "required"
```

`tests/golden/nested-scopes-ovr/repo/fairlead/contract/tokens.md`:

```markdown
+++
title = "Token contract"
directives = [
  'def tok.color.primary value="#2563eb" applies=color sum="brand colour"',
  'def tok.color.background value="#ffffff" applies=background-color sum="page background"',
]
+++
```

`tests/golden/nested-scopes-ovr/repo/web/fairlead/control.toml`:

```toml
[fairlead]
scope = "web"

[settings]
purpose = "Web frontend scope."
```

`tests/golden/nested-scopes-ovr/repo/web/src/app.ts` (a sealed key with no grant: inert, line 1 column 1):

```ts
// fairlead: ovr tok.color.primary why="web brand" owner=@web exp=2026-12-31
export const brand = "#2563eb";
```

`tests/golden/nested-scopes-ovr/repo/web/src/open.ts` (an open key: effective with `why` alone, no finding):

```ts
// fairlead: ovr tok.color.background why="web surface" owner=@web
export const surface = "#ffffff";
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `nested-scopes-ovr case 1 (...): expected file expected/check.terse.txt is missing`.

- [ ] **Step 3: Write the expected output**

`tests/golden/nested-scopes-ovr/expected/check.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=2 files=5 rules=0
errors 0 warn 1 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0
HN-X005 warn web/src/app.ts:1:1 ovr tok.color.primary is inert: sealed key without an [[allow]] | fix: fairlead new escalation
waivers: web 1 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/nested-scopes-ovr
git commit -m "test(p9): golden repository nested-scopes-ovr" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 10: Golden repository `unparseable-control` (R-SYN-033, R-ARCH-011, fail closed)

A broken child control file fails closed: its subtree is unresolved, the run exits 2, and the sibling scope is still checked (edge case 15 of chapter 13).

**Files:**
- Create: `tests/golden/unparseable-control/golden.toml`, `repo/**`, `expected/check.terse.txt`

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/unparseable-control/golden.toml`:

```toml
[golden]
name = "unparseable-control"
platforms = ["linux", "windows", "macos"]
needs_git = false
line_endings = "lf"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 2
stdout = "expected/check.terse.txt"
```

`tests/golden/unparseable-control/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
owners        = ["@andy"]

[kinds.tok]
title = "design token"
shape = "doc"
```

`tests/golden/unparseable-control/repo/fairlead/contract/tokens.md`:

```markdown
+++
title = "Token contract"
directives = [
  'def tok.color.primary value="#2563eb" applies=color sum="brand colour"',
]
+++
```

`tests/golden/unparseable-control/repo/apps/web/fairlead/control.toml` (duplicate key on line 3, a TOML error):

```toml
[fairlead]
scope = "web"
scope = "web-again"
```

`tests/golden/unparseable-control/repo/apps/web/src/app.ts` (inside the unresolved subtree, not checked):

```ts
// fairlead: use tok.color.primary
export const app = 1;
```

`tests/golden/unparseable-control/repo/apps/admin/fairlead/control.toml`:

```toml
[fairlead]
scope = "admin"
```

`tests/golden/unparseable-control/repo/apps/admin/src/a.ts` (the sibling is still checked and resolves cleanly):

```ts
// fairlead: use tok.color.primary
export const a = 1;
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `unparseable-control case 1 (...): expected file expected/check.terse.txt is missing`.

- [ ] **Step 3: Write the expected output**

`tests/golden/unparseable-control/expected/check.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=3 files=6 rules=0
errors 1 warn 0 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0
HN-C001 error apps/web/fairlead/control.toml:3:1 control file does not parse: duplicate key `scope` | fix: fix the TOML syntax of apps/web/fairlead/control.toml
waivers: root 0 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/unparseable-control
git commit -m "test(p9): golden repository unparseable-control" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 11: Golden repository `zero-rules` (R-CHK-003, R-ARCH-012, HN-N001)

A repository with no declared rules and no IDs must never print a silent green. Exit 0 with the notice; `--require-declarations` turns the same tree into exit 1.

**Files:**
- Create: `tests/golden/zero-rules/golden.toml`, `repo/**`, `expected/check.terse.txt`

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/zero-rules/golden.toml`:

```toml
[golden]
name = "zero-rules"
platforms = ["linux", "windows", "macos"]
needs_git = false
line_endings = "lf"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 0
stdout = "expected/check.terse.txt"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse", "--require-declarations"]
exit = 1
```

`tests/golden/zero-rules/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
owners        = ["@andy"]
```

`tests/golden/zero-rules/repo/src/app.ts`:

```ts
export const app = 1;
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `zero-rules case 1 (...): expected file expected/check.terse.txt is missing`.

- [ ] **Step 3: Write the expected output**

`tests/golden/zero-rules/expected/check.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=1 files=2 rules=0
errors 0 warn 0 info 1 | baselined 0 | waived 0 | observe 0 | accepted 0
HN-N001 info 0 rules declared, 0 IDs: nothing verified
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/zero-rules
git commit -m "test(p9): golden repository zero-rules" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 12: Golden repository `sparse-checkout` (R-GIT-007, R-ARCH-026, R-CHK-057)

A scope whose managerial folder is absent from a sparse checkout is reported as unknown and makes the run non-zero. The index needs the skip-worktree bit, which files alone cannot express. The golden format therefore gains two extensions, and this task implements them first.

**Runner extension (done first in this task).** In `tests/testkit/golden_spec.rs`:

- `Edit` gets `#[serde(default)] pub append: String` (it was required in Task 3) and `#[serde(default)] pub remove: bool`. When `remove` is set the path (file or directory) is deleted instead of appended to.
- `Case` gets `#[serde(default)] pub git: Vec<Vec<String>>`, the git commands run in the copy before the edits.
- `run_case` first runs each `git` command with `git(root, &args)`, then applies the edits.

```rust
// inside Edit
    #[serde(default)]
    pub remove: bool,
    // append is now #[serde(default)]

// inside apply_edit, first line
    if e.remove {
        let p = root.join(&e.path);
        return if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
    }

// inside Case
    #[serde(default)]
    pub git: Vec<Vec<String>>,

// inside run_case, before the edit loop
    for g in &case.git {
        let args: Vec<&str> = g.iter().map(String::as_str).collect();
        git(root, &args)?;
    }
```

**Files:**
- Create: `tests/golden/sparse-checkout/golden.toml`, `repo/**`, `expected/check.terse.txt`

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/sparse-checkout/golden.toml`:

```toml
[golden]
name = "sparse-checkout"
platforms = ["linux", "windows", "macos"]
needs_git = true
line_endings = "lf"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 2
stdout = "expected/check.terse.txt"
git_status_clean = true
git = [
  ["config", "core.sparseCheckout", "true"],
  ["update-index", "--skip-worktree", "apps/console/fairlead/control.toml"],
  ["update-index", "--skip-worktree", "apps/console/fairlead/docs/p.md"],
  ["update-index", "--skip-worktree", "apps/console/src/app.ts"],
]

[[case.edit]]
path = "apps/console"
remove = true
```

`tests/golden/sparse-checkout/repo/.gitignore`:

```text
.fairlead/
```

`tests/golden/sparse-checkout/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
owners        = ["@andy"]

[kinds.cmp]
title = "component"
shape = "either"
```

`tests/golden/sparse-checkout/repo/src/root.ts` (the sibling reference is qualified, R-SYN-025):

```ts
// fairlead: use console:cmp.nav-group
export const root = 1;
```

`tests/golden/sparse-checkout/repo/apps/console/fairlead/control.toml`:

```toml
[fairlead]
scope = "console"
```

`tests/golden/sparse-checkout/repo/apps/console/fairlead/docs/p.md`:

```markdown
+++
title = "Console declarations"
directives = [
  'def cmp.nav-group sum="navigation group"',
]
+++
```

`tests/golden/sparse-checkout/repo/apps/console/src/app.ts`:

```ts
// fairlead: def cmp.nav-group
export const app = 1;
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `sparse-checkout case 1 (...): expected file expected/check.terse.txt is missing` (or a runner error if the extension is not yet applied).

- [ ] **Step 3: Write the expected output**

`tests/golden/sparse-checkout/expected/check.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=2 files=2 rules=0
errors 0 warn 1 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0
HN-V013 warn apps/console scope console is absent from the sparse checkout; verdict unknown | fix: check out apps/console with git sparse-checkout add apps/console
waivers: root 0 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/sparse-checkout
git commit -m "test(p9): golden repository sparse-checkout and golden extensions" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 13: Golden repository `legacy-encoding` (R-PORT-016, R-FILE-015, HN-I003)

A Windows-1250 file is skipped with a warning, listed, and never decoded by guess. The file is created with raw bytes by a shell step, because the plan cannot carry non-UTF-8 text.

**Files:**
- Create: `tests/golden/legacy-encoding/golden.toml`, `repo/**`, `expected/check.terse.txt`

- [ ] **Step 1: Write the golden declaration, the tree and the raw-byte file**

`tests/golden/legacy-encoding/golden.toml`:

```toml
[golden]
name = "legacy-encoding"
platforms = ["linux", "windows", "macos"]
needs_git = false
line_endings = "lf"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 0
stdout = "expected/check.terse.txt"
```

`tests/golden/legacy-encoding/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
owners        = ["@andy"]

[kinds.cmp]
title = "component"
shape = "either"
```

`tests/golden/legacy-encoding/repo/fairlead/docs/c.md`:

```markdown
+++
title = "Components"
directives = [
  'def cmp.legacy sum="component in a legacy file"',
]
+++
```

Create the Windows-1250 source with raw bytes (the bytes are the cp1250 letters for ć, ó and ł inside a string literal, so the file is not valid UTF-8). Run from the repository root:

```bash
mkdir -p tests/golden/legacy-encoding/repo/src
printf '// fairlead: def cmp.legacy\nconst s = "\xe6\xf3\xb3";\n' > tests/golden/legacy-encoding/repo/src/legacy.ts
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `legacy-encoding case 1 (...): expected file expected/check.terse.txt is missing`.

- [ ] **Step 3: Write the expected output**

`tests/golden/legacy-encoding/expected/check.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=1 files=2 rules=0
errors 0 warn 1 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0
HN-I003 warn src/legacy.ts:1:1 file is not valid UTF-8 and was skipped (ENCODING-SKIP) | fix: convert the file to UTF-8 or exclude it from [scan]
waivers: root 0 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/legacy-encoding
git commit -m "test(p9): golden repository legacy-encoding" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 14: Golden repository `two-writers` (R-ARCH-142, R-CLI-008, readers never block)

A live writer from another host holds `.fairlead/lock`. A second writer is refused with exit 4 and no stdout. A reader (`check`) is never blocked by the lock. The lock file format is fixed here as TOML with keys `pid`, `host` and `start`. Contract issue C-G3 records that this format must be adopted by P7.

**Files:**
- Create: `tests/golden/two-writers/golden.toml`, `repo/**`, `expected/sync-refused.txt`, `expected/check-with-lock.terse.txt`

- [ ] **Step 1: Write the golden declaration and the tree**

`tests/golden/two-writers/golden.toml`:

```toml
[golden]
name = "two-writers"
platforms = ["linux", "windows", "macos"]
needs_git = false
line_endings = "lf"

[[case]]
args = ["sync", "--as-of", "2026-10-09"]
exit = 4
stdout = "expected/sync-refused.txt"

[[case.edit]]
path = ".fairlead/lock"
append = "pid = 4242\nhost = \"other-machine\"\nstart = \"2026-10-09T08:00:00Z\"\n"

[[case]]
args = ["check", "--as-of", "2026-10-09", "--format", "terse"]
exit = 0
stdout = "expected/check-with-lock.terse.txt"

[[case.edit]]
path = ".fairlead/lock"
append = "pid = 4242\nhost = \"other-machine\"\nstart = \"2026-10-09T08:00:00Z\"\n"
```

`tests/golden/two-writers/repo/.gitignore`:

```text
.fairlead/
```

`tests/golden/two-writers/repo/fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
owners        = ["@andy"]

[kinds.cmp]
title = "component"
shape = "either"
```

`tests/golden/two-writers/repo/fairlead/docs/c.md`:

```markdown
+++
title = "Components"
directives = [
  'def cmp.a sum="component a"',
]
+++
```

`tests/golden/two-writers/repo/src/a.ts`:

```ts
// fairlead: def cmp.a
export const a = 1;
```

- [ ] **Step 2: Run the golden test and expect FAIL**

Run: `cargo test -p fairlead --test golden golden_repositories_match_expected_bytes`
Expected: FAIL, with `two-writers case 1 (...): expected file expected/sync-refused.txt is missing`.

- [ ] **Step 3: Write the expected outputs**

`tests/golden/two-writers/expected/sync-refused.txt` is an empty file (a refusal writes to stderr, so stdout is empty). Create it with `: > tests/golden/two-writers/expected/sync-refused.txt`.

`tests/golden/two-writers/expected/check-with-lock.terse.txt`:

```text
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=1 files=3 rules=0
errors 0 warn 0 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0
waivers: root 0 active | relaxations: none
```

- [ ] **Step 4: Run and expect PASS; commit**

```bash
git add tests/golden/two-writers
git commit -m "test(p9): golden repository two-writers" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 15: Determinism matrix (R-ARCH-071, R-ARCH-080, R-ARCH-111, R-CHK-012, R-ARCH-043)

The matrix runs `check` on two real golden repositories and compares each variant with the golden bytes. Variants: cold cache (`--no-cache`), warm, warm again, deleted cache, one thread and default threads, LF and CRLF checkouts, a BOM in one file, and a second working directory.

**Files:**
- Modify: `tests/testkit/mod.rs` (add three shared helpers)
- Create: `tests/determinism.rs`

**Interfaces:**

```rust
// tests/testkit/mod.rs (added)
pub fn golden_root() -> PathBuf;
pub fn golden_expected(name: &str) -> Vec<u8>;              // expected/check.terse.txt
pub fn golden_checkout(name: &str, crlf: bool) -> (tempfile::TempDir, PathBuf);
pub fn run_fairlead(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Run;
```

- [ ] **Step 1: Write the failing test**

Create `tests/determinism.rs` with exactly this content:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)]
//! Byte identity of `check` across cache states, thread counts, line endings and directories.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{golden_checkout, golden_expected, run_fairlead, CHECK_TERSE};

const GOLDENS: [&str; 2] = ["ts-react", "rust-workspace"];

fn stdout_of(cwd: &std::path::Path, extra: &[&str], env: &[(&str, &str)]) -> Vec<u8> {
    let args: Vec<&str> = CHECK_TERSE.iter().copied().chain(extra.iter().copied()).collect();
    run_fairlead(cwd, &args, env).stdout.into_bytes()
}

#[test]
fn r_arch_071__cold_warm_and_deleted_cache_are_byte_identical() {
    for name in GOLDENS {
        let (_s, root) = golden_checkout(name, false);
        let want = golden_expected(name);
        let cold = stdout_of(&root, &["--no-cache"], &[]);
        let warm = stdout_of(&root, &[], &[]);
        let warm_again = stdout_of(&root, &[], &[]);
        let _ = std::fs::remove_dir_all(root.join(".fairlead"));
        let deleted = stdout_of(&root, &[], &[]);
        for (label, got) in [("cold", cold), ("warm", warm), ("warm again", warm_again), ("deleted cache", deleted)] {
            assert_eq!(got, want, "{name}: {label} output differs from the golden");
        }
    }
}

#[test]
fn r_arch_080__one_thread_and_default_threads_are_byte_identical() {
    for name in GOLDENS {
        let (_s, root) = golden_checkout(name, false);
        let want = golden_expected(name);
        assert_eq!(stdout_of(&root, &["--threads", "1"], &[("RAYON_NUM_THREADS", "1")]), want, "{name}: one thread");
        assert_eq!(stdout_of(&root, &[], &[]), want, "{name}: default threads");
    }
}

#[test]
fn r_arch_111__lf_and_crlf_checkouts_are_byte_identical() {
    for name in GOLDENS {
        let (_s, root) = golden_checkout(name, true);
        assert_eq!(stdout_of(&root, &[], &[]), golden_expected(name), "{name}: CRLF checkout");
    }
}

#[test]
fn r_chk_012__bom_and_crlf_do_not_change_output() {
    let (_s, root) = golden_checkout("ts-react", true);
    let file = root.join("apps/console/src/molecules/nav-group.tsx");
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(std::fs::read(&file).expect("read"));
    std::fs::write(&file, bytes).expect("write BOM");
    assert_eq!(stdout_of(&root, &[], &[]), golden_expected("ts-react"), "BOM plus CRLF");
}

#[test]
fn r_arch_043__invocation_directory_does_not_change_output() {
    let (_s, root) = golden_checkout("ts-react", false);
    let nested = root.join("apps/console/src");
    assert_eq!(stdout_of(&nested, &[], &[]), golden_expected("ts-react"), "run from a subdirectory");
}
```

- [ ] **Step 2: Run the tests and expect FAIL**

Run: `cargo test -p fairlead --test determinism`
Expected: FAIL to compile with `cannot find function golden_checkout in module testkit`.

- [ ] **Step 3: Add the shared helpers**

Append to `tests/testkit/mod.rs` (outside the `tests` module):

```rust
use self::golden_spec as gs;

pub fn golden_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

pub fn golden_expected(name: &str) -> Vec<u8> {
    std::fs::read(golden_root().join(name).join("expected/check.terse.txt")).expect("expected output exists")
}

/// A fresh copy of a golden repository. The guard must stay alive while the copy is used.
pub fn golden_checkout(name: &str, crlf: bool) -> (tempfile::TempDir, PathBuf) {
    let dir = golden_root().join(name);
    let mut g = gs::load(&dir).expect("golden.toml parses");
    if crlf {
        g.golden.line_endings = "crlf".to_string();
    }
    let scratch = tempfile::tempdir().expect("tempdir");
    let root = scratch.path().join("repo");
    gs::materialise(&dir, &g.golden, &root).expect("materialise");
    (scratch, root)
}

/// Runs the built binary in `cwd` with NO_COLOR set and FAIRLEAD_ROOT removed.
pub fn run_fairlead(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
    let mut cmd = Command::new(fairlead_bin());
    cmd.current_dir(cwd).args(args).env("NO_COLOR", "1").env_remove("FAIRLEAD_ROOT");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn fairlead");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}
```

Note: `stdout_of` in `determinism.rs` converts through `String` (`run_fairlead(...).stdout.into_bytes()`), which is lossless for the golden text. The golden runner in Task 3 compares raw bytes. A non-UTF-8 stdout would differ, and no check output is non-UTF-8 (R-ARCH-052).

- [ ] **Step 4: Run and expect PASS**

Run: `cargo test -p fairlead --test determinism`
Expected: the five tests pass once P8 provides the binary. A failure names the golden, the variant and the first differing byte, which points either at a binary defect (owned by the phase that broke determinism, R-ARCH-071, 080, 111, 043) or at an expectation defect.

- [ ] **Step 5: Commit**

```bash
git add tests/determinism.rs tests/testkit/mod.rs
git commit -m "test(p9): determinism matrix over cache, threads, line endings and cwd" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 16: Read-only property (R-ARCH-140, R-CHK-015, R-GIT-001)

Three real repositories are copied into a git repository. `check`, `context` and `explain` run on each. After every command, `git status --porcelain` must be empty. The `.fairlead/` cache is ignored by each repository's `.gitignore` (R-ARCH-070).

**Files:**
- Create: `tests/readonly.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/readonly.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)]
//! Reader verbs never change a tracked file (R-ARCH-140, R-CHK-015, R-GIT-001).
#[path = "testkit/mod.rs"]
mod testkit;

use std::path::Path;
use testkit::golden_spec as gs;
use testkit::{golden_checkout, run_fairlead, CHECK_TERSE};

/// The reader invocations per real repository. Each command runs in the same checkout.
fn readers(name: &str) -> Vec<Vec<&'static str>> {
    let check: Vec<&str> = CHECK_TERSE.to_vec();
    match name {
        "ts-react" => vec![check, vec!["context", "apps/console/src/molecules/nav-group.tsx"], vec!["explain", "HN-T001"]],
        "rust-workspace" => vec![check, vec!["context", "crates/sync-core"], vec!["explain", "fn.digest"]],
        _ => vec![check, vec!["context", "src/api/notes.ts"]],
    }
}

fn assert_clean_after_readers(name: &str, root: &Path) {
    for args in readers(name) {
        let run = run_fairlead(root, &args, &[]);
        assert_ne!(run.code, 5, "{name} {args:?}: internal error: {}", run.stderr);
        let status = gs::git_status(root).expect("git status");
        assert!(status.is_empty(), "{name} {args:?} changed the working tree:\n{status}");
    }
}

#[test]
fn r_arch_140__reader_verbs_leave_git_status_clean() {
    for name in ["ts-react", "rust-workspace", "fullstack-tauri"] {
        let (_s, root) = golden_checkout(name, false);
        assert_clean_after_readers(name, &root);
    }
}

#[test]
fn r_chk_015__check_context_explain_leave_git_status_unchanged() {
    for name in ["ts-react", "rust-workspace", "fullstack-tauri"] {
        let (_s, root) = golden_checkout(name, false);
        assert_clean_after_readers(name, &root);
    }
}

#[test]
fn r_git_001__check_never_writes_a_tracked_file() {
    let (_s, root) = golden_checkout("ts-react", false);
    let run = run_fairlead(&root, CHECK_TERSE, &[]);
    assert_ne!(run.code, 5, "internal error: {}", run.stderr);
    assert_eq!(gs::git_status(&root).expect("git status"), "", "check wrote a tracked file");
}
```

- [ ] **Step 2: Run the test and expect FAIL**

Run: `cargo test -p fairlead --test readonly`
Expected: FAIL to compile until Task 15 adds the testkit helpers. After Task 15 is in place, the tests run and fail only if a reader writes a tracked file.

- [ ] **Step 3: Run and expect PASS**

Run: `cargo test -p fairlead --test readonly`
Expected: three tests pass. Tracked-file writes are blocked by R-ARCH-140 and gitignored `.fairlead/` is the only permitted write.

- [ ] **Step 4: Commit**

```bash
git add tests/readonly.rs
git commit -m "test(p9): read-only property for reader verbs" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 17: SARIF 2.1.0 validation (R-CHK-036, R-AGT-006, E5, R-ARCH-054)

Two checks. A structural test in `tests/sarif.rs` checks the mapping rules of R-CHK-036. The `sarif-validate` xtask runs the binary on real goldens and validates every output against the official 2.1.0 schema, which is committed under `fixtures/sarif/schema/`. Schema fetching is a one-time step that records the blake3 hash, so CI never needs network access.

**Files:**
- Create: `fixtures/sarif/schema/SOURCE.toml`, `fixtures/sarif/schema/sarif-schema-2.1.0.json` (fetched, step 1)
- Create: `tests/sarif.rs`, `xtask/src/sarif.rs`
- Modify: `xtask/Cargo.toml` (add `serde_json` and a JSON-schema validator, UNVERIFIED), `xtask/src/main.rs` (dispatch `sarif-validate`)

- [ ] **Step 1: Fetch the official schema and record its hash**

```bash
mkdir -p fixtures/sarif/schema
curl -fsSL https://docs.oasis-open.org/sarif/sarif/v2.1.0/os/schemas/sarif-schema-2.1.0.json -o fixtures/sarif/schema/sarif-schema-2.1.0.json
b3sum fixtures/sarif/schema/sarif-schema-2.1.0.json
```

Create `fixtures/sarif/schema/SOURCE.toml` with the URL, the retrieval date `2026-10-09`, and the printed hash in `blake3 = "..."`. Verify: the URL and the hash must be re-checked at each release. If `b3sum` is not installed, record the hash with `cargo xtask` once the blake3 crate is available to xtask (the same digest).

- [ ] **Step 2: Write the failing structural test**

Create `tests/sarif.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)]
//! SARIF mapping of R-CHK-036: ruleId, level, partialFingerprints.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{golden_checkout, run_fairlead};

#[test]
fn r_chk_036__sarif_results_map_code_level_and_fingerprint() {
    let (_s, root) = golden_checkout("ts-react", false);
    let run = run_fairlead(&root, &["check", "--as-of", "2026-10-09", "--format", "sarif"], &[]);
    assert_eq!(run.code, 1, "ts-react has enforce errors: {}", run.stderr);
    let doc: serde_json::Value = serde_json::from_str(&run.stdout).expect("SARIF is JSON");
    assert_eq!(doc["version"], "2.1.0");
    assert_eq!(doc["runs"][0]["tool"]["driver"]["name"], "fairlead");
    let results = doc["runs"][0]["results"].as_array().expect("results array");
    assert_eq!(results.len(), 2, "two findings in ts-react");
    for r in results {
        let rule = r["ruleId"].as_str().expect("ruleId");
        assert!(rule.starts_with("HN-"), "ruleId is the diagnostic code: {rule}");
        let level = r["level"].as_str().expect("level");
        assert!(matches!(level, "error" | "warning" | "note"), "level {level}");
        let fp = r["partialFingerprints"]["fairlead/v1"].as_str().expect("fingerprint key fairlead/v1");
        assert_eq!(fp.len(), 12, "12 base32 characters (R-CHK-031)");
    }
}
```

- [ ] **Step 3: Run the test and expect FAIL**

Run: `cargo test -p fairlead --test sarif`
Expected: FAIL until P5 renders SARIF, with the failure naming the first mismatched field.

- [ ] **Step 4: Write the schema validation xtask**

Create `xtask/src/sarif.rs`:

```rust
//! `cargo xtask sarif-validate`: runs the built binary on real goldens and validates
//! every SARIF output against the official 2.1.0 schema.

#[path = "../../tests/testkit/golden_spec.rs"]
mod golden_spec;

use std::path::Path;
use std::process::Command;

pub fn validate(root: &Path) -> Result<(), String> {
    let schema_path = root.join("fixtures/sarif/schema/sarif-schema-2.1.0.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&schema_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let validator = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
    let exe = if cfg!(windows) { "fairlead.exe" } else { "fairlead" };
    let bin = root.join("target/debug").join(exe);
    for name in ["ts-react", "rust-workspace", "fullstack-tauri", "zero-rules"] {
        let dir = root.join("tests/golden").join(name);
        let g = golden_spec::load(&dir)?;
        let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
        let repo = scratch.path().join("repo");
        golden_spec::materialise(&dir, &g.golden, &repo).map_err(|e| e.to_string())?;
        let out = Command::new(&bin)
            .current_dir(&repo)
            .args(["check", "--as-of", "2026-10-09", "--format", "sarif"])
            .output()
            .map_err(|e| e.to_string())?;
        let doc: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("{name}: {e}"))?;
        let errors: Vec<String> = validator.iter_errors(&doc).map(|e| e.to_string()).collect();
        if !errors.is_empty() {
            return Err(format!("{name}: SARIF invalid: {}", errors.join("; ")));
        }
        println!("sarif-validate: {name} ok");
    }
    Ok(())
}
```

Add `serde_json = "1.0.151"`, `jsonschema = "0.UNVERIFIED"` (the version is recorded after the maturity check under R-ARCH-120, Contract issues C-T5) and `tempfile = "3.27.0"` to `xtask/Cargo.toml`. Wire `Some("sarif-validate") => sarif::validate(&root)` in `xtask/src/main.rs` and add `mod sarif;`.

- [ ] **Step 5: Run and expect PASS; commit**

```bash
cargo build -p fairlead --locked
cargo run -p xtask -- sarif-validate
cargo test -p fairlead --test sarif
git add fixtures/sarif tests/sarif.rs xtask
git commit -m "test(p9): SARIF mapping test and official schema validation" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

Expected: four `sarif-validate: <name> ok` lines and one passing structural test.

## Task 18: Secrets, fifty seeded values (R-AGT-022, R-AGT-023, R-AGT-024, E5)

The seeds are generated at test time from three classes: private-key headers (10), provider key prefixes (20) and high-entropy strings (20). Each is written into a managerial document as the `sum` field, which the floor scan covers (R-AGT-022). No secret literal is committed (GitHub push protection would otherwise flag the repository).

**Files:**
- Create: `tests/secrets.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/secrets.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)]
//! Fifty seeded secret-like values are rejected by the floor scan and never printed (E5).
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{run_fairlead, RepoBuilder};

const PEM: [&str; 10] = ["RSA", "EC", "DSA", "OPENSSH", "ENCRYPTED", "PKCS8", "PGP", "SSH2", "ED25519", "DH"];
const PREFIXES: [&str; 20] = [
    "sk-", "ghp_", "gho_", "github_pat_", "AKIA", "ASIA", "xoxb-", "xoxp-", "glpat-", "sk_live_",
    "sk_test_", "rk_live_", "AIza", "npm_", "pypi-AgEI", "SG.", "hf_", "dop_v1_", "shpat_", "eyJhbGci",
];

/// A deterministic pseudo-random alphanumeric string (linear congruential generator).
fn entropy(seed: u64, len: usize) -> String {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ALPHA[((state >> 33) as usize) % ALPHA.len()] as char
        })
        .collect()
}

/// Fifty seeded values, index 0..50: PEM headers, provider prefixes, then entropy strings.
fn seeded() -> Vec<String> {
    (0..50)
        .map(|i| match i {
            0..10 => format!("-----BEGIN {} PRIVATE KEY----- {}", PEM[i], entropy(i as u64, 40)),
            10..30 => format!("{}{}", PREFIXES[i - 10], entropy(i as u64, 36)),
            _ => entropy(i as u64, 48),
        })
        .collect()
}

#[test]
fn r_agt_022__fifty_seeded_values_are_rejected_and_never_printed() {
    let values = seeded();
    assert_eq!(values.len(), 50);
    let mut builder = RepoBuilder::new().file("fairlead/control.toml", "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n[settings]\nfolder = \"fairlead\"\nfairlead-version = \"0.1.0\"\nowners = [\"@andy\"]\n");
    for (i, v) in values.iter().enumerate() {
        builder = builder.file(&format!("fairlead/docs/s{i:02}.md"), &format!("+++\ntitle = \"s{i:02}\"\nsum = \"{v}\"\n+++\n"));
    }
    let repo = builder.build();
    let run = run_fairlead(repo.path(), &["check", "--as-of", "2026-10-09", "--format", "terse", "--max", "0"], &[]);
    assert_eq!(run.code, 1, "floor findings are enforce errors: {}", run.stderr);
    let hits = run.stdout.lines().filter(|l| l.starts_with("HN-F001")).count();
    assert_eq!(hits, 50, "one HN-F001 per seeded value");
    for v in &values {
        assert!(!run.stdout.contains(v.as_str()), "a matched value was printed");
    }
}
```

- [ ] **Step 2: Run the test and expect FAIL**

Run: `cargo test -p fairlead --test secrets`
Expected: FAIL until P5 implements the floor scan. The assertion that names the count (`left: N, right: 50`) shows how many values were rejected.

- [ ] **Step 3: Run and expect PASS; commit**

Run: `cargo test -p fairlead --test secrets`. Expected: pass.

```bash
git add tests/secrets.rs
git commit -m "test(p9): fifty seeded secret-like values rejected and redacted" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

Verify: `cargo test -p fairlead --test secrets` is the verifying command for the E5 secrets row.

## Task 19: Replay harness on synthetic history (R-ROAD-004, R-ROAD-016, R-CHK-051 precision context)

CI replays a synthetic history that is generated inside the test: a base commit, then 50 commits that each introduce one hardcoded declared value, then 20 commits that add no literal. The same function runs on real repositories when `FAIRLEAD_REPLAY_LIST` names a file of commit identifiers (one per line, from the repository in the current working directory). Real-repository replays are not part of CI, and their results go to `docs/plans/0.1/results/`.

**Files:**
- Create: `tests/replay.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/replay.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)]
//! Recall and false positives over a history (R-ROAD-004). Thresholds match
//! docs/plans/0.1/ablation-protocol.md and are asserted there by xtask.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{run_fairlead, RepoBuilder, TempRepo};

const RECALL_MIN_PCT: usize = 70;
const FP_MAX_PCT: usize = 5;
const INTRODUCING: usize = 50;
const UNRELATED: usize = 20;

const ROOT_CONTROL: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n[settings]\nfolder = \"fairlead\"\nfairlead-version = \"0.1.0\"\nowners = [\"@andy\"]\n\n[kinds.tok]\ntitle = \"design token\"\nshape = \"doc\"\n\n[rules.no-raw-hex]\ncheck = \"leak\"\ntokens = [\"tok.*\"]\nsev = \"error\"\nstage = \"enforce\"\nglob = [\"**/*.tsx\"]\nwaivable = true\nwhy = \"colours come from tokens\"\n";
const TOKENS: &str = "+++\ntitle = \"Tokens\"\ndirectives = [\n  'def tok.color.primary value=\"#2563eb\" applies=color,background-color sum=\"brand\"',\n]\n+++\n";

fn t001_count(repo: &TempRepo) -> usize {
    let run = run_fairlead(repo.path(), &["check", "--as-of", "2026-10-09", "--format", "terse", "--max", "0"], &[]);
    run.stdout.lines().filter(|l| l.starts_with("HN-T001")).count()
}

#[test]
fn r_road_004__recall_and_false_positives_on_synthetic_history() {
    let repo = RepoBuilder::new()
        .file("fairlead/control.toml", ROOT_CONTROL)
        .file("fairlead/contract/tokens.md", TOKENS)
        .git_init(true)
        .build();
    std::fs::create_dir_all(repo.path().join("src")).expect("src");
    for i in 0..INTRODUCING {
        let body = format!("// fairlead: use tok.color.primary\nexport const c{i} = {{ color: \"#2563eb\" }};\n");
        std::fs::write(repo.path().join(format!("src/c{i:02}.tsx")), body).expect("write");
        repo.git(&["add", "-A"]);
        repo.git_commit(&format!("introduce literal {i}"));
    }
    for j in 0..UNRELATED {
        std::fs::write(repo.path().join(format!("src/u{j:02}.tsx")), format!("export const u{j} = {j};\n")).expect("write");
        repo.git(&["add", "-A"]);
        repo.git_commit(&format!("unrelated {j}"));
    }
    let shas: Vec<String> = repo.git(&["rev-list", "--reverse", "HEAD"]).lines().map(str::to_owned).collect();
    assert_eq!(shas.len(), 1 + INTRODUCING + UNRELATED, "root plus the two series");
    let mut counts = Vec::with_capacity(shas.len());
    for sha in &shas {
        repo.git(&["checkout", "-q", sha]);
        counts.push(t001_count(&repo));
    }
    let introduced = (1..=INTRODUCING).filter(|&k| counts[k] > counts[k - 1]).count();
    let false_pos = (INTRODUCING + 1..shas.len()).filter(|&k| counts[k] > counts[k - 1]).count();
    assert!(introduced * 100 >= INTRODUCING * RECALL_MIN_PCT, "recall {introduced}/{INTRODUCING} is below {RECALL_MIN_PCT}%");
    assert!(false_pos * 100 <= UNRELATED * FP_MAX_PCT, "{false_pos}/{UNRELATED} false positives exceed {FP_MAX_PCT}%");
}
```

- [ ] **Step 2: Run the test and expect FAIL**

Run: `cargo test -p fairlead --test replay`
Expected: FAIL until P5 emits the leak finding (`recall 0/50 is below 70%`), or FAIL to compile until Task 15's helpers exist.

- [ ] **Step 3: Run and expect PASS; commit**

Run: `cargo test -p fairlead --test replay`. Expected: pass with recall at least 35 of 50 and at most one false positive.

```bash
git add tests/replay.rs
git commit -m "test(p9): replay harness on synthetic history for recall and false positives" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

Real-repository replay: `FAIRLEAD_REPLAY_LIST=<file>` runs the same counting over the listed commits of the repository that holds the file. The owner runs it for G0 (R-ROAD-010) and records the output under `docs/plans/0.1/results/`. The step is manual and never in CI.

## Task 20: Platform behaviour on Windows and the other platforms (R-PORT-014, R-ARCH-110)

Long paths, a case-only collision, and an antivirus-latency measurement. The Windows CI job enables long-path support in the registry (`LongPathsEnabled = 1`) before this test runs. The measurement is reported and never gates.

**Files:**
- Create: `tests/platform.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/platform.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)]
//! Path and file behaviour that must hold on Windows, Linux and macOS (R-PORT-014).
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{run_fairlead, RepoBuilder};

const ROOT_CONTROL: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n[settings]\nfolder = \"fairlead\"\nfairlead-version = \"0.1.0\"\nowners = [\"@andy\"]\n";

#[test]
fn r_port_014__paths_over_260_characters_are_walked_with_forward_slashes() {
    let deep: String = (0..12).map(|i| format!("segment-{i:02}-abcdefghijklmnopqrstu")).collect::<Vec<_>>().join("/");
    assert!(deep.len() > 260, "the path must exceed 260 characters");
    let repo = RepoBuilder::new()
        .file("fairlead/control.toml", ROOT_CONTROL)
        .file(&format!("{deep}/note.ts"), "export const n = 1;\n")
        .build();
    let run = run_fairlead(repo.path(), &["check", "--as-of", "2026-10-09", "--format", "terse"], &[]);
    assert_eq!(run.code, 0, "a long path is not an error: {}", run.stderr);
    assert!(!run.stdout.contains("\\\\?\\"), "no extended-length prefix in output");
}

#[cfg(target_os = "linux")]
#[test]
fn r_port_014__case_only_collision_is_reported_as_path_hygiene() {
    let repo = RepoBuilder::new()
        .file("fairlead/control.toml", ROOT_CONTROL)
        .file("src/Button.tsx", "export const a = 1;\n")
        .file("src/button.tsx", "export const b = 2;\n")
        .build();
    let run = run_fairlead(repo.path(), &["check", "--as-of", "2026-10-09", "--format", "terse"], &[]);
    assert_eq!(run.code, 1, "HN-P001 is a floor error");
    assert!(run.stdout.lines().any(|l| l.starts_with("HN-P001")), "case-only collision is HN-P001");
}

#[test]
#[allow(clippy::disallowed_methods)] // measurement only; the timing never reaches output or gates
fn r_port_014__antivirus_latency_is_measured_and_reported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut micros: Vec<u128> = Vec::with_capacity(200);
    for i in 0..200 {
        let tmp = dir.path().join(format!("w{i}.tmp"));
        let to = dir.path().join(format!("w{i}.txt"));
        std::fs::write(&tmp, b"x").expect("write");
        let start = std::time::Instant::now();
        std::fs::rename(&tmp, &to).expect("rename");
        micros.push(start.elapsed().as_micros());
    }
    micros.sort_unstable();
    eprintln!("rename latency (us): p50={} p95={}", micros[100], micros[190]);
    assert_eq!(micros.len(), 200);
}
```

Locked files are covered by the golden `two-writers` (a live foreign writer is refused with exit 4 and readers continue) and by the bounded-retry rename tests of P7. Those tests are cited in the traceability table, so this file does not duplicate them.

- [ ] **Step 2: Run the test and expect FAIL**

Run: `cargo test -p fairlead --test platform`
Expected: FAIL until P3 and P5 handle the long path and the case collision. The latency test passes on its own.

- [ ] **Step 3: Run and expect PASS; commit**

Run: `cargo test -p fairlead --test platform -- --nocapture`. Expected: pass on all three CI platforms; the latency line appears in the log, and the Windows job records it in the release notes.

```bash
git add tests/platform.rs
git commit -m "test(p9): platform path, collision and latency checks" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 21: Every active diagnostic code has a snapshot (R-ARCH-151)

The list of active codes is read from `fairlead-core` (the same table the product uses), so a code added to the table without a snapshot fails here. P5 writes the snapshot files under `crates/fairlead-core/tests/snapshots/`, named `diagnostics__<CODE>__<case>.snap` through `insta::assert_snapshot!("<CODE>__<case>", ...)` in `crates/fairlead-core/tests/diagnostics.rs`.

**Files:**
- Create: `tests/codes.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/codes.rs`:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout)]
//! Every active diagnostic code has at least one snapshot (R-ARCH-151).
use std::path::Path;

#[test]
fn r_arch_151__every_active_code_has_a_snapshot() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/fairlead-core/tests/snapshots");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .expect("snapshot directory exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    let missing: Vec<&str> = fairlead_core::check::all_specs()
        .iter()
        .filter(|s| fairlead_core::check::is_active(s.code))
        .map(|s| s.code.0)
        .filter(|code| !names.iter().any(|n| n.contains(&format!("{code}__"))))
        .collect();
    assert!(missing.is_empty(), "active codes without a snapshot: {missing:?}");
}
```

- [ ] **Step 2: Run the test and expect FAIL**

Run: `cargo test -p fairlead --test codes`
Expected: FAIL with the list of active codes that have no snapshot yet (the full 0.1 set until P5 lands its snapshot file).

- [ ] **Step 3: Run and expect PASS; commit**

Run: `cargo test -p fairlead --test codes`. Expected: pass once P5 has committed one snapshot per active code. Inactive codes (later milestones) are excluded by `is_active`.

```bash
git add tests/codes.rs
git commit -m "test(p9): snapshot coverage for active diagnostic codes" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 22: Fuzz targets, seed corpus and `fuzz.yml` (R-ARCH-150, E5)

Four targets in the excluded `fuzz/` crate: the directive parser, the control-file loader, the frontmatter splitter and the comment lexer. Each runs for a bounded 60 seconds in CI. The seeds are small committed files in `fixtures/corpus/<target>/`, copied into `fuzz/corpus/<target>/` by the workflow.

**Files:**
- Create: `fuzz/Cargo.toml`, `fuzz/fuzz_targets/{directive,control_file,frontmatter,lexer}.rs`
- Create: `fixtures/corpus/directive/{a.rs,b.ts}`, `fixtures/corpus/control_file/{ok.toml,dup.toml}`, `fixtures/corpus/frontmatter/{doc.md,bad.md}`, `fixtures/corpus/lexer/{strings.rs,jsx.tsx}`
- Create: `.github/workflows/fuzz.yml`
- Modify: `.gitignore` (add `fuzz/corpus/`, `fuzz/artifacts/`)

- [ ] **Step 1: Write the fuzz crate manifest**

Create `fuzz/Cargo.toml`:

```toml
[package]
name = "fairlead-fuzz"
version = "0.0.0"
edition = "2024"
publish = false

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"          # Verify: maturity and licence under R-ARCH-120 before first use
fairlead-syntax = { path = "../crates/fairlead-syntax" }
fairlead-core = { path = "../crates/fairlead-core" }

[[bin]]
name = "directive"
path = "fuzz_targets/directive.rs"
test = false
doc = false
bench = false

[[bin]]
name = "control_file"
path = "fuzz_targets/control_file.rs"
test = false
doc = false
bench = false

[[bin]]
name = "frontmatter"
path = "fuzz_targets/frontmatter.rs"
test = false
doc = false
bench = false

[[bin]]
name = "lexer"
path = "fuzz_targets/lexer.rs"
test = false
doc = false
bench = false

[workspace]
```

The empty `[workspace]` table makes `fuzz/` its own workspace root, so the product workspace never sees it (contract section 1).

- [ ] **Step 2: Write the four targets**

Create `fuzz/fuzz_targets/directive.rs`:

```rust
#![no_main]
use fairlead_syntax::{extract, render_directive_body, CommentTable, ExtractConfig};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let cfg = ExtractConfig {
        marker: "fairlead".into(),
        comments: CommentTable::builtin(),
        header_skip: Vec::new(),
        max_file: 2_000_000,
        max_directives: 500,
    };
    let e = extract(data, "rs", &cfg);
    assert_eq!(e.candidates as usize, e.directives.len() + e.dropped as usize, "R-ARCH-031");
    for d in &e.directives {
        let body = render_directive_body(d);
        let again = extract(format!("// fairlead: {body}\n").as_bytes(), "rs", &cfg);
        assert!(
            again.directives.iter().any(|x| render_directive_body(x) == body),
            "parse-render-parse is stable: {body}"
        );
    }
});
```

Create `fuzz/fuzz_targets/control_file.rs`:

```rust
#![no_main]
use fairlead_core::config::{parse_control, parse_header_lenient};
use fairlead_core::path::RepoPath;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(path) = RepoPath::new("fairlead/control.toml") {
        let _ = parse_control(&path, data, true);
        let _ = parse_control(&path, data, false);
    }
    let _ = parse_header_lenient(data);
});
```

Create `fuzz/fuzz_targets/frontmatter.rs`:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Some((front, body)) = fairlead_syntax::frontmatter::split_frontmatter(&text) {
        assert!(front.end <= text.len() && body.end <= text.len(), "ranges stay inside the text");
    }
});
```

Create `fuzz/fuzz_targets/lexer.rs`:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut last = 0usize;
    for span in fairlead_syntax::lexer::comment_spans(&text, "rs") {
        assert!(span.start >= last && span.start <= span.end && span.end <= text.len(), "spans are ordered and in bounds");
        last = span.end;
    }
});
```

`lexer::comment_spans(text: &str, ext: &str) -> Vec<Range<usize>>` is not in contract section 4. Contract issue C-T1 asks P2 to expose it for fuzzing. The target does not compile until P2 does.

- [ ] **Step 3: Write the seed corpus**

Create `fixtures/corpus/directive/a.rs`:

```rust
// fairlead: def cmp.button layer=ui.atom
// fairlead: ovr rule.no-raw-hex why="legacy" owner=@andy exp=2026-12-31
fn main() {}
```

Create `fixtures/corpus/directive/b.ts`:

```ts
/* fairlead: def fn.load */
export function load(): number { return 1; }
```

Create `fixtures/corpus/control_file/ok.toml`:

```toml
[fairlead]
edition = 1
root = true
scope = "root"
marker = "fairlead"
```

Create `fixtures/corpus/control_file/dup.toml`:

```toml
[fairlead]
scope = "a"
scope = "b"
```

Create `fixtures/corpus/frontmatter/doc.md`:

```markdown
+++
title = "Doc"
directives = ['def cmp.a sum="x"']
+++

Body.
```

Create `fixtures/corpus/frontmatter/bad.md`:

```markdown
+++
directives = "not an array"
+++
```

Create `fixtures/corpus/lexer/strings.rs`:

```rust
let s = "// fairlead: def fake"; // fairlead: def real
let r = r#"/* fairlead: x */"#;
```

Create `fixtures/corpus/lexer/jsx.tsx`:

```tsx
export const A = () => <div>{/* fairlead: def cmp.a */}</div>;
```

Note: `fixtures/**` is excluded from the repository's own `check` (Task 24), and the seeds contain no secret-like values.

- [ ] **Step 4: Write `fuzz.yml`**

Create `.github/workflows/fuzz.yml`:

```yaml
name: fuzz

on:
  push:
    branches: [main]
  pull_request:
  schedule:
    - cron: "17 3 * * 1"
  workflow_dispatch:

permissions:
  contents: read

jobs:
  fuzz:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        target: [directive, control_file, frontmatter, lexer]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - uses: Swatinem/rust-cache@v2
        with:
          workspace: fuzz -> target
      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz --locked
      - name: Copy seeds
        run: |
          mkdir -p "fuzz/corpus/${{ matrix.target }}"
          cp -r "fixtures/corpus/${{ matrix.target }}/." "fuzz/corpus/${{ matrix.target }}/"
      - name: Run for 60 seconds (R-ARCH-150)
        run: cargo fuzz run "${{ matrix.target }}" --fuzz-dir fuzz -- -max_total_time=60
      - name: Keep crashing inputs
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: fuzz-artifacts-${{ matrix.target }}
          path: fuzz/artifacts/
          if-no-files-found: ignore
```

The job uses nightly only for the fuzzer. The product stays on stable and MSRV (R-REL-006).

- [ ] **Step 5: Run and expect PASS; commit**

Run the four targets for a short bounded time on a developer machine: `cargo fuzz run lexer --fuzz-dir fuzz -- -max_total_time=10` (repeat for each target). Expected: no crash. A crash is a P2 or P3 bug, reproduced from `fuzz/artifacts/` and fixed with a regression fixture (R-ARCH-152).

```bash
git add fuzz fixtures/corpus .github/workflows/fuzz.yml .gitignore
git commit -m "test(p9): fuzz targets, seed corpus and bounded fuzz workflow" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 23: Benchmark corpus, criterion benches and `bench.yml` (R-ARCH-130, R-ARCH-131, R-CHK-016, R-CTX-072, R-ROAD-017)

`cargo xtask bench-corpus generate` writes a deterministic 100,000-file tree. The workflow measures cold, warm, `--changed` and `--files-from` runs and peak memory on Linux and Windows. `cargo xtask bench-corpus check` gates the E6 limits (from the committed protocol constants) and a 25% regression threshold against `bench/baseline.toml`. The bytes-per-token ratio of R-CTX-072 is a separate, manual measurement (see the end of this task).

**Files:**
- Create: `xtask/src/bench.rs`, `crates/fairlead-core/benches/extract.rs`, `crates/fairlead-core/benches/digest.rs`, `bench/baseline.toml`, `.github/workflows/bench.yml`
- Modify: `crates/fairlead-core/Cargo.toml` (dev-dependency `criterion`, two `[[bench]]` entries), `xtask/src/main.rs` (dispatch `bench-corpus`)

- [ ] **Step 1: Write the failing gate test**

Create `xtask/src/bench.rs` with the constant test only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_arch_130__gate_rejects_a_slow_cold_check() {
        let slow = "cold_s = 61.0\nwarm_s = 1.0\nchanged_s = 0.1\nfiles_from_s = 0.5\npeak_rss_mb = 100.0\n";
        assert!(check(slow, "").is_err());
        let fast = "cold_s = 30.0\nwarm_s = 1.0\nchanged_s = 0.1\nfiles_from_s = 0.5\npeak_rss_mb = 100.0\n";
        assert!(check(fast, "").is_ok());
    }
}
```

- [ ] **Step 2: Run the test and expect FAIL**

Run: `cargo test -p xtask r_arch_130`
Expected: FAIL to compile (`cannot find function check`).

- [ ] **Step 3: Write the generator and the gate**

Replace `xtask/src/bench.rs` with this implementation and keep the test module above it:

```rust
//! `bench-corpus generate` writes a deterministic corpus. `bench-corpus check` gates the
//! E6 limits (from ablation.rs, which the protocol test binds) and a 25% regression.

use crate::ablation::{E6_CHANGED_MAX_S, E6_COLD_CHECK_MAX_S, E6_PEAK_RSS_MAX_MB, E6_WARM_CHECK_MAX_S};
use std::path::Path;

const ROOT: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n[settings]\nfolder = \"fairlead\"\nfairlead-version = \"0.1.0\"\nowners = [\"@andy\"]\n\n[kinds.tok]\ntitle = \"design token\"\nshape = \"doc\"\n";
const TOKENS: &str = "+++\ntitle = \"Tokens\"\ndirectives = [\n  'def tok.color.primary value=\"#2563eb\" applies=color sum=\"brand\"',\n]\n+++\n";

pub fn generate(out: &Path, files: usize) -> std::io::Result<()> {
    std::fs::create_dir_all(out.join("fairlead/contract"))?;
    std::fs::write(out.join("fairlead/control.toml"), ROOT)?;
    std::fs::write(out.join("fairlead/contract/tokens.md"), TOKENS)?;
    for i in 0..files {
        let path = out.join(format!("pkg-{:04}/src/f-{:03}.ts", i / 100, i % 100));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = if i % 50 == 0 {
            "// fairlead: use tok.color.primary\nexport const v = 1;\n"
        } else {
            "export const v = 1;\n"
        };
        std::fs::write(path, body)?;
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct Measured { cold_s: f64, warm_s: f64, changed_s: f64, files_from_s: f64, peak_rss_mb: f64 }

#[derive(serde::Deserialize, Default)]
struct Baseline {
    cold_s: Option<f64>,
    warm_s: Option<f64>,
    changed_s: Option<f64>,
    peak_rss_mb: Option<f64>,
}

pub fn check(results: &str, baseline: &str) -> Result<(), String> {
    let m: Measured = toml::from_str(results).map_err(|e| e.to_string())?;
    let b: Baseline = toml::from_str(baseline).map_err(|e| e.to_string())?;
    let gates = [
        ("cold check", m.cold_s, E6_COLD_CHECK_MAX_S as f64, b.cold_s),
        ("warm check", m.warm_s, E6_WARM_CHECK_MAX_S as f64, b.warm_s),
        ("--changed", m.changed_s, E6_CHANGED_MAX_S as f64, b.changed_s),
        ("peak RSS MB", m.peak_rss_mb, E6_PEAK_RSS_MAX_MB as f64, b.peak_rss_mb),
    ];
    let mut failures = Vec::new();
    for (name, got, max, base) in gates {
        if got > max {
            failures.push(format!("{name}: {got} exceeds the E6 limit {max}"));
        }
        if let Some(base) = base {
            if got > base * 1.25 {
                failures.push(format!("{name}: {got} is more than 25% above the baseline {base}"));
            }
        }
    }
    if m.files_from_s > 1.0 {
        println!("report only (R-CHK-016 goal): --files-from took {} s", m.files_from_s);
    }
    if failures.is_empty() { Ok(()) } else { Err(failures.join("\n")) }
}
```

Wire `Some("bench-corpus")` in `xtask/src/main.rs`: `generate --files N --out DIR` calls `bench::generate`, and `check --results FILE --baseline bench/baseline.toml` reads both files and calls `bench::check`, exiting 1 on error. Add `mod bench;`.

`bench/baseline.toml` is created empty of metrics, so the first run gates only on the absolute limits:

```toml
# Metrics are recorded here after the first measured run on main (R-ARCH-130).
# Until then only the absolute E6 limits apply.
```

- [ ] **Step 4: Run and expect PASS**

Run: `cargo test -p xtask r_arch_130`. Expected: pass.

- [ ] **Step 5: Write the criterion benches**

Create `crates/fairlead-core/benches/extract.rs`:

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use fairlead_syntax::{extract, CommentTable, ExtractConfig};

fn bench(c: &mut Criterion) {
    let cfg = ExtractConfig {
        marker: "fairlead".into(),
        comments: CommentTable::builtin(),
        header_skip: Vec::new(),
        max_file: 2_000_000,
        max_directives: 500,
    };
    let text = "export const v = 1;\n".repeat(500) + "// fairlead: use tok.color.primary\n";
    c.bench_function("extract_500_lines", |b| b.iter(|| extract(text.as_bytes(), "ts", &cfg)));
}

criterion_group!(benches, bench);
criterion_main!(benches);
```

Create `crates/fairlead-core/benches/digest.rs`:

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use fairlead_core::context::effective_digest;
use fairlead_core::hash::ContentHash;

fn bench(c: &mut Criterion) {
    let own = ContentHash([7u8; 32]);
    c.bench_function("merkle_digest_100k", |b| {
        b.iter(|| {
            let mut parent = None;
            for _ in 0..100_000 {
                parent = Some(effective_digest(parent.as_ref(), &own, "0.1.0"));
            }
            parent
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
```

Add to `crates/fairlead-core/Cargo.toml`:

```toml
[dev-dependencies]
criterion = "0.5"     # Verify: maturity, licence and MSRV 1.88 under R-ARCH-120 before adding

[[bench]]
name = "extract"
harness = false

[[bench]]
name = "digest"
harness = false
```

- [ ] **Step 6: Write `bench.yml`**

Create `.github/workflows/bench.yml`:

```yaml
name: bench

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read

jobs:
  bench:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Build release binary
        run: cargo build --release -p fairlead --locked
      - name: Generate the 100,000-file corpus
        run: cargo run -p xtask --locked -- bench-corpus generate --files 100000 --out target/bench/corpus
      - name: Measure (Linux)
        if: runner.os == 'Linux'
        shell: bash
        run: |
          H=target/release/fairlead; C=target/bench/corpus; mkdir -p target/bench
          git -C "$C" init -q && git -C "$C" add -A && git -C "$C" -c user.name=bench -c user.email=bench@invalid commit -qm base
          printf 'pkg-0001/src/f-001.ts\n' > target/bench/files.txt
          t() { local s e; s=$(date +%s%N); "$@" >/dev/null 2>&1; e=$(date +%s%N); echo "scale=3; ($e-$s)/1000000000" | bc; }
          cold=$(t $H -C "$C" check --as-of 2026-10-09 --format terse --no-cache)
          warm=$(t $H -C "$C" check --as-of 2026-10-09 --format terse)
          echo "// touched" >> "$C/pkg-0000/src/f-000.ts"; git -C "$C" -c user.name=bench -c user.email=bench@invalid commit -qam change
          changed=$(t $H -C "$C" check --as-of 2026-10-09 --format terse --changed)
          ff=$(t $H -C "$C" check --as-of 2026-10-09 --format terse --files-from target/bench/files.txt)
          rss=$(/usr/bin/time -f "%M" $H -C "$C" check --as-of 2026-10-09 --format terse --no-cache 2>&1 >/dev/null | tail -1)
          printf 'cold_s = %s\nwarm_s = %s\nchanged_s = %s\nfiles_from_s = %s\npeak_rss_mb = %s\n' "$cold" "$warm" "$changed" "$ff" "$((rss / 1024))" > target/bench/results.toml
      - name: Measure (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $h = "target\release\fairlead.exe"; $c = "target\bench\corpus"
          New-Item -ItemType Directory -Force target\bench | Out-Null
          git -C $c init -q; git -C $c add -A; git -C $c -c user.name=bench -c user.email=bench@invalid commit -qm base
          Set-Content target\bench\files.txt "pkg-0001/src/f-001.ts"
          function T([scriptblock]$b) { (Measure-Command $b).TotalSeconds }
          $cold = T { & $h -C $c check --as-of 2026-10-09 --format terse --no-cache | Out-Null }
          $warm = T { & $h -C $c check --as-of 2026-10-09 --format terse | Out-Null }
          Add-Content "$c\pkg-0000\src\f-000.ts" "// touched"; git -C $c -c user.name=bench -c user.email=bench@invalid commit -qam change
          $changed = T { & $h -C $c check --as-of 2026-10-09 --format terse --changed | Out-Null }
          $ff = T { & $h -C $c check --as-of 2026-10-09 --format terse --files-from target\bench\files.txt | Out-Null }
          $p = Start-Process -FilePath $h -ArgumentList @('-C', $c, 'check', '--as-of', '2026-10-09', '--format', 'terse', '--no-cache') -NoNewWindow -PassThru -RedirectStandardOutput NUL
          $p.WaitForExit()
          $rss = [math]::Round($p.PeakWorkingSet64 / 1MB)
          "cold_s = $cold`nwarm_s = $warm`nchanged_s = $changed`nfiles_from_s = $ff`npeak_rss_mb = $rss" | Set-Content -Encoding ascii target\bench\results.toml
      - name: Gate E6 limits and regression
        run: cargo run -p xtask --locked -- bench-corpus check --results target/bench/results.toml --baseline bench/baseline.toml
      - name: Keep results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: bench-${{ matrix.os }}
          path: target/bench/results.toml
          if-no-files-found: ignore
```

Verify: GitHub hosted runners are slower than the E6 reference machine (Windows 11, Defender on). A failing gate on a hosted runner is a finding to record, not a reason to relax the limit. The owner runs the measurement on the reference machine for the E6 record.

- [ ] **Step 7: Bytes-per-token and digest records (R-CTX-072, R-ROAD-017)**

The ratio is measured once per release, outside CI, because no offline Claude tokenizer exists (architecture section 11). Run `cargo xtask bench-corpus generate --files 2000 --out target/bench/sample`, sample 20 `context` outputs with `fairlead context <path> > sample-N.txt`, count tokens of each sample with the target model's counting endpoint (owner-run, the key never enters the repository), and record the total bytes divided by the total tokens in 60-roadmap section 11. The `digest` criterion bench gives the Merkle cost at 100,000 entries.

- [ ] **Step 8: Commit**

```bash
git add xtask/src/bench.rs crates/fairlead-core/benches crates/fairlead-core/Cargo.toml bench/baseline.toml .github/workflows/bench.yml xtask/src/main.rs
git commit -m "bench(p9): corpus generator, E6 gate, criterion benches and bench workflow" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 24: Dogfood declarations, `dogfood.yml` and the docs check (R-ARCH-160, R-ARCH-161, R-ARCH-162, R-AGT-005, R-AGT-006, E7, E10)

The repository carries its own managerial folder. A root `[rules]` set covers the clock and network-crate rules, and two nested scopes (`core`, `syntax`) exist. R-ARCH-162 needs a sealed key and a real `ovr`: the sealed key is `rules.*` (sealed by default), and the real `ovr` waives the one clock read of `time.rs` (R-CHK-014). Approval of that waiver is a human act (R-AGT-026): the agent writes the `ovr` line, and the owner runs `fairlead approve` in a terminal and commits the approval record with the `ovr`.

**Files:**
- Create: `fairlead/control.toml`, `fairlead/approvals.toml`, `crates/fairlead-core/fairlead/control.toml`, `crates/fairlead-syntax/fairlead/control.toml`
- Modify: `crates/fairlead-core/src/time.rs` (one `ovr` line, after step 3)
- Create: `xtask/src/docs.rs` (docs check, with its tests), `.github/workflows/dogfood.yml`
- Modify: `xtask/src/main.rs` (dispatch `docs-check`)

- [ ] **Step 1: Write the dogfood declarations**

`fairlead/control.toml`:

```toml
[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1.0"
purpose       = "fairlead's own declarations: the rules this repository is checked against."
owners        = ["@andy"]

[scan]
exclude = ["target/**", "fixtures/**", "tests/**", "fuzz/**", "bench/**", "ablation/**", "docs/plans/**"]

[policies]
"rules.*" = "sealed"

[policies.waiver]
budget   = 25
approval = "required"

[rules.no-clock]
check    = "pattern"
forbid   = "(SystemTime|Instant)::now\\("
msg      = "clock reads belong in time.rs or check/options.rs (R-ARCH-090)"
glob     = ["crates/**/*.rs"]
exclude  = ["crates/**/tests/**", "crates/fairlead-core/src/check/options.rs"]
sev      = "error"
stage    = "enforce"
waivable = true
why      = "the product reads the clock once"

[rules.no-net-crates]
check  = "pattern"
forbid = "^(reqwest|ureq|hyper|openssl|rustls|native-tls)\\s*="
msg    = "no HTTP or TLS client in 0.1 (R-ARCH-123)"
glob   = ["crates/*/Cargo.toml"]
sev    = "error"
stage  = "enforce"
why    = "the 0.1 binary links no network stack"
```

`fairlead/approvals.toml` (written by `fairlead approve` in step 4; this is its initial content):

```toml
# fairlead/approvals.toml: [[approval]] records written by `fairlead approve` (D-34).
# Human-only: reviewed through CODEOWNERS on fairlead/.
```

`crates/fairlead-core/fairlead/control.toml` (a child: scope and purpose only, C-4):

```toml
[fairlead]
scope = "core"

[settings]
purpose = "Engine: resolve, check, context. No I/O in primitives."
```

`crates/fairlead-syntax/fairlead/control.toml`:

```toml
[fairlead]
scope = "syntax"

[settings]
purpose = "Grammar and extraction. Pure: no I/O, no git, no threads."
```

- [ ] **Step 2: Run the dogfood check and expect FAIL until the waiver exists**

Run: `cargo run -p fairlead -- check --as-of 2026-10-09 --format terse --deny-warnings`
Expected: FAIL with exit 1, because `crates/fairlead-core/src/time.rs` reads `SystemTime::now` and no waiver exists. This is the intended state; the waiver is added in step 3.

- [ ] **Step 3: Add the waiver (agent step)**

Add this line immediately above the clock read in `crates/fairlead-core/src/time.rs`, so it binds to the function that reads the clock:

```rust
// fairlead: ovr rule.no-clock why="the one clock read: AsOf::today_utc (R-CHK-014)" owner=@andy exp=2026-12-31
```

Re-run the check. Expected: exit 1 with `HN-X002 error ... ovr rule.no-clock has no approval; state=waiver-ineffective` and the underlying `HN-U001`. The agent stops here.

- [ ] **Step 4: Approve (human step, terminal only)**

The owner runs, in a terminal with a TTY and without the `CI` variable: `cargo run -p fairlead -- approve rule.no-clock --as @andy`. This writes `fairlead/approvals.toml` (R-CLI-037, R-AGT-026). Re-run `cargo run -p fairlead -- check --as-of 2026-10-09 --format terse --deny-warnings`.

Expected: exit 0, with `waived 1` in the summary. The `ovr` line and the approval record are committed together in one commit.

- [ ] **Step 5: Write the docs check and its tests**

Create `xtask/src/docs.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_arch_161__docs_have_no_dangling_rule_ids() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let dangling = dangling_ids(&root.join("docs")).expect("docs readable");
        assert!(dangling.is_empty(), "referenced but never defined: {dangling:?}");
    }

    #[test]
    fn e10__no_open_question_tagged_0_1() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let open = open_0_1_questions(&root.join("docs/70-open-questions.md")).expect("register readable");
        assert!(open.is_empty(), "open 0.1 questions: {open:?}");
    }
}
```

Run: `cargo test -p xtask docs` → FAIL to compile (`dangling_ids` and `open_0_1_questions` missing).

Then add the implementation above the test module:

```rust
//! `cargo xtask docs-check`: dangling R- and D- references in docs/ (R-ARCH-161) and
//! open 0.1 questions in the register (E10). Definitions are rule rows and bold rule lines.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn md_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "plans") {
                md_files(&p, out)?;
            }
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
    Ok(())
}

/// Every `R-XXX-nnn` or `D-nn` token of the form the spec uses.
fn ids_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
        let rule = word.len() == 11 && word.starts_with("R-") && word.as_bytes()[9] == b'-';
        let decision = word.len() == 4 && word.starts_with("D-") && word[2..].bytes().all(|b| b.is_ascii_digit());
        if rule || decision {
            out.push(word.to_string());
        }
    }
    out
}

pub fn dangling_ids(docs: &Path) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    md_files(docs, &mut files)?;
    let mut defined = BTreeSet::new();
    let mut referenced = BTreeSet::new();
    for f in files {
        for line in std::fs::read_to_string(&f)?.lines() {
            let is_def = line.starts_with("- **") || line.starts_with("| ") || line.starts_with("**");
            for id in ids_in(line) {
                if is_def {
                    defined.insert(id.clone());
                }
                referenced.insert(id);
            }
        }
    }
    Ok(referenced.difference(&defined).cloned().collect())
}

/// Rows of the register whose milestone is exactly `0.1` and whose resolution column is
/// empty or starts with "Open".
pub fn open_0_1_questions(register: &Path) -> std::io::Result<Vec<String>> {
    let mut open = Vec::new();
    for line in std::fs::read_to_string(register)?.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() >= 7 && cells[6] == "0.1" && (cells[4].is_empty() || cells[4].starts_with("Open")) {
            open.push(cells[1].to_string());
        }
    }
    Ok(open)
}
```

Column indices follow the register's layout (`| ID | Question | Options | Leaning | ... | Milestone | Sources |`). Verify the column positions against the first data row before relying on the check.

- [ ] **Step 6: Run and expect PASS**

Run: `cargo test -p xtask docs`. Expected: `r_arch_161__...` and `e10__...` pass once every referenced ID is defined and no 0.1 question is open. If either fails, the failure lists the IDs or questions. Each is a documentation defect for the owner, recorded in the Contract issues below. It is not fixed by weakening the check.

- [ ] **Step 7: Write `dogfood.yml`**

Create `.github/workflows/dogfood.yml`:

```yaml
name: dogfood

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read

jobs:
  self-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: dtolnay/rust-toolchain@stable
      - name: Build in-tree fairlead
        run: cargo build -p fairlead --locked
      - name: Check the merge result (R-AGT-005)
        run: target/debug/fairlead check --as-of "$(git log -1 --format=%cs HEAD)" --format terse --deny-warnings
      - name: Check every commit of the pull request range (R-AGT-005)
        if: github.event_name == 'pull_request'
        run: |
          for c in $(git rev-list --reverse "${{ github.event.pull_request.base.sha }}..HEAD"); do
            git -c advice.detachedHead=false checkout -q "$c"
            target/debug/fairlead check --as-of "$(git log -1 --format=%cs "$c")" --format terse --deny-warnings
          done
      - name: Pinned reference binary (R-ARCH-160, R-AGT-006)
        if: vars.FAIRLEAD_REFERENCE_VERSION != ''
        run: |
          cargo binstall -y "fairlead@${{ vars.FAIRLEAD_REFERENCE_VERSION }}"
          fairlead check --as-of "$(git log -1 --format=%cs HEAD)" --format terse --deny-warnings
      - name: SARIF for the record (R-AGT-006)
        run: target/debug/fairlead check --as-of "$(git log -1 --format=%cs HEAD)" --format sarif > fairlead.sarif || true
      - name: Docs references and open questions (R-ARCH-161, E10)
        run: cargo run -p xtask --locked -- docs-check
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: fairlead-sarif
          path: fairlead.sarif
          if-no-files-found: ignore
```

The SARIF step does not gate, because the gate is the terse check above it. The `|| true` is intentional: the artifact keeps the SARIF even when findings fail the run.

- [ ] **Step 8: Commit**

```bash
git add fairlead crates/fairlead-core/fairlead crates/fairlead-syntax/fairlead crates/fairlead-core/src/time.rs xtask/src/docs.rs xtask/src/main.rs .github/workflows/dogfood.yml
git commit -m "chore(p9): dogfood declarations, waiver, dogfood workflow and docs check" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 25: Controlled-test harness: tasks, arms, blinded scoring (R-ROAD-005, R-ROAD-009, R-ROAD-016, R-ROAD-018)

The harness is pre-registered by Task 1. The ablation has 20 tasks, 3 arms, 3 runs each. The scorer counts declared values in added lines of each diff by literal search. It never calls `fairlead check`. Diffs are scored under opaque names, and the arm mapping is written after scoring.

**Files:**
- Create: `ablation/tasks.toml`, `ablation/declared.toml`
- Modify: `xtask/src/ablation.rs` (scoring functions and tests), `xtask/src/main.rs` (dispatch `ablation-score`, `ablation-report`)

- [ ] **Step 1: Write the failing verdict test**

Append to the test module in `xtask/src/ablation.rs`:

```rust
    #[test]
    fn r_road_005__verdict_boundaries() {
        assert_eq!(verdict(50.0), "pass");
        assert_eq!(verdict(49.9), "inconclusive: between 20 and 50 percent");
        assert_eq!(verdict(19.9), "weak: kill assumption 2");
        assert_eq!(reduction_pct(10.0, 5.0), 50.0);
    }

    #[test]
    fn r_road_016__scorer_counts_literal_values_in_added_lines_only() {
        let declared = vec![("tok.color.primary".to_string(), "#2563eb".to_string())];
        let diff = "+++ b/a.tsx\n+color: \"#2563eb\"\n-color: \"#2563eb\"\n context #2563eb\n";
        assert_eq!(hardcoded_count(diff, &declared), 1);
    }
```

Run: `cargo test -p xtask r_road_0`
Expected: FAIL to compile (`verdict`, `reduction_pct`, `hardcoded_count` missing).

- [ ] **Step 2: Write the scoring functions**

Add to `xtask/src/ablation.rs` (above the test module):

```rust
/// Declared values found in added lines of one diff. Literal search only; `fairlead check`
/// is never called (R-ROAD-016). `declared` holds (id, value) pairs.
pub fn hardcoded_count(diff: &str, declared: &[(String, String)]) -> usize {
    diff.lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .map(|l| declared.iter().filter(|(_, v)| l.contains(v.as_str())).count())
        .sum()
}

/// Percent reduction of arm C against arm A, from mean counts per arm.
pub fn reduction_pct(mean_a: f64, mean_c: f64) -> f64 {
    if mean_a == 0.0 { 0.0 } else { (1.0 - mean_c / mean_a) * 100.0 }
}

/// The decision rule of the protocol (E2). Arm B is never passed to this function.
pub fn verdict(reduction: f64) -> &'static str {
    if reduction >= PASS_REDUCTION_PCT as f64 {
        "pass"
    } else if reduction < WEAK_BELOW_PCT as f64 {
        "weak: kill assumption 2"
    } else {
        "inconclusive: between 20 and 50 percent"
    }
}
```

Run: `cargo test -p xtask r_road_0`. Expected: both tests pass.

- [ ] **Step 3: Write the task list and the declared items**

`ablation/tasks.toml` (20 tasks, 10 UI on `ts-react`, 10 backend on `fullstack-tauri` and `rust-workspace`; each task has a natural path to a declared value):

```toml
# Pre-registered task list (docs/plans/0.1/ablation-protocol.md, E2). Do not edit after the first run.
tasks = [
  { id = "ui-01", track = "ui", fixture = "ts-react", prompt = "Add a brand-coloured badge to the console navigation group." },
  { id = "ui-02", track = "ui", fixture = "ts-react", prompt = "Style the shared button's selected state with the primary colour." },
  { id = "ui-03", track = "ui", fixture = "ts-react", prompt = "Give the console sidebar a highlighted row colour." },
  { id = "ui-04", track = "ui", fixture = "ts-react", prompt = "Add an error banner in the brand red used by the console." },
  { id = "ui-05", track = "ui", fixture = "ts-react", prompt = "Add a focus ring that matches the primary colour." },
  { id = "ui-06", track = "ui", fixture = "ts-react", prompt = "Tint the empty-state panel with the page background." },
  { id = "ui-07", track = "ui", fixture = "ts-react", prompt = "Set a link colour in the shared button variant." },
  { id = "ui-08", track = "ui", fixture = "ts-react", prompt = "Add a divider in the console border colour." },
  { id = "ui-09", track = "ui", fixture = "ts-react", prompt = "Add a success toast with the brand green." },
  { id = "ui-10", track = "ui", fixture = "ts-react", prompt = "Add a hover state to the navigation group using the primary colour." },
  { id = "be-01", track = "backend", fixture = "fullstack-tauri", prompt = "Add a timeout constant for note search with the documented default." },
  { id = "be-02", track = "backend", fixture = "fullstack-tauri", prompt = "Add a page-size constant for listing notes." },
  { id = "be-03", track = "backend", fixture = "fullstack-tauri", prompt = "Add a retry limit constant for the sync command." },
  { id = "be-04", track = "backend", fixture = "fullstack-tauri", prompt = "Add a maximum title length for notes." },
  { id = "be-05", track = "backend", fixture = "rust-workspace", prompt = "Add a digest buffer size to the core crate." },
  { id = "be-06", track = "backend", fixture = "rust-workspace", prompt = "Add a default port constant to the transport crate." },
  { id = "be-07", track = "backend", fixture = "rust-workspace", prompt = "Add a log-level default to the host crate." },
  { id = "be-08", track = "backend", fixture = "rust-workspace", prompt = "Add a progress interval constant for sync events." },
  { id = "be-09", track = "backend", fixture = "rust-workspace", prompt = "Add a checksum algorithm label used in a message." },
  { id = "be-10", track = "backend", fixture = "rust-workspace", prompt = "Add a maximum retry delay to the engine." },
]
```

`ablation/declared.toml` (the 30 declared items of roadmap 7.1; `items` are `id|value|property`):

```toml
items = [
  "tok.color.primary|#2563eb|color",
  "tok.color.background|#ffffff|background-color",
  "tok.color.destructive|#dc2626|color",
  "tok.color.success|#16a34a|color",
  "tok.color.border|#e5e7eb|border-color",
  "tok.color.muted|#6b7280|color",
  "tok.color.sidebar|#f8fafc|background-color",
  "tok.color.link|#1d4ed8|color",
  "tok.radius.base|0.5rem|border-radius",
  "tok.space.base|0.25rem|padding",
  "tok.font.size.base|0.875rem|font-size",
  "tok.font.weight.bold|700|font-weight",
  "tok.shadow.card|0 1px 2px #0000000d|box-shadow",
  "tok.z.modal|1000|z-index",
  "tok.motion.fast|150ms|transition-duration",
  "tok.width.sidebar|16rem|width",
  "const.note.title-max|120|max-length",
  "const.note.page-size|25|page-size",
  "const.sync.retry-max|5|retry-limit",
  "const.search.timeout-ms|800|timeout-ms",
  "const.engine.buffer-kib|64|buffer-size",
  "const.transport.port|7417|port",
  "const.host.log-level|info|log-level",
  "const.sync.progress-ms|250|progress-interval",
  "const.digest.algo|blake3|checksum",
  "const.engine.retry-delay-ms|2000|retry-delay",
  "const.core.max-file|2000000|max-file",
  "const.core.max-directives|500|max-directives",
  "const.ui.toast-ms|4000|duration",
  "const.ui.debounce-ms|300|debounce",
]
```

- [ ] **Step 4: Write the blinded scorer and the arm-B generator**

Add `ablation-score` and `ablation-report` to `xtask/src/main.rs`:

- `ablation-score --diffs DIR --declared ablation/declared.toml --out scores.csv` reads every `*.diff` under DIR, parses `items`, and writes `diff_id,hardcoded` rows. The diff ids are the opaque file names; no arm or task appears in the output.
- `ablation-report --scores scores.csv --map map.csv` joins the two files after scoring, takes the mean per arm and task track, computes `reduction_pct` of arm C against arm A and prints `verdict`. Arm B is printed as a row, never passed to `verdict`.
- `ablation-arm-b --declared ablation/declared.toml --out ablation/arms/b-agents.md` writes the static `AGENTS.md` of arm B: one prose line per declared item (`id` is the name, `value` the literal, `property` the context). It contains the same facts as the fairlead declarations, in prose.

Run an end-to-end dry run before the first measured run: `cargo xtask ablation-score --diffs fixtures/ablation-dryrun --declared ablation/declared.toml --out target/ablation/scores.csv`. Expected: the output contains no arm or task label. Record the dry run in the results log, not in the measurement.

- [ ] **Step 5: Run and expect PASS; commit**

Run: `cargo test -p xtask r_road_0`. Expected: pass.

```bash
git add ablation xtask/src/ablation.rs xtask/src/main.rs
git commit -m "test(p9): controlled-test tasks, declared items and blinded scorer" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Task 26: Syntax-card trial scoring (E8) and the TypeScript conformance fixtures (E9)

E8 scores 100 directives written by fresh agent sessions. A directive is invalid when it carries an error diagnostic, is dropped, or binds to nothing. E9 adds TypeScript cases to the syntax conformance suite. The conformance runner belongs to P2 (`crates/fairlead-syntax/tests/conformance.rs`). P9 owns the cases and the expectations.

**Files:**
- Create: `xtask/src/syntax_card.rs`, `fixtures/syntax/ts/block-binds-next-line.ts`, `fixtures/syntax/ts/block-binds-next-line.expect`, `fixtures/syntax/ts/jsx-child.tsx`, `fixtures/syntax/ts/jsx-child.expect`
- Modify: `xtask/src/main.rs` (dispatch `syntax-card-score`)

- [ ] **Step 1: Write the failing gate test**

Create `xtask/src/syntax_card.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e8__syntax_card_gate_takes_at_most_five_percent_invalid() {
        assert!(gate(100, 5).is_ok());
        assert!(gate(100, 6).is_err());
        assert!(gate(99, 0).is_err(), "the sample size is fixed at 100");
    }
}
```

Run: `cargo test -p xtask e8__`. Expected: FAIL to compile (`gate` missing).

- [ ] **Step 2: Write the scorer**

Add above the test module:

```rust
//! `syntax-card-score DIR`: each sample file holds one directive written by a fresh agent
//! session. A sample is invalid when extraction reports an error, drops it, or finds nothing.

use crate::ablation::{SYNTAX_CARD_MAX_INVALID_PCT, SYNTAX_CARD_SAMPLE};
use fairlead_syntax::{extract, CommentTable, ExtractConfig, Severity};
use std::path::Path;

fn cfg() -> ExtractConfig {
    ExtractConfig {
        marker: "fairlead".into(),
        comments: CommentTable::builtin(),
        header_skip: Vec::new(),
        max_file: 2_000_000,
        max_directives: 500,
    }
}

pub fn score(dir: &Path) -> Result<(usize, usize), String> {
    let mut total = 0usize;
    let mut invalid = 0usize;
    let mut files: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.to_string())?.filter_map(Result::ok).map(|e| e.path()).collect();
    files.sort();
    for p in files {
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("txt").to_string();
        let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
        let e = extract(&bytes, &ext, &cfg());
        total += 1;
        if e.diags.iter().any(|d| d.severity == Severity::Error) || e.dropped > 0 || e.directives.is_empty() {
            invalid += 1;
        }
    }
    Ok((total, invalid))
}

/// The E8 gate: exactly the pre-registered sample size, and no more than the allowed percentage.
pub fn gate(total: usize, invalid: usize) -> Result<(), String> {
    if total != SYNTAX_CARD_SAMPLE {
        return Err(format!("sample is {total}, the protocol fixes {SYNTAX_CARD_SAMPLE}"));
    }
    if invalid * 100 > total * SYNTAX_CARD_MAX_INVALID_PCT as usize {
        return Err(format!("{invalid}/{total} invalid exceeds {SYNTAX_CARD_MAX_INVALID_PCT}%"));
    }
    Ok(())
}
```

Wire `Some("syntax-card-score")` in `xtask/src/main.rs`: call `score(dir)`, then `gate(total, invalid)`, print the counts, and exit 1 on error. Add `mod syntax_card;`.

- [ ] **Step 3: Run and expect PASS**

Run: `cargo test -p xtask e8__`. Expected: pass.

- [ ] **Step 4: Write the TypeScript conformance cases (E9)**

The expectation format is fixed here for the P2 runner. Each `.expect` line is either `<verb> <id> lines:<first>-<last>` for a directive, or `diag <HN-code>` for a diagnostic. Lines are sorted bytewise, one newline at the end.

Create `fixtures/syntax/ts/block-binds-next-line.ts`:

```ts
/* fairlead: def fn.load */
export function load(): number {
  return 1;
}
```

Create `fixtures/syntax/ts/block-binds-next-line.expect` (a standalone block comment binds to the next code line, and the opened brace extends the extent to the matching close, R-SYN-018):

```text
def fn.load lines:2-4
```

Create `fixtures/syntax/ts/jsx-child.tsx`:

```tsx
export function Panel() {
  return (
    <div>
      {/* fairlead: def cmp.panel */}
      <span>x</span>
    </div>
  );
}
```

Create `fixtures/syntax/ts/jsx-child.expect` (a JSX child carrier binds to the next element line, R-SYN-044):

```text
def cmp.panel lines:5-5
```

Verify: P2 adds the runner that reads `fixtures/syntax/<host>/<case>.<ext>` and its `.expect` sibling. Both files must pass there. A disagreement between a fixture and the chapter-12 text is a spec bug (contract section 7).

- [ ] **Step 5: Commit**

```bash
git add xtask/src/syntax_card.rs xtask/src/main.rs fixtures/syntax/ts
git commit -m "test(p9): syntax-card trial scorer (E8) and TypeScript conformance fixtures (E9)" -m "Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>"
```

## Traceability

Every P9 rule in contract section 6 (CHK 012 015 016 051; CTX 072; AGT 005 006; ARCH 071 080 111 130 131 140 150 151 153 160 161 162), the two adjacent rules assigned to P9 (GIT-001, PORT-014) and ROAD-014 each map to a task and to a verifying command.

| Rule | Task | Verifying command |
|---|---|---|
| R-ARCH-071 cold, warm, deleted cache identical | 15 | `cargo test -p fairlead --test determinism r_arch_071` |
| R-ARCH-080 one thread equals default | 15 | `cargo test -p fairlead --test determinism r_arch_080` |
| R-ARCH-111 LF and CRLF checkouts identical | 15 | `cargo test -p fairlead --test determinism r_arch_111` |
| R-ARCH-043 invocation directory does not matter | 15 | `cargo test -p fairlead --test determinism r_arch_043` |
| R-CHK-012 Linux and Windows identical (BOM, CRLF) | 15 | `cargo test -p fairlead --test determinism r_chk_012` |
| R-CHK-015 reader verbs leave git status unchanged | 16 | `cargo test -p fairlead --test readonly r_chk_015` |
| R-ARCH-140 readers write only under `.fairlead/` | 16 | `cargo test -p fairlead --test readonly r_arch_140` |
| R-GIT-001 check writes no tracked file | 16 | `cargo test -p fairlead --test readonly r_git_001` |
| R-CHK-016 hot-path goals (`--files-from` about 1 s) | 23 | `cargo xtask bench-corpus check` (report line; not gated) |
| R-CHK-051 no false dangling references | 4 (test in `golden.rs`) | `cargo test -p fairlead --test golden r_chk_051` |
| R-CHK-051 precision, 200 blind findings at most 2% false | protocol (Task 1) | manual review; sampler not implemented (CI-11) |
| R-CTX-072 bytes-per-token and Merkle cost | 23 step 7, bench `digest` | manual token count; `cargo bench -p fairlead-core --bench digest` |
| R-AGT-005 check on the merge result and every PR commit | 24 | `dogfood.yml`, job `self-check` |
| R-AGT-006 `--as-of` commit date, pinned binary, SARIF | 24 | `dogfood.yml`; SARIF artifact `fairlead-sarif` |
| R-ARCH-130 benchmark corpus, tracked targets | 23 | `cargo run -p xtask -- bench-corpus generate` then `check`; `bench.yml` |
| R-ARCH-131 profiling precedes performance-driven change | 23 | process rule: results committed under `docs/plans/0.1/results/` before any format change |
| R-ARCH-150 fuzz directive and control loader, bounded | 22 | `fuzz.yml`; `cargo fuzz run directive --fuzz-dir fuzz -- -max_total_time=60` |
| R-ARCH-151 every active code has a snapshot | 21 | `cargo test -p fairlead --test codes` |
| R-ARCH-153 mandatory golden set includes TypeScript | 4 and 5 to 14 | `cargo test -p fairlead --test golden r_arch_153` |
| R-ARCH-160 repository passes its own check | 24 | `dogfood.yml`, job `self-check` |
| R-ARCH-161 docs have no dangling IDs | 24 | `cargo test -p xtask r_arch_161`; `cargo run -p xtask -- docs-check` |
| R-ARCH-162 nested scope with a sealed key and a real `ovr` | 24 | `cargo run -p fairlead -- check --as-of 2026-10-09 --format terse --deny-warnings` (exit 0 after approval) |
| R-PORT-014 Windows paths, collisions, latency | 20 | `cargo test -p fairlead --test platform` |
| R-ROAD-014 thresholds committed before the test | 1 | `cargo test -p xtask r_road_014` |
| R-ROAD-004 replay recall and false positives | 19 | `cargo test -p fairlead --test replay` |
| R-ROAD-005 ablation verdict, R-ROAD-016 scorer rules | 25 | `cargo test -p xtask r_road_0` |

Experience-level exit criteria (E1 to E10). Each row names the criterion, its tasks, and the verifying command. Human and gate steps are marked as such.

| Exit | Tasks | Verifying command or record |
|---|---|---|
| E1 nine success criteria | 15, 16, 18, 19, 23, 25 | the commands above; R-ROAD-002 and R-ROAD-007 are not implemented by this plan (CI-11) |
| E2 wedge effect, at least 50% reduction | 1, 25 | `cargo xtask ablation-report` on the blinded scores; verdict printed |
| E3 exception hygiene, at least 80% | 1 | protocol section; reviewer labels recorded by hand (CI-11) |
| E4 gates G0 and G1 | human | recorded in `docs/plans/0.1/results/`; not a command |
| E5 release test green on three platforms | 8, 15, 16, 17, 18, 19, 20, 22 | `cargo test --workspace`; `cargo run -p xtask -- sarif-validate`; `fuzz.yml`; MSRV job in P1 |
| E6 benchmarks | 23 | `cargo run -p xtask -- bench-corpus check --results ... --baseline bench/baseline.toml` |
| E7 dogfooding | 24 | `dogfood.yml` |
| E8 syntax and adoption trials | 26, 1 | `cargo run -p xtask -- syntax-card-score DIR` (at most 5%); adoption is a timed human trial |
| E9 TypeScript evidence | 4, 25, 26 | `cargo test -p fairlead --test golden r_arch_153`; the TypeScript conformance cases |
| E10 no open 0.1 question | 24 | `cargo test -p xtask e10` |

## Contract issues

These items need an owner decision or a contract amendment. The plan states the assumption it used in each case.

- **CI-1 (lexer API).** Section 4 names no public lexer entry point. The lexer fuzz target needs `fairlead_syntax::lexer::comment_spans(text: &str, ext: &str) -> Vec<Range<usize>>`. P2 must expose it.
- **CI-2 (fairlead-syntax public paths).** The fuzz targets and the xtask use crate-root re-exports `extract`, `ExtractConfig`, `CommentTable`, `render_directive_body`, `Severity`, and the modules `frontmatter` and `lexer`. Section 3 says the crate re-exports, but names no paths. P2 must match these paths.
- **CI-3 (golden.toml extensions).** Contract section 7 defines no `reuse` (Task 7), `remove` or `git` (Task 12) keys, and requires `append` on every edit. The plan adds them as optional keys. A dated amendment of section 7 is needed, or the owner rejects them.
- **CI-4 (lock file format).** `.fairlead/lock` is specified as holding pid, host and start time, but no format is given. Task 14 uses TOML keys `pid`, `host`, `start`. P7 must adopt this format.
- **CI-5 (expected outputs are spec-derived).** Chapters 13 and 14 are illustrative for terse wording, header counts and the waivers line. The goldens most at risk are `ts-react`, `rust-workspace`, `sparse-checkout`, `crlf-and-windows-names` and `legacy-baseline`. Their first bless needs line-by-line review.
- **CI-6 (absent-scope exit code).** R-CHK-057 says the run must not exit 0, and names no code for an absent scope. The plan assumes exit 2, treating an absent scope as unresolved for the outcome rule (C-22). Owner decision.
- **CI-7 (SARIF fingerprint key).** `partialFingerprints` has no key name in the spec. The plan asserts `fairlead/v1`. P5 must emit that key, or the contract must name one.
- **CI-8 (dependencies not in section 2).** `toml`, `serde`, `serde_json`, `tempfile` and `jsonschema` in xtask; `libfuzzer-sys` and `cargo-fuzz` in fuzz; `criterion` in fairlead-core. Each needs a R-ARCH-120 record (version, date, licence, risk) before adoption. The versions in this plan (`criterion` 0.5, `libfuzzer-sys` 0.4, the schema validator) are UNVERIFIED.
- **CI-9 (test target and xtask commands beyond section 1).** `tests/codes.rs` and the `codes` target are added (R-ARCH-151 needs one). The xtask commands `sarif-validate`, `golden-bless`, `docs-check`, `ablation-score`, `ablation-report`, `ablation-arm-b`, `syntax-card-score` and `bench-corpus` (generate, check) extend the four listed in section 1.
- **CI-10 (owner handle).** The dogfood `owners` list uses `@andy`, the owner handle assumed from the local account. The GitHub handle is not known. The owner confirms it (R-AGT-028). Approvals are human-only (R-AGT-026).
- **CI-11 (coverage gaps).** The protocol specifies the 200-finding blind precision sample (R-CHK-051, R-ROAD-003), the context token budget at p90 (R-ROAD-002), the 50 simulated parallel merges (R-ROAD-007) and the E3 hygiene labels. This plan implements none of them as automation. They are manual protocols for now. The owner decides whether to add them to P9.
- **CI-12 (R-ARCH-161 wording).** The rule says "checked by fairlead", but the spec docs carry no directives. The plan checks them with `xtask docs-check` instead. The owner chooses between this and adding `<!-- fairlead: def ... -->` registry blocks to the docs (D-40). The E10 check assumes the register column layout (milestone in column 6, leaning in column 4). Verify before relying on it.
- **CI-13 (flag placement).** The tests put global flags (`--no-cache`, `--threads`) after the verb. P8 must accept them there (clap `global = true`), or the tests move them before the verb.
- **CI-14 (action pinning).** Workflows use major tags (`actions/checkout@v4` and similar). Pin to commit SHAs before the first release (R-REL-046).
- **CI-15 (duplicated thresholds).** `tests/replay.rs` repeats the protocol constants because tests cannot import xtask. The protocol test binds only the xtask constants. Keep the two in sync, or move the constants to a shared module.

## Execution handoff

Run the tasks in this order. Task 1 needs the owner's sign-off before anything else starts (R-ROAD-014). Tasks 2 and 3 come next, then the goldens (4 to 14), then the test targets (15 to 21), then the tooling and workflows (22 to 26). Failing tests are expected at the start: the golden set stays red until P8 supplies the binary and every golden task is complete. Use superpowers:subagent-driven-development with one subagent per task, and review each golden's expected output by hand before its first bless. Human-only steps are never delegated: the approval (Task 24, step 4), the first bless of each golden, and the results log.
