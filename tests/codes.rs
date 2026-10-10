#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    non_snake_case,
    reason = "test names start with the rule id"
)]
//! Every active diagnostic code is asserted somewhere (R-ARCH-151, A-70): its text (`HN-X002`)
//! or its constant (`HN_X002`) appears in a test file, a test module, a snapshot, a trycmd case
//! or a golden expected output. The code list is the product's own table.

use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries {
        let p = e.unwrap().path();
        if p.is_dir() {
            if !p.ends_with("target") && !p.ends_with(".git") {
                files(&p, out);
            }
        } else {
            out.push(p);
        }
    }
}

/// The code tables themselves (their tests enumerate every code, which proves nothing about
/// emission).
const TABLES: [&str; 3] = [
    "crates/fairlead-core/src/check/codes.rs",
    "crates/fairlead-core/src/diag.rs",
    "crates/fairlead-syntax/src/diag.rs",
];

/// The asserting text of the repository: whole files under any `tests/` directory, and the
/// `#[cfg(test)]` tail of every source file except the code tables.
fn evidence(root: &Path) -> String {
    let mut all = Vec::new();
    files(&root.join("crates"), &mut all);
    files(&root.join("tests"), &mut all);
    let tables: Vec<PathBuf> = TABLES.iter().map(|t| root.join(t)).collect();
    let mut text = String::new();
    for p in all {
        if tables.contains(&p) || p.ends_with("codes.rs") && p.starts_with(root.join("tests")) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let in_tests = p.components().any(|c| c.as_os_str() == "tests");
        if in_tests {
            text.push_str(&src);
        } else if p.extension().is_some_and(|x| x == "rs")
            && let Some(i) = src.find("#[cfg(test)]")
        {
            text.push_str(&src[i..]);
        }
        text.push('\n');
    }
    text
}

/// 0.2 codes whose checks have not landed yet (plans/0.2 master, constraint 4). Each task that
/// lands a check removes its codes; 0.2 P7-T7 requires this list to be empty.
const PENDING_0_2: &[&str] = &[
    "HN-A003", "HN-C014", "HN-D003", "HN-E001", "HN-E002", "HN-E003", "HN-E004", "HN-E005",
    "HN-E006", "HN-E007", "HN-G006", "HN-L001", "HN-L002", "HN-M007", "HN-M008", "HN-M011",
    "HN-R004", "HN-R006", "HN-R008", "HN-T008", "HN-T009", "HN-T010", "HN-V011", "HN-V015",
    "HN-V016", "HN-X013", "HN-X015", "HN-X016",
];

fn asserted(text: &str, code: &str) -> bool {
    text.contains(code) || text.contains(&code.replace('-', "_"))
}

#[test]
fn r_arch_151__every_active_code_is_asserted_somewhere() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = evidence(&root);
    let missing: Vec<&str> = fairlead_core::check::all_specs()
        .iter()
        .filter(|s| fairlead_core::check::is_active(s.code))
        .map(|s| s.code.0)
        .filter(|code| !asserted(&text, code) && !PENDING_0_2.contains(code))
        .collect();
    assert!(
        missing.is_empty(),
        "active codes no test asserts: {missing:?}"
    );
}

#[test]
fn r_arch_151__pending_list_holds_only_unasserted_codes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = evidence(&root);
    let stale: Vec<&&str> = PENDING_0_2
        .iter()
        .filter(|code| asserted(&text, code))
        .collect();
    assert!(
        stale.is_empty(),
        "now asserted, remove from PENDING_0_2: {stale:?}"
    );
}
