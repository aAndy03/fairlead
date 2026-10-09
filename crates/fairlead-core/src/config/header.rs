//! Frozen `[fairlead]` header, parsed in a first lenient pass (D-33, R-SYN-038).
use crate::config::ConfigFailure;
use crate::diag::{Diagnostic, Location, Severity, codes};
use crate::discover::{Edition, Pin, parse_pin_version};
use crate::path::RepoPath;
use fairlead_syntax::ids::ScopeSlug;

/// The four frozen keys of `[fairlead]`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FairleadHeader {
    pub scope: ScopeSlug,
    pub edition: Option<Edition>,
    pub root: bool,
    pub marker: Option<String>,
}

pub(crate) const HEADER_KEYS: [&str; 4] = ["scope", "edition", "root", "marker"];

/// BOM stripped, CRLF and CR to LF, must be UTF-8.
pub(crate) fn decode_control(bytes: &[u8]) -> Result<String, String> {
    let lf = fairlead_syntax::normalize::lf_bytes(bytes);
    String::from_utf8(lf.into_owned()).map_err(|e| {
        format!(
            "control file is not valid UTF-8 (first bad byte at {})",
            e.utf8_error().valid_up_to()
        )
    })
}

/// 1-based line and column (scalar values) of a byte offset.
pub(crate) fn line_col(text: &str, offset: usize) -> (u32, u32) {
    let (mut line, mut col) = (1u32, 1u32);
    for (i, c) in text.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Best-effort 1-based line of a dotted key path: the first `key =` line or table header that
/// mentions the last segment. `toml::Table` carries no spans, so this is a documented heuristic.
pub(crate) fn locate_key(text: &str, dotted: &str) -> u32 {
    let last = dotted.rsplit('.').next().unwrap_or(dotted);
    if last.is_empty() {
        return 1;
    }
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim_start();
        let bare = line
            .strip_prefix(last)
            .is_some_and(|rest| rest.trim_start().starts_with('='));
        let quoted = line
            .strip_prefix('"')
            .and_then(|r| r.strip_prefix(last))
            .is_some_and(|r| r.starts_with('"'));
        let header = line.starts_with('[') && line.contains(last);
        if bare || quoted || header {
            return u32::try_from(i + 1).unwrap_or(1);
        }
    }
    1
}

pub(crate) fn error_diag(line: u32, col: u32, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        codes::HN_C001,
        Severity::Error,
        Location::at(RepoPath::root(), line, col),
        message,
    )
    .with_hint("fix the control file; `fairlead explain syntax` shows the schema")
}

/// A failure at the line of `key` (path is the root placeholder; callers use `with_path`).
pub(crate) fn failure(text: &str, key: &str, message: impl Into<String>) -> ConfigFailure {
    ConfigFailure {
        path: RepoPath::root(),
        diags: vec![error_diag(locate_key(text, key), 1, message)],
    }
}

pub(crate) fn toml_failure(text: &str, e: &toml::de::Error) -> ConfigFailure {
    let (line, col) = e.span().map(|s| line_col(text, s.start)).unwrap_or((1, 1));
    ConfigFailure {
        path: RepoPath::root(),
        diags: vec![error_diag(
            line,
            col,
            format!("control file is not valid TOML: {}", e.message()),
        )],
    }
}

/// Decode and parse into a generic table (any later-edition content is tolerated).
pub(crate) fn parse_table(bytes: &[u8]) -> Result<(String, toml::Table), ConfigFailure> {
    let text = decode_control(bytes).map_err(|m| failure("", "", m))?;
    let table = text
        .parse::<toml::Table>()
        .map_err(|e| toml_failure(&text, &e))?;
    Ok((text, table))
}

