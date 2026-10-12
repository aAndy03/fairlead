#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! 0.3 portability (32-portability-and-restructure, plans/0.3 P4) end to end through the binary.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, TempRepo};

const AS_OF: &str = "2026-10-12";
const INNER: &str = "[fairlead]\nscope = \"billing-core\"\nroot = true\nedition = 2\n";

fn repo(mount: &str) -> TempRepo {
    let repo = RepoBuilder::new()
        .file("src/a.ts", "export const a = 1;\n")
        .file("web/fairlead/control.toml", "[fairlead]\nscope = \"web\"\n")
        .file("vendor/billing/fairlead/control.toml", INNER)
        .file("vendor/billing/fairlead/gen/members", "scope.web dir=web\n")
        .file(
            "vendor/billing/src/pay.ts",
            "// fairlead: use nowhere.thing\nexport const p = 1;\n",
        )
        .git_init(true)
        .build();
    let init = repo.run(&["init", "--yes", "--as-of", AS_OF]);
    assert_eq!(init.code, 0, "{}", init.stderr);
    let ctl = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&ctl).unwrap();
    std::fs::write(&ctl, format!("{text}\n{mount}")).unwrap();
    repo
}

fn check(repo: &TempRepo) -> testkit::Run {
    repo.run(&["check", "--as-of", AS_OF, "--format", "terse"])
}

#[test]
fn r_port_030__unregistered_inner_root_is_root_dup() {
    let repo = repo("");
    let run = check(&repo);
    assert!(run.stdout.contains("HN-C004"), "{}", run.stdout);
    assert!(
        run.stdout.contains("[[mount]] scope = \"billing-core\""),
        "{}",
        run.stdout
    );
}

#[test]
fn r_port_031__outer_reads_rollup_only() {
    let repo =
        repo("[[mount]]\nscope = \"billing-core\"\npath = \"vendor/billing\"\nas = \"billing\"\n");
    // R-PORT-032: the outer sync writes nothing inside the sealed sub-root
    let before: Vec<_> = walk(&repo.path().join("vendor/billing"));
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    assert_eq!(walk(&repo.path().join("vendor/billing")), before);
    let run = check(&repo);
    assert!(
        !run.stdout.contains("HN-C004"),
        "registered: {}",
        run.stdout
    );
    // `init` baselined the sub-root before it was registered; those entries now read as fixed
    let findings: Vec<&str> = run
        .stdout
        .lines()
        .filter(|l| l.starts_with("HN-") && !l.starts_with("HN-B001"))
        .collect();
    assert!(
        findings.iter().all(|l| !l.contains("vendor/billing/")),
        "the sub-root's files are not the outer root's: {}",
        run.stdout
    );
}

#[test]
fn r_port_035__as_prefix_namespaces_ids() {
    let bare = repo("[[mount]]\nscope = \"billing-core\"\npath = \"vendor/billing\"\n");
    let run = check(&bare);
    assert!(
        run.stdout.contains("HN-C007") && run.stdout.contains("scope id web of mount billing-core"),
        "{}",
        run.stdout
    );
    let prefixed =
        repo("[[mount]]\nscope = \"billing-core\"\npath = \"vendor/billing\"\nas = \"billing\"\n");
    assert!(!check(&prefixed).stdout.contains("HN-C007"));
}

#[test]
fn r_edn_045__edition_mismatch_reported_once_at_boundary() {
    let repo =
        repo("[[mount]]\nscope = \"billing-core\"\npath = \"vendor/billing\"\nas = \"billing\"\n");
    let run = check(&repo);
    assert_eq!(run.stdout.matches("HN-V014").count(), 1, "{}", run.stdout);
    assert!(
        run.stdout.contains("on edition 2, the outer root on 1"),
        "{}",
        run.stdout
    );
}

