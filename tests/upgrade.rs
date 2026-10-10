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

/// An edition-1 repo that already sets the field edition 2 requires.
fn ready_for_2() -> TempRepo {
    let repo = repo_on(&[]);
    let control = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&control).unwrap();
    std::fs::write(
        &control,
        text.replace("[settings]\n", "[settings]\npurpose = \"test codebase\"\n"),
    )
    .unwrap();
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    repo.git(&["add", "-A"]);
    repo.git_commit("purpose");
    repo
}

fn upgrade(repo: &TempRepo, extra: &[&str]) -> testkit::Run {
    let mut args = vec!["upgrade", "--as-of", AS_OF];
    args.extend(extra);
    repo.run_in(repo.path(), &args, &ED2)
}

fn plan_hash(stdout: &str) -> String {
    stdout
        .split_whitespace()
        .find(|w| w.starts_with("b3:"))
        .unwrap()
        .to_owned()
}

fn draft_doc(repo: &TempRepo) -> std::path::PathBuf {
    std::fs::read_dir(repo.path().join("fairlead/docs/changes"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().contains("upgrade-edition-1-to-2"))
        .unwrap()
}

fn approve_by_hand(repo: &TempRepo, doc: &std::path::Path, hash: &str) {
    let text = std::fs::read_to_string(doc).unwrap();
    std::fs::write(
        doc,
        text.replace("status = \"draft\"", "status = \"ready\""),
    )
    .unwrap();
    let id = text
        .lines()
        .find_map(|l| l.strip_prefix("id = \""))
        .unwrap()
        .trim_end_matches('"')
        .to_owned();
    let fp = fairlead_core::upgrade::upgrade_fp(&id, hash);
    std::fs::write(
        repo.path().join("fairlead/approvals.toml"),
        format!(
            "[[approval]]\nfp = \"{}\"\ntarget = \"edition.upgrade\"\nscope = \"root\"\napprover = \"@owner\"\ndate = \"2026-10-10\"\n",
            fp.as_str()
        ),
    )
    .unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("approve the upgrade");
}

#[test]
fn r_edn_037__same_tree_same_plan_hash() {
    let repo = ready_for_2();
    let a = upgrade(&repo, &[]);
    assert_eq!(a.code, 4, "plan only: {}{}", a.stdout, a.stderr);
    assert!(a.stdout.contains("upgrade: edition 1 -> 2"), "{}", a.stdout);
    assert!(a.stdout.contains("not applied"));
    let b = upgrade(&repo, &[]);
    assert_eq!(plan_hash(&a.stdout), plan_hash(&b.stdout));
}

#[test]
fn r_edn_034__needs_human_blocks_apply() {
    let repo = repo_on(&[]);
    let plan = upgrade(&repo, &[]);
    assert!(
        plan.stdout.contains("needs-human: settings.purpose"),
        "{}",
        plan.stdout
    );
    let before = repo.tree_snapshot();
    let yes = upgrade(&repo, &["--yes"]);
    assert_eq!(yes.code, 4);
    assert!(
        yes.stderr.contains("needs-human: settings.purpose"),
        "{}",
        yes.stderr
    );
    assert_eq!(repo.tree_snapshot(), before);
}

#[test]
fn r_edn_031__draft_then_approval_missing_then_apply_then_no_op() {
    let repo = ready_for_2();
    let first = upgrade(&repo, &["--yes"]);
    assert_eq!(first.code, 4, "{}{}", first.stdout, first.stderr);
    assert!(first.stdout.contains("awaits approval"), "{}", first.stdout);
    let doc = draft_doc(&repo);
    let text = std::fs::read_to_string(&doc).unwrap();
    assert!(text.contains("kind = \"upgrade\"") && text.contains("impact = \"break\""));
    let control = std::fs::read_to_string(repo.path().join("fairlead/control.toml")).unwrap();
    assert!(
        control.contains("edition = 1"),
        "the draft is the only file written"
    );
    repo.git(&["add", "-A"]);
    repo.git_commit("draft");
    let missing = upgrade(&repo, &["--yes"]);
    assert_eq!(missing.code, 4);
    assert!(
        missing.stderr.contains("approval missing"),
        "{}",
        missing.stderr
    );
    approve_by_hand(&repo, &doc, &plan_hash(&first.stdout));
    let apply = upgrade(&repo, &["--yes"]);
    assert_eq!(apply.code, 0, "{}{}", apply.stdout, apply.stderr);
    assert!(
        apply.stdout.contains("upgraded to edition 2"),
        "{}",
        apply.stdout
    );
    let control = std::fs::read_to_string(repo.path().join("fairlead/control.toml")).unwrap();
    assert!(control.contains("edition = 2"), "{control}");
    let members = std::fs::read_to_string(repo.path().join("fairlead/gen/members")).unwrap();
    assert!(members.starts_with("# scope root ed=2 "), "{members}");
    let check = repo.run_in(repo.path(), &["check", "--as-of", AS_OF], &ED2);
    assert_eq!(check.code, 0, "{}{}", check.stdout, check.stderr);
    let again = upgrade(&repo, &["--yes"]);
    assert_eq!(again.code, 0);
    assert!(
        again.stdout.contains("nothing to upgrade"),
        "{}",
        again.stdout
    );
}

#[test]
fn r_edn_031__a_changed_tree_makes_the_plan_stale() {
    let repo = ready_for_2();
    assert_eq!(upgrade(&repo, &["--yes"]).code, 4);
    repo.git(&["add", "-A"]);
    repo.git_commit("draft");
    std::fs::write(
        repo.path().join("fairlead/docs/new.md"),
        "+++\ndirectives = ['def rule.extra check=pattern']\n+++\n",
    )
    .unwrap();
    let control = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&control).unwrap();
    std::fs::write(&control, format!("{text}\n[kinds.cmp]\nimpl = 1\n")).unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("more");
    let stale = upgrade(&repo, &["--yes"]);
    assert_eq!(stale.code, 4);
    assert!(
        stale.stderr.contains("plan stale, re-plan"),
        "{}",
        stale.stderr
    );
}

