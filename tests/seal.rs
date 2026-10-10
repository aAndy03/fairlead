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
    assert!(ledger.contains("# ckpt v=0.1.0 n=2 d="), "{ledger}");
    assert!(
        ledger.contains("rollup.0.1.0 kind=rollup waivers=0 subtree-waivers=0 digest="),
        "R-VER-052 rollup record: {ledger}"
    );
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
fn r_git_003__commit_makes_one_commit_of_the_seal_with_the_trailer_and_tag() {
    let repo = ready_repo();
    repo.git(&["config", "user.name", "seal test"]);
    repo.git(&["config", "user.email", "seal@test.invalid"]);
    let before = repo.git(&["rev-list", "--count", "HEAD"]);
    let run = repo.run(&[
        "seal",
        "--yes",
        "--anywhere",
        "--commit",
        "--tag",
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert!(run.stdout.contains("committed and tagged fairlead/0.1.0"));
    let after = repo.git(&["rev-list", "--count", "HEAD"]);
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() + 1,
        "exactly one commit"
    );
    let msg = repo.git(&["log", "-1", "--format=%B"]);
    assert!(
        msg.contains("seal 0.1.0") && msg.contains("Fairlead-Version: 0.1.0"),
        "{msg}"
    );
    assert!(
        repo.git_status().trim().is_empty(),
        "nothing left uncommitted"
    );
    assert!(repo.git(&["tag", "--list"]).contains("fairlead/0.1.0"));
}

#[test]
fn r_git_003__no_git_write_without_commit() {
    let repo = ready_repo();
    let before = repo.git(&["rev-parse", "HEAD"]);
    assert_eq!(
        repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF])
            .code,
        0
    );
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), before);
    assert!(
        !repo.git_status().trim().is_empty(),
        "the seal is left for the user to commit"
    );
}

#[test]
fn r_ver_014__explain_reports_the_virtual_file_version() {
    let repo = ready_repo();
    let before = repo.run(&["explain", "src/a.rs", "--format", "terse"]);
    assert!(
        before.stdout.contains("file version=unavailable commit="),
        "{}",
        before.stdout
    );
    assert_eq!(
        repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF])
            .code,
        0
    );
    let after = repo.run(&["explain", "src/a.rs", "--format", "terse"]);
    assert!(
        after.stdout.contains("file version=0.1.0 doc=d-"),
        "{}",
        after.stdout
    );
    let src = std::fs::read_to_string(repo.path().join("src/a.rs")).unwrap();
    assert!(!src.contains("0.1.0"), "never stamped into the file (D-13)");
}

#[test]
fn r_ver_014__without_git_history_is_unavailable() {
    let repo = RepoBuilder::new().file("src/a.rs", "fn a() {}\n").build();
    assert_eq!(repo.run(&["init", "--yes", "--as-of", AS_OF]).code, 0);
    let run = repo.run(&["explain", "src/a.rs", "--format", "terse"]);
    assert!(
        run.stdout
            .contains("file version=unavailable history=unavailable"),
        "{}{}",
        run.stdout,
        run.stderr
    );
}

#[test]
fn r_ver_032__after_a_seal_changed_files_need_an_open_doc() {
    let repo = ready_repo();
    repo.git(&["config", "user.name", "t"]);
    repo.git(&["config", "user.email", "t@t.invalid"]);
    let sealed = repo.run(&["seal", "--yes", "--anywhere", "--commit", "--as-of", AS_OF]);
    assert_eq!(sealed.code, 0, "{}", sealed.stderr);
    std::fs::write(repo.path().join("src/b.rs"), "fn b() {}\n").unwrap();
    let check = repo.run(&["check", "--as-of", AS_OF, "--format", "terse"]);
    assert!(check.stdout.contains("HN-D006"), "{}", check.stdout);
    assert!(check.stdout.contains("src/b.rs"), "{}", check.stdout);
    let new = repo.run(&["new", "change", "Adds b", "--field", "covers=src/**"]);
    assert_eq!(new.code, 0, "{}", new.stderr);
    let check = repo.run(&["check", "--as-of", AS_OF, "--format", "terse"]);
    assert!(!check.stdout.contains("HN-D006"), "{}", check.stdout);
}

#[test]
fn r_ver_035__a_rolling_doc_gets_a_successor_draft_at_seal() {
    let repo = ready_repo();
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
        text.replace("status = \"ready\"", "status = \"ready\"\nrolling = true"),
    )
    .unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("rolling");
    let run = repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let names: Vec<String> = std::fs::read_dir(&changes)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let successor = names
        .iter()
        .find(|n| n.starts_with("d-"))
        .expect("a successor draft");
    let s = std::fs::read_to_string(changes.join(successor)).unwrap();
    assert!(
        s.contains("status = \"draft\"") && s.contains("rolling = true"),
        "{s}"
    );
    assert!(s.contains("covers = [\"src/**\"]"), "{s}");
    assert!(names.iter().any(|n| n.starts_with("0001-")), "{names:?}");
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

