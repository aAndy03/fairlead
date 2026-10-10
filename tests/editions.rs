#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! Edition byte stability (R-EDN-051, R-EDN-002): for every supported edition a fixture codebase
//! keeps its committed generated output, and a binary of that edition regenerates it byte for byte.
//! Edition 1 is frozen from v0.1.0 (R-REL-002); its fixtures are the golden repositories.
#[path = "testkit/mod.rs"]
mod testkit;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use testkit::{golden_checkout, golden_root, run_fairlead};

fn generated_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                if !p.ends_with(".git") && !p.ends_with(".fairlead") {
                    stack.push(p);
                }
            } else if p.components().any(|c| c.as_os_str() == "gen") {
                // A CRLF checkout (crlf golden) holds CRLF copies; `sync` writes LF (R-GIT-013).
                let text = std::fs::read(&p).unwrap();
                let lf = String::from_utf8_lossy(&text)
                    .replace("\r\n", "\n")
                    .into_bytes();
                out.insert(p.strip_prefix(root).unwrap().to_path_buf(), lf);
            }
        }
    }
    out
}

#[test]
fn r_edn_051__edition_1_generated_output_is_byte_stable() {
    let mut synced = 0;
    for entry in std::fs::read_dir(golden_root()).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        let (_guard, root) = golden_checkout(&name, false);
        let before = generated_files(&root);
        let run = run_fairlead(&root, &["sync", "--as-of", "2026-10-09"], &[]);
        if run.code != 0 {
            // Goldens built to refuse (unparseable control, held lock) have nothing to compare.
            continue;
        }
        synced += 1;
        assert!(!before.is_empty(), "{name}: no committed generated files");
        assert_eq!(
            generated_files(&root),
            before,
            "{name}: sync changed committed edition-1 generated output"
        );
    }
    assert!(synced >= 6, "only {synced} goldens could be synced");
}
