//! The baseline: tolerated finding fingerprints (R-CHK-030..032, R-CHK-052, C-17).
//! `check` calls only `parse` and `contains`; `sync_plan` (P7) calls `prune` and `accept_new`.

use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::normalize::lf_bytes;
use jiff::civil::Date;

use crate::check::codes::all_specs;
use crate::check::floor::is_floor;
use crate::check::{Finding, FindingState};
use crate::config::ApprovalsFile;
use crate::diag::DiagCode;
use crate::discover::Edition;
use crate::generate::header::{HeaderKind, parse_header, render_file};
use crate::generate::{HeaderVerdict, verify_header};
use crate::hash::Fingerprint;
use crate::path::RepoPath;
use crate::render::line::TerseLine;
use crate::render::sanitize::sanitize_data;
use crate::time::AsOf;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BaselineEntry {
    pub code: DiagCode,
    pub path: RepoPath,
    /// `acc=<as-of>` of `--accept-new`; `None` for entries recorded by `init`/`seed`.
    pub accepted: Option<Date>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Baseline {
    pub entries: BTreeMap<Fingerprint, BaselineEntry>,
}

/// Why a baseline file cannot be trusted; P5a reports every variant as `HN-B002` (floor).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BaselineIssue {
    NotUtf8,
    MissingHeader,
    HashMismatch,
    ConflictMarkers,
    FloorEntry { line: u32, code: String },
    BadLine { line: u32, reason: &'static str },
    DuplicateFingerprint { line: u32 },
    NotCanonical,
}

impl BaselineIssue {
    pub fn message(&self) -> String {
        match self {
            BaselineIssue::NotUtf8 => "baseline is not valid UTF-8".to_string(),
            BaselineIssue::MissingHeader => "baseline header is missing or garbled".to_string(),
            BaselineIssue::HashMismatch => "baseline hash mismatch (hand-edited)".to_string(),
            BaselineIssue::ConflictMarkers => "baseline contains git conflict markers".to_string(),
            BaselineIssue::FloorEntry { line, code } => {
                format!("baseline line {line} holds floor finding {code}")
            }
            BaselineIssue::BadLine { line, reason } => format!("baseline line {line}: {reason}"),
            BaselineIssue::DuplicateFingerprint { line } => {
                format!("baseline line {line}: duplicate fingerprint")
            }
            BaselineIssue::NotCanonical => {
                "baseline is not in canonical form (hand-edited)".to_string()
            }
        }
    }
}

const BASE32: &str = "0123456789abcdefghjkmnpqrstvwxyz";

fn fingerprint_text_ok(s: &str) -> bool {
    s.chars().count() == 12 && s.chars().all(|c| BASE32.contains(c))
}

impl Baseline {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, fp: &Fingerprint) -> bool {
        self.entries.contains_key(fp)
    }

    /// Entry lines sorted by fingerprint, each LF-terminated, without the header.
    pub fn body(&self) -> String {
        let mut out = String::new();
        for (fp, e) in &self.entries {
            let mut line = TerseLine::new(fp.as_str())
                .with("code", e.code.0)
                .with("path", e.path.as_str());
            if let Some(d) = e.accepted {
                line = line.with("acc", &d.to_string());
            }
            out.push_str(&line.render());
            out.push('\n');
        }
        out
    }

    pub fn render(&self, edition: Edition) -> String {
        render_file(&HeaderKind::Baseline, edition.0, &self.body())
    }

    /// Strict reader: the file must verify, hold only known non-floor codes, and equal the
    /// canonical rendering of what it says (so sorting, spacing and extra keys cannot differ).
    pub fn parse(bytes: &[u8]) -> Result<Baseline, BaselineIssue> {
        let norm = lf_bytes(bytes);
        let text = std::str::from_utf8(&norm).map_err(|_| BaselineIssue::NotUtf8)?;
        match verify_header(bytes) {
            HeaderVerdict::Ok => {}
            HeaderVerdict::HasConflictMarkers => return Err(BaselineIssue::ConflictMarkers),
            HeaderVerdict::HandEdited => {
                let first = text.split('\n').next().unwrap_or("");
                return Err(if parse_header(first).is_some() {
                    BaselineIssue::HashMismatch
                } else {
                    BaselineIssue::MissingHeader
                });
            }
        }
        let (first, body) = text.split_once('\n').unwrap_or((text, ""));
        let header = parse_header(first).ok_or(BaselineIssue::MissingHeader)?;
        if header.kind != HeaderKind::Baseline {
            return Err(BaselineIssue::MissingHeader);
        }
        let mut entries = BTreeMap::new();
        for (idx, line) in body.lines().enumerate() {
            let at = u32::try_from(idx + 2).unwrap_or(u32::MAX);
            let bad = |reason: &'static str| BaselineIssue::BadLine { line: at, reason };
            let t = TerseLine::parse(line).map_err(|_| bad("malformed line"))?;
            if !fingerprint_text_ok(&t.id) {
                return Err(bad("bad fingerprint"));
            }
            let fp = Fingerprint::parse(&t.id).map_err(|_| bad("bad fingerprint"))?;
            let code_text = t.get("code").ok_or(bad("missing code"))?;
            let spec = all_specs()
                .iter()
                .find(|s| s.code.0 == code_text)
                .ok_or(bad("unknown code"))?;
            if is_floor(spec.code) {
                return Err(BaselineIssue::FloorEntry {
                    line: at,
                    code: code_text.to_string(),
                });
            }
            let path = RepoPath::new(t.get("path").ok_or(bad("missing path"))?)
                .map_err(|_| bad("bad path"))?;
            let accepted = match t.get("acc") {
                None => None,
                Some(d) => Some(d.parse::<Date>().map_err(|_| bad("bad acc date"))?),
            };
            if entries
                .insert(
                    fp,
                    BaselineEntry {
                        code: spec.code,
                        path,
                        accepted,
                    },
                )
                .is_some()
            {
                return Err(BaselineIssue::DuplicateFingerprint { line: at });
            }
        }
        let parsed = Baseline { entries };
        if parsed.render(Edition(header.edition)).as_bytes() != &*norm {
            return Err(BaselineIssue::NotCanonical);
        }
        Ok(parsed)
    }
}

