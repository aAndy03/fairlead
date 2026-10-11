#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! 0.2 P7-T3, exit criteria 2 and 4 (R-EDN-051, R-EDN-052): for every golden repository, under the
//! synthetic edition 2 of the `test-edition` feature, an edition-1 checkout is refused with the
//! bridging line; the upgrade plans, drafts, is approved, is interrupted mid-apply and finishes on
//! the re-run; re-planning finds nothing; and the upgraded tree checks.
#[path = "testkit/mod.rs"]
mod testkit;

use std::path::{Path, PathBuf};
use std::process::Command;
use testkit::golden_spec as gs;

const AS_OF: &str = "2026-10-10";
const ED2: (&str, &str) = ("FAIRLEAD_TEST_EDITION", "2");

fn golden_dirs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden");
    let mut v: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@invalid"])
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn commit(dir: &Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", msg]);
}

fn run(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> testkit::Run {
    let mut a = args.to_vec();
    a.extend(["--as-of", AS_OF]);
    testkit::run_fairlead(dir, &a, env)
}

/// The root control file: the `control.toml` whose `[fairlead] root` is true, nearest the top.
fn root_control(dir: &Path) -> Option<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                walk(&p, out);
            } else if p.file_name().is_some_and(|n| n == "control.toml") {
                out.push(p);
            }
        }
    }
    let mut all = Vec::new();
    walk(dir, &mut all);
    all.sort_by_key(|p| p.components().count());
    all.into_iter().find(|p| {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
            .and_then(|t| t.get("fairlead")?.get("root")?.as_bool())
            == Some(true)
    })
}

/// Sets `settings.purpose`, the field edition 2 requires, keeping the file's line endings.
fn add_purpose(control: &Path) {
    let text = std::fs::read_to_string(control).unwrap();
    if text.contains("purpose") {
        return;
    }
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let head = format!("[settings]{nl}");
    let new = if text.contains(&head) {
        text.replacen(&head, &format!("{head}purpose = \"golden\"{nl}"), 1)
    } else {
        format!("{text}{nl}[settings]{nl}purpose = \"golden\"{nl}")
    };
    std::fs::write(control, new).unwrap();
}

fn plan_hash(stdout: &str) -> Option<String> {
    stdout
        .split_whitespace()
        .find(|w| w.starts_with("b3:"))
        .map(str::to_owned)
}

/// Marks the upgrade doc ready and records an owner approval for the plan.
fn approve(folder: &Path, hash: &str) -> Result<(), String> {
    let doc = std::fs::read_dir(folder.join("docs/changes"))
        .map_err(|e| e.to_string())?
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().contains("upgrade-edition-1-to-2"))
        .ok_or("no upgrade draft written")?;
    let text = std::fs::read_to_string(&doc).unwrap();
    std::fs::write(
        &doc,
        text.replace("status = \"draft\"", "status = \"ready\""),
    )
    .unwrap();
    let id = text
        .lines()
        .find_map(|l| l.trim_end().strip_prefix("id = \""))
        .ok_or("draft has no id")?
        .trim_end_matches('"')
        .to_owned();
    let fp = fairlead_core::upgrade::upgrade_fp(&id, hash);
    let approvals = folder.join("approvals.toml");
    let old = std::fs::read_to_string(&approvals).unwrap_or_default();
    std::fs::write(
        &approvals,
        format!(
            "{old}\n[[approval]]\nfp = \"{}\"\ntarget = \"edition.upgrade\"\nscope = \"root\"\napprover = \"@owner\"\ndate = \"{AS_OF}\"\n",
            fp.as_str()
        ),
    )
    .unwrap();
    Ok(())
}

