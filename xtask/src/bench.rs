//! `bench-corpus generate` writes a deterministic corpus. `bench-corpus check` gates the
//! E6 limits (from ablation.rs, which the protocol test binds) and a 25% regression.

use crate::ablation::{
    E6_CHANGED_MAX_S, E6_COLD_CHECK_MAX_S, E6_PEAK_RSS_MAX_MB, E6_WARM_CHECK_MAX_S,
};
use std::path::Path;

const ROOT: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n[settings]\nfolder = \"fairlead\"\nfairlead-version = \"0.1.0\"\nowners = [\"@andy\"]\n\n[kinds.tok]\ntitle = \"design token\"\nshape = \"doc\"\n";
const TOKENS: &str = "+++\ntitle = \"Tokens\"\ndirectives = [\n  'def tok.color.primary value=\"#2563eb\" applies=color sum=\"brand\"',\n]\n+++\n";

pub fn generate(out: &Path, files: usize) -> std::io::Result<()> {
    std::fs::create_dir_all(out.join("fairlead/contract"))?;
    std::fs::write(out.join("fairlead/control.toml"), ROOT)?;
    std::fs::write(out.join("fairlead/contract/tokens.md"), TOKENS)?;
    for i in 0..files {
        let path = out.join(format!("pkg-{:04}/src/f-{:03}.ts", i / 100, i % 100));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = if i % 50 == 0 {
            "// fairlead: use tok.color.primary\nexport const v = 1;\n"
        } else {
            "export const v = 1;\n"
        };
        std::fs::write(path, body)?;
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct Measured {
    cold_s: f64,
    warm_s: f64,
    changed_s: f64,
    files_from_s: f64,
    peak_rss_mb: f64,
}

#[derive(serde::Deserialize, Default)]
struct Baseline {
    cold_s: Option<f64>,
    warm_s: Option<f64>,
    changed_s: Option<f64>,
    peak_rss_mb: Option<f64>,
}

pub fn check(results: &str, baseline: &str) -> Result<(), String> {
    let m: Measured = toml::from_str(results).map_err(|e| e.to_string())?;
    let b: Baseline = toml::from_str(baseline).map_err(|e| e.to_string())?;
    let gates = [
        ("cold check", m.cold_s, E6_COLD_CHECK_MAX_S as f64, b.cold_s),
        ("warm check", m.warm_s, E6_WARM_CHECK_MAX_S as f64, b.warm_s),
        (
            "--changed",
            m.changed_s,
            E6_CHANGED_MAX_S as f64,
            b.changed_s,
        ),
        (
            "peak RSS MB",
            m.peak_rss_mb,
            E6_PEAK_RSS_MAX_MB as f64,
            b.peak_rss_mb,
        ),
    ];
    let mut failures = Vec::new();
    for (name, got, max, base) in gates {
        if got > max {
            failures.push(format!("{name}: {got} exceeds the E6 limit {max}"));
        }
        if let Some(base) = base
            && got > base * 1.25
        {
            failures.push(format!(
                "{name}: {got} is more than 25% above the baseline {base}"
            ));
        }
    }
    if m.files_from_s > 1.0 {
        println!(
            "report only (R-CHK-016 goal): --files-from took {} s",
            m.files_from_s
        );
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_arch_130__gate_rejects_a_slow_cold_check() {
        let slow = "cold_s = 61.0\nwarm_s = 1.0\nchanged_s = 0.1\nfiles_from_s = 0.5\npeak_rss_mb = 100.0\n";
        assert!(check(slow, "").is_err());
        let fast = "cold_s = 30.0\nwarm_s = 1.0\nchanged_s = 0.1\nfiles_from_s = 0.5\npeak_rss_mb = 100.0\n";
        assert!(check(fast, "").is_ok());
    }
}
