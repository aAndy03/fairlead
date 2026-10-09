#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! Reader verbs never change a tracked file (R-ARCH-140, R-CHK-015, R-GIT-001).
#[path = "testkit/mod.rs"]
mod testkit;

use std::path::Path;
use testkit::golden_spec as gs;
use testkit::{CHECK_TERSE, golden_checkout, run_fairlead};

/// The reader invocations per real repository. Each command runs in the same checkout.
fn readers(name: &str) -> Vec<Vec<&'static str>> {
    let check: Vec<&str> = CHECK_TERSE.to_vec();
    match name {
        "ts-react" => vec![
            check,
            vec!["context", "apps/console/src/molecules/nav-group.tsx"],
            vec!["explain", "HN-T001"],
        ],
        "rust-workspace" => vec![
            check,
            vec!["context", "crates/sync-core"],
            vec!["explain", "fn.digest"],
        ],
        _ => vec![check, vec!["context", "src/api/notes.ts"]],
    }
}

fn assert_clean_after_readers(name: &str, root: &Path) {
    for args in readers(name) {
        let run = run_fairlead(root, &args, &[]);
        assert_ne!(
            run.code, 5,
            "{name} {args:?}: internal error: {}",
            run.stderr
        );
        let status = gs::git_status(root).expect("git status");
        assert!(
            status.is_empty(),
            "{name} {args:?} changed the working tree:\n{status}"
        );
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
    assert_eq!(
        gs::git_status(&root).expect("git status"),
        "",
        "check wrote a tracked file"
    );
}
