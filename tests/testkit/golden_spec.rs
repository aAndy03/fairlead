//! Pure golden-repository model. Shared by tests/golden.rs (through testkit) and
//! by xtask golden-bless (through #[path]). Keep it free of other testkit items.
#![allow(dead_code)]
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Deserialize)]
pub struct GoldenFile {
    pub golden: Meta,
    #[serde(default, rename = "case")]
    pub cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub name: String,
    pub platforms: Vec<String>,
    pub needs_git: bool,
    pub line_endings: String,
}

#[derive(Debug, Deserialize)]
pub struct Case {
    pub args: Vec<String>,
    #[serde(default = "dot")]
    pub cwd: String,
    pub exit: i32,
    pub stdout: Option<String>,
    #[serde(default)]
    pub git_status_clean: bool,
    #[serde(default, rename = "edit")]
    pub edits: Vec<Edit>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// A-38: run in the copy the previous case of this golden left behind.
    #[serde(default)]
    pub reuse: bool,
    /// A-38: git commands run in the copy before the edits. P7-T1: a command whose first word
    /// is `!` may fail (a rebase that stops on a conflict).
    #[serde(default)]
    pub git: Vec<Vec<String>>,
    /// P7-T1: after the edits, `git add -A` and a commit with this message.
    #[serde(default)]
    pub commit: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Edit {
    pub path: String,
    /// A-38: optional; empty when the edit only removes.
    #[serde(default)]
    pub append: String,
    /// A-38: delete the path (file or directory) instead of appending.
    #[serde(default)]
    pub remove: bool,
}

pub struct CaseRun {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

fn dot() -> String {
    ".".to_string()
}

pub fn load(dir: &Path) -> Result<GoldenFile, String> {
    let text = std::fs::read_to_string(dir.join("golden.toml"))
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", dir.display()))
}

/// Copies `<dir>/repo` to `dest`. Text files become CRLF when `line_endings = "crlf"`.
/// `dot-git` is renamed to `.git`. When `needs_git` is set and no repository exists,
/// one is initialised and the tree is committed once.
pub fn materialise(dir: &Path, meta: &Meta, dest: &Path) -> io::Result<()> {
    copy_tree(&dir.join("repo"), dest, meta.line_endings == "crlf")?;
    let dot_git = dest.join("dot-git");
    if dot_git.is_dir() {
        std::fs::rename(&dot_git, dest.join(".git"))?;
    }
    if meta.needs_git && !dest.join(".git").exists() {
        git(dest, &["init", "-q", "-b", "main"])?;
        git(dest, &["add", "-A"])?;
        git(dest, &["commit", "-q", "--allow-empty", "-m", "golden"])?;
    }
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path, crlf: bool) -> io::Result<()> {
    std::fs::create_dir_all(dst)?;
    let mut entries: Vec<std::fs::DirEntry> =
        std::fs::read_dir(src)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &to, crlf)?;
        } else {
            let bytes = std::fs::read(e.path())?;
            let data = if crlf { crlf_bytes(&bytes) } else { bytes };
            std::fs::write(&to, data)?;
        }
    }
    Ok(())
}

/// CRLF-converts text. Files with a NUL byte are binary and returned unchanged.
pub fn crlf_bytes(bytes: &[u8]) -> Vec<u8> {
    if bytes.contains(&0) {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 20);
    let mut prev = 0u8;
    for &b in bytes {
        if b == b'\n' && prev != b'\r' {
            out.push(b'\r');
        }
        out.push(b);
        prev = b;
    }
    out
}

fn git(dir: &Path, args: &[&str]) -> io::Result<()> {
    let out = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=fairlead-golden",
            "-c",
            "user.email=golden@invalid",
        ])
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(["-c", "advice.detachedHead=false"])
        .args(args)
        // P7-T1: fixed dates keep commit IDs, and any output naming them, reproducible.
        .env("GIT_AUTHOR_DATE", "2026-10-10T12:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-10-10T12:00:00Z")
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }
}

