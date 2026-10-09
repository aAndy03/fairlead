//! `json` renderer: { fairlead, edition, as_of, findings[], summary } (R-CHK-036, C-38).

use std::collections::BTreeMap;

use serde::Serialize;

use super::sanitize::{FIX_CAP, INTENT_CAP, LINE_CAP, clean_data};
use super::{
    OutputFormat, RenderOptions, Renderer, finish, machine_slice, sev_word, stage_word, state_word,
};
use crate::check::{CheckReport, Finding};

pub struct JsonRenderer;

#[derive(Serialize)]
struct JsonReport {
    fairlead: String,
    edition: u32,
    as_of: String,
    findings: Vec<JsonFinding>,
    summary: JsonSummary,
}

#[derive(Serialize)]
struct JsonFinding {
    code: String,
    severity: &'static str,
    stage: &'static str,
    path: String,
    line: u32,
    col: u32,
    scope: String,
    rule: Option<String>,
    related: Option<String>,
    message: String,
    intent: Option<String>,
    fix: String,
    occurrence: u32,
    fingerprint: String,
    state: &'static str,
    waived_by: Option<String>,
}

#[derive(Serialize)]
struct JsonWaiver {
    active: u32,
    expiring_30d: u32,
    budget: u32,
}

#[derive(Serialize)]
struct JsonSummary {
    errors: u32,
    warns: u32,
    infos: u32,
    baselined: u32,
    waived: u32,
    observe: u32,
    accepted: u32,
    waivers: BTreeMap<String, JsonWaiver>,
    relaxations: Vec<String>,
    exceptions_delta: i32,
    codeowners_uncovered: u32,
    requests_open: u32,
    oldest_request_days: Option<u32>,
    partial: bool,
    unknown_scopes: Vec<String>,
    zero_declarations: bool,
    omitted: usize,
}

fn json_finding(f: &Finding) -> JsonFinding {
    JsonFinding {
        code: f.code.0.to_string(),
        severity: sev_word(f.severity),
        stage: stage_word(&f.stage),
        path: clean_data(f.location.path.as_str(), LINE_CAP),
        line: f.location.line,
        col: f.location.col,
        scope: f.scope.0.as_str().to_string(),
        rule: f.rule.as_ref().map(|r| format!("rule.{}", r.0.as_str())),
        related: f.related.as_deref().map(|r| clean_data(r, LINE_CAP)),
        message: clean_data(&f.message, LINE_CAP),
        intent: f.intent.as_deref().map(|i| clean_data(i, INTENT_CAP)),
        fix: clean_data(&f.fix, FIX_CAP),
        occurrence: f.occurrence,
        fingerprint: f.fingerprint.as_str().to_string(),
        state: state_word(&f.state),
        waived_by: f.waived_by.as_ref().map(|o| o.0.as_str().to_string()),
    }
}

impl Renderer for JsonRenderer {
    fn format(&self) -> OutputFormat {
        OutputFormat::Json
    }

