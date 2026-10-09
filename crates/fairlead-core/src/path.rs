//! Repo-relative, UTF-8, forward-slash paths (50-architecture Section 10).
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use unicode_normalization::{UnicodeNormalization, is_nfc};

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum PathError {
    #[error("path must be repo-relative (no leading '/')")]
    Absolute,
    #[error("path must use '/' separators")]
    Backslash,
    #[error("path has an empty segment")]
    EmptySegment,
    #[error("path has a '.' segment")]
    DotSegment,
    #[error("path has a '..' segment")]
    ParentSegment,
    #[error("path contains a control character")]
    ControlChar,
    #[error("path starts with a drive letter")]
    DriveLetter,
    #[error("a joined segment must be a single path segment")]
    BadSegment,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoPath(String);

/// One `HN-P001` reason (R-SYN-040, R-SYN-027).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PathIssue {
    NotNfc,
    ReservedName { segment: String },
    TrailingDotOrSpace { segment: String },
}

fn validate(s: &str) -> Result<(), PathError> {
    if s.is_empty() {
        return Ok(());
    }
    if s.starts_with('/') {
        return Err(PathError::Absolute);
    }
    if s.contains('\\') {
        return Err(PathError::Backslash);
    }
    if s.chars().any(char::is_control) {
        return Err(PathError::ControlChar);
    }
    let b = s.as_bytes();
    if let (Some(first), Some(second)) = (b.first(), b.get(1))
        && *second == b':'
        && first.is_ascii_alphabetic()
    {
        return Err(PathError::DriveLetter);
    }
    for seg in s.split('/') {
        match seg {
            "" => return Err(PathError::EmptySegment),
            "." => return Err(PathError::DotSegment),
            ".." => return Err(PathError::ParentSegment),
            _ => {}
        }
    }
    Ok(())
}

impl RepoPath {
    pub fn new(s: &str) -> Result<Self, PathError> {
        validate(s)?;
        Ok(Self(s.to_string()))
    }

