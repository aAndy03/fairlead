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

#[test]
fn r_ret_043__purge_reduces_a_record_and_keeps_the_chain() {
    let repo = repo(RETAIN);
    assert_eq!(
        cycle(&repo, "First", Some("first change"), "2026-01-10").code,
        0
    );
    assert_eq!(cycle(&repo, "Second", Some("s"), "2026-02-10").code, 0);
    assert_eq!(cycle(&repo, "Third", Some("t"), "2026-03-10").code, 0);
    let l = ledger(&repo, "2026");
    let id = l
        .lines()
        .find(|x| x.contains("sum=\"first change\""))
        .and_then(|x| x.split_whitespace().next())
        .unwrap()
        .to_owned();
    let run = repo.run(&["sync", "--yes", "--purge", &id, "--as-of", "2026-04-01"]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let l = ledger(&repo, "2026");
    assert!(!l.contains("first change"), "{l}");
    assert!(l.contains(&format!("{id} purged=2026-04-01 d=")), "{l}");
    let check = repo.run(&["check", "--as-of", "2026-04-01", "--format", "terse"]);
    assert!(!check.stdout.contains("HN-G003"), "{}", check.stdout);
    let none = repo.run(&[
        "sync",
        "--yes",
        "--purge",
        "d-zzzzzzzz",
        "--as-of",
        "2026-04-01",
    ]);
    assert_eq!(none.code, 2, "{}", none.stderr);
}

#[test]
fn r_ret_043__a_deleted_sealed_record_is_tombstoned_by_the_user() {
    let repo = repo("");
    assert_eq!(cycle(&repo, "First", Some("first"), "2026-01-10").code, 0);
    let sealed = doc_files(&repo)
        .into_iter()
        .find(|f| f.starts_with("0001-"))
        .unwrap();
    std::fs::remove_file(repo.path().join("fairlead/docs/changes").join(&sealed)).unwrap();
    let run = repo.run(&["sync", "--yes", "--as-of", "2026-01-20"]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let tombs = std::fs::read_to_string(repo.path().join("fairlead/tombstones/2026.toml"))
        .unwrap_or_default();
    assert!(tombs.contains("deleted-by = \"user\""), "{tombs}");
}

#[test]
fn r_ret_044__a_draft_deleted_before_sealing_leaves_no_tombstone() {
    let repo = repo("");
    let new = repo.run(&["new", "change", "Gone", "--as-of", "2026-01-05"]);
    assert_eq!(new.code, 0, "{}", new.stderr);
    for f in doc_files(&repo) {
        std::fs::remove_file(repo.path().join("fairlead/docs/changes").join(f)).unwrap();
    }
    assert_eq!(
        repo.run(&["sync", "--yes", "--as-of", "2026-01-06"]).code,
        0
    );
    assert!(!repo.path().join("fairlead/tombstones/2026.toml").exists());
}

#[test]
fn r_ret_024__dry_run_prints_the_plan_and_compact_only_catches_up() {
    let repo = repo("");
    for (t, d) in [
        ("First", "2026-01-10"),
        ("Second", "2026-02-10"),
        ("Third", "2026-03-10"),
    ] {
        assert_eq!(cycle(&repo, t, Some("s"), d).code, 0);
    }
    assert!(
        doc_files(&repo).iter().any(|f| f.ends_with("-first.md")),
        "retention was off"
    );
    let ctl = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&ctl).unwrap();
    std::fs::write(&ctl, format!("{text}{RETAIN}")).unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("retain");
    let plan = repo.run(&[
        "seal",
        "--compact-only",
        "--dry-run",
        "--as-of",
        "2026-03-20",
    ]);
    assert_eq!(plan.code, 0, "{}{}", plan.stdout, plan.stderr);
    assert!(
        plan.stdout.contains("compacted: summary line"),
        "{}",
        plan.stdout
    );
    let run = repo.run(&["seal", "--compact-only", "--yes", "--as-of", "2026-03-20"]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert!(
        !doc_files(&repo).iter().any(|f| f.ends_with("-first.md")),
        "{:?}",
        doc_files(&repo)
    );
    let check = repo.run(&["check", "--as-of", "2026-03-20", "--format", "terse"]);
    assert!(!check.stdout.contains("HN-G003"), "{}", check.stdout);
    repo.git(&["add", "-A"]);
    repo.git_commit("compacted");
    let again = repo.run(&["seal", "--compact-only", "--yes", "--as-of", "2026-03-20"]);
    assert_eq!(again.code, 0);
    assert!(again.stdout.contains("nothing is due"), "{}", again.stdout);
}

fn scoped() -> TempRepo {
    let repo = RepoBuilder::new()
        .file("src/a.rs", "fn a() {}\n")
        .file("web/fairlead/control.toml", "[fairlead]\nscope = \"web\"\n")
        .file("web/src/b.rs", "fn b() {}\n")
        .git_init(true)
        .build();
    assert_eq!(
        repo.run(&["init", "--yes", "--as-of", "2026-01-05"]).code,
        0
    );
    repo.git(&["add", "-A"]);
    repo.git_commit("init");
    let new = repo.run(&[
        "new",
        "change",
        "Web work",
        "--scope",
        "web",
        "--field",
        "impact=fix",
        "--field",
        "covers=src/**",
        "--as-of",
        "2026-01-10",
    ]);
    assert_eq!(new.code, 0, "{}", new.stderr);
    let dir = repo.path().join("web/fairlead/docs/changes");
    let doc = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&doc)
        .unwrap()
        .replace("status = \"draft\"", "status = \"ready\"");
    std::fs::write(&doc, text).unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("ready");
    let seal = repo.run(&["seal", "--yes", "--anywhere", "--as-of", "2026-01-10"]);
    assert_eq!(seal.code, 0, "{}{}", seal.stdout, seal.stderr);
    repo.git(&["add", "-A"]);
    repo.git_commit("sealed");
    repo
}

#[test]
fn r_ret_070__demote_carries_the_ledger_lines_with_their_origin() {
    let repo = scoped();
    assert!(
        repo.path()
            .join("web/fairlead/ledger/hist/2026.hl")
            .exists()
    );
    let run = repo.run(&["move", "web", "--demote", "--yes", "--as-of", "2026-01-20"]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let l = ledger(&repo, "2026");
    assert!(l.contains("scope=web") && l.contains("ord=web:0001"), "{l}");
    let check = repo.run(&["check", "--as-of", "2026-01-20", "--format", "terse"]);
    assert!(!check.stdout.contains("HN-G003"), "{}", check.stdout);
}
