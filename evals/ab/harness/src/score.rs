//! Scores one finished session from outside (PROTOCOL.md "Measures"): M1 and M2 by hidden tests,
//! M3 by the project's `score.toml` rules, M4 by the changed files, C1 to C6 from the event log,
//! M5 from the tree. The session's repository: HEAD is the state before the session, the working
//! tree the state after it. Nothing here calls fairlead.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::glob::Set;
use crate::prepare::{copy_tree, git};
use crate::Args;

#[derive(Deserialize)]
struct Config {
    tests: String,
    #[serde(default)]
    forbid: Vec<Forbid>,
    #[serde(default)]
    contract: Vec<Contract>,
    #[serde(default)]
    owned: BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct Forbid {
    id: String,
    category: String,
    glob: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    regex: String,
}

#[derive(Deserialize)]
struct Contract {
    id: String,
    category: String,
    files: Vec<String>,
    capture: String,
    /// Captured names that are not contract names (file names such as `notes.rs`).
    #[serde(default)]
    ignore: Vec<String>,
}

#[derive(Serialize)]
struct Violation {
    id: String,
    category: String,
    /// Added by this session (after minus before, never negative).
    new: i64,
    /// Accumulated since the start of the chain (after minus pristine, never negative).
    drift: i64,
    after: i64,
}

#[derive(Serialize)]
struct ContractResult {
    id: String,
    category: String,
    ok: bool,
    /// Names some files have and others lack.
    mismatched: Vec<String>,
}

#[derive(Serialize)]
struct Tests {
    passed: Option<bool>,
    tail: String,
}

/// The tree's files and contents, at a revision or in the working tree.
type Tree = BTreeMap<String, String>;

const SKIP_DIRS: [&str; 5] = [".git", "target", "node_modules", ".fairlead", "dist"];

/// The working tree as git sees it: tracked files plus untracked ones the repository's ignore
/// rules do not exclude (a `Cargo.lock` that `cargo test` writes is not a change by the agent).
fn working_tree(dir: &Path) -> Tree {
    match git(dir, &["ls-files", "-co", "--exclude-standard", "-z"]) {
        Ok(list) => list
            .split('\0')
            .filter(|p| !p.is_empty() && !skipped(p))
            .filter_map(|p| std::fs::read_to_string(dir.join(p)).ok().map(|t| (p.to_owned(), t)))
            .collect(),
        Err(_) => walk_tree(dir),
    }
}

fn walk_tree(dir: &Path) -> Tree {
    fn walk(base: &Path, dir: &Path, out: &mut Tree) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let p = e.path();
            if p.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    walk(base, &p, out);
                }
            } else if let Ok(text) = std::fs::read_to_string(&p) {
                let rel = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                out.insert(rel, text);
            }
        }
    }
    let mut out = Tree::new();
    walk(dir, dir, &mut out);
    out
}

/// Build output and tool folders an agent may have committed (`target/` in the pilot) are not
/// part of the product, on either side of the comparison.
fn skipped(path: &str) -> bool {
    path.split('/').any(|seg| SKIP_DIRS.contains(&seg))
}

fn tree_at(dir: &Path, rev: &str) -> Tree {
    let mut out = Tree::new();
    let Ok(list) = git(dir, &["ls-tree", "-r", "--name-only", rev]) else { return out };
    for path in list.lines().filter(|p| !skipped(p)) {
        if let Ok(text) = git(dir, &["show", &format!("{rev}:{path}")]) {
            out.insert(path.to_owned(), text);
        }
    }
    out
}

/// Test code is not product code: tests must write literal values (pilot finding), so test folders
/// and Rust test modules are outside every forbid rule.
fn product_text<'a>(path: &str, text: &'a str) -> Option<&'a str> {
    if path.starts_with("tests/") || path.contains("/tests/") || path.ends_with(".test.ts") {
        return None;
    }
    Some(if path.ends_with(".rs") { text.split("#[cfg(test)]").next().unwrap_or(text) } else { text })
}

fn count(tree: &Tree, rule: &Forbid, re: &regex::Regex) -> i64 {
    let (inc, exc) = (Set::new(&rule.glob), Set::new(&rule.exclude));
    tree.iter()
        .filter(|(p, _)| inc.matches(p) && !exc.matches(p))
        .filter_map(|(p, t)| product_text(p, t))
        .map(|t| re.find_iter(t).count() as i64)
        .sum()
}

