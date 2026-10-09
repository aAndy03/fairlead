//! Pre-finding diagnostics and the `HN-` code constants (13-checks-and-diagnostics R-CHK-042).
use crate::path::RepoPath;
pub use fairlead_syntax::diag::{DiagCode, Severity, Span, SyntaxDiag};

/// Code constants. No string literal `"HN-..."` may appear outside this file and fairlead-syntax.
pub mod codes {
    pub use fairlead_syntax::diag::codes::*;

    use super::DiagCode;

    macro_rules! core_codes {
        ($($name:ident = $text:literal),* $(,)?) => {
            $(pub const $name: DiagCode = DiagCode($text);)*
            /// Every code owned by core (the HN-S* and HN-I* codes live in fairlead-syntax).
            pub const CORE_CODES: &[DiagCode] = &[$($name),*];
        };
    }

    core_codes! {
        HN_C001 = "HN-C001", HN_C002 = "HN-C002", HN_C003 = "HN-C003", HN_C004 = "HN-C004",
        HN_C005 = "HN-C005", HN_C006 = "HN-C006", HN_C007 = "HN-C007", HN_C008 = "HN-C008",
        HN_C009 = "HN-C009", HN_C011 = "HN-C011", HN_C012 = "HN-C012", HN_C013 = "HN-C013",
        HN_C014 = "HN-C014",
        HN_R001 = "HN-R001", HN_R002 = "HN-R002", HN_R004 = "HN-R004", HN_R005 = "HN-R005",
        HN_R006 = "HN-R006", HN_R007 = "HN-R007", HN_R008 = "HN-R008",
        HN_D001 = "HN-D001", HN_D002 = "HN-D002", HN_D003 = "HN-D003", HN_D004 = "HN-D004",
        HN_D005 = "HN-D005", HN_D006 = "HN-D006",
        HN_T001 = "HN-T001", HN_T002 = "HN-T002", HN_T003 = "HN-T003", HN_T004 = "HN-T004",
        HN_T005 = "HN-T005", HN_T006 = "HN-T006", HN_T007 = "HN-T007", HN_T008 = "HN-T008",
        HN_T009 = "HN-T009", HN_T010 = "HN-T010",
        HN_U001 = "HN-U001",
        HN_G001 = "HN-G001", HN_G002 = "HN-G002", HN_G003 = "HN-G003", HN_G004 = "HN-G004",
        HN_G005 = "HN-G005", HN_G006 = "HN-G006",
        HN_X001 = "HN-X001", HN_X002 = "HN-X002", HN_X003 = "HN-X003", HN_X004 = "HN-X004",
        HN_X005 = "HN-X005", HN_X006 = "HN-X006", HN_X007 = "HN-X007", HN_X008 = "HN-X008",
        HN_X009 = "HN-X009", HN_X010 = "HN-X010", HN_X011 = "HN-X011", HN_X012 = "HN-X012",
        HN_X013 = "HN-X013", HN_X014 = "HN-X014", HN_X015 = "HN-X015", HN_X016 = "HN-X016",
        HN_B001 = "HN-B001", HN_B002 = "HN-B002",
        HN_A001 = "HN-A001", HN_A002 = "HN-A002", HN_A003 = "HN-A003", HN_A004 = "HN-A004",
        HN_N001 = "HN-N001", HN_N002 = "HN-N002",
        HN_F001 = "HN-F001",
        HN_P001 = "HN-P001",
        HN_V001 = "HN-V001", HN_V002 = "HN-V002", HN_V003 = "HN-V003", HN_V004 = "HN-V004",
        HN_V005 = "HN-V005", HN_V006 = "HN-V006", HN_V007 = "HN-V007", HN_V008 = "HN-V008",
        HN_V009 = "HN-V009", HN_V010 = "HN-V010", HN_V011 = "HN-V011", HN_V012 = "HN-V012",
        HN_V013 = "HN-V013", HN_V014 = "HN-V014", HN_V015 = "HN-V015", HN_V016 = "HN-V016",
        HN_E001 = "HN-E001", HN_E002 = "HN-E002", HN_E003 = "HN-E003", HN_E004 = "HN-E004",
        HN_E005 = "HN-E005", HN_E006 = "HN-E006", HN_E007 = "HN-E007",
        HN_L001 = "HN-L001", HN_L002 = "HN-L002",
        HN_H001 = "HN-H001", HN_H002 = "HN-H002", HN_H003 = "HN-H003", HN_H004 = "HN-H004",
        HN_H005 = "HN-H005", HN_H006 = "HN-H006", HN_H007 = "HN-H007", HN_H008 = "HN-H008",
        HN_H009 = "HN-H009", HN_H010 = "HN-H010", HN_H011 = "HN-H011", HN_H012 = "HN-H012",
        HN_H013 = "HN-H013", HN_H014 = "HN-H014", HN_H015 = "HN-H015", HN_H016 = "HN-H016",
        HN_H017 = "HN-H017", HN_H018 = "HN-H018", HN_H019 = "HN-H019", HN_H020 = "HN-H020",
        HN_H021 = "HN-H021", HN_H022 = "HN-H022", HN_H023 = "HN-H023", HN_H024 = "HN-H024",
        HN_H025 = "HN-H025",
        HN_W001 = "HN-W001", HN_W002 = "HN-W002", HN_W003 = "HN-W003", HN_W004 = "HN-W004",
        HN_K001 = "HN-K001",
        HN_M001 = "HN-M001", HN_M002 = "HN-M002", HN_M003 = "HN-M003", HN_M004 = "HN-M004",
        HN_M005 = "HN-M005", HN_M006 = "HN-M006", HN_M007 = "HN-M007", HN_M008 = "HN-M008",
        HN_M009 = "HN-M009", HN_M010 = "HN-M010", HN_M011 = "HN-M011", HN_M012 = "HN-M012",
    }
}