#[test]
fn r_port_036__the_outer_root_declares_nothing_inside_a_sub_root() {
    let repo = repo(
        "[[mount]]\nscope = \"billing-core\"\npath = \"vendor/billing\"\nas = \"billing\"\n\n[[file]]\nid = \"file.theme\"\nkind = \"stylesheet\"\npath = \"vendor/billing/theme.css\"\n",
    );
    let run = check(&repo);
    assert!(run.stdout.contains("HN-M010"), "{}", run.stdout);
}

fn walk(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(format!(
                    "{} {}",
                    p.display(),
                    std::fs::read(&p).unwrap().len()
                ));
            }
        }
    }
    out.sort();
    out
}

const MOUNT: &str =
    "[[mount]]\nscope = \"billing-core\"\npath = \"vendor/billing\"\nas = \"billing\"\n";

fn adopt(repo: &TempRepo, yes: bool) -> testkit::Run {
    let mut args = vec!["move", "--adopt", "billing-core", "--as-of", AS_OF];
    if yes {
        args.push("--yes");
    }
    repo.run(&args)
}

#[test]
fn r_port_033__adopt_needs_approval() {
    let repo = repo(MOUNT);
    let plan = adopt(&repo, false);
    assert_eq!(plan.code, 4, "plan only: {}{}", plan.stdout, plan.stderr);
    assert!(
        plan.stdout.contains("drop `root = true`"),
        "{}",
        plan.stdout
    );
    let refused = adopt(&repo, true);
    assert_eq!(refused.code, 4, "{}{}", refused.stdout, refused.stderr);
    assert!(
        refused.stderr.contains("fairlead approve move-"),
        "{}",
        refused.stderr
    );
    let inner =
        std::fs::read_to_string(repo.path().join("vendor/billing/fairlead/control.toml")).unwrap();
    assert!(
        inner.contains("root = true"),
        "nothing written without approval"
    );
}

#[test]
fn r_port_033__adopt_removes_inner_root_flag() {
    let repo = repo(MOUNT);
    let refused = adopt(&repo, true);
    let fp = refused
        .stderr
        .split("approve move-")
        .nth(1)
        .and_then(|r| r.split(|c: char| !c.is_ascii_alphanumeric()).next())
        .unwrap_or_else(|| panic!("{}", refused.stderr))
        .to_owned();
    std::fs::write(
        repo.path().join("fairlead/approvals.toml"),
        format!("[[approval]]\nfp = \"{fp}\"\ntarget = \"move.reparent\"\nscope = \"root\"\napprover = \"@owner\"\ndate = \"{AS_OF}\"\n"),
    )
    .unwrap();
    repo.git(&["add", "-A"]);
    repo.git_commit("mount and approval");
    let applied = adopt(&repo, true);
    assert_eq!(applied.code, 0, "{}{}", applied.stdout, applied.stderr);
    let inner =
        std::fs::read_to_string(repo.path().join("vendor/billing/fairlead/control.toml")).unwrap();
    assert!(!inner.contains("root = true"), "{inner}");
    let outer = std::fs::read_to_string(repo.path().join("fairlead/control.toml")).unwrap();
    assert!(!outer.contains("[[mount]]"), "{outer}");
}

fn scoped() -> TempRepo {
    let repo = RepoBuilder::new()
        .file("src/a.ts", "export const a = 1;\n")
        .file("fairlead/contract/c.md", "+++\ndirectives = ['def cmp.button sum=\"Button\"']\n+++\n# Contract\n")
        .file("web/fairlead/control.toml", "[fairlead]\nscope = \"web\"\n")
        .file("web/src/b.ts", "// fairlead: use root:cmp.button\nimport { f } from '../../shared/format';\nexport const b = f;\n")
        .file("shared/format.ts", "export const f = 1;\n")
        .git_init(true)
        .build();
    let init = repo.run(&["init", "--yes", "--as-of", AS_OF]);
    assert_eq!(init.code, 0, "{}", init.stderr);
    let ctl = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&ctl).unwrap();
    std::fs::write(
        &ctl,
        format!("{text}\n[kinds.cmp]\nshape = \"doc\"\n\n[variation]\n"),
    )
    .unwrap();
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    repo.git(&["add", "-A"]);
    repo.git_commit("start");
    repo
}

