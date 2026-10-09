//! `sarif` renderer: the SARIF 2.1.0 subset of R-CHK-036, hand-built with serde (R-ARCH-054).

use core::fmt::Write as _;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::sanitize::{FIX_CAP, INTENT_CAP, LINE_CAP, clean_data};
use super::{OutputFormat, RenderOptions, Renderer, finish, machine_slice, stage_word, state_word};
use crate::check::codes::spec;
use crate::check::{CheckReport, Finding, FindingState};
use crate::diag::{DiagCode, Severity};

pub struct SarifRenderer;

const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
/// A-29: no product name in the key, so the D-47 rename stays mechanical.
const FINGERPRINT_KEY: &str = "fingerprint/v1";

#[derive(Serialize)]
struct Sarif {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<Run>,
}

#[derive(Serialize)]
struct Run {
    tool: Tool,
    results: Vec<SResult>,
    properties: RunProps,
}

#[derive(Serialize)]
struct Tool {
    driver: Driver,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Driver {
    name: &'static str,
    version: String,
    semantic_version: String,
    rules: Vec<RuleDesc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuleDesc {
    id: String,
    name: String,
    short_description: Text,
    default_configuration: Level,
}

#[derive(Serialize)]
struct Level {
    level: &'static str,
}

#[derive(Serialize)]
struct Text {
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SResult {
    rule_id: String,
    level: &'static str,
    message: Text,
    locations: Vec<Loc>,
    partial_fingerprints: BTreeMap<&'static str, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    suppressions: Vec<Suppression>,
    properties: ResultProps,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Loc {
    physical_location: Physical,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Physical {
    artifact_location: Artifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<Region>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    uri: String,
    uri_base_id: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Region {
    start_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_column: Option<u32>,
}

#[derive(Serialize)]
struct Suppression {
    kind: &'static str,
}

#[derive(Serialize)]
struct ResultProps {
    stage: &'static str,
    state: &'static str,
    scope: String,
    occurrence: u32,
    fix: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    intent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    related: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunProps {
    fairlead: String,
    edition: u32,
    as_of: String,
    partial: bool,
    omitted: usize,
    unknown_scopes: Vec<String>,
}

fn level(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warn => "warning",
        Severity::Info => "note",
    }
}

/// Percent-encode a repo-relative path for a SARIF URI reference; `/` and unreserved characters stay.
pub(crate) fn encode_uri(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(char::from(b));
            }
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

fn rule_desc(code: DiagCode) -> RuleDesc {
    match spec(code) {
        Some(s) => RuleDesc {
            id: code.0.to_string(),
            name: s.mnemonic.to_string(),
            short_description: Text {
                text: s.title.to_string(),
            },
            default_configuration: Level {
                level: level(s.default_severity),
            },
        },
        None => RuleDesc {
            id: code.0.to_string(),
            name: code.0.to_string(),
            short_description: Text {
                text: code.0.to_string(),
            },
            default_configuration: Level { level: "warning" },
        },
    }
}

fn result(f: &Finding) -> SResult {
    let loc = &f.location;
    let region = (loc.line > 0).then(|| Region {
        start_line: loc.line,
        start_column: (loc.col > 0).then_some(loc.col),
    });
    let suppressions = match f.state {
        FindingState::Baselined => vec![Suppression { kind: "external" }],
        FindingState::Waived => vec![Suppression { kind: "inSource" }],
        _ => Vec::new(),
    };
    SResult {
        rule_id: f.code.0.to_string(),
        level: level(f.severity),
        message: Text {
            text: clean_data(&f.message, LINE_CAP),
        },
        locations: vec![Loc {
            physical_location: Physical {
                artifact_location: Artifact {
                    uri: encode_uri(&clean_data(loc.path.as_str(), LINE_CAP)),
                    uri_base_id: "%SRCROOT%",
                },
                region,
            },
        }],
        partial_fingerprints: BTreeMap::from([(
            FINGERPRINT_KEY,
            f.fingerprint.as_str().to_string(),
        )]),
        suppressions,
        properties: ResultProps {
            stage: stage_word(&f.stage),
            state: state_word(&f.state),
            scope: f.scope.0.as_str().to_string(),
            occurrence: f.occurrence,
            fix: clean_data(&f.fix, FIX_CAP),
            intent: f.intent.as_deref().map(|i| clean_data(i, INTENT_CAP)),
            rule: f.rule.as_ref().map(|r| format!("rule.{}", r.0.as_str())),
            related: f.related.as_deref().map(|r| clean_data(r, LINE_CAP)),
        },
    }
}

impl Renderer for SarifRenderer {
    fn format(&self) -> OutputFormat {
        OutputFormat::Sarif
    }

    fn check(&self, report: &CheckReport, opts: &RenderOptions) -> String {
        let (slice, omitted) = machine_slice(report, opts);
        let codes: BTreeSet<DiagCode> = slice.iter().map(|f| f.code).collect();
        let version = clean_data(&report.header.fairlead_version, 40);
        let doc = Sarif {
            schema: SCHEMA,
            version: "2.1.0",
            runs: vec![Run {
                tool: Tool {
                    driver: Driver {
                        name: "fairlead",
                        version: version.clone(),
                        semantic_version: version.clone(),
                        rules: codes.into_iter().map(rule_desc).collect(),
                    },
                },
                results: slice.iter().map(|f| result(f)).collect(),
                properties: RunProps {
                    fairlead: version,
                    edition: report.header.edition.0,
                    as_of: report.header.as_of.0.to_string(),
                    partial: report.partial,
                    omitted,
                    unknown_scopes: report
                        .unknown_scopes
                        .iter()
                        .map(|u| u.0.as_str().to_string())
                        .collect(),
                },
            }],
        };
        finish(serde_json::to_string_pretty(&doc).unwrap_or_default())
    }

    fn context(&self, _out: &crate::context::ContextOutput) -> Option<String> {
        None
    }

    fn explain(&self, _out: &crate::explain::ExplainOutput) -> Option<String> {
        None
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::MaxFindings;
    use crate::diag::codes::{HN_R001, HN_T001, HN_X004};
    use crate::render::testkit::{finding, report, rule, with};

    fn opts() -> RenderOptions {
        RenderOptions {
            format: OutputFormat::Sarif,
            for_agent: false,
            max: MaxFindings::Default,
            show_observe: false,
            color: false,
            quiet: false,
        }
    }
    fn parse(out: &str) -> serde_json::Value {
        serde_json::from_str(out).unwrap()
    }

    /// Required properties of the SARIF 2.1.0 subset we emit; the full schema is validated in P9.
    fn assert_sarif_required(v: &serde_json::Value) {
        assert_eq!(v["version"], "2.1.0");
        assert!(v["$schema"].as_str().unwrap().contains("sarif"));
        assert_eq!(v["runs"].as_array().unwrap().len(), 1);
        let run = &v["runs"][0];
        assert!(!run["tool"]["driver"]["name"].as_str().unwrap().is_empty());
        let rule_ids: BTreeSet<&str> = run["tool"]["driver"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        for r in run["results"].as_array().unwrap() {
            assert!(
                rule_ids.contains(r["ruleId"].as_str().unwrap()),
                "result without a rule"
            );
            assert!(["error", "warning", "note"].contains(&r["level"].as_str().unwrap()));
            assert!(r["message"]["text"].as_str().is_some());
            let phys = &r["locations"][0]["physicalLocation"];
            let uri = phys["artifactLocation"]["uri"].as_str().unwrap();
            assert!(
                !uri.starts_with('/')
                    && !uri.contains("://")
                    && !uri.contains('\\')
                    && !uri.contains(' '),
                "{uri}"
            );
            if let Some(region) = phys.get("region") {
                assert!(region["startLine"].as_u64().unwrap() >= 1);
            }
            let fps = r["partialFingerprints"].as_object().unwrap();
            assert!(fps.contains_key(FINGERPRINT_KEY));
            assert!(fps.values().all(|f| f.as_str().is_some()));
            for s in r
                .get("suppressions")
                .and_then(|s| s.as_array())
                .into_iter()
                .flatten()
            {
                assert!(["inSource", "external"].contains(&s["kind"].as_str().unwrap()));
            }
        }
    }

    fn sample() -> CheckReport {
        report(vec![
            with(
                finding(
                    HN_T001,
                    Severity::Error,
                    "dir with space/a b.tsx",
                    44,
                    18,
                    "literal #2563eb",
                ),
                |f| {
                    f.rule = Some(rule("no-raw-hex"));
                    f.intent = Some("colours come from tokens".to_string());
                },
            ),
            with(
                finding(HN_R001, Severity::Warn, "src/b.rs", 7, 0, "dangling"),
                |f| f.state = FindingState::Baselined,
            ),
            with(
                finding(
                    HN_X004,
                    Severity::Info,
                    "fairlead/control.toml",
                    0,
                    0,
                    "stale ovr",
                ),
                |f| f.state = FindingState::Waived,
            ),
        ])
    }

    #[test]
    fn r_chk_036__sarif_subset_has_the_required_properties() {
        let out = SarifRenderer.check(&sample(), &opts());
        assert_sarif_required(&parse(&out));
        assert!(out.ends_with("}\n") && !out.contains('\r'));
    }

    #[test]
    fn r_chk_036__levels_suppressions_regions_and_uris() {
        let v = parse(&SarifRenderer.check(&sample(), &opts()));
        let results = v["runs"][0]["results"].as_array().unwrap();
        assert_eq!(
            results.len(),
            3,
            "baselined and waived findings are listed, suppressed"
        );
        let by_rule = |id: &str| results.iter().find(|r| r["ruleId"] == id).unwrap();
        let t = by_rule("HN-T001");
        let phys = |r: &serde_json::Value| r["locations"][0]["physicalLocation"].clone();
        assert_eq!(t["level"], "error");
        assert_eq!(
            phys(t)["artifactLocation"]["uri"],
            "dir%20with%20space/a%20b.tsx"
        );
        assert_eq!(phys(t)["region"]["startColumn"], 18);
        assert!(t.get("suppressions").is_none());
        assert_eq!(t["properties"]["intent"], "colours come from tokens");
        assert_eq!(t["properties"]["rule"], "rule.no-raw-hex");
        let b = by_rule("HN-R001");
        assert_eq!(b["level"], "warning");
        assert_eq!(b["suppressions"][0]["kind"], "external");
        assert!(phys(b)["region"].get("startColumn").is_none());
        let w = by_rule("HN-X004");
        assert_eq!(w["level"], "note");
        assert_eq!(w["suppressions"][0]["kind"], "inSource");
        assert!(
            phys(w).get("region").is_none(),
            "line 0 means the whole file"
        );
    }

    #[test]
    fn r_chk_036__rule_descriptors_are_unique_sorted_and_cover_every_result() {
        let v = parse(&SarifRenderer.check(&sample(), &opts()));
        let ids: Vec<&str> = v["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["HN-R001", "HN-T001", "HN-X004"]);
    }

    #[test]
    fn r_arch_052__uri_encoding_keeps_slashes_and_encodes_the_rest() {
        assert_eq!(encode_uri("a/b c/d#e%.rs"), "a/b%20c/d%23e%25.rs");
        assert_eq!(encode_uri("caf\u{e9}.rs"), "caf%C3%A9.rs");
    }

    #[test]
    fn r_arch_054__run_properties_carry_edition_as_of_and_partial() {
        let mut r = sample();
        r.partial = true;
        let v = parse(&SarifRenderer.check(&r, &opts()));
        let p = &v["runs"][0]["properties"];
        assert_eq!(
            (
                p["edition"].as_u64(),
                p["asOf"].as_str(),
                p["partial"].as_bool()
            ),
            (Some(1), Some("2026-10-08"), Some(true))
        );
    }
}