/// 1-based line and column (Unicode scalar values of the original text, C-16); line 0 = whole file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Location {
    pub path: RepoPath,
    pub line: u32,
    pub col: u32,
}

impl Location {
    pub fn file(path: RepoPath) -> Self {
        Self {
            path,
            line: 0,
            col: 0,
        }
    }
    pub fn at(path: RepoPath, line: u32, col: u32) -> Self {
        Self { path, line, col }
    }
}

/// A pre-finding: what every stage before `check` emits (syntax, config, walk, registry).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    pub code: DiagCode,
    pub severity: Severity,
    pub location: Location,
    pub scope: Option<fairlead_syntax::ids::ScopeSlug>,
    pub rule: Option<String>,
    pub related: Option<String>,
    pub message: String,
    pub expected: Option<String>,
    pub did_you_mean: Option<String>,
    pub hint: String,
    pub text: String,
}

impl Diagnostic {
    pub fn new(
        code: DiagCode,
        severity: Severity,
        location: Location,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity,
            location,
            scope: None,
            rule: None,
            related: None,
            message: message.into(),
            expected: None,
            did_you_mean: None,
            hint: String::new(),
            text: String::new(),
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Offending text, used later for the finding fingerprint.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn with_scope(mut self, scope: fairlead_syntax::ids::ScopeSlug) -> Self {
        self.scope = Some(scope);
        self
    }

    /// The lossless conversion: the caller knows the file, `SyntaxDiag` does not.
    pub fn from_syntax(path: &RepoPath, d: SyntaxDiag) -> Self {
        Self {
            code: d.code,
            severity: d.severity,
            location: Location::at(path.clone(), d.span.line, d.span.col),
            scope: None,
            rule: None,
            related: None,
            message: d.message,
            expected: d.expected,
            did_you_mean: d.did_you_mean,
            hint: d.hint,
            text: String::new(),
        }
    }
}

/// Placeholder-path conversion required by the contract; prefer [`Diagnostic::from_syntax`].
impl From<SyntaxDiag> for Diagnostic {
    fn from(d: SyntaxDiag) -> Self {
        Diagnostic::from_syntax(&RepoPath::root(), d)
    }
}

/// Total order (path, line, col, code, message): R-ARCH-051 applied to pre-findings.
pub fn sort_diagnostics(v: &mut [Diagnostic]) {
    v.sort_by(|a, b| {
        a.location
            .path
            .cmp(&b.location.path)
            .then(a.location.line.cmp(&b.location.line))
            .then(a.location.col.cmp(&b.location.col))
            .then(a.code.cmp(&b.code))
            .then_with(|| a.message.cmp(&b.message))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rp(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }

    #[test]
    fn r_arch_055__core_codes_are_unique_and_well_formed() {
        let mut seen = std::collections::BTreeSet::new();
        for c in codes::CORE_CODES {
            let s = c.0;
            let b = s.as_bytes();
            assert_eq!(b.len(), 7, "{s}");
            assert!(s.starts_with("HN-"), "{s}");
            assert!(b[3].is_ascii_uppercase(), "{s}");
            assert!(b[4..].iter().all(u8::is_ascii_digit), "{s}");
            assert!(seen.insert(s), "duplicate code {s}");
        }
        assert!(
            seen.len() > 100,
            "chapter 13 allocates well over 100 core codes"
        );
    }

    #[test]
    fn r_syn_041__control_family_codes_are_the_frozen_ones() {
        assert_eq!(codes::HN_C001.0, "HN-C001");
        assert_eq!(codes::HN_C002.0, "HN-C002");
        assert_eq!(codes::HN_C003.0, "HN-C003");
        assert_eq!(codes::HN_C004.0, "HN-C004");
        assert_eq!(codes::HN_C005.0, "HN-C005");
        assert_eq!(codes::HN_C006.0, "HN-C006");
        assert_eq!(codes::HN_P001.0, "HN-P001");
        assert_eq!(codes::HN_V013.0, "HN-V013");
    }

    #[test]
    fn r_arch_103__from_syntax_keeps_every_field_and_sets_path() {
        let sd = SyntaxDiag {
            code: codes::HN_S001,
            severity: Severity::Error,
            span: Span {
                start: 10,
                end: 14,
                line: 2,
                col: 4,
            },
            message: "unknown verb".into(),
            expected: Some("one of def use ovr flag env end".into()),
            did_you_mean: Some("def".into()),
            hint: "fix the verb".into(),
        };
        let d = Diagnostic::from_syntax(&rp("src/a.rs"), sd.clone());
        assert_eq!(d.code, sd.code);
        assert_eq!(
            d.location,
            Location {
                path: rp("src/a.rs"),
                line: 2,
                col: 4
            }
        );
        assert_eq!(d.message, "unknown verb");
        assert_eq!(
            d.expected.as_deref(),
            Some("one of def use ovr flag env end")
        );
        assert_eq!(d.did_you_mean.as_deref(), Some("def"));
        assert_eq!(d.hint, "fix the verb");
        let placeholder: Diagnostic = sd.into();
        assert!(placeholder.location.path.is_root());
    }

    #[test]
    fn r_arch_103__sort_is_path_line_col_code_message() {
        let mk = |p: &str, l: u32, c: u32, code: DiagCode, m: &str| {
            Diagnostic::new(code, Severity::Warn, Location::at(rp(p), l, c), m)
        };
        let mut v = vec![
            mk("b.rs", 1, 1, codes::HN_C001, "x"),
            mk("a.rs", 2, 1, codes::HN_C001, "x"),
            mk("a.rs", 1, 9, codes::HN_C001, "x"),
            mk("a.rs", 1, 1, codes::HN_C002, "x"),
            mk("a.rs", 1, 1, codes::HN_C001, "z"),
            mk("a.rs", 1, 1, codes::HN_C001, "y"),
        ];
        sort_diagnostics(&mut v);
        let got: Vec<(String, u32, u32, &str, String)> = v
            .iter()
            .map(|d| {
                (
                    d.location.path.to_string(),
                    d.location.line,
                    d.location.col,
                    d.code.0,
                    d.message.clone(),
                )
            })
            .collect();
        let want: Vec<(String, u32, u32, &str, String)> = vec![
            ("a.rs".into(), 1, 1, "HN-C001", "y".into()),
            ("a.rs".into(), 1, 1, "HN-C001", "z".into()),
            ("a.rs".into(), 1, 1, "HN-C002", "x".into()),
            ("a.rs".into(), 1, 9, "HN-C001", "x".into()),
            ("a.rs".into(), 2, 1, "HN-C001", "x".into()),
            ("b.rs".into(), 1, 1, "HN-C001", "x".into()),
        ];
        assert_eq!(got, want);
    }

    #[test]
    fn r_arch_103__whole_file_location_is_line_zero() {
        let l = Location::file(rp("a/b.toml"));
        assert_eq!((l.line, l.col), (0, 0));
    }
}
