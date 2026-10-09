#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! Path and file behaviour that must hold on Windows, Linux and macOS (R-PORT-014).
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, run_fairlead};

const ROOT_CONTROL: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n[settings]\nfolder = \"fairlead\"\nfairlead-version = \"0.1.0\"\nowners = [\"@andy\"]\n";

#[test]
fn r_port_014__paths_over_260_characters_are_walked_with_forward_slashes() {
    let deep: String = (0..12)
        .map(|i| format!("segment-{i:02}-abcdefghijklmnopqrstu"))
        .collect::<Vec<_>>()
        .join("/");
    assert!(deep.len() > 260, "the path must exceed 260 characters");
    let repo = RepoBuilder::new()
        .file("fairlead/control.toml", ROOT_CONTROL)
        .file(&format!("{deep}/note.ts"), "export const n = 1;\n")
        .build();
    // E-27: a set-up repository carries the shards `sync` writes.
    let sync = run_fairlead(repo.path(), &["sync", "--as-of", "2026-10-09"], &[]);
    assert_eq!(sync.code, 0, "sync over a long path: {}", sync.stderr);
    let run = run_fairlead(
        repo.path(),
        &["check", "--as-of", "2026-10-09", "--format", "terse"],
        &[],
    );
    assert_eq!(
        run.code, 0,
        "a long path is not an error: {}{}",
        run.stdout, run.stderr
    );
    assert!(
        !run.stdout.contains("\\\\?\\"),
        "no extended-length prefix in output"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn r_port_014__case_only_collision_is_reported_as_path_hygiene() {
    let repo = RepoBuilder::new()
        .file("fairlead/control.toml", ROOT_CONTROL)
        .file("src/Button.tsx", "export const a = 1;\n")
        .file("src/button.tsx", "export const b = 2;\n")
        .build();
    let run = run_fairlead(
        repo.path(),
        &["check", "--as-of", "2026-10-09", "--format", "terse"],
        &[],
    );
    assert_eq!(run.code, 1, "HN-P001 is a floor error");
    assert!(
        run.stdout.lines().any(|l| l.starts_with("HN-P001")),
        "case-only collision is HN-P001"
    );
}

#[test]
#[allow(clippy::disallowed_methods)] // measurement only; the timing never reaches output or gates
fn r_port_014__antivirus_latency_is_measured_and_reported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut micros: Vec<u128> = Vec::with_capacity(200);
    for i in 0..200 {
        let tmp = dir.path().join(format!("w{i}.tmp"));
        let to = dir.path().join(format!("w{i}.txt"));
        std::fs::write(&tmp, b"x").expect("write");
        let start = std::time::Instant::now();
        std::fs::rename(&tmp, &to).expect("rename");
        micros.push(start.elapsed().as_micros());
    }
    micros.sort_unstable();
    eprintln!(
        "rename latency (us): p50={} p95={}",
        micros[100], micros[190]
    );
    assert_eq!(micros.len(), 200);
}
