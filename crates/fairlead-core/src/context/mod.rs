//! `fairlead context`: the pure brief builder (seam S9). No I/O, no clock, no writes besides the
//! disposable token-estimate cache (R-CTX-006).
pub mod brief;
pub mod budget;
pub mod digest;
pub mod estimate;
pub mod priority;
pub mod request;

pub use digest::effective_digest;
pub use estimate::{TokenEstimate, estimate_bytes};
pub use request::{
    Budget, ContextArgError, ContextInput, ContextRequest, ContextTarget, NO_MATCH_MESSAGE,
    missing_note, parse_target,
};

/// `usize` to `u32` without a lossy cast.
#[allow(dead_code, reason = "used by the P6 gatherers (T7 onwards)")]
pub(crate) fn n32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
pub(crate) mod testutil {
    use crate::config::{ContextConfig, SevWeight};

    pub fn cfg() -> ContextConfig {
        ContextConfig {
            budget_start: 500,
            budget_path: 700,
            bytes_per_token: 4,
            tokenizer: None,
            sev_weight: SevWeight {
                error: 4,
                warn: 2,
                info: 1,
            },
        }
    }

    use std::sync::Arc;

    use fairlead_syntax::diag::Span;
    use fairlead_syntax::directive::{Carrier, Directive, Extent, Verb};
    use fairlead_syntax::extract::{Extraction, ScanStatus};
    use fairlead_syntax::ids::Id;

    use crate::classify::FileClass;
    use crate::extract::{Corpus, FileRecord};
    use crate::hash::ContentHash;
    use crate::path::RepoPath;

    pub fn directive(verb: Verb, ids: &[&str]) -> Directive {
        Directive {
            ordinal: 0,
            verb,
            must_understand: false,
            ids: ids.iter().map(|s| Id::parse(s).unwrap()).collect(),
            bare: vec![],
            args: vec![],
            note: None,
            carrier: Carrier::LineComment,
            span: Span {
                start: 0,
                end: 0,
                line: 1,
                col: 1,
            },
            extent: Extent::File,
            end_span: None,
        }
    }

    pub fn rec(path: &str, directives: Vec<Directive>) -> FileRecord {
        FileRecord {
            path: RepoPath::new(path).unwrap(),
            class: FileClass::Authored,
            size: 1,
            hash: ContentHash([1; 32]),
            extraction: Some(Arc::new(Extraction {
                status: ScanStatus::Scanned,
                candidates: crate::context::n32(directives.len()),
                dropped: 0,
                directives,
                frontmatter: None,
                diags: vec![],
            })),
            css_tokens: vec![],
            literals: vec![],
            secrets: vec![],
            pattern_hits: vec![],
            skip: None,
        }
    }

    pub fn corpus(recs: Vec<FileRecord>) -> Corpus {
        Corpus {
            records: recs.into_iter().map(|r| (r.path.clone(), r)).collect(),
        }
    }

    use fairlead_syntax::diag::{DiagCode, Severity};

    use crate::check::fingerprint::{FingerprintInput, fingerprint};
    use crate::check::{Finding, FindingState, RuleId};
    use crate::config::Stage;
    use crate::diag::Location;
    use crate::registry::ScopeId;
    use fairlead_syntax::ids::ScopeSlug;

    pub fn finding(
        code: DiagCode,
        sev: Severity,
        path: &str,
        line: u32,
        state: FindingState,
        rule: Option<&str>,
    ) -> Finding {
        let scope = ScopeId(ScopeSlug::parse("root").unwrap());
        let rp = RepoPath::new(path).unwrap();
        let rid = rule.map(|r| RuleId(Id::parse(&format!("rule.{r}")).unwrap().name));
        let fp = fingerprint(&FingerprintInput {
            code,
            rule: rid.as_ref(),
            scope: &scope,
            path: &rp,
            text: "x",
            occurrence: line,
        });
        Finding {
            code,
            severity: sev,
            stage: Stage::Enforce,
            location: Location {
                path: rp,
                line,
                col: 1,
            },
            scope,
            rule: rid,
            related: None,
            message: "m".into(),
            excerpt: None,
            intent: None,
            fix: "f".into(),
            occurrence: line,
            fingerprint: fp,
            state,
            waived_by: None,
        }
    }

    use crate::check::{CheckReport, ReportHeader, Summary};
    use crate::discover::Edition;
    use crate::time::AsOf;

    pub fn report(findings: Vec<Finding>, partial: bool) -> CheckReport {
        CheckReport {
            header: ReportHeader {
                fairlead_version: "0.1.0".into(),
                edition: Edition(1),
                as_of: AsOf::parse("2026-10-09").unwrap(),
                scopes: 1,
                files: 1,
                rules: 1,
            },
            findings,
            summary: Summary::default(),
            partial,
            unknown_scopes: vec![],
            absent_scopes: vec![],
            zero_declarations: false,
        }
    }
}
