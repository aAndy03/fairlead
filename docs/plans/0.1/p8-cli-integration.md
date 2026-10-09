# P8 CLI Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `fairlead` binary crate and the `fairlead-core::pipeline` module so that every 0.1 verb is reachable from the command line: the fixed verb set and global flags parse, each verb calls the library seams and nothing else, exit codes come only from `fairlead_core::outcome`, a worker panic becomes exit 5, output is LF-only with a TTY/`NO_COLOR`-aware format default, the reserved verbs refuse with the exact milestone text, and every 0.1 verb and flag family has a trycmd contract test.

**Architecture:** Two layers. `fairlead-core::pipeline` (module `crates/fairlead-core/src/pipeline.rs`) is the only module that chains the stages S1 to S5 (load), S6 (resolve), S7 (check), S9 (context) and the write path S11 (plan and commit); it adds sequencing and nothing else (R-ARCH-001). The `fairlead` bin (`crates/fairlead/src/`) is a thin shell: `main.rs` installs the panic boundary and returns `std::process::ExitCode`; `cli.rs` is the clap derive tree (`Cli`, `GlobalArgs`, one `*Args` per verb, reserved-verb args); `commands/*.rs` each expose `run(&GlobalArgs, &XArgs) -> CommandResult`; `exit.rs` maps `ExitCode` and `Error` to process codes; `output.rs` writes a `CommandResult` to stdout and stderr with LF-only bytes. The bin contains no check, path or file-format logic: it parses arguments, builds request values, calls `pipeline` and the library renderers, and maps results.

**Tech Stack:** Rust edition 2024, MSRV 1.88, resolver 3. Crates from contract Section 2 only: `clap` 4.6.7 (`derive`, `std`, `help`, `usage`, `error-context`, names UNVERIFIED), `camino` 1.2.6, `semver` 1.0.28, `jiff` 0.2.38, `thiserror` (UNVERIFIED version), `fairlead-core` (path), `fairlead-syntax` (path, via `fairlead-core`). Test-only: `trycmd` 1.2.1 (bin only), `tempfile` 3.27.0, `insta` 1.49.0 (json, glob) for the bin unit tests if needed. The bin does not depend on `ignore`, `git2`, `rayon`, `blake3` or `regex` directly.

**Spec:** [docs/15-cli.md](../../15-cli.md) (verbs, flag table, exit codes R-CLI-003, usage), [docs/13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) (R-CHK-040 exit mapping, R-CHK-036 formats, R-CHK-038/039 caps), [docs/50-architecture.md](../../50-architecture.md) (R-ARCH-001, 082, 100, 101), [docs/14-context.md](../../14-context.md) (context dispatch, R-CTX-003/004), [docs/decisions.md](../../../decisions.md) (D-25, D-41, D-44, D-52, D-54 win), [docs/conventions.md](../../../conventions.md). Contract: [01-contract.md](01-contract.md) Sections 3, 4.13, 5, 6 (P8 row), 7, 8, 9.

---

## Global Constraints

These apply to every task. A task that violates one is wrong even if its tests pass.

1. **The bin contains no check, path or file-format logic.** `crates/fairlead/src/**` MUST NOT contain a `HN-` string literal, a rule evaluation, a fingerprint, a glob, a path normalisation beyond `-C`/`--root` argument plumbing, or a serializer for `json`, `sarif` or `terse` line formats. Those belong to `fairlead-core` (R-ARCH-001). The bin only calls `pipeline::*`, `renderer(format)` and `commands` glue.
2. **Exit codes 0 to 5 come only from `fairlead_core::outcome::ExitCode`.** The bin maps with `exit::to_process_exit`. No integer literal exit code appears in `crates/fairlead/src/**` except inside that single mapping function and its tests. `std::process::exit` MUST NOT be called anywhere (R-ARCH-100; clippy `disallowed-methods`).
3. **Panic boundary exits 5.** `main` wraps the whole command run in `std::panic::catch_unwind`; a caught panic produces exit 5 with an internal-error line on stderr. Success is never reported after a panic (R-ARCH-101, contract Section 8 invariant 4).
4. **Readers never take the writer lock.** `check`, `context` and `explain` MUST NOT construct `WriterLock` or call `Transaction::begin`. Only `init`, `sync`, `new` and `approve` reach `write::*` commit paths (contract Section 8 invariant 6; R-CLI-008).
5. **LF-only output.** Every byte the bin writes to stdout or stderr is LF-terminated text: no CRLF, no `\r`, exactly one trailing newline when non-empty, repo-relative forward-slash paths, no absolute paths in stdout (R-CLI-006, R-ARCH-052, contract Section 8 invariant 2). On Windows the bin writes through a locked `std::io::Write` with explicit `\n`; it never uses `println!` with a CRLF-translating terminal layer.
6. **MSRV 1.88.** Code uses only APIs stable in Rust 1.88. `std::io::IsTerminal` (stable since 1.70) is the TTY test. Let-chains are stable in edition 2024 from 1.88, but this plan does not use them, so every condition is a plain `if let` that compiles the same on the MSRV job.
7. **Lints.** `fairlead` is `#![deny(unsafe_code)]`. `print_stdout` and `print_stderr` are allowed only in `output.rs` (the sole writer). `unwrap_used`, `expect_used`, `panic`, `indexing_slicing` are denied outside `#[cfg(test)]`; `todo` is denied once the owning phase has landed (P8 lands the bin, so no `todo!()` remains in P8 files).
8. **Clock.** The bin never reads a clock. When `--as-of` is absent, `commands::as_of` calls `AsOf::today_utc()` (contract Section 8 invariant 9). `--budget` reaches the library as a `Duration`, and the deadline is computed in `check/options.rs`, the one other permitted clock read (R-ARCH-090).
9. **Environment.** The bin reads the environment in exactly three modules: `commands/mod.rs` (`FAIRLEAD_ROOT` into `DiscoverRequest.env_root`; `FAIRLEAD_THREADS` into `LoadRequest.threads`, read only when `--threads` is absent), `output.rs` (`NO_COLOR`), and `commands/approve.rs` (`CI`, R-CLI-037). `main.rs` reads nothing, and `fairlead-core` never reads the environment on the bin's behalf.
10. **Data is not instructions.** Every repository string that the bin prints passes through `fairlead_core::render` (already sanitised); the bin does not print raw file text (contract Section 8 invariant 8).

## File Structure (P8 scope)

Files P8 creates or owns. Paths are repo-relative.

```text
crates/fairlead-core/src/pipeline.rs         composes S1..S12; LoadRequest, Loaded, load, resolve_loaded, check_loaded, context_loaded
crates/fairlead-core/tests/pipeline.rs       pure-ish pipeline tests over a TempRepo-less fixture (in-memory where possible)
crates/fairlead/Cargo.toml                   [[bin]] fairlead, [[test]] cli path ../../tests/cli.rs and the other test targets (declared, not all implemented here)
crates/fairlead/src/main.rs                  fn main() -> std::process::ExitCode; catch_unwind; env reads for threads and colour
crates/fairlead/src/cli.rs                   clap derive: Cli, GlobalArgs, Command, *Args, ColorArg, ReservedArgs
crates/fairlead/src/commands/mod.rs          CommandResult, request builders, threads and the environment reads of the commands (Task 4)
crates/fairlead/src/commands/init.rs         run(&GlobalArgs, &InitArgs) -> CommandResult
crates/fairlead/src/commands/check.rs        run(&GlobalArgs, &CheckArgs) -> CommandResult
crates/fairlead/src/commands/sync.rs         run(&GlobalArgs, &SyncArgs) -> CommandResult
crates/fairlead/src/commands/new.rs          run(&GlobalArgs, &NewArgs) -> CommandResult
crates/fairlead/src/commands/context.rs      run(&GlobalArgs, &ContextArgs) -> CommandResult
crates/fairlead/src/commands/explain.rs      run(&GlobalArgs, &ExplainArgs) -> CommandResult
crates/fairlead/src/commands/approve.rs      run(&GlobalArgs, &ApproveArgs) -> CommandResult
crates/fairlead/src/commands/reserved.rs     run(&GlobalArgs, ReservedVerb) -> CommandResult; refusal text and exit 2 (Task 5)
crates/fairlead/src/exit.rs                  to_process_exit(ExitCode), exit_for_error(&Error) -> ExitCode
crates/fairlead/src/output.rs                write_result(&CommandResult), format default, colour decision, LF guard
tests/cli.rs                              trycmd runner (target `cli` of package fairlead)
tests/cli/init.trycmd                     init contract cases (plan, --yes, idempotent, usage errors)
tests/cli/check.trycmd                    check contract cases (flags, exit codes 0/1/2/3 via fixtures, budget grammar)
tests/cli/sync.trycmd                     sync contract cases (dry-run, baseline, --yes no-op)
tests/cli/new.trycmd                      new contract cases (refusal to overwrite, invalid names, --print)
tests/cli/context.trycmd                  context dispatch cases (path, ID, task query exit 2, no argument)
tests/cli/explain.trycmd                  explain dispatch cases (topic, code, path, ID, unknown exit 2)
tests/cli/approve.trycmd                  approve refusals (not a TTY, CI set) and --list
tests/cli/global.trycmd                   global flags: --version, --format defaults, --threads 0 exit 2, --quiet, -C
tests/cli/reserved.trycmd                 reserved verbs: exact refusal text, exit 2
tests/cli/check-findings.trycmd           check exit 1 on a dangling reference (fixture check-findings.in)
tests/cli/check-syntax.trycmd             check exit 2 on a syntax error (fixture check-syntax.in)
tests/cli/root-missing.trycmd             no root: exit 2 with the fairlead init hint
tests/cli/*.in/                           fixture trees copied into the trycmd sandbox (Task 16)
```

Note on test files: the contract (Section 3) lists `tests/cli.rs` and `tests/cli/*.trycmd` as the CLI contract. Pure unit tests of the pipeline and the exit mapping live in the module files (`#[cfg(test)] mod tests`), per Section 7.

## Rule coverage in this phase

P8 owns exactly these 0.1 rules (contract Section 6, P8 row, 13 rules): **R-CHK-040; R-CLI-001, R-CLI-002, R-CLI-003, R-CLI-006, R-CLI-011, R-CLI-021, R-CLI-032, R-CLI-043, R-CLI-045; R-ARCH-001, R-ARCH-082, R-ARCH-101.** Every one has a task and a test named with the snake-case rule id (contract Section 7). The full mapping is in the Traceability section.

---

## Task index