    /// The repository root (empty path).
    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Last segment; empty for the root.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or("")
    }

    /// Convert a native relative path (possibly with `\`) into a `RepoPath`.
    pub fn from_native_rel(p: &camino::Utf8Path) -> Result<Self, PathError> {
        let s = p.as_str().replace('\\', "/");
        let s = s.strip_prefix("./").unwrap_or(&s);
        Self::new(s)
    }

    /// NFC, simple case folding, NFC again: the lookup key for scope membership (R-SCOPE-007).
    /// Verify: full Unicode case folding (sharp s to "ss") is not applied; NTFS and APFS use simple
    /// case mapping, which `char::to_lowercase` approximates.
    pub fn fold_key(&self) -> String {
        let nfc: String = self.0.nfc().collect();
        let lower: String = nfc.chars().flat_map(char::to_lowercase).collect();
        lower.nfc().collect()
    }

    pub fn parent(&self) -> Option<RepoPath> {
        if self.0.is_empty() {
            return None;
        }
        match self.0.rsplit_once('/') {
            Some((head, _)) => Some(Self(head.to_string())),
            None => Some(Self::root()),
        }
    }

    pub fn join(&self, seg: &str) -> Result<RepoPath, PathError> {
        if seg.contains('/') {
            return Err(PathError::BadSegment);
        }
        let joined = if self.0.is_empty() {
            seg.to_string()
        } else {
            format!("{}/{}", self.0, seg)
        };
        Self::new(&joined)
    }

    /// Extension without the dot; a dotfile such as `.gitignore` yields `gitignore` (R-SYN-006 table).
    pub fn extension(&self) -> &str {
        match self.file_name().rsplit_once('.') {
            Some((_, ext)) => ext,
            None => "",
        }
    }

    /// True when `self` equals `dir` or lies below it (every path is within the root).
    pub fn is_within(&self, dir: &RepoPath) -> bool {
        dir.is_root()
            || self.0 == dir.0
            || (self.0.starts_with(&dir.0) && self.0.as_bytes().get(dir.0.len()) == Some(&b'/'))
    }

    /// `HN-P001` reasons (R-SYN-040): non-NFC, Windows-reserved, trailing dot or space.
    pub fn hygiene(&self) -> Vec<PathIssue> {
        let mut out = Vec::new();
        if !is_nfc(&self.0) {
            out.push(PathIssue::NotNfc);
        }
        for seg in self.0.split('/').filter(|s| !s.is_empty()) {
            if seg.ends_with('.') || seg.ends_with(' ') {
                out.push(PathIssue::TrailingDotOrSpace {
                    segment: seg.to_string(),
                });
                continue;
            }
            let lower = seg.to_lowercase();
            let stem = lower.split('.').next().unwrap_or("");
            if fairlead_syntax::ids::is_reserved_windows_name(&lower)
                || fairlead_syntax::ids::is_reserved_windows_name(stem)
            {
                out.push(PathIssue::ReservedName {
                    segment: seg.to_string(),
                });
            }
        }
        out
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for RepoPath {
    type Error = PathError;
    fn try_from(s: String) -> Result<Self, PathError> {
        validate(&s)?;
        Ok(Self(s))
    }
}

impl From<RepoPath> for String {
    fn from(p: RepoPath) -> String {
        p.0
    }
}

/// Groups of paths (files or directories) that differ only in case or normalisation form and share
/// one parent, sorted by fold key then bytewise (R-SCOPE-007). A collision caused only by a
/// colliding ancestor is not repeated.
pub fn find_case_collisions<'a, I>(paths: I) -> Vec<Vec<RepoPath>>
where
    I: IntoIterator<Item = &'a RepoPath>,
{
    let mut groups: BTreeMap<String, BTreeSet<RepoPath>> = BTreeMap::new();
    for p in paths {
        let mut cur = p.clone();
        while !cur.is_root() {
            groups
                .entry(cur.fold_key())
                .or_default()
                .insert(cur.clone());
            match cur.parent() {
                Some(next) => cur = next,
                None => break,
            }
        }
    }
    groups
        .into_values()
        .filter(|g| g.len() > 1)
        .filter(|g| {
            let mut parents = g.iter().map(RepoPath::parent);
            let first = parents.next();
            parents.all(|p| Some(p) == first)
        })
        .map(|g| g.into_iter().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rp(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }

    #[test]
    fn r_arch_024__repo_path_orders_bytewise() {
        let mut v: Vec<RepoPath> = ["b", "a/b", "a", "a/a", "B"]
            .iter()
            .map(|s| rp(s))
            .collect();
        v.sort();
        let got: Vec<&str> = v.iter().map(RepoPath::as_str).collect();
        assert_eq!(got, ["B", "a", "a/a", "a/b", "b"]);
    }

    #[test]
    fn r_syn_040__new_rejects_structural_garbage() {
        for bad in [
            "/abs", "a\\b", "a//b", "../x", "a/../b", "a/./b", "C:/x", "a/", "x\u{0}y", "x\ny",
        ] {
            assert!(RepoPath::new(bad).is_err(), "{bad:?} must be rejected");
        }
        assert!(RepoPath::new("").unwrap().is_root());
        assert!(RepoPath::new("src/lib.rs").is_ok());
    }

    #[test]
    fn r_syn_040__hygiene_reports_nfc_reserved_and_trailing() {
        assert_eq!(rp("cafe\u{301}.txt").hygiene(), vec![PathIssue::NotNfc]);
        assert_eq!(
            rp("src/CON.txt").hygiene(),
            vec![PathIssue::ReservedName {
                segment: "CON.txt".into()
            }]
        );
        assert_eq!(
            rp("docs/readme ").hygiene(),
            vec![PathIssue::TrailingDotOrSpace {
                segment: "readme ".into()
            }]
        );
        assert!(rp("src/lib.rs").hygiene().is_empty());
    }

    #[test]
    fn r_scope_007__case_only_directories_collide() {
        let ps: Vec<RepoPath> = ["Ui/a.rs", "ui/a.rs", "ui/b.rs", "docs/x.md"]
            .iter()
            .map(|s| rp(s))
            .collect();
        let groups = find_case_collisions(ps.iter());
        assert_eq!(
            groups.len(),
            1,
            "only the Ui/ui pair is reported, not Ui/a.rs vs ui/a.rs"
        );
        let names: Vec<&str> = groups[0].iter().map(RepoPath::as_str).collect();
        assert_eq!(names, ["Ui", "ui"]);
    }

    #[test]
    fn r_scope_007__case_only_files_collide() {
        let ps = [rp("a/README.md"), rp("a/readme.md")];
        assert_eq!(
            find_case_collisions(ps.iter()),
            vec![vec![rp("a/README.md"), rp("a/readme.md")]]
        );
    }

    #[test]
    fn r_scope_007__fold_key_is_nfc_and_case_insensitive() {
        let a = rp("Src/Cafe\u{301}.RS"); // NFD
        let b = rp("src/caf\u{e9}.rs"); // NFC
        assert_eq!(a.fold_key(), b.fold_key());
        assert_ne!(a, b, "the on-disk spelling is preserved for display");
    }

    #[test]
    fn r_arch_024__parent_join_extension_within() {
        let p = rp("a/b/c.test.ts");
        assert_eq!(p.parent().unwrap().as_str(), "a/b");
        assert_eq!(rp("a").parent().unwrap(), RepoPath::root());
        assert!(RepoPath::root().parent().is_none());
        assert_eq!(p.extension(), "ts");
        assert_eq!(rp(".gitignore").extension(), "gitignore");
        assert_eq!(rp("Makefile").extension(), "");
        assert_eq!(RepoPath::root().join("x").unwrap().as_str(), "x");
        assert_eq!(rp("a").join("b.rs").unwrap().as_str(), "a/b.rs");
        assert!(rp("a").join("../b").is_err());
        let a = rp("a");
        assert!(p.is_within(&a));
        assert!(a.is_within(&a));
        assert!(!rp("ab/c").is_within(&a), "a name prefix is not a parent");
        assert!(p.is_within(&RepoPath::root()));
    }

    #[test]
    fn r_arch_024__from_native_rel_converts_separators() {
        assert_eq!(
            RepoPath::from_native_rel(camino::Utf8Path::new("a\\b\\c.rs"))
                .unwrap()
                .as_str(),
            "a/b/c.rs"
        );
        assert_eq!(
            RepoPath::from_native_rel(camino::Utf8Path::new("./x.rs"))
                .unwrap()
                .as_str(),
            "x.rs"
        );
    }

    #[test]
    fn r_arch_024__serde_rejects_invalid_paths() {
        assert!(serde_json::from_str::<RepoPath>("\"../x\"").is_err());
        assert_eq!(
            serde_json::from_str::<RepoPath>("\"a/b\"")
                .unwrap()
                .as_str(),
            "a/b"
        );
        assert_eq!(serde_json::to_string(&rp("a/b")).unwrap(), "\"a/b\"");
    }
}
