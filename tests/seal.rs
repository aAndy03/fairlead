#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! `fairlead seal` end to end (R-VER-050 to R-VER-059): init, a ready change doc, the plan, the
//! seal, then a clean check and an idempotent rerun.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, TempRepo};

const AS_OF: &str = "2026-10-10";

fn ready_repo() -> TempRepo {
    let repo = RepoBuilder::new()
        .file("src/a.rs", "fn a() {}\n")
        .git_init(true)
        .build();
    let init = repo.run(&["init", "--yes", "--as-of", AS_OF]);
    assert_eq!(init.code, 0, "{}", init.stderr);
    repo.git(&["add", "-A"]);
    repo.git_commit("fairlead init");
    let new = repo.run(&[
        "new",
        "change",
        "Adds a",
        "--field",
        "impact=add",
        "--field",
        "covers=src/**",
    ]);
    assert_eq!(new.code, 0, "{}", new.stderr);
    let changes = repo.path().join("fairlead/docs/changes");
    let doc = std::fs::read_dir(&changes)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&doc).unwrap();
    std::fs::write(
        &doc,
        text.replace("status = \"draft\"", "status = \"ready\""),
    )
    .unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("ready doc");
    repo
}

#[test]
fn r_ver_050__seal_without_yes_prints_the_plan_and_writes_nothing() {
    let repo = ready_repo();
    let before = repo.tree_snapshot();
    let run = repo.run(&["seal", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(run.code, 4, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stdout
            .contains("bump: none -> 0.1.0 (1 docs, 0 carried)"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("fairlead/versions/0.1.0.md"));
    assert!(run.stdout.contains("not applied"));
    assert_eq!(repo.tree_snapshot(), before);
}

#[test]
fn r_ver_059__seal_writes_exactly_its_write_set_and_check_stays_clean() {
    let repo = ready_repo();
    let run = repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let written: Vec<&str> = run
        .stdout
        .lines()
        .filter_map(|l| l.strip_prefix("wrote "))
        .collect();
    assert_eq!(written.len(), 6, "{written:?}");
    let version = std::fs::read_to_string(repo.path().join("fairlead/versions/0.1.0.md")).unwrap();
    assert!(
        version.contains("bump = \"start\"")
            && version.contains("sealed-at = \"2026-10-10T00:00:00Z\"")
    );
    let sealed =
        std::fs::read_to_string(repo.path().join("fairlead/docs/changes/0001-adds-a.md")).unwrap();
    assert!(sealed.contains("status = \"sealed\"") && sealed.contains("ordinal = 1"));
    assert!(sealed.contains("sealed = \"0.1.0\"") && sealed.contains("body-hash = \"b3:"));
    let ledger = std::fs::read_to_string(repo.path().join("fairlead/ledger/hist/2026.hl")).unwrap();
    assert!(ledger.contains("# ckpt v=0.1.0 n=1 d="), "{ledger}");
    let check = repo.run(&["check", "--as-of", AS_OF, "--format", "terse"]);
    assert_eq!(check.code, 0, "{}{}", check.stdout, check.stderr);
}

#[test]
fn r_ver_051__a_rerun_after_sealing_refuses_and_changes_nothing() {
    let repo = ready_repo();
    assert_eq!(
        repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF])
            .code,
        0
    );
    repo.git(&["add", "-A"]);
    repo.git_commit("seal 0.1.0");
    let before = repo.tree_snapshot();
    let again = repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(again.code, 4);
    assert!(
        again.stderr.contains("no change doc is ready"),
        "{}",
        again.stderr
    );
    assert_eq!(repo.tree_snapshot(), before);
}

#[test]
fn r_ver_055__seal_refuses_off_the_integration_branch() {
    let repo = ready_repo();
    repo.git(&["checkout", "-q", "-b", "feature/x"]);
    let run = repo.run(&["seal", "--yes", "--as-of", AS_OF]);
    assert_eq!(run.code, 4);
    assert!(run.stderr.contains("integration branch"), "{}", run.stderr);
}

#[test]
fn r_git_063__seal_refuses_a_dirty_tree_unless_allowed() {
    let repo = ready_repo();
    std::fs::write(repo.path().join("src/a.rs"), "fn a() { }\n").unwrap();
    let run = repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(run.code, 4);
    assert!(
        run.stderr.contains("outside the seal write-set"),
        "{}",
        run.stderr
    );
    let ok = repo.run(&[
        "seal",
        "--yes",
        "--anywhere",
        "--allow-dirty",
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(ok.code, 0, "{}", ok.stderr);
    let version = std::fs::read_to_string(repo.path().join("fairlead/versions/0.1.0.md")).unwrap();
    assert!(version.contains("dirty = true"));
}
