//! `cargo xtask query-recall --set <file> [--repo <dir>] [--bin <fairlead>]` (R-CTX-063, agent
//! protocol B-2): runs each labelled task query through `fairlead context` and reports recall at 3,
//! the share of queries whose expected scope is among the three `matched:` scopes.
//!
//! The set is TOML: `[[query]]` tables with `text` and `expect` (one scope ID or a list).
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(serde::Deserialize)]
struct Set {
    query: Vec<Query>,
}

#[derive(serde::Deserialize)]
struct Query {
    text: String,
    expect: Expect,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Expect {
    One(String),
    Many(Vec<String>),
}

impl Expect {
    fn scopes(&self) -> Vec<&str> {
        match self {
            Expect::One(s) => vec![s.as_str()],
            Expect::Many(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

/// The scopes of a brief's `matched:` line, best first; empty for `no match`.
pub fn matched(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .find_map(|l| l.trim_start_matches("note: ").strip_prefix("matched: "))
        .map(|rest| {
            rest.split(';')
                .next()
                .unwrap_or("")
                .split(',')
                .filter_map(|p| p.split_whitespace().next().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Recall at 3: (hits, total) over `(expected, matched)` pairs.
pub fn recall(results: &[(Vec<String>, Vec<String>)]) -> (usize, usize) {
    let hits = results
        .iter()
        .filter(|(want, got)| got.iter().take(3).any(|g| want.contains(g)))
        .count();
    (hits, results.len())
}

pub struct Options {
    pub set: PathBuf,
    pub repo: PathBuf,
    pub bin: Option<PathBuf>,
}

/// Runs the set; returns the report lines and the recall.
pub fn run(root: &Path, o: &Options) -> Result<(Vec<String>, f64), String> {
    let text = std::fs::read_to_string(&o.set).map_err(|e| format!("{}: {e}", o.set.display()))?;
    let set: Set = toml::from_str(&text).map_err(|e| format!("{}: {e}", o.set.display()))?;
    let bin = crate::merge_sim::fairlead_bin(root, o.bin.as_ref())?;
    let mut results = Vec::new();
    let mut lines = Vec::new();
    for q in &set.query {
        let out = Command::new(&bin)
            .args(["context", &q.text, "--format", "terse", "--budget", "0"])
            .current_dir(&o.repo)
            .output()
            .map_err(|e| format!("cannot run {}: {e}", bin.display()))?;
        let got = matched(&String::from_utf8_lossy(&out.stdout));
        let want: Vec<String> = q.expect.scopes().iter().map(|s| (*s).to_owned()).collect();
        let ok = got.iter().take(3).any(|g| want.contains(g));
        lines.push(format!(
            "{} {:?} expect={} matched={}",
            if ok { "hit " } else { "MISS" },
            q.text,
            want.join("|"),
            got.join(",")
        ));
        results.push((want, got));
    }
    let (h, n) = recall(&results);
    let r = if n == 0 { 0.0 } else { h as f64 / n as f64 };
    lines.push(format!(
        "recall@3 {h}/{n} = {:.0}%{}",
        r * 100.0,
        if r < 0.7 {
            " (under 70%: apply the kw= remedy of R-CTX-063)"
        } else {
            ""
        }
    ));
    Ok((lines, r))
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_ctx_063__matched_line_is_parsed() {
        let out = "@root d=x ed=1\nnote: matched: web 4.21, root 1.30, api 0.50; pin with --scope\nhit item cmp.a 1.00 \"a\"\n";
        assert_eq!(matched(out), ["web", "root", "api"]);
        assert!(matched("@root d=x\nnote: no match: zebra\n").is_empty());
    }

    #[test]
    fn r_ctx_063__recall_at_three() {
        let r = vec![
            (
                vec!["web".to_owned()],
                vec!["api".to_owned(), "web".to_owned()],
            ),
            (vec!["api".to_owned()], vec!["web".to_owned()]),
            (
                vec!["root".to_owned(), "web".to_owned()],
                vec!["a".into(), "b".into(), "c".into(), "web".into()],
            ),
        ];
        assert_eq!(recall(&r), (1, 3), "a hit counts only within the top three");
        let set: Set = toml::from_str("[[query]]\ntext = \"t\"\nexpect = \"web\"\n[[query]]\ntext = \"u\"\nexpect = [\"a\", \"b\"]\n").unwrap();
        assert_eq!(set.query[1].expect.scopes(), ["a", "b"]);
    }
}