#[test]
fn r_port_040__cut_keeps_the_id_and_logs_moved_from() {
    let repo = scoped();
    let run = repo.run(&[
        "move", "--cut", "web", "apps/web", "--yes", "--as-of", AS_OF,
    ]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let ctl = std::fs::read_to_string(repo.path().join("apps/web/fairlead/control.toml")).unwrap();
    assert!(
        ctl.contains("scope = \"web\"") && ctl.contains("moved-from = \"web\""),
        "{ctl}"
    );
    assert!(!repo.path().join("web/fairlead/control.toml").exists());
}

#[test]
fn r_port_040__copy_mints_a_new_id_with_derived_from() {
    let repo = scoped();
    let run = repo.run(&["move", "--copy", "web", "admin", "--yes", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let ctl = std::fs::read_to_string(repo.path().join("admin/fairlead/control.toml")).unwrap();
    assert!(
        ctl.contains("scope = \"admin\"") && ctl.contains("derived-from = \"web\""),
        "{ctl}"
    );
    assert!(
        repo.path().join("web/fairlead/control.toml").exists(),
        "the source stays"
    );
    assert!(
        !check(&repo).stdout.contains("HN-C007"),
        "no duplicate scope id"
    );
}

#[test]
fn r_port_041__attach_classifies_requires_and_lists_outward_imports() {
    let repo = scoped();
    let ctl = repo.path().join("web/fairlead/control.toml");
    std::fs::write(
        &ctl,
        "[fairlead]\nscope = \"web\"\n\n[scope]\ndetached = true\n",
    )
    .unwrap();
    std::fs::create_dir_all(repo.path().join("web/fairlead/gen")).unwrap();
    std::fs::write(
        repo.path().join("web/fairlead/gen/scope.lock"),
        "# scope web edition=1\nancestor-digest=x\nreconciled=0.1.0\nreq=cmp.button from=root dg=aaaaaaaaaaaa ver=0.1.0\nreq=tok.gone from=root dg=bbbbbbbbbbbb ver=0.1.0\n",
    )
    .unwrap();
    let run = repo.run(&["move", "--attach", "web", "--as-of", AS_OF]);
    assert_eq!(run.code, 4, "plan only: {}{}", run.stdout, run.stderr);
    for want in [
        "~ differing root:cmp.button",
        "! missing root:tok.gone: UNSATISFIED",
        "? requires candidate: web/src/b.ts:2 imports ../../shared/format",
        "drop [scope] detached",
    ] {
        assert!(run.stdout.contains(want), "{want}: {}", run.stdout);
    }
    let checked = check(&repo);
    assert!(
        checked.stdout.contains("HN-M005") && checked.stdout.contains("root:tok.gone"),
        "{}",
        checked.stdout
    );
}

#[test]
fn r_port_022__seal_writes_the_same_lock_lines_as_sync() {
    let repo = scoped();
    std::fs::write(
        repo.path().join("web/fairlead/control.toml"),
        "[fairlead]\nscope = \"web\"\n\n[scope]\nrequires = []\n",
    )
    .unwrap();
    let new = repo.run(&[
        "new",
        "change",
        "Adds b",
        "--field",
        "impact=add",
        "--field",
        "covers=**",
        "--as-of",
        AS_OF,
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
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    repo.git(&["add", "-A"]);
    repo.git_commit("ready");
    let seal = repo.run(&["seal", "--yes", "--anywhere", "--as-of", AS_OF]);
    assert_eq!(seal.code, 0, "{}{}", seal.stdout, seal.stderr);
    let lock = std::fs::read_to_string(repo.path().join("web/fairlead/gen/scope.lock")).unwrap();
    assert!(lock.contains("req=cmp.button from=root dg="), "{lock}");
    let run = check(&repo);
    assert!(!run.stdout.contains("HN-V012"), "{}", run.stdout);
}
