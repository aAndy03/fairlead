//! Diagnostics shared by every syntax stage (R-SYN-032, R-CHK-042).
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Opaque stable code "HN-S001" (R-CHK-042, R-ARCH-055). Compare by string.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DiagCode(pub &'static str);

impl Serialize for DiagCode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for DiagCode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        info_by_str(&text)
            .map(|i| i.code)
            .ok_or_else(|| serde::de::Error::custom("unknown diagnostic code"))
    }
}

pub mod codes {
    use super::DiagCode;
    pub const HN_S001: DiagCode = DiagCode("HN-S001"); // unknown verb
    pub const HN_S002: DiagCode = DiagCode("HN-S002"); // unknown kind (resolve) / lexically bad ID (extract)
    pub const HN_S004: DiagCode = DiagCode("HN-S004"); // frontmatter invalid
    pub const HN_S005: DiagCode = DiagCode("HN-S005"); // near-miss marker
    pub const HN_S006: DiagCode = DiagCode("HN-S006"); // continuation limit
    pub const HN_S007: DiagCode = DiagCode("HN-S007"); // quote / escape
    pub const HN_S008: DiagCode = DiagCode("HN-S008"); // unmatched end / unclosed region
    pub const HN_S009: DiagCode = DiagCode("HN-S009"); // ovr of a missing field (resolve)
    pub const HN_S010: DiagCode = DiagCode("HN-S010"); // binds to nothing
    pub const HN_S011: DiagCode = DiagCode("HN-S011"); // unknown key (resolve)
    pub const HN_S012: DiagCode = DiagCode("HN-S012"); // until / non-date exp in 0.1
    pub const HN_S013: DiagCode = DiagCode("HN-S013"); // doc-comment carrier
    pub const HN_S014: DiagCode = DiagCode("HN-S014"); // sym absent
    pub const HN_S015: DiagCode = DiagCode("HN-S015"); // directive-count explosion
    pub const HN_S016: DiagCode = DiagCode("HN-S016"); // incomplete directive / ovr
    pub const HN_I001: DiagCode = DiagCode("HN-I001"); // file skipped (size / NUL)
    pub const HN_I002: DiagCode = DiagCode("HN-I002"); // unknown comment syntax
    pub const HN_I003: DiagCode = DiagCode("HN-I003"); // non-UTF-8 skipped
    pub const HN_P001: DiagCode = DiagCode("HN-P001"); // reserved name in an ID (R-SYN-027)
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// Original-byte span plus 1-based line and char column of the start (C-16).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub line: u32,
    pub col: u32,
}

impl Span {
    /// Line 0 means "whole file" (contract 4).
    pub const fn whole_file() -> Span {
        Span {
            start: 0,
            end: 0,
            line: 0,
            col: 0,
        }
    }
}

/// R-SYN-032, R-ARCH-103: code, location, expected form, did-you-mean, one hint.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SyntaxDiag {
    pub code: DiagCode,
    pub severity: Severity,
    pub span: Span,
    pub message: String,
    pub expected: Option<String>,
    pub did_you_mean: Option<String>,
    pub hint: String,
}

impl SyntaxDiag {
    /// Severity comes from the registry; `expected` and `did_you_mean` start empty.
    pub fn new(
        code: DiagCode,
        span: Span,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> SyntaxDiag {
        SyntaxDiag {
            code,
            severity: info(code).map_or(Severity::Error, |i| i.severity),
            span,
            message: message.into(),
            expected: None,
            did_you_mean: None,
            hint: hint.into(),
        }
    }
    pub fn expected(mut self, e: impl Into<String>) -> SyntaxDiag {
        self.expected = Some(e.into());
        self
    }
    pub fn suggest(mut self, s: Option<String>) -> SyntaxDiag {
        self.did_you_mean = s;
        self
    }
}

/// One registry row: default severity and display name (13, R-CHK-042).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CodeInfo {
    pub code: DiagCode,
    pub name: &'static str,
    pub severity: Severity,
    pub summary: &'static str,
}