pub fn git_status(root: &Path) -> io::Result<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Appends the edit's text, creating the file (and its directories) when absent.
pub fn apply_edit(root: &Path, e: &Edit) -> io::Result<()> {
    let p = root.join(&e.path);
    if e.remove {
        return if p.is_dir() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)?;
    f.write_all(e.append.as_bytes())
}

/// Applies the case's edits, then runs `bin` in `root/cwd` with NO_COLOR set.
pub fn run_case(bin: &Path, root: &Path, case: &Case) -> io::Result<CaseRun> {
    for g in &case.git {
        let args: Vec<&str> = g.iter().map(String::as_str).collect();
        match args.split_first() {
            Some((&"!", rest)) => {
                let _ = git(root, rest);
            }
            _ => git(root, &args)?,
        }
    }
    for e in &case.edits {
        apply_edit(root, e)?;
    }
    if let Some(m) = &case.commit {
        git(root, &["add", "-A"])?;
        git(root, &["commit", "-q", "--allow-empty", "-m", m])?;
    }
    let out = Command::new(bin)
        .current_dir(root.join(&case.cwd))
        .args(&case.args)
        .env("NO_COLOR", "1")
        .env_remove("FAIRLEAD_ROOT")
        .envs(&case.env)
        .output()?;
    Ok(CaseRun {
        code: out.status.code().unwrap_or(-1),
        stdout: out.stdout,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// The first line that differs, for a readable failure message.
pub fn first_difference(want: &[u8], got: &[u8]) -> String {
    let (w, g) = (String::from_utf8_lossy(want), String::from_utf8_lossy(got));
    for (i, (a, b)) in w.lines().zip(g.lines()).enumerate() {
        if a != b {
            return format!("line {}: expected {a:?}, got {b:?}", i + 1);
        }
    }
    format!(
        "line count differs: expected {}, got {}",
        w.lines().count(),
        g.lines().count()
    )
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use std::path::PathBuf;

    const GOLDEN_TOML: &str = r#"
[golden]
name = "unit"
platforms = ["linux", "windows", "macos"]
needs_git = true
line_endings = "crlf"

[[case]]
args = ["check"]
exit = 0
git_status_clean = true

[[case.edit]]
path = "fairlead/x.txt"
append = "y\n"
"#;

    fn golden_dir(root: &Path) -> PathBuf {
        let dir = root.join("unit");
        std::fs::create_dir_all(dir.join("repo/fairlead")).expect("mkdir");
        std::fs::write(dir.join("golden.toml"), GOLDEN_TOML).expect("golden.toml");
        std::fs::write(dir.join("repo/a.txt"), "a\nb\n").expect("a.txt");
        std::fs::write(dir.join("repo/fairlead/control.toml"), "x\n").expect("control");
        dir
    }

    #[test]
    fn golden_spec__load_reads_cases_and_edits() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let g = load(&golden_dir(tmp.path())).expect("parses");
        assert_eq!(g.golden.name, "unit");
        assert_eq!(g.cases.len(), 1);
        assert_eq!(g.cases[0].cwd, ".");
        assert_eq!(g.cases[0].edits[0].path, "fairlead/x.txt");
    }

    #[test]
    fn golden_spec__materialise_applies_crlf_and_initialises_git() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = golden_dir(tmp.path());
        let g = load(&dir).expect("parses");
        let dest = tmp.path().join("copy");
        materialise(&dir, &g.golden, &dest).expect("materialise");
        assert_eq!(
            std::fs::read(dest.join("a.txt")).expect("read"),
            b"a\r\nb\r\n".to_vec()
        );
        assert!(
            dest.join(".git").exists(),
            "needs_git must initialise a repository"
        );
        assert_eq!(
            git_status(&dest).expect("status"),
            "",
            "fresh copy must be clean"
        );
    }
}
