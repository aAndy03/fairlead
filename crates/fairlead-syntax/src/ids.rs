//! Identifiers: `[scope:]kind.name` (R-SYN-007, R-SYN-024..027). Pure lexical validation.
use core::cmp::Ordering;
use core::fmt;
use serde::{Deserialize, Serialize};

/// R-CLI-031 scaffold slot value that `check` rejects (C-31).
pub const PLACEHOLDER: &str = "TODO(fairlead)";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum IdError {
    Empty,
    BadChar { at: usize },
    TooLong,
    ReservedName,
    BadKind,
    BadShape,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Empty => f.write_str("empty ID"),
            IdError::BadChar { at } => write!(f, "character not allowed at byte {at}"),
            IdError::TooLong => f.write_str("ID part too long"),
            IdError::ReservedName => f.write_str("Windows-reserved name"),
            IdError::BadKind => f.write_str("kind must be 2 to 8 lowercase letters"),
            IdError::BadShape => f.write_str("expected [scope:]kind.name"),
        }
    }
}

/// R-SYN-027: con prn aux nul com1..9 lpt1..9, trailing dot or space.
pub fn is_reserved_windows_name(s: &str) -> bool {
    if s.ends_with('.') || s.ends_with(' ') {
        return true;
    }
    let l = s.to_ascii_lowercase();
    if matches!(l.as_str(), "con" | "prn" | "aux" | "nul") {
        return true;
    }
    for p in ["com", "lpt"] {
        if let Some(d) = l.strip_prefix(p)
            && d.len() == 1
            && d.bytes().all(|b| (b'1'..=b'9').contains(&b))
        {
            return true;
        }
    }
    false
}

/// Validates `[a-z0-9]+(-[a-z0-9]+)*` of at most `max` bytes. `base` is the offset of `s` in the whole ID.
fn check_slug(s: &str, base: usize, max: usize) -> Result<(), IdError> {
    if s.is_empty() {
        return Err(IdError::BadShape);
    }
    for (i, c) in s.bytes().enumerate() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-') {
            return Err(IdError::BadChar { at: base + i });
        }
    }
    if s.len() > max {
        return Err(IdError::TooLong);
    }
    if s.starts_with('-') || s.ends_with('-') || s.contains("--") {
        return Err(IdError::BadShape);
    }
    Ok(())
}

/// Scope id: `[a-z0-9]+(-[a-z0-9]+)*`, at most 32, not Windows-reserved (R-SYN-027).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ScopeSlug(String);

