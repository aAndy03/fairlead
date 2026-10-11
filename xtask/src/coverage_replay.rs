//! `cargo xtask coverage-replay --repo <path> --commits 100 [--out CSV] [--bin PATH]` (0.2 P7-T4,
//! exit criterion 3): replays the last N first-parent commits of a repository. Each commit is
//! checked out (detached, in a temporary worktree) and `check --base <parent>` runs, so the changed
//! set is the commit's own files and coverage is judged against the change docs present at that
//! commit, by the real tool. Each `HN-D006` file goes to a CSV with an empty `label` column for
//! the human (or agent protocol A-2) labelling; `wrong_share` reads the labelled file back.

use std::path::{Path, PathBuf};
use std::process::Command;

const AS_OF: &str = "2026-10-10";

#[derive(Debug)]
pub struct Options {
    pub repo: PathBuf,
    pub commits: usize,
    pub out: PathBuf,
    pub bin: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct Summary {
    pub replayed: usize,
    /// Commits the tool could not judge (no fairlead tree, a refusal, no parent).
    pub skipped: Vec<String>,
    pub changed: usize,
    pub flagged: usize,
}

/// The paths of the `HN-D006` lines of a terse `check` output.
pub fn uncovered(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|l| l.starts_with("HN-D006 "))
        .filter_map(|l| l.split_whitespace().nth(2).map(str::to_owned))
        .collect()
}

/// One CSV field (RFC 4180): quoted when it holds a comma, a quote or a line break.
pub fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// Splits one CSV record (no embedded line breaks).
pub fn record(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The share of labelled rows whose label says the flag was wrong (`wrong`), over all labelled
/// rows; `None` before any row is labelled. Labels: `right` or `wrong`.
pub fn wrong_share(csv: &str) -> Option<f64> {
    let (mut wrong, mut labelled) = (0u32, 0u32);
    for l in csv.lines().skip(1) {
        let r = record(l);
        match r.get(4).map(|s| s.trim()) {
            Some("wrong") => {
                wrong += 1;
                labelled += 1;
            }
            Some("right") => labelled += 1,
            _ => {}
        }
    }
    (labelled > 0).then(|| f64::from(wrong) / f64::from(labelled))
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

pub fn run(root: &Path, o: &Options) -> Result<Summary, String> {
    let bin = crate::merge_sim::fairlead_bin(root, o.bin.as_ref())?;
    let n = o.commits.to_string();
    let log = git(
        &o.repo,
        &["log", "--first-parent", "-n", &n, "--format=%H%x09%P%x09%s"],
    )?;
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let wt = tmp.path().join("wt");
    let wt_s = wt.to_string_lossy().into_owned();
    git(
        &o.repo,
        &["worktree", "add", "-q", "--detach", &wt_s, "HEAD"],
    )?;
    let mut csv = String::from("commit,path,subject,finding,label\n");
    let mut s = Summary::default();
    let result = (|| -> Result<(), String> {
        for line in log.lines() {
            let mut parts = line.splitn(3, '\t');
            let (Some(c), Some(parents), subject) =
                (parts.next(), parts.next(), parts.next().unwrap_or(""))
            else {
                continue;
            };
            let short = c.get(..12).unwrap_or(c);
            let Some(parent) = parents.split_whitespace().next() else {
                s.skipped.push(format!("{short}: root commit"));
                continue;
            };
            git(&wt, &["checkout", "-q", "--detach", "--force", c])?;
            s.changed += git(&wt, &["diff", "--name-only", parent, c])?
                .lines()
                .count();
            let out = Command::new(&bin)
                .current_dir(&wt)
                .args(["check", "--base", parent, "--max", "0", "--as-of", AS_OF])
                .args(["--format", "terse", "--no-cache"])
                .env("NO_COLOR", "1")
                .env_remove("FAIRLEAD_ROOT")
                .output()
                .map_err(|e| format!("fairlead: {e}"))?;
            let code = out.status.code().unwrap_or(-1);
            if code > 3 {
                s.skipped.push(format!("{short}: check exits {code}"));
                continue;
            }
            let text = String::from_utf8_lossy(&out.stdout);
            for p in uncovered(&text) {
                csv.push_str(&format!(
                    "{},{},{},HN-D006,\n",
                    field(short),
                    field(&p),
                    field(subject)
                ));
                s.flagged += 1;
            }
            s.replayed += 1;
        }
        Ok(())
    })();
    let _ = git(&o.repo, &["worktree", "remove", "--force", &wt_s]);
    result?;
    if let Some(parent) = o.out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&o.out, csv).map_err(|e| e.to_string())?;
    Ok(s)
}

#[cfg(test)]
#[allow(non_snake_case, clippy::float_cmp)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn coverage_replay__reads_uncovered_paths_from_terse_output() {
        let out = "fairlead check 0.1.0 edition=1\n\
                   HN-D006 warn src/c.rs changed file is not covered | fix: x\n\
                   HN-V010 warn fairlead/control.toml unavailable\n\
                   HN-D006 warn lib/a b.rs changed file is not covered\n";
        assert_eq!(uncovered(out), vec!["src/c.rs", "lib/a"]);
    }

    #[test]
    fn coverage_replay__csv_fields_round_trip() {
        let row = [
            field("abc"),
            field("a,b.rs"),
            field("say \"hi\""),
            field(""),
        ]
        .join(",");
        assert_eq!(row, "abc,\"a,b.rs\",\"say \"\"hi\"\"\",");
        assert_eq!(record(&row), vec!["abc", "a,b.rs", "say \"hi\"", ""]);
    }

    #[test]
    fn coverage_replay__wrong_share_counts_labelled_rows_only() {
        let csv = "commit,path,subject,finding,label\n\
                   a,x.rs,s,HN-D006,right\n\
                   a,y.rs,s,HN-D006,wrong\n\
                   b,z.rs,s,HN-D006,\n\
                   b,w.rs,s,HN-D006,right\n";
        assert_eq!(wrong_share(csv), Some(1.0 / 3.0));
        assert_eq!(wrong_share("commit,path,subject,finding,label\n"), None);
    }
}
