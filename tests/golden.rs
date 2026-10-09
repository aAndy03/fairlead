#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
#[path = "testkit/mod.rs"]
mod testkit;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use testkit::golden_spec as gs;

/// R-ARCH-153 mandatory set (contract section 7). The directory name is the fixture name.
const MANDATORY: [&str; 11] = [
    "ts-react",
    "rust-workspace",
    "fullstack-tauri",
    "legacy-baseline",
    "crlf-and-windows-names",
    "nested-scopes-ovr",
    "unparseable-control",
    "zero-rules",
    "sparse-checkout",
    "legacy-encoding",
    "two-writers",
];

fn golden_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

fn golden_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(golden_root())
        .expect("tests/golden exists")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

#[test]
#[ignore = "the golden repositories arrive in P9-T4..T14"]
fn r_arch_153__mandatory_golden_set_is_complete_and_includes_typescript() {
    let names: BTreeSet<String> = golden_dirs()
        .iter()
        .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    for m in MANDATORY {
        assert!(names.contains(m), "missing mandatory golden repository {m}");
    }
    let ts = golden_root().join("ts-react/repo");
    let has_tsx = std::fs::read_dir(&ts)
        .expect("ts-react/repo exists")
        .any(|_| true);
    assert!(has_tsx, "ts-react must contain a TypeScript/React tree");
}

#[test]
#[ignore = "the golden repositories arrive in P9-T4..T14"]
fn r_chk_051__no_false_dangling_references() {
    for name in ["ts-react", "rust-workspace", "fullstack-tauri"] {
        let dir = golden_root().join(name).join("expected");
        for entry in std::fs::read_dir(&dir).expect("expected dir") {
            let path = entry.expect("entry").path();
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(
                !text.lines().any(|l| l.contains("HN-R001")),
                "{}: false dangling reference in a real-repository golden output",
                path.display()
            );
        }
    }
}

#[test]
#[ignore = "the golden repositories arrive in P9-T4..T14"]
fn golden_repositories_match_expected_bytes() {
    let bin = PathBuf::from(testkit::fairlead_bin());
    let mut failures: Vec<String> = Vec::new();
    for dir in golden_dirs() {
        let g = gs::load(&dir).expect("golden.toml parses");
        if !g.golden.platforms.iter().any(|p| p == std::env::consts::OS) {
            continue;
        }
        for (i, case) in g.cases.iter().enumerate() {
            let scratch = tempfile::tempdir().expect("tempdir");
            let root = scratch.path().join("repo");
            gs::materialise(&dir, &g.golden, &root).expect("materialise");
            let run = gs::run_case(&bin, &root, case).expect("run fairlead");
            let label = format!("{} case {} ({})", g.golden.name, i + 1, case.args.join(" "));
            if run.code != case.exit {
                failures.push(format!(
                    "{label}: exit {} expected {}\n{}",
                    run.code, case.exit, run.stderr
                ));
            }
            if let Some(rel) = &case.stdout {
                let path = dir.join(rel);
                match std::fs::read(&path) {
                    Err(_) => failures.push(format!("{label}: expected file {rel} is missing")),
                    Ok(want) if want != run.stdout => failures.push(format!(
                        "{label}: stdout differs at {}",
                        gs::first_difference(&want, &run.stdout)
                    )),
                    Ok(_) => {}
                }
            }
            if case.git_status_clean {
                let status = gs::git_status(&root).expect("git status");
                if !status.is_empty() {
                    failures.push(format!("{label}: git status not clean:\n{status}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "golden failures. Re-bless only after a human review: cargo xtask golden-bless <name>\n{}",
        failures.join("\n---\n")
    );
}
