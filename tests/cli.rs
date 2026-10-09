#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)] // reason: tests
//! CLI contract tests (contract Section 7). trycmd runs every case in tests/cli/*.trycmd against the built `fairlead` binary.

#[test]
fn cli_contract_cases() {
    trycmd::TestCases::new()
        .register_bin(
            "fairlead",
            std::path::Path::new(env!("CARGO_BIN_EXE_fairlead")),
        )
        .env("NO_COLOR", "1")
        .env("FAIRLEAD_ROOT", "")
        .env("CI", "")
        .case(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/cli/*.trycmd"
        ));
}

use std::path::Path;
use std::process::{Command, Output};

const ROOT_CONTROL: &str =
    "[fairlead]\nscope = \"root\"\nedition = 1\nroot = true\nmarker = \"fairlead\"\n\n[settings]\n";
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
    std::fs::write(dir.path().join("fairlead").join("control.toml"), control)
        .expect("control file");
    std::fs::create_dir_all(dir.path().join("src")).expect("src dir");
    std::fs::write(dir.path().join("src").join("lib.rs"), lib).expect("source file");
    dir
}

#[test]
fn r_chk_040__a_dangling_use_exits_1() {
    let dir = repo(
        CMP_CONTROL,
        "// fairlead: use cmp.button\npub fn answer() -> u32 { 42 }\n",
    );
    assert_eq!(
        code(&fairlead(dir.path(), &["check", "--format", "terse"])),
        1
    );
}

#[test]
fn r_chk_040__a_stale_generated_shard_exits_3() {
    // A `def` in code is an implementation site (R-SYN, 12); items are declared in docs, and the
    // registry shards `sync` writes list them.
    let dir = repo(
        CMP_CONTROL,
        "pub fn answer() -> u32 { 42 }
",
    );
    let doc = |name: &str| {
        format!(
            "+++
directives = ['def cmp.{name}']
+++
# {name}
"
        )
    };
    let docs = dir.path().join("fairlead").join("docs");
    std::fs::create_dir_all(&docs).expect("docs dir");
    std::fs::write(docs.join("cmp.md"), doc("button")).expect("write doc");
    assert_eq!(code(&fairlead(dir.path(), &["sync"])), 0);
    std::fs::write(docs.join("cmp.md"), doc("card")).expect("edit doc");
    assert_eq!(
        code(&fairlead(dir.path(), &["check", "--format", "terse"])),
        3
    );
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
    let dir = repo(
        CMP_CONTROL,
        "// fairlead: use cmp.button\npub fn answer() -> u32 { 42 }\n",
    );
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
            assert!(
                !text.contains(forbidden),
                "{} must not contain {forbidden}",
                path.display()
            );
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
