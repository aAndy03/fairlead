#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! The 0.2.2 field fixes (D-62, plans/0.2/0.2.2-field-fixes.md): each fix the third agent field
//! test (0.2.1 on a real repository) asked for, end to end through the binary.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, TempRepo};

const AS_OF: &str = "2026-10-11";

fn repo() -> TempRepo {
    let repo = RepoBuilder::new()
        .file("src/a.rs", "fn a() {}\n")
        .git_init(true)
        .build();
    let init = repo.run(&["init", "--yes", "--as-of", AS_OF]);
    assert_eq!(init.code, 0, "{}", init.stderr);
    repo
}

fn rule(repo: &TempRepo, name: &str) {
    let run = repo.run(&[
        "new",
        "rule",
        name,
        "--field",
        "forbid=XYZ",
        "--field",
        "glob=src/**",
        "--field",
        "why=w",
        "--field",
        "msg=m",
        "--yes",
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
}

#[test]
fn r_lc_006__a_due_plan_shows_in_check_and_explain_before_the_seal() {
    let repo = repo();
    rule(&repo, "no-x");
    std::fs::write(
        repo.path().join("src/a.rs"),
        "// fairlead: flag deprecate rule.no-x target=0.1.0 -- gone\nfn a() {}\n",
    )
    .unwrap();
    let due = "the deprecate plan for rule.no-x at 0.1.0 is due and blocks the next seal";
    let check = repo.run(&["check", "--as-of", AS_OF, "--format", "terse"]);
    assert!(
        check
            .stdout
            .contains(&format!("HN-L001 warn src/a.rs:1:4 {due}")),
        "{}",
        check.stdout
    );
    let explain = repo.run(&["explain", "rule.no-x", "--as-of", AS_OF]);
    assert_eq!(explain.code, 0, "{}", explain.stderr);
    assert!(
        explain.stdout.contains("findings:") && explain.stdout.contains(due),
        "{}",
        explain.stdout
    );
}
