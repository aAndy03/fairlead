//! Developer tasks for the fairlead workspace (contract section 1). Tooling, never shipped.
#![forbid(unsafe_code)]
#![allow(clippy::print_stdout, clippy::print_stderr)] // reason: a developer command line prints

mod ablation;
mod bench;
mod coverage_replay;
mod docs;
mod golden;
mod merge_sim;
mod sarif;
mod syntax_card;

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

/// `bench-corpus generate --files N --out DIR` or `bench-corpus check --results FILE --baseline FILE`.
fn bench_corpus(args: &[String]) -> ExitCode {
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let result = match args.first().map(String::as_str) {
        Some("generate") => {
            let files = flag("--files")
                .and_then(|n| n.parse().ok())
                .unwrap_or(100_000);
            let out = flag("--out").unwrap_or_else(|| "target/bench/corpus".into());
            bench::generate(std::path::Path::new(&out), files).map_err(|e| e.to_string())
        }
        Some("check") => {
            let read = |f: Option<String>| {
                f.ok_or("missing file argument".to_string())
                    .and_then(|p| std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}")))
            };
            read(flag("--results")).and_then(|r| {
                read(flag("--baseline").or(Some("bench/baseline.toml".into())))
                    .and_then(|b| bench::check(&r, &b))
            })
        }
        _ => Err("usage: bench-corpus generate|check".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask bench-corpus: {e}");
            ExitCode::from(1)
        }
    }
}

