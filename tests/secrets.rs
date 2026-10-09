#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! Fifty seeded secret-like values are rejected by the floor scan and never printed (E5).
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, run_fairlead};

const PEM: [&str; 10] = [
    "RSA",
    "EC",
    "DSA",
    "OPENSSH",
    "ENCRYPTED",
    "PKCS8",
    "PGP",
    "SSH2",
    "ED25519",
    "DH",
];
const PREFIXES: [&str; 20] = [
    "sk-",
    "ghp_",
    "gho_",
    "github_pat_",
    "AKIA",
    "ASIA",
    "xoxb-",
    "xoxp-",
    "glpat-",
    "sk_live_",
    "sk_test_",
    "rk_live_",
    "AIza",
    "npm_",
    "pypi-AgEI",
    "SG.",
    "hf_",
    "dop_v1_",
    "shpat_",
    "eyJhbGci",
];

/// A deterministic pseudo-random alphanumeric string (linear congruential generator).
fn entropy(seed: u64, len: usize) -> String {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut state = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ALPHA[((state >> 33) as usize) % ALPHA.len()] as char
        })
        .collect()
}

/// Fifty seeded values, index 0..50: PEM headers, provider prefixes, then entropy strings.
fn seeded() -> Vec<String> {
    (0..50)
        .map(|i| match i {
            0..10 => format!(
                "-----BEGIN {} PRIVATE KEY----- {}",
                PEM[i],
                entropy(i as u64, 40)
            ),
            10..30 => format!("{}{}", PREFIXES[i - 10], entropy(i as u64, 36)),
            _ => entropy(i as u64, 48),
        })
        .collect()
}

#[test]
fn r_agt_022__fifty_seeded_values_are_rejected_and_never_printed() {
    let values = seeded();
    assert_eq!(values.len(), 50);
    let mut builder = RepoBuilder::new().file("fairlead/control.toml", "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n[settings]\nfolder = \"fairlead\"\nfairlead-version = \"0.1.0\"\nowners = [\"@andy\"]\n");
    for (i, v) in values.iter().enumerate() {
        builder = builder.file(
            &format!("fairlead/docs/s{i:02}.md"),
            &format!("+++\ntitle = \"s{i:02}\"\nsum = \"{v}\"\n+++\n"),
        );
    }
    let repo = builder.build();
    let run = run_fairlead(
        repo.path(),
        &[
            "check",
            "--as-of",
            "2026-10-09",
            "--format",
            "terse",
            "--max",
            "0",
        ],
        &[],
    );
    assert_eq!(
        run.code, 1,
        "floor findings are enforce errors: {}",
        run.stderr
    );
    let hits = run
        .stdout
        .lines()
        .filter(|l| l.starts_with("HN-F001"))
        .count();
    assert_eq!(hits, 50, "one HN-F001 per seeded value");
    for v in &values {
        assert!(
            !run.stdout.contains(v.as_str()),
            "a matched value was printed"
        );
    }
}
