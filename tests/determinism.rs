#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! Byte identity of `check` across cache states, thread counts, line endings and directories.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{CHECK_TERSE, golden_checkout, golden_expected, run_fairlead};

const GOLDENS: [&str; 2] = ["ts-react", "rust-workspace"];

fn stdout_of(cwd: &std::path::Path, extra: &[&str], env: &[(&str, &str)]) -> Vec<u8> {
    let args: Vec<&str> = CHECK_TERSE
        .iter()
        .copied()
        .chain(extra.iter().copied())
        .collect();
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
        for (label, got) in [
            ("cold", cold),
            ("warm", warm),
            ("warm again", warm_again),
            ("deleted cache", deleted),
        ] {
            assert_eq!(got, want, "{name}: {label} output differs from the golden");
        }
    }
}

#[test]
fn r_arch_080__one_thread_and_default_threads_are_byte_identical() {
    for name in GOLDENS {
        let (_s, root) = golden_checkout(name, false);
        let want = golden_expected(name);
        assert_eq!(
            stdout_of(&root, &["--threads", "1"], &[("RAYON_NUM_THREADS", "1")]),
            want,
            "{name}: one thread"
        );
        assert_eq!(stdout_of(&root, &[], &[]), want, "{name}: default threads");
    }
}

#[test]
fn r_arch_111__lf_and_crlf_checkouts_are_byte_identical() {
    for name in GOLDENS {
        let (_s, root) = golden_checkout(name, true);
        assert_eq!(
            stdout_of(&root, &[], &[]),
            golden_expected(name),
            "{name}: CRLF checkout"
        );
    }
}

#[test]
fn r_chk_012__bom_and_crlf_do_not_change_output() {
    let (_s, root) = golden_checkout("ts-react", true);
    let file = root.join("apps/console/src/molecules/nav-group.tsx");
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(std::fs::read(&file).expect("read"));
    std::fs::write(&file, bytes).expect("write BOM");
    assert_eq!(
        stdout_of(&root, &[], &[]),
        golden_expected("ts-react"),
        "BOM plus CRLF"
    );
}

#[test]
fn r_arch_043__invocation_directory_does_not_change_output() {
    let (_s, root) = golden_checkout("ts-react", false);
    let nested = root.join("apps/console/src");
    assert_eq!(
        stdout_of(&nested, &[], &[]),
        golden_expected("ts-react"),
        "run from a subdirectory"
    );
}
