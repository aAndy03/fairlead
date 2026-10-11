//! The Merkle and ledger benchmark (0.2 P7-T5, exit criterion 8, R-VER-016) on the bench corpus:
//! `bench-corpus scopes` makes child scopes of its packages, `bench-corpus history` seals change
//! docs across them with the real binary, and `bench-corpus merkle` measures what a one-file
//! change re-reads (through `FAIRLEAD_STATS`), the check time that recomputes the scope digests and
//! verifies the ledger chain, and the ledger size.

use std::path::{Path, PathBuf};
use std::process::Command;

const AS_OF: &str = "2026-10-10";

/// The package directory of child scope `j` among `scopes`, spread over `packages`.
pub fn scope_dir(j: usize, scopes: usize, packages: usize) -> String {
    let step = (packages / scopes.max(1)).max(1);
    format!("pkg-{:04}", j * step)
}

/// The scopes sealed by seal `k`: every `j` with `j % seals == k`.
pub fn sealed_by(k: usize, seals: usize, scopes: usize) -> Vec<usize> {
    (0..scopes).filter(|j| j % seals.max(1) == k).collect()
}

/// Writes a control file into `scopes` package directories of a generated corpus.
pub fn add_scopes(dir: &Path, files: usize, scopes: usize) -> std::io::Result<()> {
    let packages = files.div_ceil(100);
    for j in 0..scopes {
        let s = scope_dir(j, scopes, packages);
        let d = dir.join(&s).join("fairlead");
        std::fs::create_dir_all(&d)?;
        std::fs::write(
            d.join("control.toml"),
            format!("[fairlead]\nscope = \"{s}\"\n"),
        )?;
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=bench", "-c", "user.email=bench@invalid"])
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

struct Ran {
    code: i32,
    stdout: String,
    stderr: String,
    secs: f64,
}

fn fairlead(dir: &Path, bin: &Path, args: &[&str]) -> Result<Ran, String> {
    #[allow(clippy::disallowed_methods)] // reason: a benchmark measures wall time
    let start = std::time::Instant::now();
    let out = Command::new(bin)
        .current_dir(dir)
        .args(args)
        .args(["--as-of", AS_OF])
        .env("NO_COLOR", "1")
        .env("FAIRLEAD_STATS", "1")
        .env_remove("FAIRLEAD_ROOT")
        .output()
        .map_err(|e| format!("fairlead: {e}"))?;
    Ok(Ran {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        secs: start.elapsed().as_secs_f64(),
    })
}

fn ok(dir: &Path, bin: &Path, args: &[&str]) -> Result<Ran, String> {
    let r = fairlead(dir, bin, args)?;
    if r.code == 0 {
        Ok(r)
    } else {
        Err(format!(
            "fairlead {} exited {}: {}{}",
            args.join(" "),
            r.code,
            r.stdout,
            r.stderr
        ))
    }
}

/// The `files-read` count of a run's `stats:` line.
pub fn files_read(stderr: &str) -> Option<usize> {
    stderr
        .lines()
        .find_map(|l| l.strip_prefix("stats: files-read="))
        .and_then(|n| n.trim().parse().ok())
}

/// Seals `seals` versions, each with a ready change doc and an edit in the scopes it covers.
pub fn history(
    dir: &Path,
    bin: &Path,
    files: usize,
    scopes: usize,
    seals: usize,
) -> Result<(), String> {
    let packages = files.div_ceil(100);
    if !dir.join(".git").exists() {
        git(dir, &["init", "-q", "-b", "main"])?;
    }
    std::fs::write(dir.join(".gitignore"), ".fairlead/\n").map_err(|e| e.to_string())?;
    ok(dir, bin, &["sync", "--yes"])?;
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-q", "-m", "base"])?;
    for k in 0..seals {
        for j in sealed_by(k, seals, scopes) {
            let s = scope_dir(j, scopes, packages);
            let id = format!("d-{:08}", k * 1000 + j + 1);
            let changes = dir.join(&s).join("fairlead/docs/changes");
            std::fs::create_dir_all(&changes).map_err(|e| e.to_string())?;
            std::fs::write(
                changes.join(format!("{id}-seal-{k}.md")),
                format!(
                    "+++\nid = \"{id}\"\nkind = \"change\"\nstatus = \"ready\"\nimpact = \"fix\"\nscope = \"{s}\"\ntitle = \"Seal {k} in {s}\"\ncovers = [\"src/**\"]\ncreated = \"0.1.0\"\n+++\n\n# Seal {k} in {s}\n\nBenchmark history.\n"
                ),
            )
            .map_err(|e| e.to_string())?;
            let f = dir.join(&s).join("src/f-000.ts");
            let old = std::fs::read_to_string(&f).unwrap_or_default();
            std::fs::write(&f, format!("{old}// seal {k}\n")).map_err(|e| e.to_string())?;
        }
        git(dir, &["add", "-A"])?;
        git(dir, &["commit", "-q", "-m", &format!("work {k}")])?;
        ok(dir, bin, &["seal", "--yes"])?;
        git(dir, &["add", "-A"])?;
        git(dir, &["commit", "-q", "-m", &format!("seal {k}")])?;
        println!("sealed {}/{seals}", k + 1);
    }
    Ok(())
}

fn ledger_size(dir: &Path) -> (usize, u64) {
    fn walk(d: &Path, in_ledger: bool, acc: &mut (usize, u64)) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p: PathBuf = e.path();
            let name = e.file_name();
            if name == ".git" || name == ".fairlead" {
                continue;
            }
            if p.is_dir() {
                walk(&p, in_ledger || name == "ledger", acc);
            } else if in_ledger {
                acc.0 += 1;
                acc.1 += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    let mut acc = (0, 0);
    walk(dir, false, &mut acc);
    acc
}

/// Measures the sealed corpus; returns the results as TOML lines. Fails unless a one-file change
/// re-reads exactly that file.
pub fn merkle(dir: &Path, bin: &Path) -> Result<String, String> {
    let check = ["check", "--format", "terse", "--max", "0"];
    let cold = fairlead(dir, bin, &check)?;
    let warm = fairlead(dir, bin, &check)?;
    let f = dir.join("pkg-0000/src/f-050.ts");
    let old = std::fs::read_to_string(&f).map_err(|e| e.to_string())?;
    std::fs::write(&f, format!("{old}// one change\n")).map_err(|e| e.to_string())?;
    let changed = fairlead(dir, bin, &check)?;
    std::fs::write(&f, old).map_err(|e| e.to_string())?;
    for (name, r) in [("cold", &cold), ("warm", &warm), ("changed", &changed)] {
        if r.code > 3 {
            return Err(format!("{name} check exited {}: {}", r.code, r.stderr));
        }
        if r.stdout.contains("HN-G003") || r.stdout.contains("HN-V002") {
            return Err(format!("{name} check: the sealed history does not verify"));
        }
    }
    let read = files_read(&changed.stderr).ok_or("no stats line")?;
    let (n, bytes) = ledger_size(dir);
    let toml = format!(
        "merkle_cold_check_s = {:.3}\nmerkle_warm_check_s = {:.3}\nmerkle_changed_check_s = {:.3}\nmerkle_warm_files_read = {}\nmerkle_changed_files_read = {read}\nledger_files = {n}\nledger_bytes = {bytes}\n",
        cold.secs,
        warm.secs,
        changed.secs,
        files_read(&warm.stderr).unwrap_or(0),
    );
    if read != 1 {
        return Err(format!(
            "a one-file change re-read {read} files, not 1\n{toml}"
        ));
    }
    Ok(toml)
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_ver_016__scopes_spread_over_packages_and_seals() {
        assert_eq!(scope_dir(0, 50, 1000), "pkg-0000");
        assert_eq!(scope_dir(49, 50, 1000), "pkg-0980");
        let all: usize = (0..20).map(|k| sealed_by(k, 20, 50).len()).sum();
        assert_eq!(all, 50, "every scope is sealed once");
        assert_eq!(sealed_by(3, 20, 50), vec![3, 23, 43]);
    }

    #[test]
    fn r_ver_016__reads_the_stats_line() {
        assert_eq!(files_read("x\nstats: files-read=1\n"), Some(1));
        assert_eq!(files_read("nothing"), None);
    }
}
