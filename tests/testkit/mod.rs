//! Shared helpers for every fairlead test target. Included with
//! `#[path = "testkit/mod.rs"] mod testkit;`. Each target uses a subset,
//! so dead-code warnings are allowed here.
#![allow(dead_code)]

pub mod golden_spec;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The only as-of date used by tests (time is never read in tests).
pub const NOW: &str = "2026-10-09";

/// The binary built for this package. Tests never invoke cargo.
pub fn fairlead_bin() -> &'static str {
    env!("CARGO_BIN_EXE_fairlead")
}

/// The terse check invocation used by most golden cases.
pub const CHECK_TERSE: &[&str] = &["check", "--as-of", NOW, "--format", "terse"];

pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Default)]
pub struct RepoBuilder {
    files: BTreeMap<String, Vec<u8>>,
    crlf: bool,
    git: bool,
}

impl RepoBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a UTF-8 text file at a repository-relative, forward-slash path.
    pub fn file(mut self, path: &str, text: &str) -> Self {
        self.files
            .insert(path.to_string(), text.as_bytes().to_vec());
        self
    }

    /// Adds a file with raw bytes (non-UTF-8 fixtures).
    pub fn bytes(mut self, path: &str, bytes: &[u8]) -> Self {
        self.files.insert(path.to_string(), bytes.to_vec());
        self
    }

    /// Converts every text file to CRLF when built (binary files are left alone).
    pub fn crlf(mut self, on: bool) -> Self {
        self.crlf = on;
        self
    }

    /// Runs `git init`, stages everything and commits once, so `git status` starts clean.
    pub fn git_init(mut self, on: bool) -> Self {
        self.git = on;
        self
    }

    pub fn build(self) -> TempRepo {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel, bytes) in &self.files {
            let out = dir.path().join(rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            let data = if self.crlf {
                to_crlf(bytes)
            } else {
                bytes.clone()
            };
            std::fs::write(&out, data).expect("write fixture file");
        }
        let repo = TempRepo { dir };
        if self.git {
            repo.git(&["init", "-q"]);
            repo.git(&["add", "-A"]);
            repo.git_commit("init");
        }
        repo
    }
}

/// CRLF-converts text. Files with a NUL byte are binary and returned unchanged.
pub fn to_crlf(bytes: &[u8]) -> Vec<u8> {
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

pub struct TempRepo {
    pub dir: tempfile::TempDir,
}

impl TempRepo {
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Runs fairlead in the repository root with NO_COLOR set and FAIRLEAD_ROOT removed.
    pub fn run(&self, args: &[&str]) -> Run {
        self.run_in(self.path(), args, &[])
    }

    pub fn run_in(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
        let mut cmd = Command::new(fairlead_bin());
        cmd.current_dir(cwd)
            .args(args)
            .env("NO_COLOR", "1")
            .env_remove("FAIRLEAD_ROOT");
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = cmd.output().expect("spawn fairlead");
        Run {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    /// Runs git (fixtures only) with a fixed identity. Returns stdout.
    pub fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(self.path())
            .args([
                "-c",
                "user.name=fairlead-test",
                "-c",
                "user.email=fairlead-test@invalid",
            ])
            .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    pub fn git_commit(&self, msg: &str) {
        self.git(&["commit", "-q", "--allow-empty", "-m", msg]);
    }

    /// Empty string means a clean working tree.
    pub fn git_status(&self) -> String {
        self.git(&["status", "--porcelain"])
    }

    /// Every file under the repository as forward-slash path to bytes,
    /// excluding `.fairlead/` (the disposable cache) and `.git/`.
    pub fn tree_snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        collect(self.path(), self.path(), &mut out);
        out
    }
}

fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("entry").path())
        .collect();
    entries.sort();
    for p in entries {
        let rel = p.strip_prefix(root).expect("under root");
        let key = rel.to_string_lossy().replace('\\', "/");
        if key == ".fairlead" || key == ".git" {
            continue;
        }
        if p.is_dir() {
            collect(root, &p, out);
        } else {
            out.insert(key, std::fs::read(&p).expect("read"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testkit__crlf_rewrites_text_only() {
        assert_eq!(to_crlf(b"a\nb\n"), b"a\r\nb\r\n".to_vec());
        assert_eq!(to_crlf(b"a\r\nb\n"), b"a\r\nb\r\n".to_vec());
        assert_eq!(to_crlf(b"\x00\x01\n"), b"\x00\x01\n".to_vec());
    }

    #[test]
    fn testkit__snapshot_excludes_cache_and_git() {
        let repo = RepoBuilder::new()
            .file("fairlead/control.toml", "x")
            .file(".fairlead/files.json", "{}")
            .build();
        let snap = repo.tree_snapshot();
        assert!(snap.contains_key("fairlead/control.toml"));
        assert!(!snap.keys().any(|k| k.starts_with(".fairlead/")));
    }
}
