#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! 0.3 retention and compaction (33-retention-and-compaction, plans/0.3 P5) end to end.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, TempRepo};

const RETAIN: &str = "\n[retain]\nmin-age = \"0d\"\n\n[retain.chg]\nttl = \"1v\"\n";

fn repo(retain: &str) -> TempRepo {
    let repo = RepoBuilder::new()
        .file("src/a.rs", "fn a() {}\n")
        .git_init(true)
        .build();
    let init = repo.run(&["init", "--yes", "--as-of", "2026-01-05"]);
    assert_eq!(init.code, 0, "{}", init.stderr);
    let ctl = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&ctl).unwrap();
    std::fs::write(&ctl, format!("{text}{retain}")).unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("init");
    repo
}

/// One change doc, ready with `sum` (or without), committed and sealed on `as_of`.
fn cycle(repo: &TempRepo, title: &str, sum: Option<&str>, as_of: &str) -> testkit::Run {
    let new = repo.run(&[
        "new",
        "change",
        title,
        "--field",
        "impact=fix",
        "--field",
        "covers=src/**",
        "--as-of",
        as_of,
    ]);
    assert_eq!(new.code, 0, "{}", new.stderr);
    let changes = repo.path().join("fairlead/docs/changes");
    let doc = std::fs::read_dir(&changes)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("d-"))
        .unwrap();
    let mut text = std::fs::read_to_string(&doc).unwrap();
    text = text.replace("status = \"draft\"", "status = \"ready\"");
    if let Some(s) = sum {
        text = text.replacen("sum = \"\"", &format!("sum = \"{s}\""), 1);
    }
    std::fs::write(&doc, text).unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit(title);
    let seal = repo.run(&["seal", "--yes", "--anywhere", "--as-of", as_of]);
    repo.git(&["add", "-A"]);
    repo.git_commit(&format!("seal {title}"));
    seal
}

fn ledger(repo: &TempRepo, year: &str) -> String {
    std::fs::read_to_string(repo.path().join(format!("fairlead/ledger/hist/{year}.hl")))
        .unwrap_or_default()
}

fn doc_files(repo: &TempRepo) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(repo.path().join("fairlead/docs/changes"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn r_ret_002__new_creates_sum_and_out_slots() {
    let repo = repo(RETAIN);
    let new = repo.run(&["new", "change", "Slots", "--as-of", "2026-01-05"]);
    assert_eq!(new.code, 0, "{}", new.stderr);
    let changes = repo.path().join("fairlead/docs/changes");
    let doc = std::fs::read_dir(&changes)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(doc).unwrap();
    for k in ["sum = \"\"", "out = \"\"", "why = \"\""] {
        assert!(text.contains(k), "{k}: {text}");
    }
}

#[test]
fn r_ret_025__body_deleted_id_resolvable() {
    let repo = repo(RETAIN);
    assert_eq!(
        cycle(&repo, "First", Some("first change"), "2026-01-10").code,
        0
    );
    assert_eq!(
        cycle(&repo, "Second", Some("second change"), "2026-02-10").code,
        0
    );
    assert!(
        doc_files(&repo).iter().any(|f| f.ends_with("-first.md")),
        "age 1 is not due"
    );
    let third = cycle(&repo, "Third", Some("third change"), "2026-03-10");
    assert_eq!(third.code, 0, "{}{}", third.stdout, third.stderr);
    assert!(
        !doc_files(&repo).iter().any(|f| f.ends_with("-first.md")),
        "compacted: {:?}",
        doc_files(&repo)
    );
    let l = ledger(&repo, "2026");
    let line = l
        .lines()
        .find(|x| x.contains("sum=\"first change\""))
        .unwrap_or_else(|| panic!("{l}"));
    for k in [
        "kind=change",
        "scope=root",
        "ord=0001",
        "title=First",
        "closed=",
        "out=adopted",
        "git=",
        "body=",
    ] {
        assert!(line.contains(k), "{k}: {line}");
    }
    let check = repo.run(&["check", "--as-of", "2026-03-10", "--format", "terse"]);
    assert!(!check.stdout.contains("HN-G003"), "{}", check.stdout);
    let id = line.split_whitespace().next().unwrap();
    let explain = repo.run(&["explain", id, "--as-of", "2026-03-10"]);
    assert_eq!(explain.code, 0, "{}{}", explain.stdout, explain.stderr);
    assert!(
        explain.stdout.contains("first change") && explain.stdout.contains("commit"),
        "{}",
        explain.stdout
    );
}

#[test]
fn r_ret_022__due_record_without_summary_blocks_seal() {
    let repo = repo(RETAIN);
    assert_eq!(cycle(&repo, "First", None, "2026-01-10").code, 0);
    assert_eq!(cycle(&repo, "Second", Some("s"), "2026-02-10").code, 0);
    let third = cycle(&repo, "Third", Some("t"), "2026-03-10");
    assert_eq!(third.code, 4, "{}{}", third.stdout, third.stderr);
    assert!(
        third.stderr.contains("HN-K001") && third.stderr.contains("`sum` is empty"),
        "{}",
        third.stderr
    );
}

#[test]
fn r_ret_021__two_runs_byte_identical() {
    // the same tree (history included) compacted on two machines: identical ledger bytes
    let repo = repo(RETAIN);
    assert_eq!(cycle(&repo, "First", Some("s"), "2026-01-10").code, 0);
    assert_eq!(cycle(&repo, "Second", Some("s"), "2026-02-10").code, 0);
    let new = repo.run(&[
        "new",
        "change",
        "Third",
        "--field",
        "impact=fix",
        "--field",
        "covers=src/**",
        "--field",
        "sum=t",
        "--as-of",
        "2026-03-10",
    ]);
    assert_eq!(new.code, 0, "{}", new.stderr);
    let changes = repo.path().join("fairlead/docs/changes");
    let doc = std::fs::read_dir(&changes)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("d-"))
        .unwrap();
    let text = std::fs::read_to_string(&doc)
        .unwrap()
        .replace("status = \"draft\"", "status = \"ready\"")
        .replacen("sum = \"\"", "sum = \"t\"", 1);
    std::fs::write(&doc, text).unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("third");
    let other = tempfile::tempdir().unwrap();
    copy(repo.path(), other.path());
    let a = repo.run(&["seal", "--yes", "--anywhere", "--as-of", "2026-03-10"]);
    let b = testkit::run_fairlead(
        other.path(),
        &["seal", "--yes", "--anywhere", "--as-of", "2026-03-10"],
        &[],
    );
    assert_eq!((a.code, b.code), (0, 0), "{}{}", a.stderr, b.stderr);
    let la = ledger(&repo, "2026");
    let lb = std::fs::read_to_string(other.path().join("fairlead/ledger/hist/2026.hl")).unwrap();
    assert!(la.contains("sum=s"), "{la}");
    assert_eq!(la, lb);
}

fn copy(from: &std::path::Path, to: &std::path::Path) {
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&dst).unwrap();
            copy(&e.path(), &dst);
        } else {
            std::fs::copy(e.path(), dst).unwrap();
        }
    }
}
