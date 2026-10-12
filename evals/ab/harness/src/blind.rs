//! The blind reviewer's view of a session (PROTOCOL.md, "how M1 and M2 are judged"): the diff of
//! the working tree against HEAD with every trace of the arm removed: managerial folders,
//! `AGENTS.md` and `CLAUDE.md`, `.gitattributes`, CODEOWNERS lines naming them, and directive
//! comment lines.
use std::path::Path;

use crate::prepare::git;

fn hidden_path(p: &str) -> bool {
    let name = p.rsplit('/').next().unwrap_or(p);
    p.split('/').any(|s| ["target", "node_modules"].contains(&s))
        || p.starts_with("fairlead/")
        || p.contains("/fairlead/")
        || p.starts_with(".fairlead/")
        || ["AGENTS.md", "CLAUDE.md", ".gitattributes"].contains(&name)
}

fn directive(line: &str) -> bool {
    let body = line.get(1..).unwrap_or("").trim_start();
    ["//", "#", "/*", "<!--", "--"]
        .iter()
        .any(|lead| body.strip_prefix(lead).is_some_and(|r| r.trim_start().starts_with("fairlead:")))
        || (line.starts_with(['+', '-']) && line.contains("/fairlead/"))
}

pub fn diff(dir: &Path) -> Result<String, String> {
    git(dir, &["add", "-A", "-N"])?;
    let raw = git(dir, &["diff", "HEAD", "--no-color", "--no-ext-diff", "-U2"])?;
    let mut out = String::new();
    let mut skip = false;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            let path = rest.split(" b/").next().unwrap_or(rest);
            skip = hidden_path(path);
            if !skip {
                out.push_str(&format!("=== {path}\n"));
            }
            continue;
        }
        if skip || line.starts_with("index ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            continue;
        }
        if line.starts_with("new file mode") || line.starts_with("deleted file mode") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if directive(line) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traces_are_hidden() {
        assert!(hidden_path("store/fairlead/control.toml"));
        assert!(hidden_path("AGENTS.md"));
        assert!(!hidden_path("store/src/cart.ts"));
        assert!(directive("+// fairlead: def cmp.x"));
        assert!(directive("-  # fairlead: use a"));
        assert!(!directive("+const x = 1; // not fairlead: prose"));
    }
}