#[test]
fn r_edn_031__write_set_must_equal_head() {
    let repo = ready_for_2();
    let first = upgrade(&repo, &["--yes"]);
    let doc = draft_doc(&repo);
    approve_by_hand(&repo, &doc, &plan_hash(&first.stdout));
    // A generated file differs from HEAD: the plan (and its hash) stays the same, the gate refuses.
    let members = repo.path().join("fairlead/gen/members");
    let text = std::fs::read_to_string(&members).unwrap();
    std::fs::write(&members, format!("{text}extra\n")).unwrap();
    let dirty = upgrade(&repo, &["--yes"]);
    assert_eq!(dirty.code, 4, "{}{}", dirty.stdout, dirty.stderr);
    assert!(
        dirty.stderr.contains("differs from HEAD"),
        "{}",
        dirty.stderr
    );
}

#[test]
fn r_arch_124__fetch_has_no_client() {
    let repo = repo_on(&[]);
    let run = repo.run_in(repo.path(), &["upgrade", "--fetch"], &ED2);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("no remote migration data is published"),
        "{}",
        run.stderr
    );
}

#[test]
fn r_arch_124__net_module_only_reachable_from_upgrade() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut offenders = Vec::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                let named = p.to_string_lossy().replace('\\', "/");
                let allowed = named.ends_with("fairlead-core/src/lib.rs")
                    || named.ends_with("fairlead-core/src/net.rs")
                    || named.ends_with("fairlead/src/commands/upgrade.rs");
                if !allowed && (text.contains("crate::net") || text.contains("core::net::")) {
                    offenders.push(named);
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:?}");
    let manifest = std::fs::read_to_string(root.join("crates/fairlead-core/Cargo.toml")).unwrap();
    assert!(
        manifest.contains("fetch = []"),
        "the feature exists and is empty"
    );
    assert!(
        !manifest.contains("default = [\"vendored-libgit2\", \"fetch\"]"),
        "off by default"
    );
}

#[test]
fn r_edn_007__rename_rewrites_every_directive_and_folder() {
    let repo = repo_on(&[]);
    let control = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&control).unwrap();
    std::fs::write(&control, format!("{text}\n[kinds.cmp]\nimpl = 1\n")).unwrap();
    std::fs::create_dir_all(repo.path().join("fairlead/docs")).unwrap();
    std::fs::write(
        repo.path().join("fairlead/docs/cmp.md"),
        "+++\ndirectives = ['def cmp.button']\n+++\n# Components\n",
    )
    .unwrap();
    std::fs::write(
        repo.path().join("src/a.rs"),
        "// fairlead: use cmp.button\nfn a() {\n    let s = \"// fairlead: not a directive\";\n}\n",
    )
    .unwrap();
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    repo.git(&["add", "-A"]);
    repo.git_commit("components");
    let args = ["--rename-marker", "gov", "--rename-folder", "gov"];
    let mut first_args = args.to_vec();
    first_args.push("--yes");
    let first = repo.run(&[&["upgrade", "--as-of", AS_OF][..], &first_args].concat());
    assert_eq!(first.code, 4, "{}{}", first.stdout, first.stderr);
    let doc = draft_doc_named(&repo, "fairlead", "upgrade-edition-1-to-1");
    approve_by_hand(&repo, &doc, &plan_hash(&first.stdout));
    let apply = repo.run(&[&["upgrade", "--as-of", AS_OF][..], &first_args].concat());
    assert_eq!(apply.code, 0, "{}{}", apply.stdout, apply.stderr);
    assert!(!repo.path().join("fairlead/control.toml").exists());
    let ctl = std::fs::read_to_string(repo.path().join("gov/control.toml")).unwrap();
    assert!(
        ctl.contains("marker = \"gov\"") && ctl.contains("folder = \"gov\""),
        "{ctl}"
    );
    let src = std::fs::read_to_string(repo.path().join("src/a.rs")).unwrap();
    assert!(src.starts_with("// gov: use cmp.button\n"), "{src}");
    assert!(
        src.contains("\"// fairlead: not a directive\""),
        "strings are not directives"
    );
    // A folder other than the default one is found with --root (or FAIRLEAD_ROOT), as in 0.1.
    let root = repo.path().to_string_lossy().into_owned();
    let check = repo.run(&["--root", &root, "check", "--as-of", AS_OF]);
    assert_eq!(check.code, 0, "{}{}", check.stdout, check.stderr);
}

fn draft_doc_named(repo: &TempRepo, folder: &str, part: &str) -> std::path::PathBuf {
    std::fs::read_dir(repo.path().join(folder).join("docs/changes"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().contains(part))
        .unwrap()
}
