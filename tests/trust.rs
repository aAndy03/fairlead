#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! The 0.2.1 trust patch (D-62, plans/0.2/0.2.1-trust-patch.md): each fix the agent field test of
//! 0.1.0 and 0.2.0 asked for, end to end through the binary.
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

fn rule(repo: &TempRepo, name: &str, forbid: &str, glob: &str) -> testkit::Run {
    let f = format!("forbid={forbid}");
    let g = format!("glob={glob}");
    repo.run(&[
        "new", "rule", name, "--field", &f, "--field", &g, "--field", "why=w", "--field", "msg=m",
        "--as-of", AS_OF,
    ])
}

#[test]
fn r_cli_028__new_refuses_a_rule_check_would_reject_and_writes_nothing() {
    let repo = repo();
    let before = repo.tree_snapshot();
    let bad = rule(&repo, "bad-regex", "panic!(", "src/**/*.rs");
    assert_eq!(bad.code, 2, "{}{}", bad.stdout, bad.stderr);
    assert!(
        bad.stderr.contains("is not a valid regex") && bad.stderr.contains("nothing written"),
        "{}",
        bad.stderr
    );
    let dead = rule(&repo, "dead-glob", "panic!\\(", "src");
    assert_eq!(dead.code, 2, "{}{}", dead.stdout, dead.stderr);
    assert!(
        dead.stderr
            .contains("glob src of rule.dead-glob matches no file"),
        "{}",
        dead.stderr
    );
    assert_eq!(repo.tree_snapshot(), before, "a refused new writes nothing");
    let dry = repo.run(&[
        "new",
        "rule",
        "bad-regex",
        "--field",
        "forbid=panic!(",
        "--dry-run",
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(dry.code, 2, "a dry run validates too: {}", dry.stderr);
    let good = rule(&repo, "no-panic", "panic!\\(", "src/**/*.rs");
    assert_eq!(good.code, 0, "{}{}", good.stdout, good.stderr);
    assert_eq!(repo.run(&["sync", "--as-of", AS_OF]).code, 0);
    let check = repo.run(&["check", "--as-of", AS_OF, "--format", "terse"]);
    assert!(
        check.code <= 1,
        "every verb still works: {}{}",
        check.stdout,
        check.stderr
    );
}

#[test]
fn r_file_050__explain_says_how_a_file_item_is_declared() {
    let repo = repo();
    std::fs::write(
        repo.path().join("src/b.rs"),
        "// fairlead: def file.calc\nfn b() {}\n",
    )
    .unwrap();
    let missing = repo.run(&["explain", "file.calc", "--as-of", AS_OF]);
    assert_eq!(missing.code, 2);
    assert!(
        missing.stderr.contains("[[file]] table") && missing.stderr.contains("declares nothing"),
        "{}",
        missing.stderr
    );
    let control = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&control).unwrap();
    std::fs::write(
        &control,
        format!("{text}\n[[file]]\nid = \"file.calc\"\nkind = \"rust\"\npath = \"src/a.rs\"\n"),
    )
    .unwrap();
    assert_eq!(repo.run(&["sync", "--as-of", AS_OF]).code, 0);
    let found = repo.run(&["explain", "file.calc", "--as-of", AS_OF]);
    assert_eq!(found.code, 0, "{}", found.stderr);
    assert!(found.stdout.contains("root:file.calc"), "{}", found.stdout);
}

#[test]
fn r_cli_035__explain_answers_static_topics_and_codes_without_a_root() {
    let dir = tempfile::tempdir().unwrap();
    for target in ["syntax", "approve", "docs-first", "HN-R001"] {
        let run = testkit::run_fairlead(dir.path(), &["explain", target], &[]);
        assert_eq!(run.code, 0, "{target}: {}", run.stderr);
        assert!(!run.stdout.is_empty(), "{target}");
    }
    let kinds = testkit::run_fairlead(dir.path(), &["explain", "kinds"], &[]);
    assert_eq!(kinds.code, 2, "kinds reads the codebase");
    assert!(kinds.stderr.contains("fairlead init"), "{}", kinds.stderr);
}

#[test]
fn r_cli_035__help_text_describes_the_running_release() {
    let dir = tempfile::tempdir().unwrap();
    let card = testkit::run_fairlead(dir.path(), &["explain", "syntax"], &[]);
    assert!(
        !card.stdout.contains("arrive in 0.2") && !card.stdout.contains("until= is 0.2"),
        "{}",
        card.stdout
    );
    assert!(
        card.stdout.contains("until=MAJOR.MINOR.PATCH"),
        "{}",
        card.stdout
    );
    let repo = repo();
    let bad = repo.run(&["explain", "no-such-thing", "--as-of", AS_OF]);
    assert!(
        bad.stderr.contains("docs-first") && bad.stderr.contains("authority"),
        "every topic is listed: {}",
        bad.stderr
    );
}

#[test]
fn r_cli_021__for_agent_names_hidden_observe_findings_and_the_next_step() {
    let repo = repo();
    assert_eq!(rule(&repo, "no-fn-a", "fn a", "src/**/*.rs").code, 0);
    assert_eq!(repo.run(&["sync", "--as-of", AS_OF]).code, 0);
    let quiet = repo.run(&["check", "--for-agent", "--as-of", AS_OF]);
    assert!(
        quiet
            .stdout
            .contains("observe: 1 hidden; show them with fairlead check --show-observe"),
        "{}",
        quiet.stdout
    );
    std::fs::write(
        repo.path().join("src/b.rs"),
        "// fairlead: use rule.nosuch\nfn b() {}\n",
    )
    .unwrap();
    let found = repo.run(&["check", "--for-agent", "--as-of", AS_OF]);
    assert!(
        found.stdout.contains("next: fairlead explain HN-"),
        "{}",
        found.stdout
    );
    let plain = repo.run(&["check", "--format", "terse", "--as-of", AS_OF]);
    assert!(
        !plain.stdout.contains("next:") && !plain.stdout.contains("observe: 1 hidden"),
        "terse output keeps its contract: {}",
        plain.stdout
    );
}

#[test]
fn r_chk_009__the_summary_says_when_an_error_does_not_fail_the_run() {
    // HN-A002 (no CODEOWNERS for the managerial folder) is an error at stage warn.
    let repo = repo();
    let run = repo.run(&[
        "check",
        "--no-baseline",
        "--format",
        "human",
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(run.code, 0, "{}", run.stdout);
    assert!(
        run.stdout
            .contains("1 error is at stage warn and does not fail the run"),
        "{}",
        run.stdout
    );
    let terse = repo.run(&[
        "check",
        "--no-baseline",
        "--format",
        "terse",
        "--as-of",
        AS_OF,
    ]);
    assert!(
        !terse.stdout.contains("stage warn"),
        "terse keeps its contract"
    );
}

#[test]
fn r_cli_037__an_empty_approval_list_says_so() {
    let repo = repo();
    let run = repo.run(&["approve", "--list", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.stdout, "no approvals recorded\n");
}