/// The names captured in every file of one side (a glob: `backend/src/**/*.rs`).
fn names(tree: &Tree, side: &str, re: &regex::Regex) -> BTreeSet<String> {
    let set = Set::new(&[side.to_owned()]);
    tree.iter()
        .filter(|(p, _)| set.matches(p))
        // Rust test modules mention made-up commands to test rejection; they are not the contract.
        .map(|(p, t)| if p.ends_with(".rs") { t.split("#[cfg(test)]").next().unwrap_or(t) } else { t.as_str() })
        .flat_map(|t| re.captures_iter(t).filter_map(|c| c.get(1)).map(|m| m.as_str().to_owned()).collect::<Vec<_>>())
        .collect()
}

/// Runs the tests command in a throwaway copy of `dir` with `hidden` test folders laid over it.
fn run_tests(dir: &Path, command: &str, hidden: &[PathBuf]) -> Tests {
    if hidden.is_empty() {
        return Tests { passed: None, tail: "no hidden tests".into() };
    }
    let tmp = std::env::temp_dir().join(format!("ab-tests-{}-{}", std::process::id(), Instant::now().elapsed().as_nanos()));
    let _ = std::fs::remove_dir_all(&tmp);
    if std::fs::create_dir_all(&tmp).is_err() || copy_tree(dir, &tmp).is_err() {
        return Tests { passed: Some(false), tail: "could not copy the repository".into() };
    }
    for h in hidden {
        if copy_tree(h, &tmp).is_err() {
            return Tests { passed: Some(false), tail: format!("could not copy {}", h.display()) };
        }
    }
    let log = tmp.join(".ab-test.log");
    let Ok(file) = std::fs::File::create(&log) else {
        return Tests { passed: Some(false), tail: "no log file".into() };
    };
    let bash = std::env::var("AB_BASH").unwrap_or_else(|_| "bash".into());
    let child = Command::new(bash)
        .args(["-c", command])
        .current_dir(&tmp)
        .stdout(Stdio::from(file.try_clone().expect("log handle")))
        .stderr(Stdio::from(file))
        .spawn();
    let passed = match child {
        Err(e) => {
            let _ = std::fs::write(&log, format!("cannot run bash: {e}"));
            false
        }
        Ok(mut c) => {
            let start = Instant::now();
            loop {
                match c.try_wait() {
                    Ok(Some(status)) => break status.success(),
                    Ok(None) if start.elapsed() > Duration::from_secs(600) => {
                        let _ = c.kill();
                        break false;
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(200)),
                    Err(_) => break false,
                }
            }
        }
    };
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let tail: Vec<&str> = text.lines().rev().take(25).collect();
    let _ = std::fs::remove_dir_all(&tmp);
    Tests { passed: Some(passed), tail: tail.into_iter().rev().collect::<Vec<_>>().join("\n") }
}

/// M5: source files and lines, repeated six-line blocks, and tests.
fn structure(tree: &Tree) -> serde_json::Value {
    let source: Vec<(&String, &String)> = tree
        .iter()
        .filter(|(p, _)| [".ts", ".rs", ".css"].iter().any(|e| p.ends_with(e)))
        .collect();
    let lines: usize = source.iter().map(|(_, t)| t.lines().filter(|l| !l.trim().is_empty()).count()).sum();
    let mut blocks: HashMap<String, usize> = HashMap::new();
    for (_, t) in &source {
        let norm: Vec<&str> = t.lines().map(str::trim).filter(|l| l.len() > 3).collect();
        for w in norm.windows(6) {
            *blocks.entry(w.join("\n")).or_default() += 1;
        }
    }
    let repeated: usize = blocks.values().filter(|n| **n > 1).map(|n| n - 1).sum();
    let tests: usize = source
        .iter()
        .map(|(_, t)| {
            t.lines()
                .filter(|l| {
                    let l = l.trim_start();
                    l.starts_with("#[test]") || l.starts_with("test(") || l.starts_with("it(")
                })
                .count()
        })
        .sum();
    serde_json::json!({ "files": source.len(), "lines": lines, "repeated_blocks": repeated, "tests": tests })
}

