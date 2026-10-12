//! One arm's starting repository (PROTOCOL.md, "Arms" and the pre-run revision).
//!
//! - maintenance, arm C: the project as adopted, then `fairlead init --yes` (the agent block).
//! - maintenance, arms A and B: every managerial folder and directive comment removed; B adds the
//!   project's hand-written `AGENTS.md`.
//! - from scratch: an empty folder with `SPEC.md`; B adds the general `AGENTS.md`; C runs
//!   `fairlead init --yes` in it.
//!
//! Every repository is a fresh git repository with one commit, so a session's changes are a diff.
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Args;

const SKIP: [&str; 4] = ["target", "node_modules", ".fairlead", "Cargo.lock"];
const TEXT: [&str; 9] = ["ts", "rs", "toml", "md", "css", "json", "js", "txt", ""];

pub fn run(args: &Args) -> Result<(), String> {
    let project = args.path("project")?;
    let track = args.need("track")?;
    let arm = args.need("arm")?;
    let out = args.path("out")?;
    let as_of = args.get("as-of").unwrap_or("2026-10-12");
    if out.exists() && std::fs::read_dir(&out).map_err(err)?.next().is_some() {
        return Err(format!("{} is not empty", out.display()));
    }
    std::fs::create_dir_all(&out).map_err(err)?;
    match track {
        "maint" => {
            copy_tree(&project.join("repo"), &out)?;
            if arm != "C" {
                strip_fairlead(&out)?;
            }
            if arm == "B" {
                std::fs::copy(project.join("arm-b-AGENTS.md"), out.join("AGENTS.md")).map_err(err)?;
            }
        }
        "scratch" => {
            std::fs::copy(project.join("spec.md"), out.join("SPEC.md")).map_err(err)?;
            if arm == "B" {
                let general = project.join("../../scratch-arm-b-AGENTS.md");
                std::fs::copy(general, out.join("AGENTS.md")).map_err(err)?;
            }
        }
        other => return Err(format!("unknown track {other}")),
    }
    git(&out, &["init", "-q"])?;
    if arm == "C" {
        let bin = args.path("fairlead")?;
        let o = Command::new(&bin)
            .args(["init", "--yes", "--as-of", as_of])
            .current_dir(&out)
            .output()
            .map_err(|e| format!("cannot run {}: {e}", bin.display()))?;
        if !o.status.success() {
            return Err(format!(
                "fairlead init failed: {}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ));
        }
    }
    git(&out, &["add", "-A"])?;
    git(&out, &["-c", "user.name=ab", "-c", "user.email=ab@example.invalid", "commit", "-q", "--allow-empty", "-m", "start"])?;
    let commit = git(&out, &["rev-parse", "HEAD"])?;
    println!("{}", serde_json::json!({ "dir": out, "commit": commit.trim() }));
    Ok(())
}

fn err(e: std::io::Error) -> String {
    e.to_string()
}

pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let o = Command::new("git")
        .args(["-c", "core.autocrlf=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !o.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr)));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

pub fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(from).map_err(err)? {
        let entry = entry.map_err(err)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SKIP.contains(&name.as_ref()) || name == ".git" {
            continue;
        }
        let dst = to.join(name.as_ref());
        if entry.file_type().map_err(err)?.is_dir() {
            std::fs::create_dir_all(&dst).map_err(err)?;
            copy_tree(&entry.path(), &dst)?;
        } else {
            std::fs::copy(entry.path(), &dst).map_err(err)?;
        }
    }
    Ok(())
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(err)? {
        let entry = entry.map_err(err)?;
        if entry.file_type().map_err(err)?.is_dir() {
            files(&entry.path(), out)?;
        } else {
            out.push(entry.path());
        }
    }
    Ok(())
}

/// A line that is only a fairlead directive comment.
fn is_directive(line: &str) -> bool {
    let t = line.trim_start();
    ["//", "#", "/*", "<!--", "--"]
        .iter()
        .any(|lead| t.strip_prefix(lead).is_some_and(|r| r.trim_start().starts_with("fairlead:")))
}

/// Arms A and B: no managerial folder, no directive comment, no CODEOWNERS line for them.
fn strip_fairlead(dir: &Path) -> Result<(), String> {
    remove_managerial(dir)?;
    let mut all = Vec::new();
    files(dir, &mut all)?;
    for f in all {
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("");
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !TEXT.contains(&ext) && name != "CODEOWNERS" {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let kept: Vec<&str> = text
            .split_inclusive('\n')
            .filter(|l| {
                !is_directive(l)
                    && !(name == "CODEOWNERS" && l.contains("/fairlead/"))
                    && !(name == ".gitignore" && l.contains("fairlead"))
            })
            .collect();
        let new = kept.concat();
        if new != text {
            std::fs::write(&f, new).map_err(err)?;
        }
    }
    Ok(())
}

fn remove_managerial(dir: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(err)? {
        let entry = entry.map_err(err)?;
        if !entry.file_type().map_err(err)?.is_dir() {
            continue;
        }
        let p = entry.path();
        if entry.file_name() == "fairlead" && p.join("control.toml").exists() {
            std::fs::remove_dir_all(&p).map_err(err)?;
        } else {
            remove_managerial(&p)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directive_lines() {
        assert!(is_directive("// fairlead: def cmp.x\n"));
        assert!(is_directive("  # fairlead: use a\n"));
        assert!(!is_directive("let s = \"// fairlead: x\";\n"));
        assert!(!is_directive("// a fairlead: comment that is prose\n"));
    }
}