const fn row(
    code: DiagCode,
    name: &'static str,
    severity: Severity,
    summary: &'static str,
) -> CodeInfo {
    CodeInfo {
        code,
        name,
        severity,
        summary,
    }
}

/// Every code this crate or its consumers raise for syntax and file scanning. Sorted by code.
pub const REGISTRY: &[CodeInfo] = &[
    row(
        codes::HN_I001,
        "FILE-SKIPPED",
        Severity::Info,
        "file skipped: too large or binary",
    ),
    row(
        codes::HN_I002,
        "UNKNOWN-COMMENT-SYNTAX",
        Severity::Info,
        "no comment leaders known for this extension",
    ),
    row(
        codes::HN_I003,
        "ENCODING-SKIP",
        Severity::Warn,
        "file is not valid UTF-8 and was not scanned",
    ),
    row(
        codes::HN_P001,
        "PATH-HYGIENE",
        Severity::Error,
        "reserved Windows name in an ID or scope id",
    ),
    row(
        codes::HN_S001,
        "UNKNOWN-VERB",
        Severity::Error,
        "unknown directive verb",
    ),
    row(
        codes::HN_S002,
        "UNKNOWN-KIND",
        Severity::Error,
        "ID with an undeclared or malformed kind",
    ),
    row(
        codes::HN_S004,
        "FRONTMATTER",
        Severity::Error,
        "frontmatter is not valid TOML or directives is not an array of strings",
    ),
    row(
        codes::HN_S005,
        "MARKER-NEAR-MISS",
        Severity::Warn,
        "comment looks like a directive but the marker is misspelled",
    ),
    row(
        codes::HN_S006,
        "CONTINUATION-LIMIT",
        Severity::Error,
        "more than 8 continuation lines or 1 KB",
    ),
    row(
        codes::HN_S007,
        "QUOTE",
        Severity::Error,
        "unterminated quote or unknown escape",
    ),
    row(
        codes::HN_S008,
        "REGION",
        Severity::Error,
        "unmatched end or unclosed +end region",
    ),
    row(
        codes::HN_S009,
        "OVR-FIELD",
        Severity::Error,
        "ovr of a field the item does not have",
    ),
    row(
        codes::HN_S010,
        "BINDS-NOTHING",
        Severity::Warn,
        "directive binds to nothing",
    ),
    row(
        codes::HN_S011,
        "UNKNOWN-KEY",
        Severity::Warn,
        "unknown key on a directive",
    ),
    row(
        codes::HN_S012,
        "EXPIRY",
        Severity::Error,
        "until= or a non-ISO exp in edition 1",
    ),
    row(
        codes::HN_S013,
        "DOC-COMMENT-CARRIER",
        Severity::Error,
        "marker inside a doc comment would leak into rustdoc/TSDoc",
    ),
    row(
        codes::HN_S014,
        "SYM-ABSENT",
        Severity::Warn,
        "sym= name not found in the extent",
    ),
    row(
        codes::HN_S015,
        "DIRECTIVE-COUNT",
        Severity::Warn,
        "more directives in one file than max-directives",
    ),
    row(
        codes::HN_S016,
        "OVR-INCOMPLETE",
        Severity::Error,
        "incomplete directive",
    ),
];

pub fn info(code: DiagCode) -> Option<&'static CodeInfo> {
    info_by_str(code.0)
}

pub fn info_by_str(code: &str) -> Option<&'static CodeInfo> {
    REGISTRY.iter().find(|i| i.code.0 == code)
}

