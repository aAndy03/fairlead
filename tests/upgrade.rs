#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! Editions, pins and `upgrade` end to end (chapter 22). Edition 2 is the synthetic test edition
//! of the `test-edition` feature, selected per run with `FAIRLEAD_TEST_EDITION=2` (Q-1).
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, TempRepo};

const AS_OF: &str = "2026-10-10";
const ED2: [(&str, &str); 1] = [("FAIRLEAD_TEST_EDITION", "2")];

fn repo_on(edition: &[(&str, &str)]) -> TempRepo {
    let repo = RepoBuilder::new()
        .file("src/a.rs", "fn a() {}\n")
        .git_init(true)
        .build();
    let init = repo.run_in(repo.path(), &["init", "--yes", "--as-of", AS_OF], edition);
    assert_eq!(init.code, 0, "{}", init.stderr);
    repo.git(&["add", "-A"]);
    repo.git_commit("fairlead init");
    repo
}

#[test]
fn r_edn_012__refusal_has_exact_install_line() {
    let repo = repo_on(&[]);
    let run = repo.run_in(repo.path(), &["check", "--as-of", AS_OF], &ED2);
    assert_eq!(run.code, 4, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr
            .contains("cargo install fairlead --version 0.1.0 --locked"),
        "{}",
        run.stderr
    );
}

#[test]
fn r_edn_013__newer_edition_refused_exit_4() {
    let repo = repo_on(&ED2);
    let control = std::fs::read_to_string(repo.path().join("fairlead/control.toml")).unwrap();
    assert!(control.contains("edition = 2"), "{control}");
    let run = repo.run(&["check", "--as-of", AS_OF]);
    assert_eq!(run.code, 4, "{}{}", run.stdout, run.stderr);
    assert!(run.stderr.contains("pinned to edition 2"), "{}", run.stderr);
}

#[test]
fn r_edn_011__writers_refuse_older_edition() {
    let repo = repo_on(&[]);
    let before = repo.tree_snapshot();
    for verb in [
        vec!["sync", "--yes"],
        vec!["new", "change", "x"],
        vec!["seal", "--yes", "--anywhere"],
        vec!["check"],
    ] {
        let mut args = verb.clone();
        args.extend(["--as-of", AS_OF]);
        let run = repo.run_in(repo.path(), &args, &ED2);
        assert_eq!(run.code, 4, "{verb:?}: {}{}", run.stdout, run.stderr);
    }
    assert_eq!(repo.tree_snapshot(), before, "nothing written");
}
