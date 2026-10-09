//! `cargo xtask golden-bless <name>`: regenerates the expected files of one golden
//! repository by running the same model the test uses. Human review is required before
//! committing the diff. Refuses to run when CI is set.

#[path = "../../tests/testkit/golden_spec.rs"]
mod golden_spec;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Pure refusal rule, unit-tested with a plain boolean.
pub fn refuse_if_ci(ci: bool) -> Result<(), String> {
    if ci {
        Err(
            "golden-bless refuses to run in CI (CI is set); blessing is a local, reviewed act"
                .into(),
        )
    } else {
        Ok(())
    }
}

pub fn bless(root: &Path, name: &str) -> Result<(), String> {
    #[allow(clippy::disallowed_methods)]
    // reason: xtask is developer tooling; CI must refuse a bless
    let ci = std::env::var_os("CI").is_some();
    refuse_if_ci(ci)?;
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["build", "-p", "fairlead", "--locked"])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("cargo build -p fairlead failed".into());
    }
    let exe = if cfg!(windows) {
        "fairlead.exe"
    } else {
        "fairlead"
    };
    let bin: PathBuf = root.join("target/debug").join(exe);
    let dir = root.join("tests/golden").join(name);
    let g = golden_spec::load(&dir)?;
    for case in &g.cases {
        let Some(rel) = &case.stdout else { continue };
        let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
        let repo = scratch.path().join("repo");
        golden_spec::materialise(&dir, &g.golden, &repo).map_err(|e| e.to_string())?;
        let run = golden_spec::run_case(&bin, &repo, case).map_err(|e| e.to_string())?;
        if run.code != case.exit {
            return Err(format!(
                "{name}: exit {} expected {}; not blessed",
                run.code, case.exit
            ));
        }
        std::fs::write(dir.join(rel), &run.stdout).map_err(|e| e.to_string())?;
        println!("blessed {name}/{rel}");
    }
    println!("review with: git diff tests/golden/{name}");
    Ok(())
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn xtask__golden_bless_refuses_in_ci() {
        assert!(refuse_if_ci(true).is_err());
        assert!(refuse_if_ci(false).is_ok());
    }
}
