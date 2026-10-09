//! Developer tasks for the fairlead workspace (contract section 1). Tooling, never shipped.
#![forbid(unsafe_code)]
#![allow(clippy::print_stdout, clippy::print_stderr)] // reason: a developer command line prints

use std::process::ExitCode;

/// Crates that must never appear in `Cargo.lock` (R-ARCH-120, R-ARCH-123, R-AGT-007, D-05, D-26).
const FORBIDDEN: &[(&str, &str)] = &[
    ("serde_yaml", "no YAML (D-05)"),
    ("serde_yml", "no YAML (D-05), RUSTSEC-2025-0068"),
    ("serde_yaml_ng", "no YAML (D-05)"),
    ("bincode", "discontinued (R-ARCH-120)"),
    ("sled", "pre-1.0 store (R-ARCH-120)"),
    ("jwalk", "not assessed (R-ARCH-120)"),
    ("serde-sarif", "SARIF is hand-built (R-ARCH-054)"),
    ("reqwest", "no network stack in 0.1 (R-ARCH-123)"),
    ("ureq", "no network stack in 0.1 (R-ARCH-123)"),
    ("hyper", "no network stack in 0.1 (R-ARCH-123)"),
    ("openssl", "no network stack in 0.1 (R-ARCH-123)"),
    ("openssl-sys", "no network stack in 0.1 (R-ARCH-123)"),
    ("rustls", "no network stack in 0.1 (R-ARCH-123)"),
    ("native-tls", "no network stack in 0.1 (R-ARCH-123)"),
    ("curl", "no network stack in 0.1 (R-ARCH-123)"),
    ("curl-sys", "no network stack in 0.1 (R-ARCH-123)"),
    ("libssh2-sys", "no network stack in 0.1 (R-ARCH-123)"),
    ("notify", "no background watchers (D-26)"),
    ("daemonize", "no background processes (D-26)"),
];

/// Names of every `[[package]]` in a `Cargo.lock` text, in file order.
fn lock_package_names(lock: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut in_package = false;
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            in_package = true;
        } else if in_package && let Some(rest) = line.strip_prefix("name = \"") {
            if let Some(name) = rest.strip_suffix('"') {
                names.push(name);
            }
            in_package = false;
        }
    }
    names
}

/// Forbidden crates present in a lock file, with the reason for each.
fn forbidden_in_lock(lock: &str) -> Vec<(String, &'static str)> {
    let mut hits = Vec::new();
    for name in lock_package_names(lock) {
        for (bad, why) in FORBIDDEN {
            if name == *bad {
                hits.push((name.to_string(), *why));
            }
        }
    }
    hits.sort();
    hits.dedup();
    hits
}

fn check_deps() -> ExitCode {
    let lock = match std::fs::read_to_string("Cargo.lock") {
        Ok(text) => text,
        Err(err) => {
            eprintln!("xtask check-deps: cannot read Cargo.lock: {err}");
            return ExitCode::from(2);
        }
    };
    let hits = forbidden_in_lock(&lock);
    if hits.is_empty() {
        println!("xtask check-deps: no forbidden crate in Cargo.lock");
        return ExitCode::SUCCESS;
    }
    for (name, why) in &hits {
        eprintln!("xtask check-deps: forbidden crate `{name}`: {why}");
    }
    ExitCode::from(1)
}

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("check-deps") => check_deps(),
        Some(other) => {
            eprintln!("xtask: task `{other}` is not implemented in this build");
            ExitCode::from(2)
        }
        None => {
            eprintln!("xtask: usage: cargo xtask <check-deps>");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = "version = 4\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"reqwest\"\nversion = \"0.12.0\"\n\n[[package]]\nname = \"bincode\"\nversion = \"1.3.3\"\n";

    #[test]
    fn lists_package_names_in_order() {
        assert_eq!(
            lock_package_names(LOCK),
            vec!["serde", "reqwest", "bincode"]
        );
    }

    #[test]
    fn finds_forbidden_crates() {
        let hits = forbidden_in_lock(LOCK);
        let names: Vec<&str> = hits.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["bincode", "reqwest"]);
    }

    #[test]
    fn clean_lock_has_no_hits() {
        assert!(forbidden_in_lock("[[package]]\nname = \"serde\"\n").is_empty());
    }
}