impl ScopeSlug {
    pub fn parse(s: &str) -> Result<Self, IdError> {
        ScopeSlug::parse_at(s, 0)
    }
    fn parse_at(s: &str, base: usize) -> Result<Self, IdError> {
        if s.is_empty() {
            return Err(IdError::Empty);
        }
        check_slug(s, base, 32)?;
        if is_reserved_windows_name(s) {
            return Err(IdError::ReservedName);
        }
        Ok(ScopeSlug(s.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Kind: 2 to 8 lowercase ASCII letters (R-SYN-024).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Kind(String);

impl Kind {
    pub fn parse(s: &str) -> Result<Self, IdError> {
        Kind::parse_at(s, 0)
    }
    fn parse_at(s: &str, base: usize) -> Result<Self, IdError> {
        for (i, c) in s.bytes().enumerate() {
            if !c.is_ascii_lowercase() {
                return Err(IdError::BadChar { at: base + i });
            }
        }
        if !(2..=8).contains(&s.len()) {
            return Err(IdError::BadKind);
        }
        Ok(Kind(s.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Name: `slug *("." slug)`, at most 64 bytes in total, each segment at most 32.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Name(String);

impl Name {
    pub fn parse(s: &str) -> Result<Self, IdError> {
        Name::parse_at(s, 0)
    }
    fn parse_at(s: &str, base: usize) -> Result<Self, IdError> {
        if s.is_empty() {
            return Err(IdError::BadShape);
        }
        let mut off = base;
        for seg in s.split('.') {
            check_slug(seg, off, 32)?;
            if is_reserved_windows_name(seg) {
                return Err(IdError::ReservedName);
            }
            off += seg.len() + 1;
        }
        if s.len() > 64 {
            return Err(IdError::TooLong);
        }
        Ok(Name(s.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum IdQualifier {
    Bare,
    Root,
    Scope(ScopeSlug),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Id {
    pub qualifier: IdQualifier,
    pub kind: Kind,
    pub name: Name,
}

impl Id {
    /// Grammar of R-SYN-007. Never panics.
    pub fn parse(s: &str) -> Result<Id, IdError> {
        if s.is_empty() {
            return Err(IdError::Empty);
        }
        let (qualifier, rest, base) = match s.split_once(':') {
            None => (IdQualifier::Bare, s, 0usize),
            Some(("", r)) => (IdQualifier::Root, r, 1),
            Some((sc, r)) => (IdQualifier::Scope(ScopeSlug::parse(sc)?), r, sc.len() + 1),
        };
        if rest.contains(':') {
            return Err(IdError::BadShape);
        }
        let (kind_s, name_s) = rest.split_once('.').ok_or(IdError::BadShape)?;
        if kind_s.is_empty() {
            return Err(IdError::BadKind);
        }
        let kind = Kind::parse_at(kind_s, base)?;
        let name = Name::parse_at(name_s, base + kind_s.len() + 1)?;
        Ok(Id {
            qualifier,
            kind,
            name,
        })
    }

    /// `"kind.name"` without the qualifier.
    pub fn kind_name(&self) -> String {
        format!("{}.{}", self.kind.0, self.name.0)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.qualifier {
            IdQualifier::Bare => {}
            IdQualifier::Root => f.write_str(":")?,
            IdQualifier::Scope(s) => write!(f, "{}:", s.0)?,
        }
        write!(f, "{}.{}", self.kind.0, self.name.0)
    }
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Id {
    /// Bytewise on the canonical text (contract 4.1, R-ARCH-060).
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_syn_007__id_forms_bare_root_and_scoped() {
        let a = Id::parse("cmp.sync-button").unwrap();
        assert_eq!(a.qualifier, IdQualifier::Bare);
        assert_eq!(a.kind_name(), "cmp.sync-button");
        let r = Id::parse(":tok.primary").unwrap();
        assert_eq!(r.qualifier, IdQualifier::Root);
        assert_eq!(r.to_string(), ":tok.primary");
        let s = Id::parse("web:cmp.button").unwrap();
        assert_eq!(s.to_string(), "web:cmp.button");
        let dotted = Id::parse("adr.0003.sub-part").unwrap();
        assert_eq!(dotted.name.as_str(), "0003.sub-part");
    }

    #[test]
    fn r_syn_007__id_is_ascii_lowercase_only() {
        assert_eq!(Id::parse("Cmp.x"), Err(IdError::BadChar { at: 0 }));
        assert_eq!(Id::parse("cmp.Xy"), Err(IdError::BadChar { at: 4 }));
        assert_eq!(
            Id::parse("cmp.\u{e9}t\u{e9}"),
            Err(IdError::BadChar { at: 4 })
        );
    }

    #[test]
    fn r_syn_007__id_shape_errors() {
        assert_eq!(Id::parse(""), Err(IdError::Empty));
        assert_eq!(Id::parse("cmp"), Err(IdError::BadShape));
        assert_eq!(Id::parse("cmp."), Err(IdError::BadShape));
        assert_eq!(Id::parse("a:b:cmp.x"), Err(IdError::BadShape));
        assert_eq!(Id::parse("cmp.x..y"), Err(IdError::BadShape));
        assert_eq!(Id::parse("cmp.-x"), Err(IdError::BadShape));
        assert_eq!(
            Id::parse("c.x"),
            Err(IdError::BadKind),
            "kind needs 2..=8 letters"
        );
        assert_eq!(Id::parse("abcdefghi.x"), Err(IdError::BadKind));
        assert!(matches!(
            Id::parse("tauri-command.x"),
            Err(IdError::BadChar { at: 5 })
        ));
    }

    #[test]
    fn r_syn_007__id_length_limits() {
        let long_name = format!("cmp.{}", "a".repeat(65));
        assert_eq!(Id::parse(&long_name), Err(IdError::TooLong));
        let ok_name = format!("cmp.{}.{}", "a".repeat(32), "b".repeat(31));
        assert!(Id::parse(&ok_name).is_ok());
        let long_scope = format!("{}:cmp.x", "a".repeat(33));
        assert_eq!(Id::parse(&long_scope), Err(IdError::TooLong));
    }

    #[test]
    fn r_syn_007__ord_is_bytewise_on_display() {
        let mut v: Vec<Id> = ["web:cmp.b", ":cmp.a", "cmp.a", "cmp.b"]
            .iter()
            .map(|s| Id::parse(s).unwrap())
            .collect();
        v.sort();
        let shown: Vec<String> = v.iter().map(|i| i.to_string()).collect();
        let mut expect = shown.clone();
        expect.sort();
        assert_eq!(shown, expect);
        assert_eq!(shown.first().map(String::as_str), Some(":cmp.a"));
    }

    #[test]
    fn r_syn_027__windows_reserved_names() {
        for n in [
            "con", "PRN", "aux", "nul", "com1", "COM9", "lpt1", "lpt9", "web.", "x ",
        ] {
            assert!(is_reserved_windows_name(n), "{n}");
        }
        for n in ["com0", "com10", "console", "web", "lpt"] {
            assert!(!is_reserved_windows_name(n), "{n}");
        }
    }

    #[test]
    fn r_syn_027__reserved_scope_and_name_segments_are_rejected() {
        assert_eq!(ScopeSlug::parse("con"), Err(IdError::ReservedName));
        assert_eq!(Id::parse("nul:cmp.x"), Err(IdError::ReservedName));
        assert_eq!(Id::parse("cmp.aux"), Err(IdError::ReservedName));
        assert_eq!(Id::parse("cmp.ok.com3"), Err(IdError::ReservedName));
        assert!(ScopeSlug::parse("web").is_ok());
    }

    #[test]
    fn r_syn_007__parse_never_panics_on_junk() {
        for s in [
            "::",
            ":",
            ".",
            ":.",
            "..",
            "a:",
            ":a",
            "\u{0}",
            "\u{feff}cmp.x",
            "cmp.x\n",
            "é:é.é",
            "-:-.-",
            "a:b.c:d",
        ] {
            let _ = Id::parse(s);
        }
    }

    #[test]
    fn r_cli_031__placeholder_constant() {
        assert_eq!(PLACEHOLDER, "TODO(fairlead)");
    }
}