/// `ablation-score --diffs DIR [--declared F] --out F`, `ablation-report --scores F --map F`,
/// `ablation-arm-b [--declared F] --out F` (P9-T25).
fn ablation_task(task: &str, args: &[String]) -> Result<(), String> {
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let need = |name: &str| flag(name).ok_or_else(|| format!("missing {name}"));
    let read = |p: &str| std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
    let write = |p: &str, text: &str| {
        if let Some(dir) = std::path::Path::new(p).parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{p}: {e}"))?;
        }
        std::fs::write(p, text).map_err(|e| format!("{p}: {e}"))
    };
    let declared = || {
        let path = flag("--declared").unwrap_or_else(|| "ablation/declared.toml".into());
        ablation::parse_declared(&read(&path)?)
    };
    match task {
        "ablation-score" => {
            let text = ablation::score_dir(std::path::Path::new(&need("--diffs")?), &declared()?)?;
            write(&need("--out")?, &text)
        }
        "ablation-report" => {
            print!(
                "{}",
                ablation::report(&read(&need("--scores")?)?, &read(&need("--map")?)?)?
            );
            Ok(())
        }
        _ => write(&need("--out")?, &ablation::arm_b_agents(&declared()?)),
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let task = args.next();
    match task.as_deref() {
        Some("check-deps") => check_deps(),
        Some("bench-corpus") => bench_corpus(&args.collect::<Vec<_>>()),
        Some(t @ ("ablation-score" | "ablation-report" | "ablation-arm-b")) => {
            match ablation_task(t, &args.collect::<Vec<_>>()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("xtask {t}: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("syntax-card-score") => {
            let dir = args.next().unwrap_or_default();
            let result = syntax_card::score(std::path::Path::new(&dir)).and_then(|(t, i)| {
                println!("syntax-card samples={t} invalid={i}");
                syntax_card::gate(t, i)
            });
            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("xtask syntax-card-score: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("sarif-validate") => {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or(std::path::Path::new("."));
            match sarif::validate(root) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("xtask sarif-validate: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("docs-check") => {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or(std::path::Path::new("."));
            match docs::check(root) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("xtask docs-check: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("golden-bless") => {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or(std::path::Path::new("."));
            match golden::bless(root, args.next().as_deref().unwrap_or("")) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("xtask golden-bless: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("coverage-replay") => {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or(std::path::Path::new("."));
            let mut o = coverage_replay::Options {
                repo: std::path::PathBuf::from("."),
                commits: 100,
                out: root.join("target/coverage-replay.csv"),
                bin: None,
            };
            while let Some(a) = args.next() {
                let v = args.next().unwrap_or_default();
                match a.as_str() {
                    "--repo" => o.repo = std::path::PathBuf::from(v),
                    "--commits" => o.commits = v.parse().unwrap_or(100),
                    "--out" => o.out = std::path::PathBuf::from(v),
                    "--bin" => o.bin = Some(std::path::PathBuf::from(v)),
                    _ => {
                        eprintln!("xtask coverage-replay: unknown option {a}");
                        return ExitCode::from(2);
                    }
                }
            }
            match coverage_replay::run(root, &o) {
                Ok(s) => {
                    println!(
                        "coverage-replay commits={} replayed={} skipped={} changed-files={} flagged={} csv={}",
                        o.commits,
                        s.replayed,
                        s.skipped.len(),
                        s.changed,
                        s.flagged,
                        o.out.display()
                    );
                    for k in &s.skipped {
                        println!("  skipped {k}");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("xtask coverage-replay: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("coverage-score") => {
            // Exit criterion 3: at most 5% of the labelled flags are wrong.
            let path = args.next().unwrap_or_default();
            match std::fs::read_to_string(&path) {
                Ok(csv) => match coverage_replay::wrong_share(&csv) {
                    Some(w) => {
                        println!("coverage-score wrong-share={:.1}% (gate 5%)", w * 100.0);
                        if w <= 0.05 {
                            ExitCode::SUCCESS
                        } else {
                            ExitCode::from(1)
                        }
                    }
                    None => {
                        eprintln!("xtask coverage-score: no labelled row in {path}");
                        ExitCode::from(1)
                    }
                },
                Err(e) => {
                    eprintln!("xtask coverage-score: {path}: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("merge-sim") => {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or(std::path::Path::new("."));
            let mut o = merge_sim::Options {
                branches: 50,
                seed: 1,
                bin: None,
                keep: false,
            };
            while let Some(a) = args.next() {
                if a == "--keep" {
                    o.keep = true;
                    continue;
                }
                let v = args.next().unwrap_or_default();
                match a.as_str() {
                    "--branches" => o.branches = v.parse().unwrap_or(50),
                    "--seed" => o.seed = v.parse().unwrap_or(1),
                    "--bin" => o.bin = Some(std::path::PathBuf::from(v)),
                    _ => {
                        eprintln!("xtask merge-sim: unknown option {a}");
                        return ExitCode::from(2);
                    }
                }
            }
            match merge_sim::run(root, &o) {
                Ok(r) => {
                    println!(
                        "merge-sim branches={} seed={} merges={} conflicted={} continuity={} gen-left={} unreported={} unrepairable={} failed-checks={}",
                        o.branches,
                        o.seed,
                        r.merges,
                        r.conflicted,
                        r.continuity,
                        r.gen_left.len(),
                        r.unreported.len(),
                        r.unrepairable.len(),
                        r.failed_checks.len()
                    );
                    for l in r
                        .gen_left
                        .iter()
                        .chain(&r.unreported)
                        .chain(&r.unrepairable)
                        .chain(&r.failed_checks)
                    {
                        println!("  {l}");
                    }
                    if r.passed() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(e) => {
                    eprintln!("xtask merge-sim: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some(other) => {
            eprintln!("xtask: task `{other}` is not implemented in this build");
            ExitCode::from(2)
        }
        None => {
            eprintln!(
                "xtask: usage: cargo xtask <check-deps | bench-corpus | docs-check | ablation-score | ablation-report | ablation-arm-b | syntax-card-score DIR | sarif-validate | golden-bless NAME | merge-sim [--branches N] [--seed N] [--bin PATH] | coverage-replay --repo PATH [--commits N] [--out CSV] | coverage-score CSV>"
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    const REQWEST_LOCK: &str = "version = 4

[[package]]
name = \"reqwest\"
version = \"0.12.0\"
";

    fn names(hits: &[(String, &str)]) -> Vec<String> {
        hits.iter().map(|(n, _)| n.clone()).collect()
    }

    #[test]
    fn r_arch_123__detects_an_http_client_in_the_lock() {
        assert_eq!(names(&forbidden_in_lock(REQWEST_LOCK)), vec!["reqwest"]);
    }

    #[test]
    fn r_arch_123__dependency_lists_do_not_count() {
        let text = "[[package]]
name = \"app\"
version = \"0.1.0\"
dependencies = [
 \"reqwest\",
]
";
        assert!(forbidden_in_lock(text).is_empty());
    }

    #[test]
    fn r_arch_123__contract_forbidden_names_are_all_listed() {
        for name in [
            "serde_yaml",
            "serde_yml",
            "serde_yaml_ng",
            "bincode",
            "sled",
            "jwalk",
            "serde-sarif",
            "reqwest",
            "ureq",
            "hyper",
            "openssl",
            "rustls",
            "native-tls",
        ] {
            assert!(
                FORBIDDEN.iter().any(|(n, _)| *n == name),
                "{name} must be forbidden"
            );
        }
    }

    #[test]
    fn r_arch_123__clean_lock_has_no_hits_and_names_are_read() {
        let text = "[[package]]
name = \"serde\"
version = \"1.0.229\"

[[package]]
name = \"blake3\"
";
        assert!(forbidden_in_lock(text).is_empty());
        assert_eq!(lock_package_names(text), vec!["serde", "blake3"]);
    }

    #[test]
    fn r_arch_123__each_hit_is_reported_once() {
        let text = "[[package]]
name = \"hyper\"
version = \"1.0.0\"

[[package]]
name = \"hyper\"
version = \"0.14.0\"
";
        assert_eq!(names(&forbidden_in_lock(text)), vec!["hyper"]);
    }
}