| # | Task | Rules (P8 and supporting) | Files |
|---|---|---|---|
| 1 | Bin manifest, exit mapping, panic boundary | R-CLI-003, R-ARCH-101 | Cargo.toml, main.rs, exit.rs, output.rs |
| 2 | clap tree and every [0.1] verb argument | R-CLI-001, 002, 006, 021, 025, 032, 043, 045 | cli.rs |
| 3 | Output: format default, colour, LF | R-CLI-006 | output.rs |
| 4 | Shared glue: CommandResult, requests, threads, as-of | R-ARCH-082, R-CLI-004 | commands/mod.rs |
| 5 | Reserved verbs | R-CLI-001 | commands/reserved.rs |
| 6 | Pipeline: load and resolve (read path S1 to S6) | R-ARCH-001, R-CLI-004, R-CLI-008 | pipeline.rs |
| 7 | Pipeline: check, context, path arguments, explain input | R-ARCH-001, R-CHK-040, R-CLI-032 | pipeline.rs |
| 8 | Pipeline: write path (plan, lock, commit) | R-ARCH-001, R-CLI-002, R-CLI-007, R-CLI-008 | pipeline.rs |
| 9 | check command | R-CHK-040, R-CLI-021 | commands/check.rs |
| 10 | context command | R-CLI-032, R-CLI-033 | commands/context.rs |
| 11 | explain and init commands | R-CLI-002, R-CLI-043, R-CLI-035 | commands/explain.rs, commands/init.rs |
| 12 | sync and new commands | R-CLI-002, R-CLI-043, R-CLI-028 | commands/sync.rs, commands/new.rs |
| 13 | approve command | R-CLI-045, R-CLI-037 | commands/approve.rs |
| 14 | Dispatch and main wiring | R-ARCH-001, R-ARCH-101, R-CLI-003 | cli.rs, main.rs |
| 15 | CLI runner and global, reserved, usage trycmd cases | R-CLI-001, R-CLI-003, R-CLI-006 | tests/cli.rs, tests/cli/global.trycmd, tests/cli/reserved.trycmd |
| 16 | Per-verb trycmd cases | R-CHK-040, R-CLI-021, R-CLI-032, R-CLI-043, R-CLI-045 | tests/cli/*.trycmd |
| 17 | Stateful and static guards | R-CHK-040, R-CLI-002, R-CLI-011, R-ARCH-001, R-ARCH-082 | tests/cli.rs |

Every P8 rule in Section 6 appears in at least one row, and the Traceability section maps each rule to its tests. Tasks 1 to 3 and 5 need no repository on disk. Tasks 6 to 8 need the library stages P3 to P7 to exist.

---

### Task 1: Bin manifest, exit mapping and panic boundary

**Files:**
- Create: `crates/fairlead/Cargo.toml`
- Create: `crates/fairlead/src/main.rs`
- Create: `crates/fairlead/src/exit.rs`
- Create: `crates/fairlead/src/output.rs` (this task adds only the internal-error line; Task 3 adds the rest)

**Interfaces**
- Consumes: `fairlead_core::outcome::{ExitCode, VerbOutcome}` (contract 4.3), `fairlead_core::error::{Error, ErrorClass}` (contract 4.3), `fairlead_core::discover::PinRefusal` (contract 4.4, tests only).
- Produces: `exit::to_process_exit(ExitCode) -> std::process::ExitCode`; `exit::exit_for_error(&Error) -> ExitCode`; `main::run_guarded<F: FnOnce() -> ExitCode>(F) -> ExitCode`; `output::write_internal_error(&str)`.

- [ ] **Step 1: Write the manifest.**

```toml
[package]
name = "fairlead"
version = "0.1.0"
description = "Repository-local rules and context for agents (0.1 alpha)"
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
publish = false

[lints]
workspace = true

[[bin]]
name = "fairlead"
path = "src/main.rs"

[dependencies]
fairlead-core = { path = "../fairlead-core" }
camino = "1.2.6"
clap = { version = "4.6.7", features = ["derive", "std", "help", "usage", "error-context"] }
semver = "1.0.28"

[dev-dependencies]
tempfile = "3.27.0"
trycmd = "1.2.1"

[[test]]
name = "cli"
path = "../../tests/cli.rs"
```

Only the `cli` test target is declared here. P9 adds `golden`, `determinism`, `readonly`, `sarif`, `secrets`, `replay` and `platform` when their files exist, because a `[[test]]` whose path is missing breaks `cargo metadata`. `camino` and `semver` are the Section 2 versions, reused here because the bin's argument types need them.

- [ ] **Step 2: Write the failing tests.** Create `exit.rs` with only the test module below. Create `main.rs` with `mod exit; mod output;` and the `#[cfg(test)] mod tests` block from the Step 3 `main.rs` listing, without `run_guarded` or `main` yet. Run `cargo test -p fairlead exit::` and expect a compile error (`cannot find function to_process_exit`).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fairlead_core::discover::PinRefusal;
    use fairlead_core::outcome::VerbOutcome;

    #[test]
    fn r_cli_003__each_exit_code_maps_to_its_status() {
        assert_eq!(to_process_exit(ExitCode::Ok), std::process::ExitCode::from(0));
        assert_eq!(to_process_exit(ExitCode::Findings), std::process::ExitCode::from(1));
        assert_eq!(to_process_exit(ExitCode::Usage), std::process::ExitCode::from(2));
        assert_eq!(to_process_exit(ExitCode::StaleGenerated), std::process::ExitCode::from(3));
        assert_eq!(to_process_exit(ExitCode::Refused), std::process::ExitCode::from(4));
        assert_eq!(to_process_exit(ExitCode::Internal), std::process::ExitCode::from(5));
    }

    #[test]
    fn r_cli_003__error_classes_map_to_usage_refused_internal() {
        let usage = Error::NoRoot { searched_from: "/work".to_string() };
        let refused = Error::Pin(PinRefusal {
            install_line: "cargo install fairlead --version 0.1.0".to_string(),
            reason: "binary older than the pin".to_string(),
        });
        let internal = Error::Internal("boom".to_string());
        assert_eq!(exit_for_error(&usage), ExitCode::Usage);
        assert_eq!(exit_for_error(&refused), ExitCode::Refused);
        assert_eq!(exit_for_error(&internal), ExitCode::Internal);
    }

    #[test]
    fn r_cli_003__verb_outcome_follows_precedence_5_2_4_1_3_0() {
        let mut outcome = VerbOutcome::ok();
        outcome.signal(ExitCode::StaleGenerated);
        outcome.signal(ExitCode::Findings);
        outcome.signal(ExitCode::Refused);
        assert_eq!(outcome.exit_code(), ExitCode::Refused);
        outcome.signal(ExitCode::Usage);
        assert_eq!(outcome.exit_code(), ExitCode::Usage);
        outcome.signal(ExitCode::Internal);
        assert_eq!(outcome.exit_code(), ExitCode::Internal);
    }
}
```

- [ ] **Step 3: Minimal implementation.** Put this above the test module in `exit.rs`:

```rust
//! Maps library results to process exit codes (R-CLI-003, R-ARCH-100). The only status mapping in the bin.

use fairlead_core::error::{Error, ErrorClass};
use fairlead_core::outcome::ExitCode;

/// The status byte comes from `ExitCode` alone (`repr(u8)`, values of R-CLI-003).
pub fn to_process_exit(code: ExitCode) -> std::process::ExitCode {
    std::process::ExitCode::from(code as u8)
}

/// Usage -> 2, Refused -> 4, Internal -> 5 (C-10). An error never maps to 0, 1 or 3.
pub fn exit_for_error(error: &Error) -> ExitCode {
    match error.class() {
        ErrorClass::Usage => ExitCode::Usage,
        ErrorClass::Refused => ExitCode::Refused,
        ErrorClass::Internal => ExitCode::Internal,
    }
}
```

Put this in `output.rs`:

```rust
//! The only module that writes to stdout and stderr (contract Section 3). Output is LF-only (R-CLI-006).

use std::io::Write;

/// The one line a caught panic or a failed write produces (R-ARCH-101).
pub fn write_internal_error(detail: &str) {
    let line = format!("fairlead: internal error (exit 5): {detail}; no result is reported\n");
    let mut err = std::io::stderr().lock();
    // A failed write to a closed stderr has nowhere left to be reported.
    let _ = err.write_all(line.as_bytes());
    let _ = err.flush();
}
```

Write `main.rs` in full (module declarations, `main`, `run_guarded`, `panic_text`, and the tests from Step 2's `run_guarded` tests):

```rust
//! The `fairlead` binary: panic boundary (R-ARCH-101) and process exit. Holds no check, path or file-format logic (R-ARCH-001).

#![deny(unsafe_code)]

mod exit;
mod output;

use std::panic::{self, AssertUnwindSafe};

use fairlead_core::outcome::ExitCode;

fn main() -> std::process::ExitCode {
    // The default hook is silenced; the boundary prints the single line R-ARCH-101 asks for.
    panic::set_hook(Box::new(|_| {}));
    exit::to_process_exit(run_guarded(|| ExitCode::Ok))
}

/// Runs one invocation. A panic becomes exit 5; success is never reported after a panic (R-ARCH-101).
pub fn run_guarded<F>(run: F) -> ExitCode
where
    F: FnOnce() -> ExitCode,
{
    match panic::catch_unwind(AssertUnwindSafe(run)) {
        Ok(code) => code,
        Err(payload) => {
            output::write_internal_error(&panic_text(payload.as_ref()));
            ExitCode::Internal
        }
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "unknown panic".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_arch_101__panic_inside_a_command_exits_5() {
        assert_eq!(run_guarded(|| panic!("worker exploded")), ExitCode::Internal);
    }

    #[test]
    fn r_arch_101__without_a_panic_the_result_is_kept() {
        assert_eq!(run_guarded(|| ExitCode::Findings), ExitCode::Findings);
    }
}
```

- [ ] **Step 4: Run `cargo test -p fairlead` and expect PASS** (the exit, panic and outcome tests all pass).

- [ ] **Step 5: Commit.** Message: `feat(fairlead): bin skeleton, exit mapping and panic boundary` followed by the trailer `Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>`.

---

### Task 2: clap tree and every [0.1] verb argument

**Files:**
- Create: `crates/fairlead/src/cli.rs`
- Modify: `crates/fairlead/src/main.rs` (add `mod cli;`)

**Interfaces**
- Consumes: `fairlead_core::render::OutputFormat` (contract 4.10), `fairlead_core::time::AsOf` with `AsOf::parse` (contract 4.3).
- Produces: `cli::Cli { global: GlobalArgs, command: Option<Command> }`; `cli::GlobalArgs` (contract 4.13 fields, plus `Default`); `cli::ColorArg { Auto, Never }`; `cli::Command` (contract 4.13 variants); `cli::{InitArgs, CheckArgs, SyncArgs, NewArgs, ContextArgs, ExplainArgs, ApproveArgs, ReservedArgs}`; `cli::PresetArg { NoPreset, Rust, Ts }`.

- [ ] **Step 1: Write the failing tests.** Add `mod cli;` to `main.rs` and create `cli.rs` with only the test module below. Run `cargo test -p fairlead cli::` and expect a compile error (`Cli` not found).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("fairlead").chain(args.iter().copied()))
    }

    #[test]
    fn r_cli_001__verb_set_is_exactly_the_ten_verbs() {
        let mut names: Vec<String> = Cli::command()
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["approve", "check", "context", "explain", "init", "move", "new", "seal", "sync", "upgrade"]
        );
    }

    #[test]
    fn r_cli_001__reserved_verbs_parse_so_they_can_refuse() {
        assert!(matches!(parse(&["move", "a", "b"]).unwrap().command, Some(Command::Move(_))));
        assert!(matches!(parse(&["seal", "--commit"]).unwrap().command, Some(Command::Seal(_))));
        assert!(matches!(parse(&["upgrade"]).unwrap().command, Some(Command::Upgrade(_))));
    }

    #[test]
    fn r_cli_006__global_flags_parse_before_and_after_the_verb() {
        let before = parse(&["--quiet", "--no-cache", "check"]).unwrap();
        let after = parse(&["check", "--no-cache", "--color", "never", "--quiet"]).unwrap();
        assert!(before.global.quiet && before.global.no_cache);
        assert!(after.global.quiet && after.global.no_cache);
        assert_eq!(after.global.color, ColorArg::Never);
    }

    #[test]
    fn r_cli_006__format_accepts_the_four_words_only() {
        assert!(parse(&["check", "--format", "sarif"]).is_ok());
        assert!(parse(&["check", "--format", "yaml"]).is_err());
    }

    #[test]
    fn r_cli_006__version_is_a_display_request_not_an_error() {
        let err = parse(&["--version"]).err().unwrap();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn r_cli_006__threads_zero_parses_and_is_rejected_by_the_glue() {
        assert_eq!(parse(&["--threads", "0", "check"]).unwrap().global.threads, Some(0));
    }

    #[test]
    fn r_cli_002__writers_accept_dry_run() {
        assert!(parse(&["init", "--dry-run"]).is_ok());
        assert!(parse(&["sync", "--dry-run"]).is_ok());
        assert!(parse(&["new", "rule", "no-raw-hex", "--dry-run"]).is_ok());
    }

    #[test]
    fn r_cli_021__check_flags_parse_with_budget_max_and_changed() {
        let cli = parse(&["check", "apps/web", "--max", "0", "--budget", "90s", "--changed", "--for-agent"])
            .unwrap();
        match cli.command {
            Some(Command::Check(args)) => {
                assert_eq!(args.paths, vec!["apps/web".to_string()]);
                assert_eq!(args.max, Some(0));
                assert_eq!(args.budget.as_deref(), Some("90s"));
                assert!(args.changed && args.for_agent);
            }
            _ => panic!("expected the check verb"),
        }
    }

    #[test]
    fn r_cli_032__context_target_is_optional() {
        match parse(&["context"]).unwrap().command {
            Some(Command::Context(args)) => assert!(args.target.is_none()),
            _ => panic!("expected the context verb"),
        }
        match parse(&["context", "apps/web/src"]).unwrap().command {
            Some(Command::Context(args)) => assert_eq!(args.target.as_deref(), Some("apps/web/src")),
            _ => panic!("expected the context verb"),
        }
    }

    #[test]
    fn r_cli_043__sync_and_new_accept_yes_as_a_no_op() {
        assert!(parse(&["sync", "--yes"]).is_ok());
        assert!(parse(&["new", "rule", "x", "--yes"]).is_ok());
    }

    #[test]
    fn r_cli_045__approve_is_a_verb_and_new_approval_is_only_data() {
        assert!(matches!(parse(&["approve", "abcd1234efgh"]).unwrap().command, Some(Command::Approve(_))));
        // `new approval` parses (the kind is data); the planner refuses it in Task 12.
        assert!(matches!(parse(&["new", "approval", "x"]).unwrap().command, Some(Command::New(_))));
    }
}
```

- [ ] **Step 2: Minimal implementation.** Put this above the test module in `cli.rs` (the test module stays at the bottom of the file):

```rust
//! clap derive tree: the fixed verb set (R-CLI-001), common flags (R-CLI-006) and the
//! [0.1] arguments of each verb from the flag table of 15-cli.md. Parsing only; no behaviour here.

use camino::Utf8PathBuf;
use clap::builder::TypedValueParser as _;
use clap::{Args, Parser, Subcommand, ValueEnum};
use fairlead_core::render::OutputFormat;
use fairlead_core::time::AsOf;

#[derive(Parser, Debug)]
#[command(name = "fairlead", version, about = "Repository-local rules and context for agents", arg_required_else_help = true)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Flags of R-CLI-006 and the flag table that apply to every verb. All are `global`, so they parse before or after the verb.
#[derive(Args, Debug, Clone, Default)]
pub struct GlobalArgs {
    /// Codebase root; wins over FAIRLEAD_ROOT and the upward search (R-CLI-004).
    #[arg(long, global = true, value_name = "DIR")]
    pub root: Option<Utf8PathBuf>,
    /// Run as if started in DIR; the root search starts there (R-CLI-006).
    #[arg(short = 'C', global = true, value_name = "DIR")]
    pub cwd: Option<Utf8PathBuf>,
    /// Output format: human, terse or json; sarif for check only (R-CLI-006).
    #[arg(long, global = true, value_name = "FORMAT", value_parser = format_parser())]
    pub format: Option<OutputFormat>,
    /// Suppress informational output; errors still print.
    #[arg(long, global = true)]
    pub quiet: bool,
    /// Colour: auto (TTY and NO_COLOR unset) or never (R-CLI-006).
    #[arg(long, global = true, value_enum, default_value = "auto", value_name = "WHEN")]
    pub color: ColorArg,
    /// Date for exp= and date expiry; default UTC today (D-37, R-CHK-014).
    #[arg(long = "as-of", global = true, value_name = "YYYY-MM-DD", value_parser = parse_as_of)]
    pub as_of: Option<AsOf>,
    /// Ignore the .fairlead/ cache; output stays byte-identical (R-CHK-013).
    #[arg(long = "no-cache", global = true)]
    pub no_cache: bool,
    /// Worker threads; FAIRLEAD_THREADS sets the same value and the flag wins (D-54).
    #[arg(long, global = true, value_name = "N")]
    pub threads: Option<usize>,
}

/// `--color` values (R-CLI-006).
#[derive(ValueEnum, Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ColorArg {
    #[default]
    Auto,
    Never,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Scaffold the control file, agent block, ignore lines and baseline; plan first (R-CLI-012).
    Init(InitArgs),
    /// Scan and report drift; never writes (R-CLI-021).
    Check(CheckArgs),
    /// Write generated files, the baseline with --baseline, and the cache (R-CLI-025).
    Sync(SyncArgs),
    /// Scaffold one authored item; never overwrites (R-CLI-028).
    New(NewArgs),
    /// Budgeted brief for an agent: a path, an ID, or the session start (R-CLI-032).
    Context(ContextArgs),
    /// Unbudgeted depth on a file, an ID, a code or a topic (R-CLI-035).
    Explain(ExplainArgs),
    /// Record or list human approvals; terminal-gated (R-CLI-037).
    Approve(ApproveArgs),
    /// Arrives in 0.2 (R-CLI-040).
    Move(ReservedArgs),
    /// Arrives in 0.2 (R-CLI-041).
    Seal(ReservedArgs),
    /// Arrives in 0.2 (R-CLI-042).
    Upgrade(ReservedArgs),
}

#[derive(Args, Debug, Clone)]
pub struct InitArgs {
    /// Print the plan and write nothing; exit 0 (R-CLI-002).
    #[arg(long)]
    pub dry_run: bool,
    /// Apply the printed plan; without it init exits 4 (R-CLI-043).
    #[arg(long)]
    pub yes: bool,
    /// Ecosystem preset: none, rust or ts (R-CLI-012).
    #[arg(long, value_enum, default_value = "none")]
    pub preset: PresetArg,
    /// Skip the baseline (R-CLI-019).
    #[arg(long = "no-baseline")]
    pub no_baseline: bool,
    /// Install git and agent hooks, merged and never default-on (R-CLI-020).
    #[arg(long)]
    pub hooks: bool,
    /// Add the CODEOWNERS line for the managerial folder (R-CLI-015).
    #[arg(long)]
    pub codeowners: bool,
    /// Instruction file that receives the managed block; repeatable (R-CLI-012).
    #[arg(long = "agents", value_name = "FILE")]
    pub agents: Vec<String>,
}

/// `--preset` values (R-CLI-012). Mapped to the planner's `Preset` in Task 11.
#[derive(ValueEnum, Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PresetArg {
    #[default]
    #[value(name = "none")]
    NoPreset,
    Rust,
    Ts,
}

#[derive(Args, Debug, Clone)]
pub struct CheckArgs {
    /// Report filter: only these paths are reported; global checks stay whole-registry (R-CLI-021).
    #[arg(value_name = "PATH")]
    pub paths: Vec<String>,
    /// Terse output, no excerpts, 2 KB cap (R-CHK-039).
    #[arg(long = "for-agent")]
    pub for_agent: bool,
    /// Cap printed findings; 0 prints all (R-CHK-038).
    #[arg(long, value_name = "N")]
    pub max: Option<u32>,
    /// Only findings with this code (R-CHK-038).
    #[arg(long, value_name = "CODE")]
    pub rule: Option<String>,
    /// Print observe-stage findings (R-CLI-021).
    #[arg(long = "show-observe")]
    pub show_observe: bool,
    /// Any non-baselined warning fails the run (R-CHK-040).
    #[arg(long = "deny-warnings")]
    pub deny_warnings: bool,
    /// Zero declarations exits 1 instead of 0 (R-CHK-003).
    #[arg(long = "require-declarations")]
    pub require_declarations: bool,
    /// Report only newline-separated paths; `-` reads stdin (R-CLI-022).
    #[arg(long = "files-from", value_name = "FILE")]
    pub files_from: Option<String>,
    /// Ignore the baseline (R-CLI-021).
    #[arg(long = "no-baseline")]
    pub no_baseline: bool,
    /// Stop at the deadline: integer plus ms, s or m (C-14).
    #[arg(long, value_name = "DURATION")]
    pub budget: Option<String>,
    /// Changed files only: working tree and index against HEAD (D-43).
    #[arg(long)]
    pub changed: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SyncArgs {
    /// Print the plan and write nothing; exit 0 (R-CLI-025).
    #[arg(long)]
    pub dry_run: bool,
    /// Accepted and ignored: sync applies directly (R-CLI-043).
    #[arg(long)]
    pub yes: bool,
    /// Write the baseline; prunes unless --accept-new is given (R-CHK-030).
    #[arg(long)]
    pub baseline: bool,
    /// Add current findings to the baseline; needs --baseline and --why (R-CHK-032).
    #[arg(long = "accept-new", requires = "baseline")]
    pub accept_new: bool,
    /// Reason recorded with accepted findings (R-CHK-032).
    #[arg(long, value_name = "TEXT", requires = "accept_new")]
    pub why: Option<String>,
    /// Accept changed vendor-owned hashes (R-FILE-040).
    #[arg(long = "accept-vendor")]
    pub accept_vendor: bool,
    /// Overwrite a hand-edited generated file; the edit is quarantined first (R-CLI-026).
    #[arg(long)]
    pub force: bool,
    /// Write only that scope's generated files (R-SCOPE-063).
    #[arg(long, value_name = "ID")]
    pub scope: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct NewArgs {
    /// Kind of the item: a declared kind, or scope, rule, escalation or handoff.
    pub kind: String,
    /// Slug name, at most 64 characters (R-CLI-029).
    pub name: String,
    /// Scope that receives the item (R-CLI-028).
    #[arg(long, value_name = "ID")]
    pub scope: Option<String>,
    /// Scaffold field K=V; repeatable (R-CLI-028).
    #[arg(long = "field", value_name = "K=V")]
    pub fields: Vec<String>,
    /// Write the scaffold to stdout only (R-CLI-028).
    #[arg(long)]
    pub print: bool,
    /// Print the plan and write nothing; exit 0 (R-CLI-002).
    #[arg(long)]
    pub dry_run: bool,
    /// Accepted and ignored: new applies directly (R-CLI-043).
    #[arg(long)]
    pub yes: bool,
    /// Folder for `new scope` (R-CLI-030).
    #[arg(long, value_name = "PATH")]
    pub at: Option<String>,
    /// Lift identical file-local lines into one scope-level item (R-SCOPE-013).
    #[arg(long)]
    pub hoist: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ContextArgs {
    /// A path (wins), an ID (explain lookup), or nothing for the session-start brief (R-CLI-032).
    pub target: Option<String>,
    /// Token budget; 0 is unlimited (R-CTX-044).
    #[arg(long, value_name = "N")]
    pub budget: Option<u32>,
    /// Pin one scope (R-CLI-032).
    #[arg(long, value_name = "S")]
    pub scope: Option<String>,
    /// Expand inherited rules (R-CTX-022).
    #[arg(long)]
    pub full: bool,
    /// Index depth for a whole-tree L0 (R-CTX-071).
    #[arg(long, value_name = "N")]
    pub depth: Option<u32>,
    /// Print only the effective digest (R-CTX-033).
    #[arg(long)]
    pub digest: bool,
    /// Count with the configured tokenizer; exit 2 if none (R-CTX-043).
    #[arg(long = "exact-tokens")]
    pub exact_tokens: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ExplainArgs {
    /// A file path, an ID, a diagnostic code or a topic (R-CLI-035).
    pub target: String,
}

#[derive(Args, Debug, Clone)]
pub struct ApproveArgs {
    /// A 12-character fingerprint, or the ID of an item with a pending ovr (C-20).
    pub target: Option<String>,
    /// Approver handle recorded on the approval (R-CLI-037).
    #[arg(long = "as", value_name = "@HANDLE")]
    pub as_handle: Option<String>,
    /// List recorded approvals; read-only (R-CLI-037).
    #[arg(long)]
    pub list: bool,
    /// Revoke an approval by fingerprint (R-CLI-037).
    #[arg(long, value_name = "FP")]
    pub revoke: Option<String>,
}

/// Arguments of a verb that arrives later. They are accepted so the refusal is exact (R-CLI-001).
#[derive(Args, Debug, Clone, Default)]
pub struct ReservedArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub rest: Vec<String>,
}

fn format_parser() -> impl clap::builder::TypedValueParser<Value = OutputFormat> {
    clap::builder::PossibleValuesParser::new(["human", "terse", "json", "sarif"])
        .map(|word: String| format_from_word(&word))
}

fn format_from_word(word: &str) -> OutputFormat {
    match word {
        "terse" => OutputFormat::Terse,
        "json" => OutputFormat::Json,
        "sarif" => OutputFormat::Sarif,
        _ => OutputFormat::Human,
    }
}

fn parse_as_of(text: &str) -> Result<AsOf, String> {
    AsOf::parse(text).map_err(|error| error.to_string())
}
```

Verify: clap 4.6.7 derive accepts `default_value = "auto"` for a `ValueEnum` field, `#[value(name = "none")]` on a variant, and `value_parser = format_parser()` with an `impl TypedValueParser` returned from a function. `camino::Utf8PathBuf` implements `FromStr`, which clap uses for the path fields. `trailing_var_arg` on a `Vec<String>` positional is valid without `num_args`. Adjust the attribute spelling if the compiler rejects it; the flag set does not change.

- [ ] **Step 3: Run `cargo test -p fairlead cli::` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead): clap tree for the fixed verb set and [0.1] flags` and the trailer.

---

### Task 3: Output decisions and the LF writer

**Files:**
- Modify: `crates/fairlead/src/output.rs` (keeps `write_internal_error` from Task 1)

**Interfaces**
- Consumes: `fairlead_core::render::OutputFormat` (contract 4.10), `cli::ColorArg` (Task 2).
- Produces: `output::default_format(Option<OutputFormat>, bool) -> OutputFormat`; `output::colour_enabled(ColorArg, bool, bool) -> bool`; `output::no_color_set() -> bool`; `output::stdout_is_tty() -> bool`; `output::lf_only(&str) -> String`; `output::write_text(&str, &str) -> std::io::Result<()>`.

- [ ] **Step 1: Write the failing tests.** Append this test module to `output.rs` and run `cargo test -p fairlead output::` (expect a compile error for the missing functions).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ColorArg;

    #[test]
    fn r_cli_006__format_defaults_to_human_on_a_tty_and_terse_when_piped() {
        assert_eq!(default_format(None, true), OutputFormat::Human);
        assert_eq!(default_format(None, false), OutputFormat::Terse);
    }

    #[test]
    fn r_cli_006__an_explicit_format_always_wins() {
        assert_eq!(default_format(Some(OutputFormat::Json), true), OutputFormat::Json);
        assert_eq!(default_format(Some(OutputFormat::Sarif), false), OutputFormat::Sarif);
    }

    #[test]
    fn r_cli_006__colour_is_off_when_not_a_tty_or_no_color_is_set() {
        assert!(!colour_enabled(ColorArg::Auto, false, false));
        assert!(!colour_enabled(ColorArg::Auto, true, true));
        assert!(colour_enabled(ColorArg::Auto, true, false));
        assert!(!colour_enabled(ColorArg::Never, true, false));
    }

    #[test]
    fn r_cli_006__output_is_lf_only_with_one_trailing_newline() {
        assert_eq!(lf_only("a\r\nb\r\n\r\n"), "a\nb\n");
        assert_eq!(lf_only("x\r"), "x\n");
        assert_eq!(lf_only(""), "");
        assert!(!lf_only("a\r\n").contains('\r'));
    }
}
```

`OutputFormat` must derive `PartialEq` and `Debug` for `assert_eq!`; the contract's "every type derives Debug, Clone, PartialEq, Eq" rule covers it. Verify against P5 `render/mod.rs`.

- [ ] **Step 2: Minimal implementation.** Replace the `use std::io::Write;` line at the top of `output.rs` with the block below and append the functions before the test module:

```rust
use std::io::{IsTerminal, Write};

use fairlead_core::render::OutputFormat;

use crate::cli::ColorArg;

/// R-CLI-006 and C-28: an explicit `--format` wins; otherwise human on a TTY, terse when piped.
pub fn default_format(explicit: Option<OutputFormat>, stdout_is_tty: bool) -> OutputFormat {
    match explicit {
        Some(format) => format,
        None if stdout_is_tty => OutputFormat::Human,
        None => OutputFormat::Terse,
    }
}

/// R-CLI-006: colour is off when not a TTY or when NO_COLOR is set; `--color never` always wins.
pub fn colour_enabled(color: ColorArg, stdout_is_tty: bool, no_color_set: bool) -> bool {
    match color {
        ColorArg::Never => false,
        ColorArg::Auto => stdout_is_tty && !no_color_set,
    }
}

/// NO_COLOR counts when present and not empty (no-color.org).
pub fn no_color_set() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
}

pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// R-CLI-006, R-ARCH-052: LF-only text with exactly one trailing newline when non-empty.
pub fn lf_only(text: &str) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    if unified.is_empty() {
        return unified;
    }
    let mut out = String::with_capacity(unified.len() + 1);
    out.push_str(unified.trim_end_matches('\n'));
    out.push('\n');
    out
}

/// Writes a verb's stdout and stderr text. Every byte goes through `lf_only`; a failed write is an error the caller maps to exit 5.
pub fn write_text(stdout: &str, stderr: &str) -> std::io::Result<()> {
    if !stdout.is_empty() {
        let mut out = std::io::stdout().lock();
        out.write_all(lf_only(stdout).as_bytes())?;
        out.flush()?;
    }
    if !stderr.is_empty() {
        let mut err = std::io::stderr().lock();
        err.write_all(lf_only(stderr).as_bytes())?;
        err.flush()?;
    }
    Ok(())
}
```

`lf_only("a\r\nb\r\n\r\n")` gives `"a\nb\n"` because trailing newlines are trimmed before one is re-added. Renderers in P5 already emit LF, so this guard changes no correct output.

- [ ] **Step 3: Run `cargo test -p fairlead output::` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead): output decisions for format, colour and LF-only text` and the trailer.

---

### Task 4: Shared command glue, threads and the as-of date

**Files:**
- Create: `crates/fairlead/src/commands/mod.rs`
- Modify: `crates/fairlead/src/main.rs` (add `mod commands;`)

**Interfaces**
- Consumes: `discover::DiscoverRequest` (contract 4.4), `pipeline::LoadRequest` (contract 4.13), `time::AsOf::today_utc` (contract 4.3), `outcome::{ExitCode, VerbOutcome}`, `error::Error`, `path::RepoPath`.
- Produces: `commands::CommandResult { stdout, stderr, outcome }` with `text(String)`, `failed(&Error)` and `exit()`; `commands::discover_request(&GlobalArgs, bool) -> Result<DiscoverRequest, Error>`; `commands::load_request(&GlobalArgs, bool, Option<BTreeSet<RepoPath>>) -> Result<LoadRequest, Error>`; `commands::as_of(&GlobalArgs) -> AsOf`; `commands::working_dir(&GlobalArgs) -> Result<Utf8PathBuf, Error>`; `commands::resolve_threads(Option<usize>, Option<&str>) -> Result<Option<usize>, Error>`; `commands::env_root_from(Option<OsString>) -> Result<Option<Utf8PathBuf>, Error>`.

- [ ] **Step 1: Write the failing tests.** Create `commands/mod.rs` containing only the test module below and `mod commands;` in `main.rs`. Run `cargo test -p fairlead commands::` and expect a compile error.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fairlead_core::error::ErrorClass;

    #[test]
    fn r_arch_082__the_flag_wins_over_the_environment() {
        assert!(matches!(resolve_threads(Some(3), Some("8")), Ok(Some(3))));
    }

    #[test]
    fn r_arch_082__the_environment_is_used_when_the_flag_is_absent() {
        assert!(matches!(resolve_threads(None, Some(" 8 ")), Ok(Some(8))));
        assert!(matches!(resolve_threads(None, None), Ok(None)));
    }

    #[test]
    fn r_arch_082__zero_and_non_integers_are_usage_errors() {
        let results = [
            resolve_threads(Some(0), None),
            resolve_threads(None, Some("0")),
            resolve_threads(None, Some("many")),
        ];
        for result in results {
            assert!(matches!(result.err().map(|e| e.class()), Some(ErrorClass::Usage)));
        }
    }

    #[test]
    fn r_cli_004__an_empty_fairlead_root_counts_as_unset() {
        assert!(env_root_from(Some(OsString::new())).unwrap().is_none());
        assert!(env_root_from(None).unwrap().is_none());
    }

    #[test]
    fn r_cli_003__a_failed_command_result_carries_the_class_exit_code() {
        let result = CommandResult::failed(&Error::Internal("boom".to_string()));
        assert_eq!(result.exit(), ExitCode::Internal);
        assert_eq!(result.stderr, "boom\n");
    }
}
```

- [ ] **Step 2: Minimal implementation.** Put this above the test module in `commands/mod.rs`:

```rust
//! Command modules and the glue they share: `CommandResult`, request builders, threads and the as-of date.
//! Environment reads here are limited to FAIRLEAD_ROOT and FAIRLEAD_THREADS (constraint 9).

pub mod approve;
pub mod check;
pub mod context;
pub mod explain;
pub mod init;
pub mod new;
pub mod reserved;
pub mod sync;

use std::collections::BTreeSet;
use std::ffi::OsString;

use camino::Utf8PathBuf;
use fairlead_core::discover::DiscoverRequest;
use fairlead_core::error::Error;
use fairlead_core::outcome::{ExitCode, VerbOutcome};
use fairlead_core::path::RepoPath;
use fairlead_core::pipeline::LoadRequest;
use fairlead_core::time::AsOf;

use crate::cli::GlobalArgs;
use crate::exit::exit_for_error;

/// What a verb hands to `output::write_text`, plus the exit code it signalled.
#[derive(Default)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub outcome: VerbOutcome,
}

impl CommandResult {
    pub fn text(stdout: String) -> Self {
        Self { stdout, ..Self::default() }
    }

    /// An error stops the verb: its Display goes to stderr and its class sets the exit code.
    pub fn failed(error: &Error) -> Self {
        let mut outcome = VerbOutcome::ok();
        outcome.signal(exit_for_error(error));
        Self { stdout: String::new(), stderr: format!("{error}\n"), outcome }
    }

    pub fn exit(&self) -> ExitCode {
        self.outcome.exit_code()
    }
}

/// `-C` joined onto the working directory when relative; discover does the cleaning and the existence check (C-10).
pub fn working_dir(global: &GlobalArgs) -> Result<Utf8PathBuf, Error> {
    let raw = std::env::current_dir()
        .map_err(|e| Error::Internal(format!("cannot read the working directory: {e}")))?;
    let base = Utf8PathBuf::from_path_buf(raw)
        .map_err(|p| Error::Usage(format!("working directory is not UTF-8: {}", p.display())))?;
    Ok(match &global.cwd {
        None => base,
        Some(dir) if dir.is_absolute() => dir.clone(),
        Some(dir) => base.join(dir),
    })
}

pub fn binary_version() -> Result<semver::Version, Error> {
    semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|e| Error::Internal(format!("binary version is not semver: {e}")))
}

/// FAIRLEAD_ROOT as the discover request's `env_root`; an empty value counts as unset (R-CLI-004).
pub fn env_root_from(value: Option<OsString>) -> Result<Option<Utf8PathBuf>, Error> {
    match value {
        None => Ok(None),
        Some(text) if text.is_empty() => Ok(None),
        Some(text) => Utf8PathBuf::from_path_buf(std::path::PathBuf::from(text))
            .map(Some)
            .map_err(|p| Error::Usage(format!("FAIRLEAD_ROOT is not UTF-8: {}", p.display()))),
    }
}

pub fn discover_request(global: &GlobalArgs, require_root: bool) -> Result<DiscoverRequest, Error> {
    Ok(DiscoverRequest {
        root_flag: global.root.clone(),
        env_root: env_root_from(std::env::var_os("FAIRLEAD_ROOT"))?,
        cwd: working_dir(global)?,
        binary_version: binary_version()?,
        require_root,
    })
}

/// D-54: a flag wins; `0` or a non-integer is a usage error (exit 2).
pub fn resolve_threads(flag: Option<usize>, env: Option<&str>) -> Result<Option<usize>, Error> {
    let requested = match (flag, env) {
        (Some(n), _) => n,
        (None, Some(text)) => text.trim().parse::<usize>().map_err(|_| {
            Error::Usage(format!("FAIRLEAD_THREADS must be a whole number of at least 1, got {text:?}"))
        })?,
        (None, None) => return Ok(None),
    };
    if requested == 0 {
        return Err(Error::Usage("the thread count must be at least 1".to_string()));
    }
    Ok(Some(requested))
}

fn env_threads() -> Result<Option<String>, Error> {
    match std::env::var("FAIRLEAD_THREADS") {
        Ok(text) => Ok(Some(text)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(Error::Usage("FAIRLEAD_THREADS is not valid UTF-8".to_string()))
        }
    }
}

/// The read-path request. `FAIRLEAD_THREADS` is read only when `--threads` is absent (flag wins).
pub fn load_request(
    global: &GlobalArgs,
    want_changed: bool,
    only: Option<BTreeSet<RepoPath>>,
) -> Result<LoadRequest, Error> {
    let env = if global.threads.is_none() { env_threads()? } else { None };
    let threads = resolve_threads(global.threads, env.as_deref())?;
    Ok(LoadRequest {
        discover: discover_request(global, true)?,
        use_cache: !global.no_cache,
        threads,
        want_changed,
        only,
    })
}

/// `--as-of`, or UTC today when absent (the only clock read in the bin, through the library type).
pub fn as_of(global: &GlobalArgs) -> AsOf {
    global.as_of.clone().unwrap_or_else(AsOf::today_utc)
}
```

`commands/mod.rs` also needs the `ExitCode` import used by the test; the test uses `ExitCode`, so add `use fairlead_core::outcome::ExitCode;` in the test module (already imported via `super::*` because the module imports `ExitCode`). Verify `Utf8PathBuf::is_absolute` and `join` exist in camino 1.2.6.

- [ ] **Step 3: Run `cargo test -p fairlead commands::` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead): shared command glue, threads and as-of resolution` and the trailer.

---

### Task 5: Reserved verbs refuse with the milestone text

**Files:**
- Create: `crates/fairlead/src/commands/reserved.rs`

**Interfaces**
- Consumes: `error::Error::Reserved { verb, milestone }` (contract 4.3), `commands::CommandResult::failed` (Task 4), `cli::GlobalArgs`.
- Produces: `commands::reserved::ReservedVerb { Move, Seal, Upgrade }` with `name()` and `milestone()`; `commands::reserved::run(&GlobalArgs, ReservedVerb) -> CommandResult`.

- [ ] **Step 1: Write the failing test.** Create `reserved.rs` with only this test module, and run `cargo test -p fairlead reserved::`. Expect a compile error.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::GlobalArgs;
    use fairlead_core::outcome::ExitCode;

    #[test]
    fn r_cli_001__each_reserved_verb_refuses_with_its_milestone_and_exit_2() {
        let cases = [
            (ReservedVerb::Move, "move arrives in 0.2\n"),
            (ReservedVerb::Seal, "seal arrives in 0.2\n"),
            (ReservedVerb::Upgrade, "upgrade arrives in 0.2\n"),
        ];
        for (verb, text) in cases {
            let result = run(&GlobalArgs::default(), verb);
            assert_eq!(result.stderr, text);
            assert!(result.stdout.is_empty());
            assert_eq!(result.exit(), ExitCode::Usage);
        }
    }
}
```

- [ ] **Step 2: Minimal implementation.** Put this above the test module:

```rust
//! Verbs that arrive in a later milestone (R-CLI-001). They parse, then refuse with exit 2 before any work.

use fairlead_core::error::Error;

use crate::cli::GlobalArgs;
use crate::commands::CommandResult;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReservedVerb {
    Move,
    Seal,
    Upgrade,
}

impl ReservedVerb {
    pub fn name(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Seal => "seal",
            Self::Upgrade => "upgrade",
        }
    }

    /// 15-cli.md verb summary: `move`, `seal` and `upgrade` are all 0.2 (D-38).
    pub fn milestone(self) -> &'static str {
        "0.2"
    }
}

/// The refusal is `Error::Reserved`, so its Display text is P3's; the test below pins the exact line.
pub fn run(_global: &GlobalArgs, verb: ReservedVerb) -> CommandResult {
    CommandResult::failed(&Error::Reserved { verb: verb.name(), milestone: verb.milestone() })
}
```

Verify: P3 `error.rs` renders `Error::Reserved` as exactly `<verb> arrives in <milestone>`. If it does not, the failing test in Step 1 shows the mismatch, and the fix belongs in P3's Display, not here.

- [ ] **Step 3: Run `cargo test -p fairlead reserved::` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead): reserved verbs refuse with the milestone text and exit 2` and the trailer.

---

## Pipeline (fairlead-core) and its three extensions

`pipeline.rs` is P8's module (contract 4.13, Section 5). It sequences the seams and holds no rule, path or format logic of its own (R-ARCH-001). Three extensions to the contract's 4.13 shapes are needed to make the sequence implementable. Each one is recorded under Contract issues with its owner:

1. **`Loaded` gains `root_control: ControlFile` and `cwd_rel: RepoPath`.** `discover` already returns both (contract 4.4 `Discovered`); `load` keeps them. Without the root control, `check` cannot read `[baseline] file` and `context` cannot read `[context]`.
2. **Write-path functions are added** (`plan_new`, `plan_sync`, `plan_approve_loaded`, `commit_plan`). Section 4.13 names no write-path composition, and `commands/*` must not chain `write::*` itself, because that would put sequencing in the bin.
3. **Two P3/P5 conversions are called but unnamed** in the contract: the planner for extraction settings (`extract::extract_plan_from_controls`) and the baseline-issue diagnostic (`impl From<BaselineIssue> for Diagnostic`). Both are listed in Contract issues.

Tests for this layer sit in `crates/fairlead-core/tests/pipeline.rs`. They use `tempfile` (a dev-dependency of `fairlead-core`, Section 2) and write real files, which is how the read path is exercised end to end. Each pure helper is also covered by a unit test in its module.

---

### Task 6: Pipeline load and resolve (S1 to S6)

**Files:**
- Create: `crates/fairlead-core/src/pipeline.rs`
- Modify: `crates/fairlead-core/src/lib.rs` (add `pub mod pipeline;`; P3 leaves it out until P8)
- Create: `crates/fairlead-core/tests/pipeline.rs`

**Interfaces**
- Consumes: `discover::{discover, DiscoverRequest, Discovered, Root}` (S1); `walk::{walk, WalkOptions, WalkResult}` (S2); `config::{load_controls, ControlSet, ControlFile}` (S3); `extract::{extract_plan_from_controls (P3 constructor, Contract issues), extract_files, Corpus}` (S4); `facts::{collect_facts, Facts}` (S5); `registry::{resolve, ResolveInput, Registry}` (S6); `cache::{Cache, CacheKey}`; `fairlead_syntax::GRAMMAR_EDITION`; `time::AsOf`; `error::Error`; `diag::Diagnostic`.
- Produces: `pipeline::LoadRequest`, `pipeline::Loaded` (contract fields plus the two extensions), `pipeline::load(&LoadRequest) -> Result<Loaded, Error>`, `pipeline::resolve_loaded(&Loaded, AsOf) -> Registry`.

- [ ] **Step 1: Write the failing tests.** Create `crates/fairlead-core/tests/pipeline.rs` with this content. Run `cargo test -p fairlead-core --test pipeline` and expect a compile error (no `pipeline` module).

```rust
use camino::Utf8PathBuf;
use fairlead_core::discover::DiscoverRequest;
use fairlead_core::error::ErrorClass;
use fairlead_core::pipeline::{load, resolve_loaded, LoadRequest};
use fairlead_core::time::AsOf;

const ROOT_CONTROL: &str =
    "[fairlead]\nscope = \"root\"\nedition = 1\nroot = true\nmarker = \"fairlead\"\n\n[settings]\n";

fn temp_root(with_control: bool) -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    if with_control {
        std::fs::create_dir_all(dir.path().join("fairlead")).unwrap();
        std::fs::write(dir.path().join("fairlead").join("control.toml"), ROOT_CONTROL).unwrap();
    }
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    (dir, root)
}

fn request(root: &Utf8PathBuf) -> LoadRequest {
    LoadRequest {
        discover: DiscoverRequest {
            root_flag: Some(root.clone()),
            env_root: None,
            cwd: root.clone(),
            binary_version: semver::Version::new(0, 1, 0),
            require_root: true,
        },
        use_cache: false,
        threads: Some(1),
        want_changed: false,
        only: None,
    }
}

#[test]
fn r_cli_004__load_without_a_root_is_a_usage_error() {
    let (_dir, root) = temp_root(false);
    let error = load(&request(&root)).err().unwrap();
    assert!(matches!(error.class(), ErrorClass::Usage));
}

#[test]
fn r_arch_001__load_then_resolve_yields_the_root_scope() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let registry = resolve_loaded(&loaded, AsOf::parse("2026-10-09").unwrap());
    assert_eq!(registry.root_scope().0.as_str(), "root");
}

#[test]
fn r_cli_008__readers_never_take_the_writer_lock() {
    let (_dir, root) = temp_root(true);
    let _loaded = load(&request(&root)).unwrap();
    assert!(!root.join(".fairlead").join("lock").exists());
}
```

- [ ] **Step 2: Minimal implementation.** Write `pipeline.rs` with this content:

```rust
//! Composes the seams of contract Section 5. The only module that calls several stages.
//! Sequencing only: no rule, path or file-format logic lives here (R-ARCH-001).

use std::collections::BTreeSet;

use fairlead_syntax::GRAMMAR_EDITION;

use crate::cache::{Cache, CacheKey};
use crate::config::{ControlFile, ControlSet};
use crate::diag::Diagnostic;
use crate::discover::{discover, DiscoverRequest, Discovered, Root};
use crate::error::Error;
use crate::extract::{extract_files, extract_plan_from_controls};
use crate::facts::{collect_facts, Facts};
use crate::path::RepoPath;
use crate::registry::{resolve, Registry, ResolveInput};
use crate::time::AsOf;
use crate::walk::{walk, WalkOptions, WalkResult};
use crate::extract::Corpus;

pub struct LoadRequest {
    pub discover: DiscoverRequest,
    pub use_cache: bool,
    pub threads: Option<usize>,
    pub want_changed: bool,
    pub only: Option<BTreeSet<RepoPath>>,
}

/// Everything a reader verb needs, gathered once. `root_control` and `cwd_rel` are the P8 extensions of Contract issues CI-1.
pub struct Loaded {
    pub root: Root,
    pub root_control: ControlFile,
    pub cwd_rel: RepoPath,
    pub controls: ControlSet,
    pub walk: WalkResult,
    pub corpus: Corpus,
    pub facts: Facts,
    pub load_diags: Vec<Diagnostic>,
    pub cache: Cache,
}

/// discover -> walk -> controls -> extract -> facts (S1 to S5). Reader verbs call this; only `write::*` takes the writer lock.
pub fn load(req: &LoadRequest) -> Result<Loaded, Error> {
    let Discovered { root, root_control, cwd_rel } = discover(&req.discover)?;
    let walk_options = WalkOptions {
        excludes: root_control.scan.exclude.clone(),
        max_file: root_control.scan.max_file,
        threads: req.threads,
    };
    let walked = walk(&root, &walk_options)?;
    let controls = crate::config::load_controls(&root, &walked)?;
    let plan = extract_plan_from_controls(&root_control, &controls);
    let cache = Cache::open(&root, req.use_cache, cache_key(&root));
    let corpus = extract_files(&walked, &root, &plan, &cache, req.only.as_ref())?;
    let facts = collect_facts(&root, &walked, &controls, req.want_changed)?;
    let load_diags = collect_load_diags(&walked, &controls);
    Ok(Loaded { root, root_control, cwd_rel, controls, walk: walked, corpus, facts, load_diags, cache })
}

pub fn resolve_loaded(loaded: &Loaded, as_of: AsOf) -> Registry {
    resolve(&ResolveInput {
        root: &loaded.root,
        controls: &loaded.controls,
        corpus: &loaded.corpus,
        facts: &loaded.facts,
        as_of,
    })
}

fn cache_key(root: &Root) -> CacheKey {
    CacheKey {
        fairlead_version: env!("CARGO_PKG_VERSION").to_string(),
        grammar: GRAMMAR_EDITION,
        config_hash: root.config_hash.clone(),
        as_of: None,
    }
}

/// Walk diagnostics, every control file's parse failures and its parse-time violations, in path order.
fn collect_load_diags(walked: &WalkResult, controls: &ControlSet) -> Vec<Diagnostic> {
    let mut diags = walked.diags.clone();
    for result in controls.files.values() {
        match result {
            Ok(control) => diags.extend(control.violations.iter().cloned()),
            Err(failure) => diags.extend(failure.diags.iter().cloned()),
        }
    }
    diags
}
```

The module needs `Corpus` in its import list from `crate::extract`; the import block above lists it on its own line. Verify: `extract::extract_plan_from_controls` is the P3 constructor named in Contract issues CI-3, and `Cache::open` is infallible (contract 4.7).

- [ ] **Step 3: Run `cargo test -p fairlead-core --test pipeline` and expect PASS** for the three tests.

- [ ] **Step 4: Commit** with message `feat(fairlead-core): pipeline load and resolve over the read-path seams` and the trailer.

---

### Task 7: Pipeline check, context and path helpers

**Files:**
- Modify: `crates/fairlead-core/src/pipeline.rs`
- Modify: `crates/fairlead-core/tests/pipeline.rs` (append tests)

**Interfaces**
- Consumes: `check::{run, CheckInput, CheckOptions, CheckReport}` (S7); `check::baseline::{Baseline, BaselineIssue}` (P5); `context::{build, parse_target, ContextArgError, ContextInput, ContextOutput, ContextRequest, ContextTarget}` (S9); `explain::{parse_explain_target, ExplainTarget}` (P7); `generate` through `check::run` (S10).
- Produces: `pipeline::CheckContext { diags, baseline }`; `pipeline::check_context(&Loaded, &CheckOptions) -> CheckContext`; `pipeline::check_loaded(&Loaded, &Registry, &CheckOptions) -> Result<CheckReport, Error>`; `pipeline::context_loaded(&Loaded, &Registry, &CheckReport, &ContextRequest) -> ContextOutput`; `pipeline::normalise(&Utf8Path) -> Utf8PathBuf`; `pipeline::to_root_relative(&Root, &Utf8Path, &str) -> Result<RepoPath, Error>`; `pipeline::context_target(&Root, &Utf8Path, Option<&str>) -> Result<ContextTarget, Error>`; `pipeline::explain_target(&Root, &Utf8Path, &str) -> Result<ExplainTarget, Error>`.

- [ ] **Step 1: Write the failing tests.** Append to `crates/fairlead-core/tests/pipeline.rs`:

```rust
use fairlead_core::check::{CheckOptions, MaxFindings, Ramp};
use fairlead_core::context::ContextTarget;
use fairlead_core::pipeline::{check_context, check_loaded, context_target, explain_target, to_root_relative};

fn check_options() -> CheckOptions {
    CheckOptions {
        as_of: AsOf::parse("2026-10-09").unwrap(),
        changed: false,
        files_from: None,
        paths: Vec::new(),
        rule_filter: None,
        max: MaxFindings::Default,
        ramp: Ramp { show_observe: false, deny_warnings: false, nudge_days: [60, 120] },
        for_agent: false,
        no_baseline: false,
        require_declarations: false,
        budget: None,
    }
}

#[test]
fn r_chk_040__an_empty_root_checks_clean_with_exit_0() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let registry = resolve_loaded(&loaded, AsOf::parse("2026-10-09").unwrap());
    let options = check_options();
    let report = check_loaded(&loaded, &registry, &options).unwrap();
    assert!(report.zero_declarations);
    assert_eq!(report.outcome(&options).exit_code(), fairlead_core::outcome::ExitCode::Ok);
}

#[test]
fn r_arch_001__a_missing_baseline_file_means_no_baseline() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let context = check_context(&loaded, &check_options());
    assert!(context.baseline.is_none());
    assert!(context.diags.is_empty());
}

#[test]
fn r_cli_032__a_task_query_is_a_usage_error_until_0_3() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let error = context_target(&loaded.root, &loaded.root.dir, Some("add a progress bar")).err().unwrap();
    assert!(matches!(error.class(), ErrorClass::Usage));
    assert!(error.to_string().contains("task queries need 0.3"));
}

#[test]
fn r_cli_032__no_argument_is_the_session_start() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let target = context_target(&loaded.root, &loaded.root.dir, None).unwrap();
    assert!(matches!(target, ContextTarget::SessionStart));
}

#[test]
fn r_cli_032__an_existing_path_is_a_path_query() {
    let (_dir, root) = temp_root(true);
    std::fs::create_dir_all(root.join("apps/web")).unwrap();
    std::fs::write(root.join("apps/web/app.ts"), "export {};\n").unwrap();
    let loaded = load(&request(&root)).unwrap();
    match context_target(&loaded.root, &loaded.root.dir, Some("apps/web/app.ts")).unwrap() {
        ContextTarget::Path(path) => assert_eq!(path.as_str(), "apps/web/app.ts"),
        _ => panic!("expected a path query"),
    }
}

#[test]
fn r_cli_032__an_id_shaped_word_is_an_id_lookup() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let target = context_target(&loaded.root, &loaded.root.dir, Some("cmp.button")).unwrap();
    assert!(matches!(target, ContextTarget::IdLookup(_)));
}

#[test]
fn r_cli_032__a_path_above_the_root_is_a_usage_error() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let error = to_root_relative(&loaded.root, &loaded.root.dir, "../../../../outside").err().unwrap();
    assert!(matches!(error.class(), ErrorClass::Usage));
}

#[test]
fn r_cli_035__an_existing_file_beats_an_id_shaped_word_but_not_a_topic() {
    let (_dir, root) = temp_root(true);
    std::fs::write(root.join("cmp.button"), "x\n").unwrap();
    let loaded = load(&request(&root)).unwrap();
    let file = explain_target(&loaded.root, &loaded.root.dir, "cmp.button").unwrap();
    assert!(matches!(file, fairlead_core::explain::ExplainTarget::File(_)));
    let topic = explain_target(&loaded.root, &loaded.root.dir, "syntax").unwrap();
    assert!(matches!(topic, fairlead_core::explain::ExplainTarget::Topic(_)));
}
```

Run `cargo test -p fairlead-core --test pipeline` and expect compile errors for the missing helpers.

- [ ] **Step 2: Minimal implementation.** Append the block below to the end of `pipeline.rs`. Its `use` lines belong in the import block at the top of the file, so move them there when typing the file. The block:

```rust
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};

use crate::check::baseline::Baseline;
use crate::check::{run as check_run, CheckInput, CheckOptions, CheckReport};
use crate::context::{build as build_context, parse_target, ContextArgError, ContextInput, ContextOutput, ContextRequest, ContextTarget};
use crate::explain::{parse_explain_target, ExplainTarget};

/// Load-time diagnostics plus the baseline of this run (R-CHK-030). Bytes come from `Facts.generated_on_disk`, so no second read occurs.
pub struct CheckContext {
    pub diags: Vec<Diagnostic>,
    pub baseline: Option<Baseline>,
}

pub fn check_context(loaded: &Loaded, options: &CheckOptions) -> CheckContext {
    let mut diags = loaded.load_diags.clone();
    let mut baseline = None;
    if !options.no_baseline {
        if let Ok(path) = RepoPath::new(&loaded.root_control.baseline.file) {
            if let Some(bytes) = loaded.facts.generated_on_disk.get(&path) {
                match Baseline::parse(bytes) {
                    Ok(parsed) => baseline = Some(parsed),
                    Err(issue) => diags.push(Diagnostic::from(issue)),
                }
            }
        }
    }
    CheckContext { diags, baseline }
}

pub fn check_loaded(loaded: &Loaded, registry: &Registry, options: &CheckOptions) -> Result<CheckReport, Error> {
    let context = check_context(loaded, options);
    let input = CheckInput {
        registry,
        corpus: &loaded.corpus,
        facts: &loaded.facts,
        load_diags: &context.diags,
        baseline: context.baseline.as_ref(),
        options,
    };
    Ok(check_run(&input))
}

pub fn context_loaded(
    loaded: &Loaded,
    registry: &Registry,
    report: &CheckReport,
    request: &ContextRequest,
) -> ContextOutput {
    build_context(&ContextInput {
        registry,
        corpus: &loaded.corpus,
        report,
        config: &loaded.root_control.context,
        request,
        cache: &loaded.cache,
    })
}

/// Lexical normalisation: `.` dropped, `..` removes the previous component. Containment is decided later by `strip_prefix`.
pub fn normalise(path: &Utf8Path) -> Utf8PathBuf {
    let mut out = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_str()),
        }
    }
    out
}

/// A path argument as a repo-relative path (C-13): relative to the working directory, then the root. Outside the root is `PathOutsideRoot`.
pub fn to_root_relative(root: &Root, cwd: &Utf8Path, text: &str) -> Result<RepoPath, Error> {
    let typed = Utf8Path::new(text);
    let joined = if typed.is_absolute() { typed.to_path_buf() } else { cwd.join(typed) };
    let absolute = normalise(&joined);
    let outside = || Error::PathOutsideRoot { path: text.to_string() };
    let relative = absolute.strip_prefix(&root.dir).map_err(|_| outside())?;
    RepoPath::new(relative.as_str()).map_err(|_| outside())
}

pub fn context_target(root: &Root, cwd: &Utf8Path, arg: Option<&str>) -> Result<ContextTarget, Error> {
    let exists = |path: &RepoPath| root.dir.join(path.as_str()).exists();
    let root_relative = |text: &str| to_root_relative(root, cwd, text).map_err(|_| ContextArgError::OutsideRoot);
    parse_target(arg, &exists, &root_relative).map_err(|error| match error {
        ContextArgError::NoMatchTaskQuery => {
            Error::Usage("no match: task queries need 0.3; pass a path or an ID".to_string())
        }
        ContextArgError::OutsideRoot => Error::PathOutsideRoot { path: arg.unwrap_or_default().to_string() },
    })
}

/// C-21 order: topic and `HN-` code words are never paths; otherwise an existing path wins over an ID.
pub fn explain_target(root: &Root, cwd: &Utf8Path, arg: &str) -> Result<ExplainTarget, Error> {
    let exists = |path: &RepoPath| root.dir.join(path.as_str()).exists();
    let topic_or_code = matches!(arg, "syntax" | "kinds" | "approve") || arg.starts_with("HN-");
    let text = if topic_or_code {
        arg.to_string()
    } else {
        match to_root_relative(root, cwd, arg) {
            Ok(path) if exists(&path) => path.as_str().to_string(),
            _ => arg.to_string(),
        }
    };
    parse_explain_target(&text, &exists)
}
```

Notes: `check::run` is imported under the alias `check_run` so the call reads clearly in this module. `RepoPath::new("")` (the root itself) is not defined by the contract; Contract issues CI-6 records the question. `explain_target` relies on `parse_explain_target` (contract 4.12) accepting a root-relative string.

- [ ] **Step 3: Run `cargo test -p fairlead-core --test pipeline` and expect PASS.** The `r_arch_001__a_baseline_is_read_from_facts...` test checks that an absent baseline file yields `None`; the baseline-present path is covered by P5 and by the golden tests in P9.

- [ ] **Step 4: Commit** with message `feat(fairlead-core): pipeline check, context and path helpers` and the trailer.

---

### Task 8: Pipeline write path (plan, lock, commit)

**Files:**
- Modify: `crates/fairlead-core/src/pipeline.rs` (append)
- Modify: `crates/fairlead-core/tests/pipeline.rs` (append)

**Interfaces**
- Consumes: `write::{new_plan, sync_plan, approve_preview, approve_plan, NewRequest, SyncRequest, ApproveRequest, ApprovePreview, Plan, WriterLock, Transaction, CommitReport, Recovery, PidLiveness}` (P7, contract 4.12); `check::run` through the `check_run` alias; `check_context` (Task 7).
- Produces: `pipeline::WriteOutcome { recovery, report }`; `pipeline::plan_new`; `pipeline::plan_sync`; `pipeline::approve_preview_loaded`; `pipeline::plan_approve_loaded`; `pipeline::commit_plan`. `plan_init` lives with the init command (Task 11), because its first scan needs an in-memory root (Contract issues CI-5).

- [ ] **Step 1: Write the failing tests.** Append to `tests/pipeline.rs`:

```rust
use std::collections::BTreeMap;

use fairlead_core::pipeline::{commit_plan, plan_new};
use fairlead_core::write::{LivenessAnswer, NewRequest, PidLiveness, WriterLock};

struct AlwaysAlive;

impl PidLiveness for AlwaysAlive {
    fn is_alive(&self, _pid: u32, _host: &str) -> LivenessAnswer {
        LivenessAnswer::Alive
    }
}

fn rule_request() -> NewRequest {
    NewRequest {
        kind: "rule".to_string(),
        name: "no-raw-hex".to_string(),
        scope: None,
        fields: BTreeMap::new(),
        print: false,
        at: None,
        hoist: false,
    }
}

#[test]
fn r_arch_001__commit_plan_writes_and_releases_the_lock() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let as_of = AsOf::parse("2026-10-09").unwrap();
    let registry = resolve_loaded(&loaded, as_of.clone());
    let plan = plan_new(&loaded, &registry, &rule_request(), as_of).unwrap();
    let outcome = commit_plan(&loaded.root, plan, &AlwaysAlive).unwrap();
    assert!(!outcome.report.written.is_empty());
    assert!(!root.join(".fairlead").join("lock").exists());
}

#[test]
fn r_cli_008__a_held_lock_refuses_the_commit_with_a_refusal() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let as_of = AsOf::parse("2026-10-09").unwrap();
    let registry = resolve_loaded(&loaded, as_of.clone());
    let plan = plan_new(&loaded, &registry, &rule_request(), as_of).unwrap();
    let probe = AlwaysAlive;
    let _held = WriterLock::acquire(&loaded.root, &probe).unwrap();
    let error = commit_plan(&loaded.root, plan, &probe).err().unwrap();
    assert!(matches!(error.class(), ErrorClass::Refused));
}
```

Run `cargo test -p fairlead-core --test pipeline` and expect compile errors for `commit_plan` and `plan_new`.

- [ ] **Step 2: Minimal implementation.** Add these imports to the top import block of `pipeline.rs`: `use crate::write::{approve_plan, approve_preview, new_plan, sync_plan, ApprovePreview, ApproveRequest, CommitReport, NewRequest, Plan, PidLiveness, Recovery, SyncRequest, Transaction, WriterLock};` and `use crate::time::AsOf;` (already present). Append:

```rust
/// What a writer reports: the recovery it performed before writing, and what it wrote (R-CLI-007).
pub struct WriteOutcome {
    pub recovery: Recovery,
    pub report: CommitReport,
}

/// `new` plan (R-CLI-028). Pure: never touches disk.
pub fn plan_new(loaded: &Loaded, registry: &Registry, request: &NewRequest, as_of: AsOf) -> Result<Plan, Error> {
    new_plan(request, registry, &loaded.facts, as_of)
}

/// `sync` plan (R-CLI-025, R-CLI-046). The check runs first, so `sync_plan` sees the findings `check` reports.
pub fn plan_sync(
    loaded: &Loaded,
    registry: &Registry,
    options: &CheckOptions,
    request: &SyncRequest,
) -> Result<Plan, Error> {
    let context = check_context(loaded, options);
    let input = CheckInput {
        registry,
        corpus: &loaded.corpus,
        facts: &loaded.facts,
        load_diags: &context.diags,
        baseline: context.baseline.as_ref(),
        options,
    };
    let report = check_run(&input);
    sync_plan(request, &input, &report, loaded.root.edition, env!("CARGO_PKG_VERSION"))
}

/// `approve`, step 1: the preview the reviewer confirms (R-CLI-037).
pub fn approve_preview_loaded(loaded: &Loaded, registry: &Registry, request: &ApproveRequest) -> Result<ApprovePreview, Error> {
    approve_preview(request, registry, &loaded.facts)
}

/// `approve`, step 2: the plan, built only after the terminal gate has passed (R-CLI-037).
pub fn plan_approve_loaded(loaded: &Loaded, preview: &ApprovePreview, approver: &str, as_of: AsOf) -> Result<Plan, Error> {
    approve_plan(preview, approver, as_of, &loaded.facts.approvals)
}

/// Lock, then recover, then begin, then commit (R-CLI-007, R-CLI-008). The only function that reaches `Transaction`.
/// The lock drops when this function returns, after the commit.
pub fn commit_plan(root: &Root, plan: Plan, probe: &dyn PidLiveness) -> Result<WriteOutcome, Error> {
    let lock = WriterLock::acquire(root, probe)?;
    let recovery = Transaction::recover(root, &lock)?;
    let transaction = Transaction::begin(root, &lock, plan.set)?;
    let report = transaction.commit()?;
    Ok(WriteOutcome { recovery, report })
}
```

Verify: `Transaction::recover` is called first on every writer (contract 4.12), and `Transaction::begin` takes the `WriteSet` by value, so `plan.set` moves out of `plan`.

- [ ] **Step 3: Run `cargo test -p fairlead-core --test pipeline` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead-core): pipeline write path with lock, recovery and commit order` and the trailer.

---

### Task 9: `check` command

**Files:**
- Create: `crates/fairlead/src/commands/check.rs`

**Interfaces**
- Consumes: `pipeline::{load, resolve_loaded, check_loaded, to_root_relative}`; `check::{CheckOptions, MaxFindings, Ramp, all_specs}` (P5); `diag::DiagCode`; `render::{renderer, OutputFormat, RenderOptions}` (P5); `outcome::VerbOutcome`; `output::{default_format, colour_enabled, no_color_set, stdout_is_tty}`.
- Produces: `commands::check::run(&GlobalArgs, &CheckArgs) -> CommandResult`; helpers `default_options(AsOf) -> CheckOptions`, `parse_budget(&str) -> Result<Duration, Error>`, `max_findings(Option<u32>) -> MaxFindings`, `check_format(Option<OutputFormat>, bool, bool) -> OutputFormat`.

- [ ] **Step 1: Write the failing tests.** Create `check.rs` with only this test module; run `cargo test -p fairlead check::` and expect a compile error.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn r_cli_021__budget_takes_ms_s_or_m_and_rejects_the_rest() {
        assert_eq!(parse_budget("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_budget("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_budget("500ms").unwrap(), Duration::from_millis(500));
        for bad in ["1h", "", "abc", "s", "-5s"] {
            assert!(parse_budget(bad).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn r_cli_021__max_zero_prints_all_and_absent_is_the_default() {
        assert!(matches!(max_findings(None), MaxFindings::Default));
        assert!(matches!(max_findings(Some(0)), MaxFindings::All));
        assert!(matches!(max_findings(Some(5)), MaxFindings::Limit(5)));
    }

    #[test]
    fn r_chk_040__an_unknown_rule_code_is_a_usage_error() {
        assert!(rule_filter(&Some("HN-NOPE-999".to_string())).is_err());
        assert!(matches!(rule_filter(&Some("HN-S001".to_string())), Ok(Some(_))));
    }

    #[test]
    fn r_cli_021__for_agent_implies_terse() {
        assert!(matches!(check_format(None, true, true), OutputFormat::Terse));
        assert!(matches!(check_format(Some(OutputFormat::Json), true, false), OutputFormat::Json));
    }
}
```

- [ ] **Step 2: Minimal implementation.** Put this above the test module:

```rust
//! `fairlead check` (R-CHK-040, R-CLI-021). Reads and renders only; the exit code is the report's outcome.

use std::collections::BTreeSet;
use std::time::Duration;

use camino::Utf8Path;
use fairlead_core::check::{all_specs, CheckOptions, MaxFindings, Ramp};
use fairlead_core::diag::DiagCode;
use fairlead_core::error::Error;
use fairlead_core::outcome::VerbOutcome;
use fairlead_core::path::RepoPath;
use fairlead_core::pipeline::{check_loaded, load, resolve_loaded, to_root_relative, Loaded};
use fairlead_core::render::{renderer, OutputFormat, RenderOptions};
use fairlead_core::time::AsOf;

use crate::cli::{CheckArgs, GlobalArgs};
use crate::commands::{as_of, load_request, working_dir, CommandResult};
use crate::output::{colour_enabled, default_format, no_color_set, stdout_is_tty};

pub fn run(global: &GlobalArgs, args: &CheckArgs) -> CommandResult {
    match check(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn check(global: &GlobalArgs, args: &CheckArgs) -> Result<CommandResult, Error> {
    let tty = stdout_is_tty();
    let format = check_format(global.format.clone(), tty, args.for_agent);
    let cwd = working_dir(global)?;
    let loaded = load(&load_request(global, args.changed, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let options = check_options(global, args, &loaded, &cwd)?;
    let report = check_loaded(&loaded, &registry, &options)?;
    let outcome: VerbOutcome = report.outcome(&options);
    let render = RenderOptions {
        format,
        for_agent: args.for_agent,
        max: max_findings(args.max),
        show_observe: args.show_observe,
        color: colour_enabled(global.color, tty, no_color_set()),
        quiet: global.quiet,
    };
    let stdout = renderer(render.format.clone()).check(&report, &render);
    Ok(CommandResult { stdout, stderr: String::new(), outcome })
}

fn check_options(global: &GlobalArgs, args: &CheckArgs, loaded: &Loaded, cwd: &Utf8Path) -> Result<CheckOptions, Error> {
    let mut options = default_options(as_of(global));
    options.changed = args.changed;
    options.files_from = match &args.files_from {
        Some(spec) => Some(read_files_from(spec)?),
        None => None,
    };
    options.paths = args
        .paths
        .iter()
        .map(|path| to_root_relative(&loaded.root, cwd, path))
        .collect::<Result<Vec<RepoPath>, Error>>()?;
    options.rule_filter = rule_filter(&args.rule)?;
    options.max = max_findings(args.max);
    options.ramp = Ramp { show_observe: args.show_observe, deny_warnings: args.deny_warnings, nudge_days: [60, 120] };
    options.for_agent = args.for_agent;
    options.no_baseline = args.no_baseline;
    options.require_declarations = args.require_declarations;
    options.budget = args.budget.as_deref().map(parse_budget).transpose()?;
    Ok(options)
}

/// The defaults shared with `context`: nothing filtered, every finding kept, baseline honoured.
pub fn default_options(as_of: AsOf) -> CheckOptions {
    CheckOptions {
        as_of,
        changed: false,
        files_from: None,
        paths: Vec::new(),
        rule_filter: None,
        max: MaxFindings::Default,
        ramp: Ramp { show_observe: false, deny_warnings: false, nudge_days: [60, 120] },
        for_agent: false,
        no_baseline: false,
        require_declarations: false,
        budget: None,
    }
}

/// C-14: an integer plus `ms`, `s` or `m`. Overflow and anything else is a usage error.
pub fn parse_budget(text: &str) -> Result<Duration, Error> {
    let bad = || Error::Usage(format!("--budget takes an integer with ms, s or m (90s, 2m); got {text:?}"));
    let (digits, unit) = if let Some(d) = text.strip_suffix("ms") {
        (d, "ms")
    } else if let Some(d) = text.strip_suffix('s') {
        (d, "s")
    } else if let Some(d) = text.strip_suffix('m') {
        (d, "m")
    } else {
        return Err(bad());
    };
    let value: u64 = digits.parse().map_err(|_| bad())?;
    match unit {
        "ms" => Ok(Duration::from_millis(value)),
        "s" => Ok(Duration::from_secs(value)),
        _ => Ok(Duration::from_secs(value.checked_mul(60).ok_or_else(bad)?)),
    }
}

pub fn max_findings(value: Option<u32>) -> MaxFindings {
    match value {
        None => MaxFindings::Default,
        Some(0) => MaxFindings::All,
        Some(n) => MaxFindings::Limit(n),
    }
}

/// `--for-agent` implies terse (R-CHK-039). Otherwise an explicit format wins, then human on a TTY, terse when piped.
pub fn check_format(explicit: Option<OutputFormat>, tty: bool, for_agent: bool) -> OutputFormat {
    if for_agent {
        return OutputFormat::Terse;
    }
    default_format(explicit, tty)
}

/// `--rule CODE`: `DiagCode` holds a static string, so the code is looked up in the spec table, never built from input.
fn rule_filter(text: &Option<String>) -> Result<Option<DiagCode>, Error> {
    match text {
        None => Ok(None),
        Some(code) => all_specs()
            .iter()
            .find(|spec| spec.code.0 == code.as_str())
            .map(|spec| Some(spec.code))
            .ok_or_else(|| Error::Usage(format!("unknown diagnostic code {code}"))),
    }
}

/// `--files-from FILE` or `-` (stdin): one repo-relative path per line; blank lines are skipped (R-CLI-022).
fn read_files_from(spec: &str) -> Result<BTreeSet<RepoPath>, Error> {
    let text = if spec == "-" {
        std::io::read_to_string(std::io::stdin()).map_err(|e| Error::Internal(e.to_string()))?
    } else {
        std::fs::read_to_string(spec).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Usage(format!("--files-from: no such file {spec}"))
            } else {
                Error::Internal(e.to_string())
            }
        })?
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| RepoPath::new(line).map_err(|_| Error::Usage(format!("--files-from line is not a repo path: {line}"))))
        .collect()
}
```

Two decisions in this code. `--budget` maps the deadline to the library (`CheckOptions.budget`), so the bin never reads a clock. `--files-from` restricts reporting only; extraction stays whole-tree (R-CLI-022). A `paths` argument naming the root itself is the CI-6 question.

- [ ] **Step 3: Run `cargo test -p fairlead check::` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead): check command with budget, max and rule filters` and the trailer.

---

### Task 10: `context` command

**Files:**
- Create: `crates/fairlead/src/commands/context.rs`

**Interfaces**
- Consumes: `pipeline::{load, resolve_loaded, check_loaded, context_target, context_loaded}`; `context::{Budget, ContextRequest, ContextStatus, ContextTarget}` (P6); `render::{renderer, OutputFormat}`; `commands::check::default_options`; `registry::ScopeId` (contract 4.8), `fairlead_syntax::ids::ScopeSlug` (contract 4.1).
- Produces: `commands::context::run(&GlobalArgs, &ContextArgs) -> CommandResult`; helpers `budget(Option<u32>) -> Budget`, `context_format(Option<OutputFormat>, bool) -> OutputFormat`, `status_outcome(&ContextStatus) -> VerbOutcome`.

- [ ] **Step 1: Write the failing tests.** Create `context.rs` with only this module; run `cargo test -p fairlead context::` and expect a compile error.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fairlead_core::outcome::ExitCode;

    #[test]
    fn r_cli_032__budget_zero_is_unlimited_and_absent_is_the_default() {
        assert!(matches!(budget(None), Budget::Default));
        assert!(matches!(budget(Some(0)), Budget::Unlimited));
        assert!(matches!(budget(Some(700)), Budget::Tokens(700)));
    }

    #[test]
    fn r_cli_032__human_behaves_as_terse_and_json_is_kept() {
        assert!(matches!(context_format(None, true), OutputFormat::Terse));
        assert!(matches!(context_format(Some(OutputFormat::Human), true), OutputFormat::Terse));
        assert!(matches!(context_format(Some(OutputFormat::Json), false), OutputFormat::Json));
    }

    #[test]
    fn r_cli_033__truncated_mandatory_content_exits_2() {
        let outcome = status_outcome(&ContextStatus::TruncatedMandatory { min_budget: 40 });
        assert_eq!(outcome.exit_code(), ExitCode::Usage);
        assert_eq!(status_outcome(&ContextStatus::Ok).exit_code(), ExitCode::Ok);
    }
}
```

- [ ] **Step 2: Minimal implementation.** Put this above the test module:

```rust
//! `fairlead context` (R-CLI-032 to R-CLI-034). Builds the brief through the pipeline; text and JSON are two renderings of one structure (C-15).

use fairlead_core::check::CheckOptions;
use fairlead_core::context::{Budget, ContextRequest, ContextStatus};
use fairlead_core::error::Error;
use fairlead_core::outcome::{ExitCode, VerbOutcome};
use fairlead_core::pipeline::{check_loaded, context_loaded, context_target, load, resolve_loaded};
use fairlead_core::render::{renderer, OutputFormat};
use fairlead_core::registry::ScopeId;
use fairlead_syntax::ids::ScopeSlug;

use crate::cli::{ContextArgs, GlobalArgs};
use crate::commands::check::default_options;
use crate::commands::{as_of, load_request, working_dir, CommandResult};
use crate::output::{default_format, stdout_is_tty};

pub fn run(global: &GlobalArgs, args: &ContextArgs) -> CommandResult {
    match context(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn context(global: &GlobalArgs, args: &ContextArgs) -> Result<CommandResult, Error> {
    let cwd = working_dir(global)?;
    let loaded = load(&load_request(global, false, None)?)?;
    let target = context_target(&loaded.root, &cwd, args.target.as_deref())?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let mut options: CheckOptions = default_options(as_of(global));
    if let fairlead_core::context::ContextTarget::Path(path) = &target {
        options.paths = vec![path.clone()];
    }
    let report = check_loaded(&loaded, &registry, &options)?;
    let request = ContextRequest {
        target,
        budget: budget(args.budget),
        scope_pin: scope_pin(args.scope.as_deref())?,
        full: args.full,
        depth: args.depth,
        digest_only: args.digest,
        exact_tokens: args.exact_tokens,
        as_of: as_of(global),
    };
    let output = context_loaded(&loaded, &registry, &report, &request);
    let format = context_format(global.format.clone(), stdout_is_tty());
    let stdout = renderer(format)
        .context(&output)
        .ok_or_else(|| Error::Internal("the renderer produced no context text".to_string()))?;
    Ok(CommandResult { stdout, stderr: String::new(), outcome: status_outcome(&output.status) })
}

/// `--budget N`: 0 is unlimited (R-CTX-044); absent is the configured default.
pub fn budget(value: Option<u32>) -> Budget {
    match value {
        None => Budget::Default,
        Some(0) => Budget::Unlimited,
        Some(n) => Budget::Tokens(n),
    }
}

/// C-28: context renders terse lines or JSON only; `human` behaves as terse.
pub fn context_format(explicit: Option<OutputFormat>, tty: bool) -> OutputFormat {
    match default_format(explicit, tty) {
        OutputFormat::Json => OutputFormat::Json,
        _ => OutputFormat::Terse,
    }
}

/// R-CLI-033, R-CTX-041: mandatory content that does not fit the budget exits 2.
pub fn status_outcome(status: &ContextStatus) -> VerbOutcome {
    let mut outcome = VerbOutcome::ok();
    if matches!(status, ContextStatus::TruncatedMandatory { .. }) {
        outcome.signal(ExitCode::Usage);
    }
    outcome
}

pub fn scope_pin(text: Option<&str>) -> Result<Option<ScopeId>, Error> {
    match text {
        None => Ok(None),
        Some(slug) => ScopeSlug::parse(slug)
            .map(|parsed| Some(ScopeId(parsed)))
            .map_err(|_| Error::Usage(format!("--scope takes a scope slug, got {slug:?}"))),
    }
}
```

Verify: `ContextTarget` is referenced by its full path in the body, so no import is needed for it. `ScopeSlug` is `fairlead_syntax::ids::ScopeSlug` (contract 4.1) and `ScopeId` is `fairlead_core::registry::ScopeId` (contract 4.8); if the contract module paths differ in the code, use the paths the modules export. `renderer(format)` returns `&'static dyn Renderer`, so the call chain above borrows nothing from a temporary.

- [ ] **Step 3: Run `cargo test -p fairlead context::` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead): context command with budget, format and truncation exit` and the trailer.

---

### Task 11: `explain` and `init` commands

**Files:**
- Create: `crates/fairlead/src/commands/explain.rs` (wiring only; its behaviour is pinned by `tests/cli/explain.trycmd` in Task 16)
- Create: `crates/fairlead/src/commands/init.rs`
- Modify: `crates/fairlead-core/src/pipeline.rs` (append `init_root`, `init_first_scan`)
- Modify: `crates/fairlead-core/tests/pipeline.rs` (append one test)

**Interfaces**
- Consumes: `explain::{explain as explain_text, ExplainOutput, ExplainTarget}` (P7); `write::{detect_init, init_control_bytes, init_plan, InitDetection, InitRequest, Plan, Preset, SystemLiveness, CommitReport}` (P7; Contract issues CI-4, CI-5, CI-7); `pipeline::{commit_plan, explain_target, check_loaded, load, resolve_loaded}`; `commands::check::default_options`.
- Produces: `pipeline::init_root(&Utf8Path, &[u8]) -> Root`; `pipeline::init_first_scan(&Root, &[u8], Option<usize>, &CheckOptions) -> Result<CheckReport, Error>`; `commands::init::{run, Gate, gate}`; `commands::explain::run`.

- [ ] **Step 1: Write the failing tests.** Append to `crates/fairlead-core/tests/pipeline.rs`:

```rust
use fairlead_core::pipeline::{init_first_scan, init_root};

#[test]
fn r_cli_017__the_first_scan_of_a_bare_tree_has_no_findings() {
    let (_dir, root) = temp_root(false);
    let bytes = ROOT_CONTROL.as_bytes();
    let tree = init_root(&root, bytes);
    let report = init_first_scan(&tree, bytes, Some(1), &check_options()).unwrap();
    assert!(report.findings.is_empty());
}
```

Append to `commands/init.rs` (create the file with only this module first, and run `cargo test -p fairlead init::` expecting a compile error):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_cli_002__dry_run_wins_and_without_yes_nothing_is_applied() {
        assert_eq!(gate(true, true), Gate::DryRun);
        assert_eq!(gate(false, false), Gate::NotApplied);
        assert_eq!(gate(false, true), Gate::Apply);
    }

    #[test]
    fn r_cli_043__init_needs_yes_to_apply() {
        assert_ne!(gate(false, false), Gate::Apply);
    }
}
```

- [ ] **Step 2: Minimal implementation.**

Append to `pipeline.rs`. Add to the import block: `use camino::Utf8Path;` (already present), `use std::collections::BTreeMap;`, `use crate::config::parse_control;`, `use crate::discover::{Pin, RootSource, SUPPORTED_EDITION};`, `use crate::hash::hash_normalised;`, `use crate::walk::walk;` (present), `use crate::check::{run as check_run, CheckInput, CheckOptions, CheckReport};` (present).

```rust
/// The `Root` of a tree with no control file yet (`init`). The managerial folder is the default `fairlead` (R-CLI-012).
pub fn init_root(dir: &Utf8Path, control_bytes: &[u8]) -> Root {
    Root {
        dir: dir.to_path_buf(),
        folder: "fairlead".to_string(),
        source: RootSource::Flag,
        edition: SUPPORTED_EDITION,
        pin: Pin { version: None, edition: SUPPORTED_EDITION },
        config_hash: hash_normalised(control_bytes),
    }
}

/// The first scan `init` reports (R-CLI-019): the control file it would write, checked in memory. Only the walk and reads touch disk.
pub fn init_first_scan(root: &Root, control_bytes: &[u8], threads: Option<usize>, options: &CheckOptions) -> Result<CheckReport, Error> {
    let path = RepoPath::new("fairlead/control.toml")
        .map_err(|_| Error::Internal("the managed control path is not a repo path".to_string()))?;
    let control = parse_control(&path, control_bytes, true).map_err(|failure| Error::Config(Box::new(failure)))?;
    let walked = walk(root, &WalkOptions { excludes: control.scan.exclude.clone(), max_file: control.scan.max_file, threads })?;
    let controls = ControlSet { files: BTreeMap::from([(path, Ok(control.clone()))]) };
    let plan = extract_plan_from_controls(&control, &controls);
    let cache = Cache::open(root, false, cache_key(root));
    let corpus = extract_files(&walked, root, &plan, &cache, None)?;
    let facts = collect_facts(root, &walked, &controls, false)?;
    let registry = resolve(&ResolveInput { root, controls: &controls, corpus: &corpus, facts: &facts, as_of: options.as_of.clone() });
    Ok(check_run(&CheckInput {
        registry: &registry,
        corpus: &corpus,
        facts: &facts,
        load_diags: &[],
        baseline: None,
        options,
    }))
}
```

Create `commands/init.rs`, with the test module from Step 1 at the bottom:

```rust
//! `fairlead init` (R-CLI-012, R-CLI-002, R-CLI-043): plan first. Writes only when `--yes` is given.

use fairlead_core::error::Error;
use fairlead_core::outcome::{ExitCode, VerbOutcome};
use fairlead_core::pipeline::{commit_plan, init_first_scan, init_root};
use fairlead_core::write::{detect_init, init_control_bytes, init_plan, InitRequest, Plan, Preset, SystemLiveness};

use crate::cli::{GlobalArgs, InitArgs, PresetArg};
use crate::commands::check::default_options;
use crate::commands::{as_of, discover_request, CommandResult};
use crate::commands::sync::changed_list;

pub fn run(global: &GlobalArgs, args: &InitArgs) -> CommandResult {
    match init(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Gate {
    DryRun,
    Apply,
    NotApplied,
}

/// `--dry-run` prints the plan and exits 0; without `--yes` the plan is printed and nothing applied (exit 4).
pub fn gate(dry_run: bool, yes: bool) -> Gate {
    if dry_run {
        Gate::DryRun
    } else if yes {
        Gate::Apply
    } else {
        Gate::NotApplied
    }
}

fn init(global: &GlobalArgs, args: &InitArgs) -> Result<CommandResult, Error> {
    let dir = discover_request(global, false)?.cwd;
    let detected = detect_init(&dir)?;
    let control_bytes = init_control_bytes(&detected);
    let root = init_root(&dir, &control_bytes);
    let first_scan = init_first_scan(&root, &control_bytes, global.threads, &default_options(as_of(global)))?;
    let request = InitRequest {
        preset: preset(args.preset),
        no_baseline: args.no_baseline,
        hooks: args.hooks,
        codeowners: args.codeowners,
        agents: args.agents.clone(),
        yes: args.yes,
    };
    let plan = init_plan(&request, &detected, &first_scan)?;
    Ok(match gate(args.dry_run, args.yes) {
        Gate::DryRun => CommandResult::text(plan.render_text()),
        Gate::NotApplied => not_applied(&plan),
        Gate::Apply => {
            let outcome = commit_plan(&root, plan, &SystemLiveness)?;
            CommandResult::text(changed_list(&outcome.report))
        }
    })
}

fn not_applied(plan: &Plan) -> CommandResult {
    let mut outcome = VerbOutcome::ok();
    outcome.signal(ExitCode::Refused);
    CommandResult {
        stdout: format!("{}not applied; apply with: fairlead init --yes\n", plan.render_text()),
        stderr: String::new(),
        outcome,
    }
}

fn preset(arg: PresetArg) -> Preset {
    match arg {
        PresetArg::NoPreset => Preset::None,
        PresetArg::Rust => Preset::Rust,
        PresetArg::Ts => Preset::Ts,
    }
}
```

`commands/explain.rs`:

```rust
//! `fairlead explain` (R-CLI-035). Target classification is in the pipeline (C-21); this module renders.

use fairlead_core::error::Error;
use fairlead_core::explain::{explain as explain_text, ExplainTarget};
use fairlead_core::pipeline::{check_loaded, explain_target, load, resolve_loaded};
use fairlead_core::render::renderer;

use crate::cli::{ExplainArgs, GlobalArgs};
use crate::commands::check::default_options;
use crate::commands::{as_of, load_request, working_dir, CommandResult};
use crate::output::{default_format, stdout_is_tty};

pub fn run(global: &GlobalArgs, args: &ExplainArgs) -> CommandResult {
    match explain(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn explain(global: &GlobalArgs, args: &ExplainArgs) -> Result<CommandResult, Error> {
    let cwd = working_dir(global)?;
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let target = explain_target(&loaded.root, &cwd, &args.target)?;
    let report = match &target {
        ExplainTarget::File(_) => Some(check_loaded(&loaded, &registry, &default_options(as_of(global)))?),
        _ => None,
    };
    let output = explain_text(&target, &registry, report.as_ref())?;
    let format = default_format(global.format.clone(), stdout_is_tty());
    let stdout = renderer(format)
        .explain(&output)
        .ok_or_else(|| Error::Usage("explain has no output in this format; use human or terse".to_string()))?;
    Ok(CommandResult::text(stdout))
}
```

- [ ] **Step 3: Run `cargo test -p fairlead-core --test pipeline` and `cargo test -p fairlead init::` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead): explain and init commands; init first scan in memory` and the trailer.

---

### Task 12: `sync` and `new` commands

**Files:**
- Create: `crates/fairlead/src/commands/sync.rs`
- Create: `crates/fairlead/src/commands/new.rs`

**Interfaces**
- Consumes: `write::{BaselineMode, SyncRequest, NewRequest, CommitReport, SystemLiveness}` (P7); `pipeline::{load, resolve_loaded, plan_sync, plan_new, commit_plan}`; `commands::check::default_options`; `commands::context::scope_pin`; `path::RepoPath`.
- Produces: `commands::sync::{run, baseline_mode, changed_list}`; `commands::new::{run, parse_fields}`.

- [ ] **Step 1: Write the failing tests.** Create `sync.rs` and `new.rs` each with only their test module below. Run `cargo test -p fairlead sync:: new::` and expect compile errors.

```rust
// sync.rs
#[cfg(test)]
mod tests {
    use super::*;

    fn args(baseline: bool, accept_new: bool, why: Option<&str>) -> SyncArgs {
        SyncArgs {
            dry_run: false,
            yes: false,
            baseline,
            accept_new,
            why: why.map(str::to_string),
            accept_vendor: false,
            force: false,
            scope: None,
        }
    }

    #[test]
    fn r_cli_025__baseline_mode_follows_the_flags() {
        assert!(matches!(baseline_mode(&args(false, false, None)), Ok(BaselineMode::Off)));
        assert!(matches!(baseline_mode(&args(true, false, None)), Ok(BaselineMode::Prune)));
        assert!(matches!(baseline_mode(&args(true, true, Some("legacy"))), Ok(BaselineMode::AcceptNew { .. })));
        assert!(baseline_mode(&args(true, true, None)).is_err());
    }
}
```

```rust
// new.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_cli_028__fields_are_k_equals_v_and_repeats_are_refused() {
        let fields = parse_fields(&["title=Progress bar".to_string(), "expr=a=b".to_string()]).unwrap();
        assert_eq!(fields.get("title").map(String::as_str), Some("Progress bar"));
        assert_eq!(fields.get("expr").map(String::as_str), Some("a=b"));
        assert!(parse_fields(&["no-equals".to_string()]).is_err());
        assert!(parse_fields(&["k=1".to_string(), "k=2".to_string()]).is_err());
    }
}
```

- [ ] **Step 2: Minimal implementation.**

`sync.rs`:

```rust
//! `fairlead sync` (R-CLI-025, R-CLI-046, R-CLI-043). Applies directly; `--yes` is accepted and has no effect.

use fairlead_core::error::Error;
use fairlead_core::pipeline::{commit_plan, load, plan_sync, resolve_loaded};
use fairlead_core::write::{BaselineMode, CommitReport, SyncRequest, SystemLiveness};

use crate::cli::{GlobalArgs, SyncArgs};
use crate::commands::check::default_options;
use crate::commands::context::scope_pin;
use crate::commands::{as_of, load_request, CommandResult};

pub fn run(global: &GlobalArgs, args: &SyncArgs) -> CommandResult {
    match sync(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn sync(global: &GlobalArgs, args: &SyncArgs) -> Result<CommandResult, Error> {
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let options = default_options(as_of(global));
    let request = SyncRequest {
        baseline: baseline_mode(args)?,
        accept_vendor: args.accept_vendor,
        force: args.force,
        scope: scope_pin(args.scope.as_deref())?,
    };
    let plan = plan_sync(&loaded, &registry, &options, &request)?;
    if args.dry_run {
        return Ok(CommandResult::text(plan.render_text()));
    }
    let outcome = commit_plan(&loaded.root, plan, &SystemLiveness)?;
    Ok(CommandResult::text(changed_list(&outcome.report)))
}

/// `--baseline` alone prunes; `--accept-new` needs `--why` (R-CHK-032); no `--baseline` leaves the baseline alone.
pub fn baseline_mode(args: &SyncArgs) -> Result<BaselineMode, Error> {
    if !args.baseline {
        return Ok(BaselineMode::Off);
    }
    match (args.accept_new, args.why.as_deref()) {
        (true, Some(why)) => Ok(BaselineMode::AcceptNew { why: why.to_string() }),
        (true, None) => Err(Error::Usage("--accept-new needs --why TEXT".to_string())),
        (false, _) => Ok(BaselineMode::Prune),
    }
}

/// One line per written file, sorted by the library; "no changes" when nothing was written (R-CLI-027).
pub fn changed_list(report: &CommitReport) -> String {
    if report.written.is_empty() {
        return "no changes\n".to_string();
    }
    report.written.iter().map(|path| format!("wrote {}\n", path.as_str())).collect()
}
```

`new.rs`:

```rust
//! `fairlead new` (R-CLI-028 to R-CLI-031). Never overwrites (the planner refuses, exit 4); applies directly (R-CLI-043).

use std::collections::BTreeMap;

use fairlead_core::error::Error;
use fairlead_core::path::RepoPath;
use fairlead_core::pipeline::{commit_plan, load, plan_new, resolve_loaded};
use fairlead_core::write::{NewRequest, SystemLiveness};

use crate::cli::{GlobalArgs, NewArgs};
use crate::commands::context::scope_pin;
use crate::commands::sync::changed_list;
use crate::commands::{as_of, load_request, CommandResult};

pub fn run(global: &GlobalArgs, args: &NewArgs) -> CommandResult {
    match new(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn new(global: &GlobalArgs, args: &NewArgs) -> Result<CommandResult, Error> {
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    let request = NewRequest {
        kind: args.kind.clone(),
        name: args.name.clone(),
        scope: scope_pin(args.scope.as_deref())?,
        fields: parse_fields(&args.fields)?,
        print: args.print,
        at: optional_path(args.at.as_deref())?,
        hoist: args.hoist,
    };
    let plan = plan_new(&loaded, &registry, &request, as_of(global))?;
    if args.print {
        // The planner puts the scaffold text in `notes` for `--print` and writes nothing (Contract issues CI-8).
        return Ok(CommandResult::text(plan.notes.join("\n")));
    }
    if args.dry_run {
        return Ok(CommandResult::text(plan.render_text()));
    }
    let outcome = commit_plan(&loaded.root, plan, &SystemLiveness)?;
    Ok(CommandResult::text(changed_list(&outcome.report)))
}

/// `--field K=V`, repeatable. The first `=` splits; a repeated key is a usage error.
pub fn parse_fields(items: &[String]) -> Result<BTreeMap<String, String>, Error> {
    let mut fields = BTreeMap::new();
    for item in items {
        let (key, value) = item
            .split_once('=')
            .filter(|(key, _)| !key.is_empty())
            .ok_or_else(|| Error::Usage(format!("--field takes K=V, got {item:?}")))?;
        if fields.insert(key.to_string(), value.to_string()).is_some() {
            return Err(Error::Usage(format!("--field {key} is given twice")));
        }
    }
    Ok(fields)
}

fn optional_path(text: Option<&str>) -> Result<Option<RepoPath>, Error> {
    match text {
        None => Ok(None),
        Some(path) => RepoPath::new(path)
            .map(Some)
            .map_err(|_| Error::Usage(format!("--at takes a repo-relative folder, got {path:?}"))),
    }
}
```

Make `changed_list` `pub` in sync.rs (it is). The `scope_pin` function is `pub` from Task 10. Verify: the `sync` and `init` commands reference `changed_list` from `commands::sync`.

- [ ] **Step 3: Run `cargo test -p fairlead sync:: new::` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead): sync and new commands; writes only through commit_plan` and the trailer.

---

### Task 13: `approve` command and its terminal gate

**Files:**
- Create: `crates/fairlead/src/commands/approve.rs`

**Interfaces**
- Consumes: `error::{Error, Refusal, RefusalKind}`; `write::{ApproveRequest, ApprovePreview, SystemLiveness}`; `pipeline::{load, resolve_loaded, approve_preview_loaded, plan_approve_loaded, commit_plan}`; `output::write_text`; `std::io::IsTerminal`.
- Produces: `commands::approve::run(&GlobalArgs, &ApproveArgs) -> CommandResult`; `commands::approve::terminal_gate(bool, bool, bool) -> Result<(), Error>`; `commands::approve::retype_matches(&str, &str) -> bool`.

- [ ] **Step 1: Write the failing tests.** Create `approve.rs` with only this test module; run `cargo test -p fairlead approve::` and expect a compile error.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_cli_037__the_terminal_gate_refuses_a_pipe_and_ci() {
        assert!(matches!(terminal_gate(false, true, false).err(), Some(Error::Refused(_))));
        assert!(matches!(terminal_gate(true, false, false).err(), Some(Error::Refused(_))));
        assert!(matches!(terminal_gate(true, true, true).err(), Some(Error::Refused(_))));
        assert!(terminal_gate(true, true, false).is_ok());
    }

    #[test]
    fn r_cli_037__the_reviewer_retypes_the_first_four_characters() {
        assert!(retype_matches("ab12", "ab12"));
        assert!(retype_matches(" ab12 \n", "ab12"));
        assert!(!retype_matches("ab1", "ab12"));
        assert!(!retype_matches("", "ab12"));
    }
}
```

- [ ] **Step 2: Minimal implementation.**

```rust
//! `fairlead approve` (R-CLI-037, R-CLI-039, R-CLI-045). A human records an approval at a terminal. Its own verb, never a `new` target.

use std::io::IsTerminal;

use fairlead_core::error::{Error, Refusal, RefusalKind};
use fairlead_core::pipeline::{approve_preview_loaded, commit_plan, load, plan_approve_loaded, resolve_loaded};
use fairlead_core::hash::Fingerprint;
use fairlead_core::write::{ApproveRequest, SystemLiveness};

use crate::cli::{ApproveArgs, GlobalArgs};
use crate::commands::{as_of, load_request, CommandResult};
use crate::commands::sync::changed_list;
use crate::output::write_text;

pub fn run(global: &GlobalArgs, args: &ApproveArgs) -> CommandResult {
    match approve(global, args) {
        Ok(result) => result,
        Err(error) => CommandResult::failed(&error),
    }
}

fn approve(global: &GlobalArgs, args: &ApproveArgs) -> Result<CommandResult, Error> {
    let loaded = load(&load_request(global, false, None)?)?;
    let registry = resolve_loaded(&loaded, as_of(global));
    if args.list {
        // Listing is read-only and needs no terminal (R-CLI-037).
        return Ok(CommandResult::text(list_text(&loaded)));
    }
    let ci_set = std::env::var_os("CI").is_some_and(|value| !value.is_empty());
    terminal_gate(std::io::stdin().is_terminal(), std::io::stdout().is_terminal(), ci_set)?;
    let request = ApproveRequest {
        target: args.target.clone().unwrap_or_default(),
        as_handle: args.as_handle.clone(),
        list: false,
        revoke: args.revoke.as_deref().map(Fingerprint::from_text).transpose()?,
    };
    let preview = approve_preview_loaded(&loaded, &registry, &request)?;
    write_text(&format!("{}\nType the first 4 characters of the fingerprint to confirm: ", preview.ovr_line), "")
        .map_err(|e| Error::Internal(e.to_string()))?;
    let mut typed = String::new();
    std::io::stdin().read_line(&mut typed).map_err(|e| Error::Internal(e.to_string()))?;
    if !retype_matches(&typed, &preview.confirm_prefix) {
        return Err(Error::Refused(Refusal {
            kind: RefusalKind::PlanNotConfirmed,
            message: "the confirmation did not match; nothing was recorded".to_string(),
            hint: "retype the first 4 characters of the fingerprint exactly".to_string(),
        }));
    }
    let approver = args.as_handle.clone().ok_or_else(|| Error::Usage("approve needs --as @handle".to_string()))?;
    let plan = plan_approve_loaded(&loaded, &preview, &approver, as_of(global))?;
    let outcome = commit_plan(&loaded.root, plan, &SystemLiveness)?;
    Ok(CommandResult::text(changed_list(&outcome.report)))
}

/// R-CLI-037: refused (exit 4) when stdin or stdout is not a TTY, or when `CI` is set, so an unattended agent cannot self-approve.
pub fn terminal_gate(stdin_tty: bool, stdout_tty: bool, ci_set: bool) -> Result<(), Error> {
    if ci_set {
        return Err(Error::Refused(Refusal {
            kind: RefusalKind::CiSet,
            message: "approve is refused when CI is set".to_string(),
            hint: "approve at a terminal, outside CI".to_string(),
        }));
    }
    if !(stdin_tty && stdout_tty) {
        return Err(Error::Refused(Refusal {
            kind: RefusalKind::NotATty,
            message: "approve needs a terminal on stdin and stdout".to_string(),
            hint: "run fairlead approve in an interactive terminal".to_string(),
        }));
    }
    Ok(())
}

/// The reviewer's typed confirmation must equal the preview's 4-character prefix (surrounding whitespace ignored).
pub fn retype_matches(typed: &str, prefix: &str) -> bool {
    typed.trim() == prefix
}

fn list_text(loaded: &fairlead_core::pipeline::Loaded) -> String {
    loaded
        .facts
        .approvals
        .approvals
        .iter()
        .map(|a| format!("{} {} {} {}\n", a.fp.to_string(), a.target, a.approver, a.date))
        .collect()
}
```

Verify: `Fingerprint::from_text(&str) -> Result<Fingerprint, _>` is the parse of a typed fingerprint. The contract names no constructor, so Contract issues CI-9 requires P3 to provide it. `Fingerprint` and `Id` must implement `Display` for `list_text`. `ApproveRequest.target` is a `String` (contract 4.12), so an absent target becomes the empty string, which `approve_preview` rejects with a usage error.

- [ ] **Step 3: Run `cargo test -p fairlead approve::` and expect PASS.**

- [ ] **Step 4: Commit** with message `feat(fairlead): approve command behind the terminal and CI gate` and the trailer.

---

### Task 14: Dispatch and main wiring

**Files:**
- Modify: `crates/fairlead/src/cli.rs` (append imports, `run_from_env`, `dispatch`, `emit`, and two tests)
- Modify: `crates/fairlead/src/main.rs` (replace the Task 1 closure `|| ExitCode::Ok` with `cli::run_from_env`)

**Interfaces**
- Consumes: every `commands::*::run`; `commands::reserved::{run, ReservedVerb}`; `commands::CommandResult`; `output::{write_text, write_internal_error}`; `run_guarded` (Task 1).
- Produces: `cli::run_from_env() -> ExitCode`; `cli::dispatch(Cli) -> ExitCode`; `cli::emit(&CommandResult) -> ExitCode`.

- [ ] **Step 1: Write the failing tests.** Add to the test module of `cli.rs`:

```rust
    #[test]
    fn r_cli_003__a_missing_verb_is_a_usage_error() {
        assert_eq!(dispatch(Cli { global: GlobalArgs::default(), command: None }), ExitCode::Usage);
    }

    #[test]
    fn r_cli_001__a_reserved_verb_dispatches_to_exit_2() {
        let cli = Cli { global: GlobalArgs::default(), command: Some(Command::Seal(ReservedArgs::default())) };
        assert_eq!(dispatch(cli), ExitCode::Usage);
    }
```

Run `cargo test -p fairlead cli::` and expect a compile error (`dispatch` not found).

- [ ] **Step 2: Minimal implementation.** Add these imports at the top of `cli.rs`, then append the functions above the test module:

```rust
use fairlead_core::error::Error;
use fairlead_core::outcome::ExitCode;

use crate::commands::{self, reserved::ReservedVerb, CommandResult};
use crate::output::{write_internal_error, write_text};

/// One invocation, start to finish: parse, dispatch, write, exit. clap prints its own help and version text.
pub fn run_from_env() -> ExitCode {
    match Cli::try_parse() {
        Ok(cli) => dispatch(cli),
        Err(error) => {
            let code = if error.use_stderr() { ExitCode::Usage } else { ExitCode::Ok };
            // clap writes its own text with LF line endings on every platform; a failed print has no other channel.
            let _ = error.print();
            code
        }
    }
}

pub fn dispatch(cli: Cli) -> ExitCode {
    let global = cli.global;
    let result = match cli.command {
        None => CommandResult::failed(&Error::Usage("no verb given; run `fairlead --help`".to_string())),
        Some(Command::Init(args)) => commands::init::run(&global, &args),
        Some(Command::Check(args)) => commands::check::run(&global, &args),
        Some(Command::Sync(args)) => commands::sync::run(&global, &args),
        Some(Command::New(args)) => commands::new::run(&global, &args),
        Some(Command::Context(args)) => commands::context::run(&global, &args),
        Some(Command::Explain(args)) => commands::explain::run(&global, &args),
        Some(Command::Approve(args)) => commands::approve::run(&global, &args),
        Some(Command::Move(_)) => commands::reserved::run(&global, ReservedVerb::Move),
        Some(Command::Seal(_)) => commands::reserved::run(&global, ReservedVerb::Seal),
        Some(Command::Upgrade(_)) => commands::reserved::run(&global, ReservedVerb::Upgrade),
    };
    emit(&result)
}

/// The one place stdout and stderr are written. A failed write is exit 5; success is never reported after it (R-ARCH-101).
pub fn emit(result: &CommandResult) -> ExitCode {
    match write_text(&result.stdout, &result.stderr) {
        Ok(()) => result.exit(),
        Err(error) => {
            write_internal_error(&error.to_string());
            ExitCode::Internal
        }
    }
}
```

In `main.rs`, replace the closure `run_guarded(|| ExitCode::Ok)` with `run_guarded(cli::run_from_env)`. `main.rs` keeps `mod cli;`, `mod commands;`, `mod exit;` and `mod output;`. The `ExitCode` import is still used by `run_guarded`.

- [ ] **Step 3: Run `cargo test -p fairlead` and expect PASS** for every unit test in the crate.

- [ ] **Step 4: Commit** with message `feat(fairlead): wire dispatch, output and the panic boundary into main` and the trailer.

---

### Task 15: CLI runner and the global and reserved trycmd cases

**Files:**
- Create: `tests/cli.rs`
- Create: `tests/cli/global.trycmd`
- Create: `tests/cli/reserved.trycmd`

**Interfaces**
- Consumes: the built `fairlead` binary; `trycmd` 1.2.1 (dev-dependency).
- Produces: the runner `cli_contract_cases`, which every later `.trycmd` file joins.

- [ ] **Step 1: Write the runner and the cases.** This task adds acceptance tests for work done in Tasks 1 to 14, so it is expected to PASS on first run. A failure means the case text or the binary disagrees with the rule named in the case's section heading comment; fix the side the rule supports.

`tests/cli.rs`:

```rust
//! CLI contract tests (contract Section 7). trycmd runs every case in tests/cli/*.trycmd against the built `fairlead` binary.

#[test]
fn cli_contract_cases() {
    trycmd::TestCases::new()
        .env("NO_COLOR", "1")
        .env("FAIRLEAD_ROOT", "")
        .env("CI", "")
        .case(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/cli/*.trycmd"));
}
```

`tests/cli/global.trycmd` (R-CLI-001, R-CLI-003, R-CLI-006, R-ARCH-082):

```text
$ fairlead --version
fairlead 0.1.0

$ fairlead --threads 0 check
the thread count must be at least 1
? 2

$ fairlead --format yaml check
error: invalid value 'yaml' for '--format <FORMAT>'
...
? 2

$ fairlead --color sometimes check
error: invalid value 'sometimes' for '--color <WHEN>'
...
? 2

$ fairlead --quiet
no verb given; run `fairlead --help`
? 2
```

`tests/cli/reserved.trycmd` (R-CLI-001):

```text
$ fairlead move a b
move arrives in 0.2
? 2

$ fairlead seal --commit
seal arrives in 0.2
? 2

$ fairlead upgrade --yes
upgrade arrives in 0.2
? 2
```

Verify: trycmd 1.2.1 `.trycmd` syntax is `$ ` command lines, expected output beneath each, `...` for elided lines, and a final `? <status>` line for a non-zero exit. `TestCases::env` sets a variable for the spawned binary. The `fairlead` binary is found through the package's `[[bin]]` (the `cli` test target of Task 1). Confirm each against trycmd 1.2.1 before the first run. The exact clap error lines depend on clap 4.6.7. If a clap wording differs, change only the expected line.

- [ ] **Step 2: Run `cargo test -p fairlead --test cli` and expect PASS.**

- [ ] **Step 3: Commit** with message `test(fairlead): trycmd runner with global and reserved-verb contract cases` and the trailer.

---

### Task 16: Per-verb trycmd cases with fixture trees

**Files:**
- Create: `tests/cli/check.trycmd`, `tests/cli/check-findings.trycmd`, `tests/cli/check-syntax.trycmd`, `tests/cli/context.trycmd`, `tests/cli/explain.trycmd`, `tests/cli/approve.trycmd`, `tests/cli/init.trycmd`, `tests/cli/sync.trycmd`, `tests/cli/new.trycmd`, `tests/cli/root-missing.trycmd`
- Create the fixture trees (sibling directories named after the case file, copied into the case's sandbox by trycmd): `tests/cli/check.in/`, `tests/cli/check-findings.in/`, `tests/cli/check-syntax.in/`, `tests/cli/context.in/`, `tests/cli/explain.in/`, `tests/cli/approve.in/`, `tests/cli/sync.in/`, `tests/cli/new.in/`

**Interfaces**
- Consumes: the bin verbs of Tasks 9 to 14; the exit codes of R-CLI-003 and R-CHK-040.
- Produces: one case file per verb and flag family (contract Section 7).

- [ ] **Step 1: Write the fixtures and the cases.**

Fixture files. Every fixture tree is `fairlead/control.toml` plus `src/lib.rs`:

| Tree | `fairlead/control.toml` | `src/lib.rs` |
|---|---|---|
| `check.in`, `context.in`, `explain.in`, `approve.in`, `sync.in`, `new.in` | `ROOT_CONTROL` | `CLEAN_LIB` |
| `check-findings.in` | `FINDINGS_CONTROL` | `FINDINGS_LIB` |
| `check-syntax.in` | `ROOT_CONTROL` | `SYNTAX_LIB` |

The texts are:

```toml
# ROOT_CONTROL
[fairlead]
scope = "root"
edition = 1
root = true
marker = "fairlead"

[settings]
```

```toml
# FINDINGS_CONTROL: ROOT_CONTROL plus a declared kind, so `use cmp.button` is a dangling reference (HN-R001), not an unknown kind
[fairlead]
scope = "root"
edition = 1
root = true
marker = "fairlead"

[settings]

[kinds.cmp]
title = "Component"
shape = "code"
```

```rust
// CLEAN_LIB
pub fn answer() -> u32 {
    42
}
```

```rust
// FINDINGS_LIB
// fairlead: use cmp.button
pub fn answer() -> u32 {
    42
}
```

```rust
// SYNTAX_LIB: `bogus` is not a verb, so the directive is a syntax error (HN-S001)
// fairlead: bogus cmp.button
pub fn answer() -> u32 {
    42
}
```

Case files:

`tests/cli/check.trycmd` (R-CHK-040, R-CLI-021):

```text
$ fairlead check --format terse
...

$ fairlead check --format terse --require-declarations
...
? 1

$ fairlead check --budget 0ms --format terse
...
? 2

$ fairlead check --budget 1h
--budget takes an integer with ms, s or m (90s, 2m); got "1h"
? 2

$ fairlead check --rule HN-NOPE-999
unknown diagnostic code HN-NOPE-999
? 2

$ fairlead check --max 0 --format terse
...

$ fairlead check --format sarif --for-agent
...
```

`tests/cli/check-findings.trycmd` (R-CHK-040, exit 1 at enforce severity):

```text
$ fairlead check --format terse
...
? 1

$ fairlead check --format terse --rule HN-R001
...
? 1
```

`tests/cli/check-syntax.trycmd` (R-CHK-040, exit 2 for a syntax error):

```text
$ fairlead check --format terse
...
? 2
```

`tests/cli/context.trycmd` (R-CLI-032, R-CLI-033, C-13):

```text
$ fairlead context --format terse
...

$ fairlead context --format terse --digest
...

$ fairlead context "add a progress bar"
no match: task queries need 0.3; pass a path or an ID
? 2

$ fairlead context --budget abc
error: invalid value 'abc' for '--budget <N>'
...
? 2

$ fairlead context apps/web/../../../../outside
...
? 2
```

`tests/cli/explain.trycmd` (R-CLI-035, C-21):

```text
$ fairlead explain HN-S001
...

$ fairlead explain syntax
...

$ fairlead explain no-such-thing
...
? 2
```

`tests/cli/approve.trycmd` (R-CLI-037, R-CLI-045):

```text
$ fairlead approve --list
...

$ fairlead approve abcdef012345 --as @andy
...
? 4
```

`tests/cli/init.trycmd` (R-CLI-002, R-CLI-012, R-CLI-043; empty sandbox, no `.in`):

```text
$ fairlead init --dry-run
...

$ fairlead init
...
not applied; apply with: fairlead init --yes
? 4
```

`tests/cli/sync.trycmd` (R-CLI-025, R-CLI-043):

```text
$ fairlead sync --baseline --accept-new
--accept-new needs --why TEXT
? 2

$ fairlead sync --dry-run
...

$ fairlead sync --yes
...
```

`tests/cli/new.trycmd` (R-CLI-028, R-CLI-029, R-CLI-045):

```text
$ fairlead new rule no-raw-hex --field no-equals
--field takes K=V, got "no-equals"
? 2

$ fairlead new rule Bad_Name
...
? 2

$ fairlead new approval x
...
? 2

$ fairlead new rule no-raw-hex --dry-run
...
```

`tests/cli/root-missing.trycmd` (R-CLI-004; empty sandbox, no `.in`):

```text
$ fairlead check
[..]fairlead init[..]
? 2
```

Verify before the first run: trycmd 1.2.1 copies a sibling `<case>.in/` directory into the sandbox that runs the case, and the `.trycmd` file runs there. If `.trycmd` cannot sandbox, switch the affected files to the `.toml` form with `fs.sandbox = true`, or pass `--root` to an absolute fixture path. The exit-code expectations rely on Contract issues CI-11 (unknown keys reach the output), on the syntax family's exit 2 (C-22), and on P7 mapping an undeclared kind and an invalid slug to usage (exit 2). Fixture kind keys follow 12-directive-syntax; confirm them there. Clap wording in the `error:` lines follows clap 4.6.7.

- [ ] **Step 2: Run `cargo test -p fairlead --test cli` and expect PASS.** A mismatch is a case-text problem or a missing dependency from P3 to P7; correct the case text only when the bin's own text (the refusals and usage messages) is right.

- [ ] **Step 3: Commit** with message `test(fairlead): per-verb trycmd contract cases with fixture trees` and the trailer.

---

### Task 17: Stateful and static guards in the CLI test target

**Files:**
- Modify: `tests/cli.rs` (append below the trycmd runner)

**Interfaces**
- Consumes: the built binary through `CARGO_BIN_EXE_fairlead`; `tempfile`.
- Produces: integration tests for behaviour that needs a sequence of invocations on disk, and two static guards over the bin's sources.

- [ ] **Step 1: Write the tests.** Append to `tests/cli.rs`:

```rust
use std::path::Path;
use std::process::{Command, Output};

const ROOT_CONTROL: &str = "[fairlead]\nscope = \"root\"\nedition = 1\nroot = true\nmarker = \"fairlead\"\n\n[settings]\n";
const CMP_CONTROL: &str = "[fairlead]\nscope = \"root\"\nedition = 1\nroot = true\nmarker = \"fairlead\"\n\n[settings]\n\n[kinds.cmp]\ntitle = \"Component\"\nshape = \"code\"\n";

fn fairlead(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fairlead"))
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("FAIRLEAD_ROOT", "")
        .env("CI", "")
        .args(args)
        .output()
        .expect("the fairlead binary runs")
}

fn code(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}

fn repo(control: &str, lib: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("fairlead")).expect("fairlead dir");
    std::fs::write(dir.path().join("fairlead").join("control.toml"), control).expect("control file");
    std::fs::create_dir_all(dir.path().join("src")).expect("src dir");
    std::fs::write(dir.path().join("src").join("lib.rs"), lib).expect("source file");
    dir
}

#[test]
fn r_chk_040__a_dangling_use_exits_1() {
    let dir = repo(CMP_CONTROL, "// fairlead: use cmp.button\npub fn answer() -> u32 { 42 }\n");
    assert_eq!(code(&fairlead(dir.path(), &["check", "--format", "terse"])), 1);
}

#[test]
fn r_chk_040__a_stale_generated_shard_exits_3() {
    let dir = repo(CMP_CONTROL, "// fairlead: def cmp.button\npub fn answer() -> u32 { 42 }\n");
    assert_eq!(code(&fairlead(dir.path(), &["sync"])), 0);
    std::fs::write(dir.path().join("src").join("lib.rs"), "// fairlead: def cmp.card\npub fn answer() -> u32 { 42 }\n")
        .expect("edit source");
    assert_eq!(code(&fairlead(dir.path(), &["check", "--format", "terse"])), 3);
}

#[test]
fn r_cli_002__init_without_yes_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(code(&fairlead(dir.path(), &["init"])), 4);
    assert!(!dir.path().join("fairlead").exists());
}

#[test]
fn r_cli_043__sync_yes_is_accepted_and_changes_nothing() {
    let dir = repo(ROOT_CONTROL, "pub fn answer() -> u32 { 42 }\n");
    assert_eq!(code(&fairlead(dir.path(), &["sync"])), 0);
    assert_eq!(code(&fairlead(dir.path(), &["sync", "--yes"])), 0);
}

#[test]
fn r_cli_011__no_environment_variable_disables_a_check() {
    let dir = repo(CMP_CONTROL, "// fairlead: use cmp.button\npub fn answer() -> u32 { 42 }\n");
    let output = Command::new(env!("CARGO_BIN_EXE_fairlead"))
        .current_dir(dir.path())
        .env("FAIRLEAD_DISABLE_CHECKS", "1")
        .env("FAIRLEAD_SKIP_CHECKS", "1")
        .env("NO_CHECKS", "1")
        .env("CI", "")
        .args(["check", "--format", "terse"])
        .output()
        .expect("the fairlead binary runs");
    assert_eq!(code(&output), 1);
}

#[test]
fn r_arch_082__threads_zero_exits_2_before_discovery() {
    let dir = tempfile::tempdir().expect("temp dir");
    let output = fairlead(dir.path(), &["--threads", "0", "check"]);
    assert_eq!(code(&output), 2);
    assert!(String::from_utf8_lossy(&output.stderr).contains("at least 1"));
}

#[test]
fn r_cli_032__two_runs_on_one_tree_are_byte_identical() {
    let dir = repo(ROOT_CONTROL, "pub fn answer() -> u32 { 42 }\n");
    let first = fairlead(dir.path(), &["context", "--format", "terse"]);
    let second = fairlead(dir.path(), &["context", "--format", "terse"]);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(code(&first), code(&second));
}

#[test]
fn r_cli_006__stdout_has_no_carriage_return() {
    let dir = repo(ROOT_CONTROL, "pub fn answer() -> u32 { 42 }\n");
    let output = fairlead(dir.path(), &["check", "--format", "terse"]);
    assert!(!output.stdout.contains(&b'\r'));
}

fn bin_sources() -> Vec<(std::path::PathBuf, String)> {
    let mut found = Vec::new();
    let mut pending = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("bin src is readable") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).expect("bin source is UTF-8");
                found.push((path, text));
            }
        }
    }
    found
}

#[test]
fn r_arch_001__the_bin_has_no_check_path_or_format_logic() {
    for (path, text) in bin_sources() {
        for forbidden in ["HN-", "std::process::exit", "blake3", "fingerprint("] {
            assert!(!text.contains(forbidden), "{} must not contain {forbidden}", path.display());
        }
    }
}

#[test]
fn r_arch_101__main_runs_each_command_inside_the_panic_boundary() {
    let main = bin_sources()
        .into_iter()
        .find(|(path, _)| path.ends_with("main.rs"))
        .expect("main.rs exists")
        .1;
    assert!(main.contains("run_guarded(cli::run_from_env)"));
}
```

`tests/cli.rs` now has the runner and these tests, and it must not use `Cli::parse()` (it exits the process). Verify: `FAIRLEAD_DISABLE_CHECKS`, `FAIRLEAD_SKIP_CHECKS` and `NO_CHECKS` are not read by any code, so the test passes only if no such switch exists (R-CLI-011). The `r_arch_001` guard runs over `crates/fairlead/src` only; the fixture texts live in `tests/`, which the guard does not read.

- [ ] **Step 2: Run `cargo test -p fairlead --test cli` and expect PASS.** The guard tests catch regressions: the bin must not gain `HN-` literals, the process-exit call, or hashing.

- [ ] **Step 3: Commit** with message `test(fairlead): stateful CLI tests and static guards on the bin` and the trailer.

---

## Traceability

Every P8 rule of contract Section 6 (13 rules: CHK 040; CLI 001 002 003 006 011 021 032 043 045; ARCH 001 082 101) maps to at least one task and one test whose name starts with the snake-case rule id (contract Section 7). Trycmd cases are named by file.

| Rule | Milestone | Task(s) | Tests |
|---|---|---|---|
| R-CHK-040 (exit mapping for `check`) | 0.1 | 7, 9, 16, 17 | `r_chk_040__an_empty_root_checks_clean_with_exit_0` (T7); `r_chk_040__an_unknown_rule_code_is_a_usage_error` (T9); `r_chk_040__a_dangling_use_exits_1`, `r_chk_040__a_stale_generated_shard_exits_3` (T17); `check.trycmd`, `check-findings.trycmd`, `check-syntax.trycmd` |
| R-CLI-001 (fixed verb set, reserved verbs exit 2) | 0.1 | 2, 5, 14, 15 | `r_cli_001__verb_set_is_exactly_the_ten_verbs`, `r_cli_001__reserved_verbs_parse_so_they_can_refuse` (T2); `r_cli_001__each_reserved_verb_refuses_with_its_milestone_and_exit_2` (T5); `r_cli_001__a_reserved_verb_dispatches_to_exit_2` (T14); `reserved.trycmd` |
| R-CLI-002 (dry run, plan-first writers) | 0.1 | 2, 11, 16, 17 | `r_cli_002__writers_accept_dry_run` (T2); `r_cli_002__dry_run_wins_and_without_yes_nothing_is_applied` (T11); `r_cli_002__init_without_yes_writes_nothing` (T17); `init.trycmd` |
| R-CLI-003 (exit codes, precedence 5, 2, 4, 1, 3, 0) | 0.1 | 1, 4, 14, 16 | `r_cli_003__each_exit_code_maps_to_its_status`, `r_cli_003__error_classes_map_to_usage_refused_internal`, `r_cli_003__verb_outcome_follows_precedence_5_2_4_1_3_0` (T1); `r_cli_003__a_failed_command_result_carries_the_class_exit_code` (T4); `r_cli_003__a_missing_verb_is_a_usage_error` (T14); exit lines of every `.trycmd` |
| R-CLI-006 (common flags, colour, LF output) | 0.1 | 2, 3, 15, 17 | four parse tests (T2); `r_cli_006__format_defaults_to_human_on_a_tty_and_terse_when_piped`, `r_cli_006__an_explicit_format_always_wins`, `r_cli_006__colour_is_off_when_not_a_tty_or_no_color_is_set`, `r_cli_006__output_is_lf_only_with_one_trailing_newline` (T3); `global.trycmd`; `r_cli_006__stdout_has_no_carriage_return` (T17) |
| R-CLI-011 (no environment switch disables checks) | 0.1 | 17 | `r_cli_011__no_environment_variable_disables_a_check` |
| R-CLI-021 (`check` synopsis and flags) | 0.1 | 2, 9, 16 | `r_cli_021__check_flags_parse_with_budget_max_and_changed` (T2); `r_cli_021__budget_takes_ms_s_or_m_and_rejects_the_rest`, `r_cli_021__max_zero_prints_all_and_absent_is_the_default`, `r_cli_021__for_agent_implies_terse` (T9); `check.trycmd` |
| R-CLI-032 (`context` dispatch) | 0.1 | 2, 7, 10, 16, 17 | `r_cli_032__context_target_is_optional` (T2); `r_cli_032__a_task_query_is_a_usage_error_until_0_3`, `r_cli_032__no_argument_is_the_session_start`, `r_cli_032__an_existing_path_is_a_path_query`, `r_cli_032__an_id_shaped_word_is_an_id_lookup`, `r_cli_032__a_path_above_the_root_is_a_usage_error` (T7); `r_cli_032__budget_zero_is_unlimited_and_absent_is_the_default`, `r_cli_032__human_behaves_as_terse_and_json_is_kept` (T10); `r_cli_032__two_runs_on_one_tree_are_byte_identical` (T17); `context.trycmd` |
| R-CLI-043 (`--yes` accepted by `sync` and `new`; `init` needs it) | 0.1 | 2, 11, 12, 16, 17 | `r_cli_043__sync_and_new_accept_yes_as_a_no_op` (T2); `r_cli_043__init_needs_yes_to_apply` (T11); `r_cli_025__baseline_mode_follows_the_flags` (T12); `r_cli_043__sync_yes_is_accepted_and_changes_nothing` (T17); `sync.trycmd` |
| R-CLI-045 (`approve` is its own verb) | 0.1 | 2, 13, 16 | `r_cli_045__approve_is_a_verb_and_new_approval_is_only_data` (T2); `r_cli_037__the_terminal_gate_refuses_a_pipe_and_ci`, `r_cli_037__the_reviewer_retypes_the_first_four_characters` (T13); `approve.trycmd`; `new.trycmd` (`new approval`) |
| R-ARCH-001 (the bin holds no check, path or format logic; the pipeline only sequences) | 0.1 | 6, 7, 8, 14, 17 | `r_arch_001__load_then_resolve_yields_the_root_scope` (T6); `r_cli_008__readers_never_take_the_writer_lock` (T6); `r_arch_001__a_missing_baseline_file_means_no_baseline` (T7); `r_arch_001__commit_plan_writes_and_releases_the_lock` (T8); `r_arch_001__the_bin_has_no_check_path_or_format_logic` (T17) |
| R-ARCH-082 (thread count by flag and environment) | 0.1 | 2, 4, 17 | `r_arch_082__the_flag_wins_over_the_environment`, `r_arch_082__the_environment_is_used_when_the_flag_is_absent`, `r_arch_082__zero_and_non_integers_are_usage_errors` (T4); `r_arch_082__threads_zero_exits_2_before_discovery` (T17); the `--threads` parse test (T2) |
| R-ARCH-101 (panic boundary exits 5) | 0.1 | 1, 14, 17 | `r_arch_101__panic_inside_a_command_exits_5`, `r_arch_101__without_a_panic_the_result_is_kept` (T1); `r_arch_101__main_runs_each_command_inside_the_panic_boundary` (T17) |

Every task's test list names its rule ids in the snake-case form. Supporting tests of other rules (`r_cli_004__...`, `r_cli_033__...`, `r_cli_035__...`, `r_cli_017__...`) are listed in their tasks but are owned by P3, P6 and P7.

---

## Contract issues

These are gaps or conflicts in [01-contract.md](01-contract.md) that the plan had to close. Each must be decided by its owner before the task that depends on it. The plan's choice is the one in the code above, and the owner may change it, in which case the affected task changes with it.

- **CI-1 (Section 4.13 `Loaded`, P8).** The plan adds `root_control: ControlFile` (`discover` already returns it; check needs the baseline path, context needs `[context]`) and `cwd_rel: RepoPath`. No 0.1 path in this plan reads `cwd_rel`, so drop it unless the owner wants it for later milestones.
- **CI-2 (Section 4.9 / 4.12, P5).** `check_context` converts a `BaselineIssue` into the HN-B002 diagnostic with `Diagnostic::from(issue)`. The contract names no such conversion; P5 provides `impl From<BaselineIssue> for Diagnostic`.
- **CI-3 (Section 4.5 / 4.13, P3).** `load` needs `extract::extract_plan_from_controls(&ControlFile, &ControlSet) -> ExtractPlan`, which maps the root's `[comments]`, `[scan]` and `marker` into `ExtractConfig`, and `StyleKinds` and leak properties into the plan. The contract names no constructor.
- **CI-4 (Section 4.4 / 4.12, P3 and P7).** `init` runs with no root. `Discovered` requires a `root_control`, so discover cannot report "no root yet" with `require_root = false`. The plan needs `write::detect_init(&Utf8Path) -> Result<InitDetection, Error>` (read-only detection, R-CLI-018) and `write::init_control_bytes(&InitDetection) -> Vec<u8>` (the bytes init would write). `init_root` builds the `Root` in memory.
- **CI-5 (Section 4.12, P7).** The planner `init_plan` needs the first scan of the in-memory control file (R-CLI-019). The plan adds `pipeline::init_first_scan` (Task 11) and leaves the `init_plan` signature unchanged.
- **CI-6 (Section 4.3, P3).** The root itself is not a `RepoPath` in the contract. `RepoPath::new("")` is the natural form, or `RepoPath::root()`. Until P3 decides, `check .` and `context .` return `PathOutsideRoot` (exit 2), which is wrong. Decide before Task 9's tests run.
- **CI-7 (Section 4.12, P7).** The bin prints plans with `Plan::render_text(&self) -> String`. Its lines are `action path detail`, and it is P7's text, not the bin's.
- **CI-8 (Section 4.12, P7).** `new --print` returns the scaffold in `Plan.notes` with an empty `WriteSet`. P7 must document this; the bin reads `notes` and writes nothing.
- **CI-9 (Section 4.3, P3).** `approve --revoke` needs `Fingerprint::from_text(&str) -> Result<Fingerprint, Error>`, and `Fingerprint` and `Id` need `Display` (used by `list_text`).
- **CI-10 (R-ARCH-103, P3 or P5).** `Error::diagnostics()` has no renderer in the contract, so the bin prints only the error's Display. A pure `render::diagnostic_lines(&[Diagnostic]) -> String` is needed, with the excerpt and caret of R-ARCH-103.
- **CI-11 (Section 4.6, P3).** `ControlFile.unknown` holds `UnknownKey` values, not diagnostics, so HN-C002 never reaches `load_diags`. P3 must place those diagnostics in `violations` (or a sibling list the pipeline collects). Without it, the unknown-key cases of the trycmd fixtures cannot pass.
- **CI-12 (Section 4.12, P7).** The concrete `PidLiveness` (called `SystemLiveness` in this plan, in `write::lock`) is not named. The contract names only the trait. P7 provides the struct; Section 2's pid-liveness crate is UNVERIFIED.
- **CI-13 (Section 3 and 4.13, P8).** `commands/mod.rs` is an extra module that holds `CommandResult` and the shared glue (the contract places `CommandResult` under `commands/*.rs`). `reserved::run` takes `ReservedVerb`, not `&ReservedArgs`, because `ReservedArgs` cannot know its verb. Both are P8's own choices, recorded for P9's harnesses.
- **CI-14 (Section 4.3, P3).** `Error::Reserved` Display must read exactly `<verb> arrives in <milestone>`. Task 5's test pins the text, so a different Display fails there first.

---

## Execution handoff

- **Order.** Tasks 1 to 5 need the P1 workspace (`[workspace.package]` with edition, rust-version and license-file, and the lint tables) and the P3 types they name (`ExitCode`, `Error`, `VerbOutcome`, `PinRefusal`, `AsOf`). Tasks 6 to 8 need P4 to P7 seams. Tasks 9 to 14 need P5 to P7. Tasks 15 to 17 need the whole bin. Task 14 is the first point where the binary runs end to end.
- **Blocking questions.** CI-1, CI-6 and CI-11 block the first pipeline and check tests. CI-4 and CI-5 block Task 11. CI-9 blocks Task 13. Raise them with the owning phase before starting the dependent task, not during it.
- **Verify first.** The APIs marked Verify in the text: clap 4.6.7 derive attributes (`global`, `value_enum`, `default_value`, `trailing_var_arg`, `#[value(name)]`) and `TypedValueParser::map` on `PossibleValuesParser`; `camino` `FromStr` and `Utf8PathBuf::from_path_buf`; `std::process::ExitCode: PartialEq` and `Debug`; `Option::is_some_and` and `std::io::read_to_string` (stable by 1.88); `IsTerminal`; `panic::set_hook(Box::new(|_| {}))` inference; trycmd 1.2.1 `.trycmd` syntax, `TestCases::env`, and sibling `.in` sandboxing.
- **Commits.** One commit per task step 4, each message ending with `Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>`. No commit uses `--no-verify`.
- **Gates before merge.** `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo test -p fairlead` and `cargo test -p fairlead-core --test pipeline`. The MSRV job (P1, `msrv.yml`) runs on 1.88. P9 adds the remaining `[[test]]` targets (`golden`, `determinism`, `readonly`, `sarif`, `secrets`, `replay`, `platform`) when their files exist, and the golden repositories of Section 7 are P9's.