#[test]
fn r_lc_003__symbolic_target_recorded_at_seal() {
    let repo = ready_repo();
    let control = repo.path().join("fairlead/control.toml");
    let mut text = std::fs::read_to_string(&control).unwrap();
    text.push_str("\n[kinds.engine]\nimpl = 1\n");
    std::fs::write(&control, text).unwrap();
    std::fs::write(
        repo.path().join("fairlead/docs/engine.md"),
        "+++\ndirectives = [\n  'def engine.old',\n  'flag deprecate engine.old target=next-minor',\n]\n+++\n# Engines\n",
    )
    .unwrap();
    let sync = repo.run(&["sync", "--yes", "--as-of", AS_OF]);
    assert_eq!(sync.code, 0, "{}{}", sync.stdout, sync.stderr);
    repo.git(&["add", "-A"]);
    repo.git_commit("plan");
    let run = repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let ledger = std::fs::read_to_string(repo.path().join("fairlead/ledger/hist/2026.hl")).unwrap();
    assert!(
        ledger.contains("plan root:engine.old deprecate target=0.2.0"),
        "resolved against start 0.1.0 (nothing sealed before): {ledger}"
    );
}

fn ready_doc(repo: &TempRepo, title: &str, impact: &str, covers: &str) {
    let new = repo.run(&[
        "new",
        "change",
        title,
        "--field",
        &format!("impact={impact}"),
        "--field",
        &format!("covers={covers}"),
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(new.code, 0, "{}", new.stderr);
    let changes = repo.path().join("fairlead/docs/changes");
    for e in std::fs::read_dir(&changes).unwrap() {
        let p = e.unwrap().path();
        let text = std::fs::read_to_string(&p).unwrap();
        if text.contains("status = \"draft\"") {
            std::fs::write(&p, text.replace("status = \"draft\"", "status = \"ready\"")).unwrap();
        }
    }
}

fn with_engines(repo: &TempRepo, defs: &str) {
    let control = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&control).unwrap();
    if !text.contains("[kinds.engine]") {
        std::fs::write(&control, format!("{text}\n[kinds.engine]\nimpl = 1\n")).unwrap();
    }
    std::fs::write(
        repo.path().join("fairlead/docs/engine.md"),
        format!("+++\ndirectives = [\n{defs}]\n+++\n# Engines\n"),
    )
    .unwrap();
    let sync = repo.run(&["sync", "--yes", "--as-of", AS_OF]);
    assert_eq!(sync.code, 0, "{}{}", sync.stdout, sync.stderr);
}

#[test]
fn r_lc_020__seal_writes_tombstone_with_by_alias() {
    let repo = ready_repo();
    with_engines(
        &repo,
        "  'def engine.old',\n  'def engine.new',\n  'flag replace engine.old to=engine.new target=0.2.0',\n",
    );
    repo.git(&["add", "-A"]);
    repo.git_commit("plan");
    let first = repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(first.code, 0, "{}{}", first.stdout, first.stderr);
    repo.git(&["add", "-A"]);
    repo.git_commit("seal 0.1.0");
    with_engines(&repo, "  'def engine.new',\n");
    ready_doc(&repo, "Removes old", "break", "engine.old");
    repo.git(&["add", "-A"]);
    repo.git_commit("remove old");
    let tombs = repo.path().join("fairlead/tombstones/2026.toml");
    assert!(!tombs.exists(), "sync leaves a planned removal to the seal");
    let second = repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(second.code, 0, "{}{}", second.stdout, second.stderr);
    let t = std::fs::read_to_string(&tombs).unwrap();
    assert!(
        t.contains(
            "id = \"engine.old\"\nremoved = \"0.2.0\"\nby = \"engine.new\"\nwhy = \"Removes old\""
        ),
        "{t}"
    );
    let ledger = std::fs::read_to_string(repo.path().join("fairlead/ledger/hist/2026.hl")).unwrap();
    assert!(
        ledger.contains("tomb root:engine.old removed=0.2.0"),
        "{ledger}"
    );
}

#[test]
fn r_cli_046__sync_tombstones_user_deletion() {
    let repo = ready_repo();
    with_engines(&repo, "  'def engine.keep',\n  'def engine.gone',\n");
    repo.git(&["add", "-A"]);
    repo.git_commit("engines");
    std::fs::write(
        repo.path().join("fairlead/docs/engine.md"),
        "+++\ndirectives = [\n  'def engine.keep',\n]\n+++\n# Engines\n",
    )
    .unwrap();
    let sync = repo.run(&["sync", "--yes", "--as-of", AS_OF]);
    assert_eq!(sync.code, 0, "{}{}", sync.stdout, sync.stderr);
    assert!(
        sync.stdout.contains("wrote fairlead/tombstones/2026.toml"),
        "{}",
        sync.stdout
    );
    let t = std::fs::read_to_string(repo.path().join("fairlead/tombstones/2026.toml")).unwrap();
    assert!(
        t.contains("id = \"engine.gone\"") && t.contains("deleted-by = \"sync\""),
        "{t}"
    );
    let again = repo.run(&["sync", "--yes", "--as-of", AS_OF]);
    assert!(
        !again.stdout.contains("tombstones"),
        "once only: {}",
        again.stdout
    );
}

#[test]
fn r_ret_041__tombstones_not_in_default_context() {
    let repo = ready_repo();
    with_engines(&repo, "  'def engine.keep',\n  'def engine.gone',\n");
    std::fs::write(
        repo.path().join("fairlead/docs/engine.md"),
        "+++\ndirectives = [\n  'def engine.keep',\n]\n+++\n# Engines\n",
    )
    .unwrap();
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    let ctx = repo.run(&["context", "--as-of", AS_OF]);
    assert_eq!(ctx.code, 0, "{}", ctx.stderr);
    assert!(!ctx.stdout.contains("engine.gone"), "{}", ctx.stdout);
}

#[test]
fn r_lc_031__seal_fingerprints_an_entry_once_from_its_material() {
    let repo = ready_repo();
    std::fs::create_dir_all(repo.path().join("fairlead/docs/graveyard")).unwrap();
    std::fs::create_dir_all(repo.path().join("ui")).unwrap();
    std::fs::write(
        repo.path().join("ui/ThemeToggle.tsx"),
        "export function ThemeToggle() {\n  return useTheme();\n}\n",
    )
    .unwrap();
    std::fs::write(
        repo.path().join("fairlead/docs/graveyard/dark-toggle.md"),
        "+++\nid = \"grv.dark-toggle-v1\"\nstate = \"rejected\"\nat = \"0.1.0\"\nwhat = \"Per-component dark-mode toggles\"\nwhy = \"Token drift\"\nmatch = [\"dark mode\", \"ui/**/*Toggle*\"]\n+++\n\n# Dark toggle\n",
    )
    .unwrap();
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    repo.git(&["add", "-A"]);
    repo.git_commit("graveyard");
    let run = repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let e = std::fs::read_to_string(repo.path().join("fairlead/docs/graveyard/dark-toggle.md"))
        .unwrap();
    assert!(
        e.contains("fingerprint = { ids = [\"themetoggle\"], minhash = \"b3:"),
        "{e}"
    );
    assert!(e.ends_with("+++\n\n# Dark toggle\n"), "body kept: {e}");
}

#[test]
fn r_lc_034__context_prints_the_graveyard_and_abandoned_candidates() {
    let repo = ready_repo();
    std::fs::create_dir_all(repo.path().join("fairlead/docs/graveyard")).unwrap();
    std::fs::write(
        repo.path().join("fairlead/docs/graveyard/dark-toggle.md"),
        "+++\nid = \"grv.dark-toggle-v1\"\nstate = \"rejected\"\nwhat = \"toggles\"\nwhy = \"drift\"\nmatch = [\"dark mode\"]\n+++\n",
    )
    .unwrap();
    std::fs::write(
        repo.path().join("fairlead/docs/changes/d-0000000z-old.md"),
        "+++\nid = \"d-0000000z\"\nkind = \"change\"\nstatus = \"abandoned\"\ntitle = \"Old idea\"\ncovers = []\nrevisits = \"grv.dark-toggle-v1\"\n+++\n",
    )
    .unwrap();
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    let ctx = repo.run(&["context", "src/a.rs", "--as-of", AS_OF]);
    assert_eq!(ctx.code, 0, "{}", ctx.stderr);
    assert!(
        ctx.stdout
            .contains("graveyard: 1 rejected under here (tripwire grv.dark-toggle-v1=1)"),
        "{}",
        ctx.stdout
    );
    assert!(
        ctx.stdout
            .contains("graveyard candidate? d-0000000z (abandoned): Old idea"),
        "{}",
        ctx.stdout
    );
}

#[test]
fn r_lc_032__new_change_matching_the_graveyard_needs_revisits() {
    let repo = ready_repo();
    std::fs::create_dir_all(repo.path().join("fairlead/docs/graveyard")).unwrap();
    std::fs::write(
        repo.path().join("fairlead/docs/graveyard/dark-toggle.md"),
        "+++\nid = \"grv.dark-toggle-v1\"\nstate = \"rejected\"\nat = \"1.4.0\"\nwhat = \"Per-component toggles\"\nwhy = \"Token drift\"\nmatch = [\"dark mode\"]\n+++\n",
    )
    .unwrap();
    let refused = repo.run(&["new", "change", "Add a dark mode switch", "--as-of", AS_OF]);
    assert_eq!(refused.code, 4, "{}{}", refused.stdout, refused.stderr);
    assert!(
        refused
            .stderr
            .contains("grv.dark-toggle-v1 (dark mode; rejected at 1.4.0)"),
        "{}",
        refused.stderr
    );
    let ok = repo.run(&[
        "new",
        "change",
        "Add a dark mode switch",
        "--field",
        "revisits=grv.dark-toggle-v1",
        "--field",
        "revisit-why=tokens are generated now",
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(ok.code, 0, "{}{}", ok.stdout, ok.stderr);
}