pub fn run(args: &Args) -> Result<(), String> {
    let project = args.path("project")?;
    let dir = args.path("dir")?;
    let team = args.need("team")?;
    let task = args.need("task")?;
    let setup = args.get("setup") == Some("true");
    let config: Config = toml::from_str(
        &std::fs::read_to_string(project.join("score.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("score.toml: {e}"))?;

    let pristine = match args.get("pristine") {
        Some(r) => r.to_owned(),
        None => git(&dir, &["rev-list", "--max-parents=0", "HEAD"])?.trim().to_owned(),
    };
    let (after, before, start) = (working_tree(&dir), tree_at(&dir, "HEAD"), tree_at(&dir, &pristine));

    let mut violations = Vec::new();
    for f in &config.forbid {
        let re = regex::Regex::new(&f.regex).map_err(|e| format!("{}: {e}", f.id))?;
        let (a, b, p) = (count(&after, f, &re), count(&before, f, &re), count(&start, f, &re));
        violations.push(Violation {
            id: f.id.clone(),
            category: f.category.clone(),
            new: (a - b).max(0),
            drift: (a - p).max(0),
            after: a,
        });
    }
    let mut contracts = Vec::new();
    for c in &config.contract {
        let re = regex::Regex::new(&c.capture).map_err(|e| format!("{}: {e}", c.id))?;
        let sets: Vec<BTreeSet<String>> = c
            .files
            .iter()
            .map(|f| names(&after, f, &re).into_iter().filter(|n| !c.ignore.contains(n)).collect())
            .collect();
        let all: BTreeSet<String> = sets.iter().flatten().cloned().collect();
        let mismatched: Vec<String> = all.into_iter().filter(|n| !sets.iter().all(|s| s.contains(n))).collect();
        contracts.push(ContractResult { id: c.id.clone(), category: c.category.clone(), ok: mismatched.is_empty(), mismatched });
    }

    // M4 and ownership: the files this session changed.
    let changed: Vec<String> = after
        .keys()
        .chain(before.keys())
        .filter(|p| after.get(*p) != before.get(*p))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let under = |p: &str, prefixes: &[String]| prefixes.iter().any(|x| p == x.trim_end_matches('/') || p.starts_with(x.as_str()));
    let own = config.owned.get(team).cloned().unwrap_or_default();
    let shared = config.owned.get("shared").cloned().unwrap_or_default();
    let others: Vec<String> = config
        .owned
        .iter()
        .filter(|(t, _)| t.as_str() != team && t.as_str() != "shared")
        .flat_map(|(_, v)| v.clone())
        .collect();
    let tests_dir = |p: &str| p.starts_with("tests/") || p.contains("/tests/");
    let foreign: Vec<&String> = if setup {
        Vec::new()
    } else {
        changed.iter().filter(|p| under(p, &others) && !under(p, &own) && !tests_dir(p)).collect()
    };
    let outside: Vec<&String> = changed
        .iter()
        .filter(|p| !under(p, &own) && !under(p, &shared) && !tests_dir(p))
        .collect();
    let ceremony: Vec<&String> = changed
        .iter()
        .filter(|p| p.ends_with(".md") || p.starts_with("fairlead/") || p.contains("/fairlead/") || p.starts_with("docs/"))
        .collect();

    // M1 and M2: the hidden tests of this task, then of every earlier task.
    let hidden_root = project.join("hidden");
    let this = hidden_root.join(task);
    let m1 = run_tests(&dir, &config.tests, if this.exists() { std::slice::from_ref(&this) } else { &[] });
    let earlier: Vec<PathBuf> = args
        .get("earlier")
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| hidden_root.join(s))
        .filter(|p| p.exists())
        .collect();
    let m2 = run_tests(&dir, &config.tests, &earlier);

    let mut by_category: BTreeMap<String, i64> = BTreeMap::new();
    for v in &violations {
        *by_category.entry(v.category.clone()).or_default() += v.new;
    }
    // A setup step builds a skeleton; its contracts are partial by design (pilot finding).
    let contract_breaks = if setup { 0 } else { contracts.iter().filter(|c| !c.ok).count() as i64 };
    if contract_breaks > 0 {
        *by_category.entry("contracts".into()).or_default() += contract_breaks;
    }
    if !foreign.is_empty() {
        *by_category.entry("ownership".into()).or_default() += foreign.len() as i64;
    }
    let m3_new: i64 = by_category.values().sum();
    let m3_drift: i64 = violations.iter().map(|v| v.drift).sum::<i64>() + contract_breaks;

    let events = match args.get("events") {
        Some(f) => serde_json::to_value(crate::events::summarise(&std::fs::read_to_string(f).unwrap_or_default()))
            .map_err(|e| e.to_string())?,
        None => serde_json::Value::Null,
    };
    let out = serde_json::json!({
        "task": task,
        "team": team,
        "m1_task_done": m1.passed,
        "m2_nothing_broken": m2.passed,
        "m3_new": m3_new,
        "m3_drift": m3_drift,
        "m3_by_category": by_category,
        "violations": violations,
        "contracts": contracts,
        "m4_changed": changed.len(),
        "m4_outside_team": outside,
        "ownership_breaks": foreign,
        "ceremony_files": ceremony,
        "m5": structure(&after),
        "cost": events,
        "tests_tail": { "task": m1.tail, "earlier": m2.tail },
    });
    println!("{}", serde_json::to_string(&out).map_err(|e| e.to_string())?);
    Ok(())
}