/// Why `--accept-new` was refused.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AcceptRefusal {
    MissingWhy,
    /// Floor findings are never accepted (R-CHK-032, R-CHK-008).
    FloorFinding {
        codes: Vec<DiagCode>,
    },
    /// Authority-family findings need an `approve` record per fingerprint (R-CHK-052).
    NeedsApproval {
        fingerprints: Vec<Fingerprint>,
    },
}

impl AcceptRefusal {
    pub fn message(&self) -> String {
        match self {
            AcceptRefusal::MissingWhy => "--accept-new needs --why \"<reason>\"".to_string(),
            AcceptRefusal::FloorFinding { codes } => format!(
                "floor findings are never accepted: {}",
                codes.iter().map(|c| c.0).collect::<Vec<_>>().join(", ")
            ),
            AcceptRefusal::NeedsApproval { fingerprints } => format!(
                "authority findings need an approve record: {}",
                fingerprints
                    .iter()
                    .map(Fingerprint::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// `HN-X*`, `HN-A*`, `HN-C*` (R-CHK-052).
fn is_authority_family(code: DiagCode) -> bool {
    ["HN-X", "HN-A", "HN-C"]
        .iter()
        .any(|p| code.0.starts_with(p))
}

impl Baseline {
    /// Fingerprints in the baseline with no current finding: fixed, prune candidates (`HN-B001`).
    pub fn stale_fingerprints(&self, current: &[Finding]) -> Vec<Fingerprint> {
        let live: BTreeSet<&Fingerprint> = current.iter().map(|f| &f.fingerprint).collect();
        self.entries
            .keys()
            .filter(|fp| !live.contains(fp))
            .cloned()
            .collect()
    }

    /// `sync --baseline`: drop fixed entries, never add (R-CHK-032).
    pub fn prune(&self, current: &[Finding]) -> Baseline {
        let live: BTreeSet<&Fingerprint> = current.iter().map(|f| &f.fingerprint).collect();
        Baseline {
            entries: self
                .entries
                .iter()
                .filter(|(fp, _)| live.contains(fp))
                .map(|(fp, e)| (fp.clone(), e.clone()))
                .collect(),
        }
    }

    /// `--accept-new --why`: add `new` findings marked `acc=<as-of>`. All-or-nothing; refuses
    /// floor findings and unapproved authority-family findings (R-CHK-032, R-CHK-052).
    pub fn accept_new(
        &self,
        new: &[Finding],
        why: &str,
        as_of: AsOf,
        approvals: &ApprovalsFile,
    ) -> Result<Baseline, AcceptRefusal> {
        if sanitize_data(why, 200).is_empty() {
            return Err(AcceptRefusal::MissingWhy);
        }
        let candidates: Vec<&Finding> = new
            .iter()
            .filter(|f| matches!(f.state, FindingState::New | FindingState::WaiverIneffective))
            .filter(|f| !self.contains(&f.fingerprint))
            .collect();
        let mut floors: Vec<DiagCode> = candidates
            .iter()
            .filter(|f| is_floor(f.code))
            .map(|f| f.code)
            .collect();
        floors.sort();
        floors.dedup();
        if !floors.is_empty() {
            return Err(AcceptRefusal::FloorFinding { codes: floors });
        }
        let mut missing: Vec<Fingerprint> = candidates
            .iter()
            .filter(|f| is_authority_family(f.code))
            .filter(|f| !approvals.approvals.iter().any(|a| a.fp == f.fingerprint))
            .map(|f| f.fingerprint.clone())
            .collect();
        missing.sort();
        missing.dedup();
        if !missing.is_empty() {
            return Err(AcceptRefusal::NeedsApproval {
                fingerprints: missing,
            });
        }
        let mut entries = self.entries.clone();
        for f in candidates {
            entries.insert(
                f.fingerprint.clone(),
                BaselineEntry {
                    code: f.code,
                    path: f.location.path.clone(),
                    accepted: Some(as_of.0),
                },
            );
        }
        Ok(Baseline { entries })
    }

    /// `init` (R-CLI-019): record the first scan's new findings, without floor and authority-family
    /// findings (those must be fixed or approved) and without `acc=`.
    pub fn seed(findings: &[Finding]) -> Baseline {
        let mut entries = BTreeMap::new();
        for f in findings
            .iter()
            .filter(|f| matches!(f.state, FindingState::New))
        {
            if is_floor(f.code) || is_authority_family(f.code) {
                continue;
            }
            entries.insert(
                f.fingerprint.clone(),
                BaselineEntry {
                    code: f.code,
                    path: f.location.path.clone(),
                    accepted: None,
                },
            );
        }
        Baseline { entries }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        non_snake_case,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]
    use super::*;
    use crate::diag::codes::{HN_F001, HN_R001, HN_T001};
    use crate::generate::header::{HeaderKind, render_file};
    use crate::render::testkit::fp;

    fn n(i: u32) -> Fingerprint {
        fp(&format!("{i:0>12}"))
    }

    fn entry(code: DiagCode, path: &str, acc: Option<Date>) -> BaselineEntry {
        BaselineEntry {
            code,
            path: RepoPath::new(path).unwrap(),
            accepted: acc,
        }
    }

    fn two() -> Baseline {
        let mut b = Baseline::default();
        b.entries
            .insert(n(2), entry(HN_T001, "apps/web/src/Panel.tsx", None));
        b.entries.insert(
            n(1),
            entry(HN_R001, "a b/c.rs", Some(jiff::civil::date(2026, 10, 9))),
        );
        b
    }

    fn file(body: &str) -> Vec<u8> {
        render_file(&HeaderKind::Baseline, 1, body).into_bytes()
    }

    #[test]
    fn r_chk_030__render_is_header_then_lines_sorted_by_fingerprint() {
        let text = two().render(Edition(1));
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("# baseline edition=1 body="));
        assert_eq!(
            lines[1],
            "000000000001 code=HN-R001 path=\"a b/c.rs\" acc=2026-10-09"
        );
        assert_eq!(
            lines[2],
            "000000000002 code=HN-T001 path=apps/web/src/Panel.tsx"
        );
        assert!(text.ends_with('\n') && !text.contains('\r'));
    }

    #[test]
    fn r_chk_030__parse_inverts_render_byte_for_byte() {
        let b = two();
        let text = b.render(Edition(1));
        assert_eq!(Baseline::parse(text.as_bytes()), Ok(b.clone()));
        assert_eq!(
            Baseline::parse(text.replace('\n', "\r\n").as_bytes()),
            Ok(b),
            "a CRLF checkout parses too"
        );
    }

    #[test]
    fn r_chk_030__empty_baseline_is_a_header_only_file() {
        let text = Baseline::default().render(Edition(1));
        assert_eq!(text.lines().count(), 1);
        assert_eq!(Baseline::parse(text.as_bytes()), Ok(Baseline::default()));
    }

    #[test]
    fn r_chk_024__a_hand_edit_is_a_hash_mismatch() {
        let text = two().render(Edition(1)).replace("HN-T001", "HN-R001");
        assert_eq!(
            Baseline::parse(text.as_bytes()),
            Err(BaselineIssue::HashMismatch)
        );
    }

    #[test]
    fn r_chk_024__missing_header_and_conflict_markers_are_reported() {
        assert_eq!(
            Baseline::parse(b"000000000001 code=HN-T001 path=a.rs\n"),
            Err(BaselineIssue::MissingHeader)
        );
        let good = two().render(Edition(1));
        let conflicted = format!("<<<<<<< HEAD\n{good}=======\n{good}>>>>>>> b\n");
        assert_eq!(
            Baseline::parse(conflicted.as_bytes()),
            Err(BaselineIssue::ConflictMarkers)
        );
        assert_eq!(Baseline::parse(&[0xff, 0xfe]), Err(BaselineIssue::NotUtf8));
    }

    #[test]
    fn r_chk_008__a_floor_entry_is_rejected_even_with_a_valid_hash() {
        let bytes = file("000000000003 code=HN-F001 path=a.rs\n");
        assert_eq!(
            Baseline::parse(&bytes),
            Err(BaselineIssue::FloorEntry {
                line: 2,
                code: "HN-F001".to_string()
            })
        );
        assert!(is_floor(HN_F001));
    }

    #[test]
    fn r_chk_030__malformed_unknown_duplicate_and_unsorted_lines_are_rejected() {
        assert!(matches!(
            Baseline::parse(&file("garbage\n")),
            Err(BaselineIssue::BadLine { line: 2, .. })
        ));
        assert!(matches!(
            Baseline::parse(&file("000000000004 code=HN-Z999 path=a.rs\n")),
            Err(BaselineIssue::BadLine { .. })
        ));
        assert!(matches!(
            Baseline::parse(&file("000000000004 code=HN-T001\n")),
            Err(BaselineIssue::BadLine { .. })
        ));
        let dup = "000000000004 code=HN-T001 path=a.rs\n000000000004 code=HN-T001 path=a.rs\n";
        assert_eq!(
            Baseline::parse(&file(dup)),
            Err(BaselineIssue::DuplicateFingerprint { line: 3 })
        );
        let unsorted = "000000000005 code=HN-T001 path=a.rs\n000000000004 code=HN-T001 path=b.rs\n";
        assert_eq!(
            Baseline::parse(&file(unsorted)),
            Err(BaselineIssue::NotCanonical)
        );
        let extra_key = "000000000004 code=HN-T001 path=a.rs note=x\n";
        assert_eq!(
            Baseline::parse(&file(extra_key)),
            Err(BaselineIssue::NotCanonical)
        );
    }

    #[test]
    fn r_chk_030__contains_and_len() {
        let b = two();
        assert!(b.contains(&n(1)) && !b.contains(&n(9)));
        assert_eq!((b.len(), b.is_empty()), (2, false));
    }

    #[test]
    fn r_chk_030__a_scope_header_is_not_a_baseline() {
        let bytes = render_file(&HeaderKind::Scope("root".to_string()), 1, "").into_bytes();
        assert_eq!(Baseline::parse(&bytes), Err(BaselineIssue::MissingHeader));
    }

    use crate::check::FindingState;
    use crate::config::{Approval, ApprovalsFile};
    use crate::diag::Severity;
    use crate::diag::codes::{HN_A001, HN_X004};
    use crate::render::testkit::{finding, with};
    use crate::time::AsOf;

    fn f(code: DiagCode, path: &str) -> Finding {
        finding(code, Severity::Error, path, 3, 1, "m")
    }

    fn day() -> AsOf {
        AsOf(jiff::civil::date(2026, 10, 9))
    }

    fn no_approvals() -> ApprovalsFile {
        ApprovalsFile {
            approvals: Vec::new(),
        }
    }

    #[test]
    fn r_chk_032__prune_drops_fixed_entries_and_never_adds() {
        let keep = f(HN_T001, "a.rs");
        let gone = n(8);
        let mut b = Baseline::default();
        b.entries
            .insert(keep.fingerprint.clone(), entry(HN_T001, "a.rs", None));
        b.entries
            .insert(gone.clone(), entry(HN_T001, "old.rs", None));
        let brand_new = f(HN_R001, "new.rs");
        let pruned = b.prune(&[keep.clone(), brand_new.clone()]);
        assert!(pruned.contains(&keep.fingerprint));
        assert!(!pruned.contains(&gone));
        assert!(
            !pruned.contains(&brand_new.fingerprint),
            "prune never adds (R-CHK-032)"
        );
        assert_eq!(b.stale_fingerprints(&[keep, brand_new]), vec![gone]);
    }

    #[test]
    fn r_chk_032__accept_new_needs_a_reason() {
        let r = Baseline::default().accept_new(&[f(HN_T001, "a.rs")], "  ", day(), &no_approvals());
        assert_eq!(r, Err(AcceptRefusal::MissingWhy));
    }

    #[test]
    fn r_chk_032__accept_new_marks_each_added_line_with_the_as_of_date() {
        let new = f(HN_T001, "a.rs");
        let mut b = Baseline::default();
        b.entries.insert(n(1), entry(HN_R001, "old.rs", None));
        let out = b
            .accept_new(&[new.clone()], "legacy panel", day(), &no_approvals())
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(
            out.entries[&new.fingerprint].accepted,
            Some(jiff::civil::date(2026, 10, 9))
        );
        assert_eq!(
            out.entries[&n(1)],
            b.entries[&n(1)],
            "existing entries are untouched"
        );
    }

    #[test]
    fn r_chk_032__floor_findings_are_never_accepted() {
        let b = Baseline::default();
        let r = b.accept_new(
            &[f(HN_T001, "a.rs"), f(HN_F001, "b.rs")],
            "because",
            day(),
            &no_approvals(),
        );
        assert_eq!(
            r,
            Err(AcceptRefusal::FloorFinding {
                codes: vec![HN_F001]
            })
        );
    }

    #[test]
    fn r_chk_052__authority_family_needs_an_approve_record_covering_the_line() {
        let x = f(HN_X004, "a.rs");
        let a = f(HN_A001, "AGENTS.md");
        let r =
            Baseline::default().accept_new(&[x.clone(), a.clone()], "why", day(), &no_approvals());
        let mut want = vec![x.fingerprint.clone(), a.fingerprint.clone()];
        want.sort();
        assert_eq!(r, Err(AcceptRefusal::NeedsApproval { fingerprints: want }));

        let approval = |fp: &Fingerprint| Approval {
            fp: fp.clone(),
            target: fairlead_syntax::ids::Id::parse("rule.no-raw-hex").unwrap(),
            scope: fairlead_syntax::ids::ScopeSlug::parse("web").unwrap(),
            approver: "@design".to_string(),
            date: jiff::civil::date(2026, 10, 8),
            renewal: 0,
        };
        let covered = ApprovalsFile {
            approvals: vec![approval(&x.fingerprint), approval(&a.fingerprint)],
        };
        let out = Baseline::default()
            .accept_new(&[x, a], "why", day(), &covered)
            .unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn r_chk_032__already_baselined_waived_and_known_findings_are_not_added() {
        let known = f(HN_T001, "a.rs");
        let mut b = Baseline::default();
        b.entries
            .insert(known.fingerprint.clone(), entry(HN_T001, "a.rs", None));
        let waived = with(f(HN_R001, "w.rs"), |x| x.state = FindingState::Waived);
        let baselined = with(f(HN_R001, "b.rs"), |x| x.state = FindingState::Baselined);
        let out = b
            .accept_new(&[known, waived, baselined], "why", day(), &no_approvals())
            .unwrap();
        assert_eq!(out, b);
    }

    #[test]
    fn r_chk_030__seed_records_new_non_floor_non_authority_findings_without_acc() {
        let seeded = Baseline::seed(&[
            f(HN_T001, "a.rs"),
            f(HN_F001, "secret.md"),
            f(HN_X004, "x.rs"),
            with(f(HN_R001, "b.rs"), |x| x.state = FindingState::Baselined),
        ]);
        assert_eq!(seeded.len(), 1);
        assert!(seeded.entries.values().all(|e| e.accepted.is_none()));
    }
}