/// Nearest candidate within edit distance 2 (and shorter than the word's length), ties by order.
pub fn did_you_mean<'a>(word: &str, candidates: &[&'a str]) -> Option<&'a str> {
    if word.is_empty() {
        return None;
    }
    let mut best: Option<(usize, &'a str)> = None;
    for c in candidates {
        let d = edit_distance(word, c);
        if d <= 2
            && d < word.chars().count().max(c.chars().count())
            && best.is_none_or(|(bd, _)| d < bd)
        {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = Vec::with_capacity(b.len() + 1);
        cur.push(i + 1);
        for (j, cb) in b.iter().enumerate() {
            let sub = prev.get(j).copied().unwrap_or(0) + usize::from(ca != cb);
            let del = prev.get(j + 1).copied().unwrap_or(0) + 1;
            let ins = cur.get(j).copied().unwrap_or(0) + 1;
            cur.push(sub.min(del).min(ins));
        }
        prev = cur;
    }
    prev.get(b.len()).copied().unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde::de::IntoDeserializer;

    #[test]
    fn r_syn_032__registry_has_each_syntax_code_once_and_never_s003() {
        let mut seen = std::collections::BTreeSet::new();
        for c in REGISTRY {
            assert!(seen.insert(c.code.0), "duplicate {}", c.code.0);
            assert!(
                c.code.0.starts_with("HN-S")
                    || c.code.0.starts_with("HN-I")
                    || c.code.0 == "HN-P001"
            );
        }
        assert!(!seen.contains("HN-S003"), "S003 is unallocated");
        for expected in [
            "HN-S001", "HN-S002", "HN-S004", "HN-S005", "HN-S006", "HN-S007", "HN-S008", "HN-S009",
            "HN-S010", "HN-S011", "HN-S012", "HN-S013", "HN-S014", "HN-S015", "HN-S016", "HN-I001",
            "HN-I002", "HN-I003", "HN-P001",
        ] {
            assert!(seen.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn r_syn_032__severities_match_table_13() {
        assert_eq!(
            info(codes::HN_S001).map(|i| i.severity),
            Some(Severity::Error)
        );
        assert_eq!(
            info(codes::HN_S005).map(|i| i.severity),
            Some(Severity::Warn)
        );
        assert_eq!(
            info(codes::HN_S010).map(|i| i.severity),
            Some(Severity::Warn)
        );
        assert_eq!(
            info(codes::HN_S013).map(|i| i.severity),
            Some(Severity::Error)
        );
        assert_eq!(
            info(codes::HN_I001).map(|i| i.severity),
            Some(Severity::Info)
        );
        assert_eq!(
            info(codes::HN_I003).map(|i| i.severity),
            Some(Severity::Warn)
        );
        assert!(Severity::Info < Severity::Warn && Severity::Warn < Severity::Error);
    }

    #[test]
    fn r_syn_032__did_you_mean_finds_close_verbs_only() {
        let verbs = ["def", "use", "ovr", "flag", "env", "end"];
        assert_eq!(did_you_mean("deff", &verbs), Some("def"));
        assert_eq!(did_you_mean("fag", &verbs), Some("flag"));
        assert_eq!(did_you_mean("frobnicate", &verbs), None);
        assert_eq!(did_you_mean("", &verbs), None);
    }

    #[test]
    fn r_syn_032__diag_code_deserializes_through_the_registry() {
        let d: serde::de::value::StrDeserializer<'_, serde::de::value::Error> =
            "HN-S007".into_deserializer();
        assert_eq!(DiagCode::deserialize(d).unwrap(), codes::HN_S007);
        let bad: serde::de::value::StrDeserializer<'_, serde::de::value::Error> =
            "HN-Z999".into_deserializer();
        assert!(DiagCode::deserialize(bad).is_err());
    }

    #[test]
    fn r_syn_032__new_diag_takes_severity_from_registry() {
        let d = SyntaxDiag::new(
            codes::HN_S010,
            Span::whole_file(),
            "binds to nothing",
            "put code after it",
        );
        assert_eq!(d.severity, Severity::Warn);
        assert_eq!(d.span.line, 0, "0 line means whole file");
        assert!(d.expected.is_none() && d.did_you_mean.is_none());
    }
}
