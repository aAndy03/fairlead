//! `cargo xtask sarif-validate`: runs the built binary on real goldens and validates
//! every SARIF output against the official 2.1.0 schema (R-CHK-036, E5).

use crate::golden::golden_spec;
use std::path::Path;
use std::process::Command;

const GOLDENS: [&str; 4] = [
    "ts-react",
    "rust-workspace",
    "fullstack-tauri",
    "zero-rules",
];

/// The two `language` patterns of the published 2.1.0 schema have a stray `]`, which a strict
/// validator rejects as not a regex. The file stays byte-identical to the download
/// (SOURCE.toml); the corrected pattern is applied in memory only.
const ERRATUM: &str = r#""^[a-zA-Z]{2}|^[a-zA-Z]{2}-[a-zA-Z]{2}]?$""#;
const CORRECTED: &str = r#""^[a-zA-Z]{2}(-[a-zA-Z]{2})?$""#;

fn compile(schema_text: &str) -> Result<jsonschema::Validator, String> {
    if schema_text.matches(ERRATUM).count() != 2 {
        return Err("the schema no longer holds the known language-pattern erratum twice; review the in-memory fix".into());
    }
    let schema: serde_json::Value = serde_json::from_str(&schema_text.replace(ERRATUM, CORRECTED))
        .map_err(|e| e.to_string())?;
    jsonschema::validator_for(&schema).map_err(|e| e.to_string())
}

pub fn validate(root: &Path) -> Result<(), String> {
    let schema_path = root.join("fixtures/sarif/schema/sarif-schema-2.1.0.json");
    let text = std::fs::read_to_string(&schema_path)
        .map_err(|e| format!("{}: {e}", schema_path.display()))?;
    let validator = compile(&text)?;
    let exe = if cfg!(windows) {
        "fairlead.exe"
    } else {
        "fairlead"
    };
    let bin = root.join("target/debug").join(exe);
    for name in GOLDENS {
        let dir = root.join("tests/golden").join(name);
        let g = golden_spec::load(&dir)?;
        let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
        let repo = scratch.path().join("repo");
        golden_spec::materialise(&dir, &g.golden, &repo).map_err(|e| e.to_string())?;
        let out = Command::new(&bin)
            .current_dir(&repo)
            .env_remove("FAIRLEAD_ROOT")
            .args(["check", "--as-of", "2026-10-09", "--format", "sarif"])
            .output()
            .map_err(|e| format!("{}: {e}", bin.display()))?;
        let doc: serde_json::Value =
            serde_json::from_slice(&out.stdout).map_err(|e| format!("{name}: {e}"))?;
        let errors: Vec<String> = validator
            .iter_errors(&doc)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        if !errors.is_empty() {
            return Err(format!("{name}: SARIF invalid: {}", errors.join("; ")));
        }
        println!("sarif-validate: {name} ok");
    }
    Ok(())
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    #[test]
    fn r_chk_036__the_pinned_schema_compiles_and_rejects_a_bad_version() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fixtures/sarif/schema/sarif-schema-2.1.0.json"
        );
        let v = super::compile(&std::fs::read_to_string(path).unwrap()).unwrap();
        let ok = serde_json::json!({"version": "2.1.0", "runs": []});
        let bad = serde_json::json!({"version": "9.9", "runs": []});
        assert!(v.is_valid(&ok));
        assert!(!v.is_valid(&bad));
    }
}