pub(crate) fn header_from_table(
    text: &str,
    h: &toml::Table,
) -> Result<FairleadHeader, ConfigFailure> {
    let scope_s = match h.get("scope") {
        Some(toml::Value::String(s)) => s.as_str(),
        Some(_) => {
            return Err(failure(
                text,
                "fairlead.scope",
                "[fairlead] scope must be a string",
            ));
        }
        None => {
            return Err(failure(
                text,
                "fairlead",
                "[fairlead] scope is required in every control file",
            ));
        }
    };
    let scope = ScopeSlug::parse(scope_s).map_err(|e| {
        failure(
            text,
            "fairlead.scope",
            format!("invalid scope id {scope_s:?}: {e:?}"),
        )
    })?;
    let edition = match h.get("edition") {
        None => None,
        Some(toml::Value::Integer(n)) => match u32::try_from(*n) {
            Ok(v) => Some(Edition(v)),
            Err(_) => {
                return Err(failure(
                    text,
                    "fairlead.edition",
                    "edition must be a non-negative integer",
                ));
            }
        },
        Some(_) => {
            return Err(failure(
                text,
                "fairlead.edition",
                "edition must be an integer such as 1",
            ));
        }
    };
    let root = match h.get("root") {
        None => false,
        Some(toml::Value::Boolean(b)) => *b,
        Some(_) => return Err(failure(text, "fairlead.root", "root must be true or false")),
    };
    let marker = match h.get("marker") {
        None => None,
        Some(toml::Value::String(m))
            if !m.is_empty()
                && m.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') =>
        {
            Some(m.clone())
        }
        Some(_) => {
            return Err(failure(
                text,
                "fairlead.marker",
                "marker must be a non-empty word (letters, digits, - and _)",
            ));
        }
    };
    Ok(FairleadHeader {
        scope,
        edition,
        root,
        marker,
    })
}

fn fairlead_table<'a>(text: &str, t: &'a toml::Table) -> Result<&'a toml::Table, ConfigFailure> {
    match t.get("fairlead") {
        Some(toml::Value::Table(h)) => Ok(h),
        _ => Err(failure(
            text,
            "fairlead",
            "a control file must contain a [fairlead] header table",
        )),
    }
}

/// First, lenient pass: only `[fairlead]` is interpreted (D-33).
pub fn parse_header_lenient(bytes: &[u8]) -> Result<FairleadHeader, ConfigFailure> {
    let (text, table) = parse_table(bytes)?;
    let h = fairlead_table(&text, &table)?;
    header_from_table(&text, h)
}