    fn check(&self, report: &CheckReport, opts: &RenderOptions) -> String {
        let (slice, omitted) = machine_slice(report, opts);
        let s = &report.summary;
        let doc = JsonReport {
            fairlead: clean_data(&report.header.fairlead_version, 40),
            edition: report.header.edition.0,
            as_of: report.header.as_of.0.to_string(),
            findings: slice.iter().map(json_finding).collect(),
            summary: JsonSummary {
                errors: s.errors,
                warns: s.warns,
                infos: s.infos,
                baselined: s.baselined,
                waived: s.waived,
                observe: s.observe,
                accepted: s.accepted,
                waivers: s
                    .waivers
                    .iter()
                    .map(|(k, w)| {
                        let w = JsonWaiver {
                            active: w.active,
                            expiring_30d: w.expiring_30d,
                            budget: w.budget,
                        };
                        (k.0.as_str().to_string(), w)
                    })
                    .collect(),
                relaxations: s
                    .relaxations
                    .iter()
                    .map(|r| clean_data(r, LINE_CAP))
                    .collect(),
                exceptions_delta: s.exceptions_delta,
                codeowners_uncovered: s.codeowners_uncovered,
                requests_open: s.requests_open,
                oldest_request_days: s.oldest_request_days,
                partial: report.partial,
                unknown_scopes: report
                    .unknown_scopes
                    .iter()
                    .map(|u| u.0.as_str().to_string())
                    .collect(),
                zero_declarations: report.zero_declarations,
                omitted,
            },
        };
        finish(serde_json::to_string_pretty(&doc).unwrap_or_default())
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::{FindingState, MaxFindings};
    use crate::config::Stage;
    use crate::diag::Severity;
    use crate::diag::codes::{HN_R001, HN_T001};
    use crate::render::testkit::{finding, fp, report, rule, scope, with};
    use std::collections::BTreeSet;

    fn opts() -> RenderOptions {
        RenderOptions {
            format: OutputFormat::Json,
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

    fn sample() -> Finding {
        with(
            finding(
                HN_T001,
                Severity::Error,
                "apps/web/src/Panel.tsx",
                44,
                18,
                "literal #2563eb is tok.primary",
            ),
            |f| {
                f.rule = Some(rule("no-raw-hex"));
                f.scope = scope("web");
                f.intent = Some("colours come from tokens".to_string());
                f.fix = "use var(--primary)".to_string();
                f.fingerprint = fp("k3m9q2x7a1bz");
            },
        )
    }

    #[test]
    fn r_chk_036__json_has_exactly_the_keys_of_c38() {
        let out = JsonRenderer.check(&report(vec![sample()]), &opts());
        let v = parse(&out);
        let keys: BTreeSet<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            BTreeSet::from(["fairlead", "edition", "as_of", "findings", "summary"])
        );
        assert_eq!(
            (
                v["fairlead"].as_str(),
                v["edition"].as_u64(),
                v["as_of"].as_str()
            ),
            (Some("0.1.0"), Some(1), Some("2026-10-08"))
        );
        assert!(out.ends_with("}\n") && !out.contains('\r'));
    }

    #[test]
    fn r_chk_035__json_carries_every_field_of_a_finding() {
        let v = parse(&JsonRenderer.check(&report(vec![sample()]), &opts()));
        let f = &v["findings"][0];
        let keys: BTreeSet<&str> = f.as_object().unwrap().keys().map(String::as_str).collect();
        let want = [
            "code",
            "severity",
            "stage",
            "path",
            "line",
            "col",
            "scope",
            "rule",
            "related",
            "message",
            "intent",
            "fix",
            "occurrence",
            "fingerprint",
            "state",
            "waived_by",
        ];
        assert_eq!(keys, BTreeSet::from(want));
        assert_eq!(f["code"], "HN-T001");
        assert_eq!(f["rule"], "rule.no-raw-hex");
        assert_eq!(f["fingerprint"], "k3m9q2x7a1bz");
        assert_eq!(f["state"], "new");
        assert!(f["waived_by"].is_null() && f["related"].is_null());
    }

    #[test]
    fn r_chk_036__json_lists_baselined_waived_and_observe_findings_with_their_state() {
        let r = report(vec![
            with(finding(HN_R001, Severity::Error, "a.rs", 1, 1, "b"), |f| {
                f.state = FindingState::Baselined
            }),
            with(finding(HN_R001, Severity::Error, "b.rs", 1, 1, "w"), |f| {
                f.state = FindingState::Waived
            }),
            with(finding(HN_T001, Severity::Error, "c.rs", 1, 1, "o"), |f| {
                f.stage = Stage::Observe
            }),
        ]);
        let v = parse(&JsonRenderer.check(&r, &opts()));
        let states: Vec<&str> = v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["state"].as_str().unwrap())
            .collect();
        assert_eq!(states, vec!["baselined", "waived", "new"]);
        assert_eq!(v["findings"][2]["stage"], "observe");
    }

    #[test]
    fn r_chk_038__json_is_complete_by_default_and_reports_what_a_limit_omits() {
        let findings: Vec<_> = (0..30)
            .map(|i| {
                finding(
                    HN_R001,
                    Severity::Error,
                    &format!("src/f{i:02}.rs"),
                    1,
                    1,
                    "x",
                )
            })
            .collect();
        let r = report(findings);
        let len = |v: &serde_json::Value| v["findings"].as_array().unwrap().len();
        let all = parse(&JsonRenderer.check(&r, &opts()));
        assert_eq!(
            (len(&all), all["summary"]["omitted"].as_u64()),
            (30, Some(0))
        );
        let five = RenderOptions {
            max: MaxFindings::Limit(5),
            ..opts()
        };
        let cut = parse(&JsonRenderer.check(&r, &five));
        assert_eq!(
            (len(&cut), cut["summary"]["omitted"].as_u64()),
            (5, Some(25))
        );
        assert_eq!(cut["summary"]["errors"], 30, "totals are never truncated");
    }

    #[test]
    fn r_agt_017__json_text_is_sanitised_and_redacted() {
        let token = format!("ghp_{}", "a1B2".repeat(9));
        let msg = format!("\u{1b}[31mred\u{1b}[0m {token}\n\u{202e}x");
        let f = finding(HN_T001, Severity::Error, "a.rs", 1, 1, &msg);
        let v = parse(&JsonRenderer.check(&report(vec![f]), &opts()));
        assert_eq!(v["findings"][0]["message"], "red [redacted:github-token] x");
    }
}