/// `Ok(None)` when the golden upgraded; `Ok(Some(reason))` when it does not apply; `Err` on a
/// failure.
fn upgrade_golden(dir: &Path) -> Result<Option<String>, String> {
    let g = gs::load(dir)?;
    if !g.golden.platforms.iter().any(|p| p == std::env::consts::OS) {
        return Ok(Some("not on this platform".into()));
    }
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    gs::materialise(dir, &g.golden, &root).map_err(|e| e.to_string())?;
    if !root.join(".git").exists() {
        git(&root, &["init", "-q", "-b", "main"]);
        commit(&root, "golden");
    }
    let Some(control) = root_control(&root) else {
        return Ok(Some("no parseable root control file".into()));
    };
    let first = run(&root, &["check"], &[]);
    if first.code > 3 {
        return Ok(Some(format!("edition-1 check exits {}", first.code)));
    }
    // R-EDN-011, R-EDN-012: an edition-1 checkout under an edition-2 binary is refused, and the
    // refusal names the binary to install.
    let refused = run(&root, &["check"], &[ED2]);
    if refused.code != 4 || !refused.stderr.contains("cargo install fairlead --version") {
        return Err(format!(
            "old-edition checkout not refused with the bridging line: exit {}\n{}",
            refused.code, refused.stderr
        ));
    }
    add_purpose(&control);
    let synced = run(&root, &["sync", "--yes"], &[]);
    if synced.code != 0 {
        return Err(format!("sync exits {}: {}", synced.code, synced.stderr));
    }
    commit(&root, "purpose");
    let plan = run(&root, &["upgrade"], &[ED2]);
    let hash = match (plan.code, plan_hash(&plan.stdout)) {
        (4, Some(h)) if plan.stdout.contains("upgrade: edition 1 -> 2") => h,
        _ => {
            return Err(format!(
                "plan: exit {}\n{}{}",
                plan.code, plan.stdout, plan.stderr
            ));
        }
    };
    let draft = run(&root, &["upgrade", "--yes"], &[ED2]);
    if !draft.stdout.contains("awaits approval") {
        return Err(format!("draft: {}{}", draft.stdout, draft.stderr));
    }
    commit(&root, "upgrade draft");
    let folder = control.parent().unwrap();
    approve(folder, &hash)?;
    commit(&root, "approve");
    // R-EDN-052: an apply interrupted after its third rename is finished by the next run.
    let crashed = run(
        &root,
        &["upgrade", "--yes"],
        &[ED2, ("FAIRLEAD_TEST_CRASH_AFTER", "3")],
    );
    if crashed.code == 0 || !root.join(".fairlead/sync.journal").exists() {
        return Err(format!(
            "the injected crash did not interrupt the apply: exit {}\n{}{}",
            crashed.code, crashed.stdout, crashed.stderr
        ));
    }
    let resumed = run(&root, &["upgrade", "--yes"], &[ED2]);
    if resumed.code != 0 {
        return Err(format!(
            "re-run after the interruption: exit {}\n{}{}",
            resumed.code, resumed.stdout, resumed.stderr
        ));
    }
    let edition = std::fs::read_to_string(&control).unwrap();
    if !edition.contains("edition = 2") {
        return Err(format!("not upgraded after the re-run:\n{edition}"));
    }
    // R-EDN-051: re-planning an upgraded tree is empty.
    let again = run(&root, &["upgrade"], &[ED2]);
    if again.code != 0 || !again.stdout.contains("nothing to upgrade") {
        return Err(format!(
            "re-plan not empty: {}{}",
            again.stdout, again.stderr
        ));
    }
    let check = run(&root, &["check"], &[ED2]);
    if check.code > 2 {
        return Err(format!(
            "upgraded tree does not check: exit {}\n{}{}",
            check.code, check.stdout, check.stderr
        ));
    }
    let old = run(&root, &["check"], &[]);
    if old.code != 4 {
        return Err(format!(
            "an edition-1 binary accepts the upgraded tree: exit {}",
            old.code
        ));
    }
    Ok(None)
}

#[test]
fn r_edn_051__every_golden_upgrades_idempotently_and_survives_an_interruption() {
    let mut failures = Vec::new();
    let mut skipped = Vec::new();
    let mut upgraded = 0;
    for dir in golden_dirs() {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        match upgrade_golden(&dir) {
            Ok(None) => upgraded += 1,
            Ok(Some(why)) => skipped.push(format!("{name}: {why}")),
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    println!("upgraded {upgraded}; skipped: {skipped:?}");
    assert!(failures.is_empty(), "{}", failures.join("\n---\n"));
    assert!(
        upgraded >= 12,
        "too few goldens upgraded: {upgraded}; skipped {skipped:?}"
    );
}
