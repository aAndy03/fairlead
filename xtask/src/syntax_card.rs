//! `syntax-card-score DIR`: each sample file holds one directive written by a fresh agent
//! session. A sample is invalid when extraction reports an error, drops it, or finds nothing.

use crate::ablation::{SYNTAX_CARD_MAX_INVALID_PCT, SYNTAX_CARD_SAMPLE};
use fairlead_syntax::{ExtractConfig, Severity, extract};
use std::path::Path;

pub fn score(dir: &Path) -> Result<(usize, usize), String> {
    let cfg = ExtractConfig::default();
    let mut total = 0usize;
    let mut invalid = 0usize;
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    for p in files {
        let ext = p
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("txt")
            .to_string();
        let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let e = extract(&bytes, &ext, &cfg);
        total += 1;
        if e.diags.iter().any(|d| d.severity == Severity::Error)
            || e.dropped > 0
            || e.directives.is_empty()
        {
            invalid += 1;
        }
    }
    Ok((total, invalid))
}

/// The E8 gate: exactly the pre-registered sample size, and no more than the allowed percentage.
pub fn gate(total: usize, invalid: usize) -> Result<(), String> {
    if total != SYNTAX_CARD_SAMPLE {
        return Err(format!(
            "sample is {total}, the protocol fixes {SYNTAX_CARD_SAMPLE}"
        ));
    }
    if invalid * 100 > total * SYNTAX_CARD_MAX_INVALID_PCT as usize {
        return Err(format!(
            "{invalid}/{total} invalid exceeds {SYNTAX_CARD_MAX_INVALID_PCT}%"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn e8__syntax_card_gate_takes_at_most_five_percent_invalid() {
        assert!(gate(100, 5).is_ok());
        assert!(gate(100, 6).is_err());
        assert!(gate(99, 0).is_err(), "the sample size is fixed at 100");
    }

    #[test]
    fn e8__score_counts_errors_drops_and_empty_samples_as_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let w = |name: &str, text: &str| std::fs::write(dir.path().join(name), text).unwrap();
        w("01.ts", "// fairlead: use tok.primary\nconst a = 1;\n");
        w(
            "02.ts",
            "// fairlead: frobnicate tok.primary\nconst a = 1;\n",
        );
        w("03.ts", "const a = 1;\n");
        assert_eq!(score(dir.path()).unwrap(), (3, 2));
    }
}
