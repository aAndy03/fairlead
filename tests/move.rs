#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! `fairlead move` end to end (R-CLI-040, R-CLI-044, R-PORT-004 to R-PORT-006).
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, TempRepo};

const AS_OF: &str = "2026-10-10";

fn repo(files: &[(&str, &str)]) -> TempRepo {
    let mut b = RepoBuilder::new().git_init(true);
    for (p, t) in files {
        b = b.file(p, t);
    }
    let repo = b.build();
    let init = repo.run(&["init", "--yes", "--as-of", AS_OF]);
    assert_eq!(init.code, 0, "{}", init.stderr);
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    repo.git(&["add", "-A"]);
    repo.git_commit("start");
    repo
}

fn read(repo: &TempRepo, p: &str) -> String {
    std::fs::read_to_string(repo.path().join(p)).unwrap()
}

#[test]
fn r_cli_044__move_rewrites_own_text_lists_code_sites() {
    let repo = repo(&[
        ("src/ui/Button.tsx", "export const Button = 1;\n"),
        (
            "src/app.tsx",
            "import { Button } from './ui/Button';\nexport const App = Button;\n",
        ),
        (
            "fairlead/docs/changes/d-0000000a-ui.md",
            "+++\nid = \"d-0000000a\"\nkind = \"change\"\nstatus = \"draft\"\nscope = \"root\"\ntitle = \"UI\"\ncovers = [\"src/ui/**\"]\ncreated = \"0.1.0\"\n+++\n# UI\n",
        ),
    ]);
    let plan = repo.run(&["move", "src/ui", "web/ui", "--as-of", AS_OF]);
    assert_eq!(plan.code, 4, "plan only: {}{}", plan.stdout, plan.stderr);
    assert!(
        plan.stdout.contains("code-site src/app.tsx:1"),
        "{}",
        plan.stdout
    );
    let run = repo.run(&["move", "src/ui", "web/ui", "--yes", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert!(repo.path().join("web/ui/Button.tsx").exists());
    assert!(!repo.path().join("src/ui/Button.tsx").exists());
    assert!(
        read(&repo, "fairlead/docs/changes/d-0000000a-ui.md").contains("covers = [\"web/ui/**\"]"),
        "fairlead's own text follows the move"
    );
    assert_eq!(
        read(&repo, "src/app.tsx"),
        "import { Button } from './ui/Button';\nexport const App = Button;\n",
        "code is listed, never edited"
    );
    let check = repo.run(&["check", "--as-of", AS_OF]);
    assert_ne!(
        check.code, 3,
        "generated files were regenerated: {}",
        check.stdout
    );
}

#[test]
fn r_chk_044__move_writes_restructure_doc() {
    let repo = repo(&[("src/a.rs", "fn a() {}\n")]);
    let run = repo.run(&["move", "src/a.rs", "lib/a.rs", "--yes", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let doc = std::fs::read_dir(repo.path().join("fairlead/docs/changes"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().contains("restructure"))
        .expect("a restructure doc");
    let text = std::fs::read_to_string(doc).unwrap();
    assert!(
        text.contains("covers = [\"src/a.rs\", \"lib/a.rs\"]")
            && text.contains("status = \"draft\""),
        "{text}"
    );
}

/// Root, scope `a` (declares `rule.r` and grants `sub` an exception), and `a/sub`.
fn nested() -> TempRepo {
    repo(&[
        (
            "a/fairlead/control.toml",
            "[fairlead]\nscope = \"a\"\n\n[rules.r]\ncheck = \"pattern\"\nforbid = \"FORBIDDEN\"\nmsg = \"no\"\n\n[[allow]]\ntarget = \"rule.r\"\nfor = [\"sub\"]\nwhy = \"legacy code\"\nexp = \"2027-01-01\"\n",
        ),
        (
            "a/sub/fairlead/control.toml",
            "[fairlead]\nscope = \"sub\"\n",
        ),
        (
            "a/sub/x.ts",
            "// fairlead: ovr rule.r why=\"legacy\" owner=@platform exp=2026-12-31\nexport const X = 1;\n",
        ),
        ("b/readme.md", "# b\n"),
    ])
}

#[test]
fn r_port_004__loosening_reparent_refused() {
    let repo = nested();
    let plan = repo.run(&["move", "a/sub", "b/sub", "--dry-run", "--as-of", AS_OF]);
    assert_eq!(plan.code, 0, "{}{}", plan.stdout, plan.stderr);
    assert!(
        plan.stdout.contains("- rule.r (scope sub)"),
        "{}",
        plan.stdout
    );
    let before = repo.tree_snapshot();
    let run = repo.run(&["move", "a/sub", "b/sub", "--yes", "--as-of", AS_OF]);
    assert_eq!(run.code, 4, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains("fairlead approve move-"),
        "{}",
        run.stderr
    );
    assert_eq!(repo.tree_snapshot(), before);
}

#[test]
fn r_port_006__lapsed_grant_listed_inert() {
    let repo = nested();
    let plan = repo.run(&["move", "a/sub", "b/sub", "--dry-run", "--as-of", AS_OF]);
    assert!(
        plan.stdout.contains("~ inert ovr rule.r at a/sub/x.ts:1"),
        "{}",
        plan.stdout
    );
}

#[test]
fn r_port_005__interrupted_move_reported() {
    let repo = repo(&[("src/a.rs", "fn a() {}\n")]);
    std::fs::create_dir_all(repo.path().join(".fairlead")).unwrap();
    std::fs::write(
        repo.path().join(".fairlead/sync.journal"),
        "{\"tx\":\"x\",\"entries\":[],\"kind\":\"Move\"}",
    )
    .unwrap();
    let check = repo.run(&["check", "--as-of", AS_OF, "--format", "terse"]);
    assert!(check.stdout.contains("HN-M008"), "{}", check.stdout);
    assert_eq!(
        repo.run(&["sync", "--yes", "--as-of", AS_OF]).code,
        0,
        "a writer resolves it"
    );
    let after = repo.run(&["check", "--as-of", AS_OF, "--format", "terse"]);
    assert!(!after.stdout.contains("HN-M008"), "{}", after.stdout);
}

#[test]
fn r_git_062__move_refuses_a_dirty_write_set() {
    let repo = repo(&[("src/a.rs", "fn a() {}\n")]);
    std::fs::write(repo.path().join("src/a.rs"), "fn a() { /* local */ }\n").unwrap();
    let run = repo.run(&["move", "src/a.rs", "lib/a.rs", "--yes", "--as-of", AS_OF]);
    assert_eq!(run.code, 4, "{}{}", run.stdout, run.stderr);
    assert!(run.stderr.contains("differs from HEAD"), "{}", run.stderr);
}

fn two_scopes() -> TempRepo {
    let r = repo(&[
        ("lib/fairlead/control.toml", "[fairlead]\nscope = \"lib\"\n"),
        (
            "lib/fairlead/docs/cmp.md",
            "+++\ndirectives = ['def cmp.x sum=\"x\"']\n+++\n# Components\n",
        ),
        ("lib/src/x.ts", "export const X = 1;\n"),
        ("app/fairlead/control.toml", "[fairlead]\nscope = \"app\"\n"),
        (
            "app/main.ts",
            "// fairlead: use lib:cmp.x\nexport const M = 1;\n",
        ),
    ]);
    let control = r.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&control).unwrap();
    std::fs::write(&control, format!("{text}\n[kinds.cmp]\nimpl = 1\n")).unwrap();
    assert_eq!(r.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    r.git(&["add", "-A"]);
    r.git_commit("kinds");
    r
}

#[test]
fn r_port_010__delete_requires_zero_live_references() {
    let repo = two_scopes();
    let refused = repo.run(&["move", "lib", "--delete", "--yes", "--as-of", AS_OF]);
    assert_eq!(refused.code, 4, "{}{}", refused.stdout, refused.stderr);
    assert!(
        refused
            .stderr
            .contains("live reference app/main.ts:1 -> lib:cmp.x"),
        "{}",
        refused.stderr
    );
    let forced = repo.run(&[
        "move", "lib", "--delete", "--force", "--yes", "--as-of", AS_OF,
    ]);
    assert_eq!(
        forced.code, 4,
        "--force still needs an approval: {}",
        forced.stderr
    );
    let fp = forced
        .stderr
        .split("approve move-")
        .nth(1)
        .unwrap()
        .chars()
        .take(12)
        .collect::<String>();
    std::fs::write(
        repo.path().join("fairlead/approvals.toml"),
        format!("[[approval]]\nfp = \"{fp}\"\ntarget = \"move.reparent\"\nscope = \"root\"\napprover = \"@owner\"\ndate = \"2026-10-10\"\n"),
    )
    .unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("approve the delete");
    let done = repo.run(&[
        "move", "lib", "--delete", "--force", "--yes", "--as-of", AS_OF,
    ]);
    assert_eq!(done.code, 0, "{}{}", done.stdout, done.stderr);
    assert!(!repo.path().join("lib/fairlead/control.toml").exists());
    assert!(
        repo.path().join("lib/src/x.ts").exists(),
        "code is never deleted"
    );
    let tombs = read(&repo, "fairlead/tombstones/2026.toml");
    assert!(
        tombs.contains("id = \"cmp.x\"") && tombs.contains("deleted-by = \"delete\""),
        "{tombs}"
    );
    let check = repo.run(&["check", "--as-of", AS_OF, "--format", "terse"]);
    assert!(
        check.stdout.contains("HN-M007"),
        "dangling: {}",
        check.stdout
    );
}

#[test]
fn r_lc_023__demote_leaves_a_scope_tombstone_and_applies() {
    let repo = two_scopes();
    let run = repo.run(&["move", "app", "--demote", "--yes", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert!(!repo.path().join("app/fairlead/control.toml").exists());
    let tombs = read(&repo, "fairlead/tombstones/2026.toml");
    assert!(
        tombs.contains("id = \"app\"") && tombs.contains("by = \"demote\""),
        "{tombs}"
    );
    let lib = repo.run(&["move", "lib", "--demote", "--dry-run", "--as-of", AS_OF]);
    assert!(
        lib.stdout
            .contains("fairlead/docs/lib/cmp.md (applies=lib/**)"),
        "{}",
        lib.stdout
    );
}

#[test]
fn r_port_013__detach_refused_unless_portable_or_empty() {
    let repo = two_scopes();
    std::fs::create_dir_all(repo.path().join("fairlead/docs")).unwrap();
    std::fs::write(
        repo.path().join("fairlead/docs/cmp.md"),
        "+++\ndirectives = ['def cmp.shared sum=\"s\"']\n+++\n# Shared\n",
    )
    .unwrap();
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    repo.git(&["add", "-A"]);
    repo.git_commit("shared");
    let run = repo.run(&["move", "app", "--detach", "--yes", "--as-of", AS_OF]);
    assert_eq!(run.code, 4, "{}{}", run.stdout, run.stderr);
    assert!(run.stderr.contains("is not portable"), "{}", run.stderr);
}

#[test]
fn r_port_013__detached_scope_is_marked_and_reported() {
    let repo = two_scopes();
    let run = repo.run(&["move", "app", "--detach", "--yes", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert!(read(&repo, "app/fairlead/control.toml").contains("[scope]\ndetached = true"));
    let check = repo.run(&["check", "--as-of", AS_OF, "--format", "terse"]);
    assert!(check.stdout.contains("HN-M011"), "{}", check.stdout);
}