/// Root-file variant used by discovery before anything else (R-ARCH-013): the header plus the pin.
/// A root without `edition` is `HN-C001` (C-34).
pub fn parse_pin_lenient(bytes: &[u8]) -> Result<(FairleadHeader, Pin), ConfigFailure> {
    let (text, table) = parse_table(bytes)?;
    let header = header_from_table(&text, fairlead_table(&text, &table)?)?;
    let Some(edition) = header.edition else {
        return Err(failure(
            &text,
            "fairlead",
            "the root control file requires `edition` in [fairlead]",
        ));
    };
    let version = match table.get("settings") {
        Some(toml::Value::Table(s)) => match s.get("fairlead-version") {
            None => None,
            Some(toml::Value::String(v)) => Some(parse_pin_version(v).ok_or_else(|| {
                failure(
                    &text,
                    "settings.fairlead-version",
                    format!("fairlead-version {v:?} must look like \"0.1\" or \"0.1.0\""),
                )
            })?),
            Some(_) => {
                return Err(failure(
                    &text,
                    "settings.fairlead-version",
                    "fairlead-version must be a string",
                ));
            }
        },
        _ => None,
    };
    Ok((header, Pin { version, edition }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;

    const ROOT: &str =
        "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n";

    #[test]
    fn r_syn_038__header_reads_all_four_frozen_keys() {
        let h = parse_header_lenient(ROOT.as_bytes()).unwrap();
        assert_eq!(h.scope.as_str(), "root");
        assert_eq!(h.edition, Some(Edition(1)));
        assert!(h.root);
        assert_eq!(h.marker.as_deref(), Some("fairlead"));
    }

    #[test]
    fn r_syn_038__non_root_header_carries_only_scope() {
        let h = parse_header_lenient(b"[fairlead]\nscope = \"web\"\n").unwrap();
        assert_eq!((h.edition, h.root, h.marker), (None, false, None));
    }

    #[test]
    fn r_syn_038__header_requires_a_valid_scope() {
        for bad in [
            "[fairlead]\n",
            "[fairlead]\nscope = 3\n",
            "[fairlead]\nscope = \"Bad_Scope\"\n",
            "[fairlead]\nscope = \"con\"\n",
            "[settings]\nfolder = \"fairlead\"\n",
        ] {
            let f = parse_header_lenient(bad.as_bytes()).unwrap_err();
            assert_eq!(f.diags[0].code, codes::HN_C001, "{bad:?}");
        }
    }

    #[test]
    fn r_syn_038__wrong_types_in_header_are_hc001() {
        for bad in [
            "[fairlead]\nscope = \"a\"\nedition = \"1\"\n",
            "[fairlead]\nscope = \"a\"\nedition = -1\n",
            "[fairlead]\nscope = \"a\"\nroot = \"yes\"\n",
            "[fairlead]\nscope = \"a\"\nmarker = \"\"\n",
            "[fairlead]\nscope = \"a\"\nmarker = \"has space\"\n",
        ] {
            assert!(parse_header_lenient(bad.as_bytes()).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn r_syn_038__lenient_pass_ignores_later_tables_and_unknown_header_keys() {
        let text = "[fairlead]\nscope = \"a\"\nedition = 9\nfuture-key = [1, 2]\n\n[settings]\nfairlead-version = 5\n\n[future.table]\nx = 1\n";
        let h = parse_header_lenient(text.as_bytes()).unwrap();
        assert_eq!(
            h.edition,
            Some(Edition(9)),
            "a later edition still yields its header (D-33)"
        );
    }

    #[test]
    fn r_syn_039__duplicate_keys_are_a_parse_error_with_a_line() {
        let f = parse_header_lenient(b"[fairlead]\nscope = \"a\"\nscope = \"b\"\n").unwrap_err();
        assert_eq!(f.diags[0].code, codes::HN_C001);
        assert!(f.diags[0].location.line >= 2, "{:?}", f.diags[0].location);
    }

    #[test]
    fn r_syn_039__invalid_toml_is_hc001() {
        let f = parse_header_lenient(b"[fairlead\nscope = \n").unwrap_err();
        assert_eq!(f.diags[0].code, codes::HN_C001);
        assert!(
            f.diags[0].message.contains("TOML"),
            "{}",
            f.diags[0].message
        );
    }

    #[test]
    fn r_syn_038__bom_crlf_and_non_utf8() {
        let crlf = format!("\u{feff}{}", ROOT.replace('\n', "\r\n"));
        assert!(parse_header_lenient(crlf.as_bytes()).is_ok());
        let f = parse_header_lenient(b"[fairlead]\nscope = \"a\xFF\"\n").unwrap_err();
        assert!(
            f.diags[0].message.contains("UTF-8"),
            "{}",
            f.diags[0].message
        );
    }

    #[test]
    fn r_syn_038__pin_needs_edition_and_pads_the_version() {
        let text = format!("{ROOT}\n[settings]\nfairlead-version = \"0.1\"\n");
        let (h, pin) = parse_pin_lenient(text.as_bytes()).unwrap();
        assert!(h.root);
        assert_eq!(pin.edition, Edition(1));
        assert_eq!(pin.version, Some(semver::Version::new(0, 1, 0)));
        let (_, none) = parse_pin_lenient(ROOT.as_bytes()).unwrap();
        assert_eq!(none.version, None);
        let f = parse_pin_lenient(b"[fairlead]\nscope = \"root\"\nroot = true\n").unwrap_err();
        assert!(
            f.diags[0].message.contains("edition"),
            "C-34: a root without edition is HN-C001"
        );
        let f = parse_pin_lenient(
            format!("{ROOT}\n[settings]\nfairlead-version = \"abc\"\n").as_bytes(),
        )
        .unwrap_err();
        assert!(f.diags[0].message.contains("fairlead-version"));
    }

    #[test]
    fn r_syn_039__line_col_counts_scalar_values() {
        assert_eq!(line_col("ab\ncd", 4), (2, 2));
        assert_eq!(
            line_col("é=1\nx", 4),
            (1, 4),
            "a multi-byte char counts once"
        );
        assert_eq!(locate_key("[a]\nfoo = 1\n[b]\nbar = 2\n", "b.bar"), 4);
        assert_eq!(locate_key("x = 1\n", "missing"), 1);
    }
}
