#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! SARIF mapping of R-CHK-036: ruleId, level, partialFingerprints.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{golden_checkout, run_fairlead};

#[test]
fn r_chk_036__sarif_results_map_code_level_and_fingerprint() {
    let (_s, root) = golden_checkout("ts-react", false);
    let run = run_fairlead(
        &root,
        &["check", "--as-of", "2026-10-09", "--format", "sarif"],
        &[],
    );
    assert_eq!(run.code, 1, "ts-react has enforce errors: {}", run.stderr);
    let doc: serde_json::Value = serde_json::from_str(&run.stdout).expect("SARIF is JSON");
    assert_eq!(doc["version"], "2.1.0");
    assert_eq!(doc["runs"][0]["tool"]["driver"]["name"], "fairlead");
    let results = doc["runs"][0]["results"].as_array().expect("results array");
    assert_eq!(results.len(), 2, "two findings in ts-react");
    for r in results {
        let rule = r["ruleId"].as_str().expect("ruleId");
        assert!(
            rule.starts_with("HN-"),
            "ruleId is the diagnostic code: {rule}"
        );
        let level = r["level"].as_str().expect("level");
        assert!(
            matches!(level, "error" | "warning" | "note"),
            "level {level}"
        );
        let fp = r["partialFingerprints"]["fingerprint/v1"]
            .as_str()
            .expect("fingerprint key fingerprint/v1 (A-29)");
        assert_eq!(fp.len(), 12, "12 base32 characters (R-CHK-031)");
    }
}
