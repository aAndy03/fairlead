//! The global version of declared intent: `MAJOR.MINOR.PATCH` (D-42), bumped by the largest
//! change-doc impact (20-versions, bump table). Independent of any package version (R-VER-004).

use std::fmt;

/// A sealed or open version. Exactly three numeric parts; a two-part value, a pre-release or
/// build suffix is not a version (D-42, R-VER-070).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    pub const fn new(major: u64, minor: u64, patch: u64) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }

    /// Strict parse of `MAJOR.MINOR.PATCH`: no leading zeros, no sign, no suffix.
    pub fn parse(s: &str) -> Option<Version> {
        let mut parts = s.split('.');
        let mut next = || -> Option<u64> {
            let p = parts.next()?;
            let canonical = !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && (p == "0" || !p.starts_with('0'));
            canonical.then(|| p.parse().ok()).flatten()
        };
        let v = Version::new(next()?, next()?, next()?);
        parts.next().is_none().then_some(v)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Change-doc `impact`; the largest pending impact decides the bump.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Impact {
    Fix,
    Add,
    Break,
}

impl Impact {
    pub fn parse(s: &str) -> Option<Impact> {
        match s {
            "fix" => Some(Impact::Fix),
            "add" => Some(Impact::Add),
            "break" => Some(Impact::Break),
            _ => None,
        }
    }
}

/// The open version after `sealed`. A fresh codebase opens at `start` (R-VER-003). While `MAJOR`
/// is 0 each bump shifts one level down: `break` bumps minor, `add` and `fix` bump patch (R-VER-006).
pub fn bump(sealed: Option<Version>, impact: Impact, start: Version) -> Version {
    let Some(s) = sealed else {
        return start;
    };
    let level = if s.major == 0 {
        match impact {
            Impact::Break => Impact::Add,
            Impact::Add | Impact::Fix => Impact::Fix,
        }
    } else {
        impact
    };
    match level {
        Impact::Break => Version::new(s.major + 1, 0, 0),
        Impact::Add => Version::new(s.major, s.minor + 1, 0),
        Impact::Fix => Version::new(s.major, s.minor, s.patch + 1),
    }
}

/// `S` (highest sealed) and `O` (open), both derived on every read, never stored (R-VER-001,
/// R-VER-002).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct VersionLine {
    pub sealed: Option<Version>,
    pub open: Version,
}

/// The version a file name `versions/<MAJOR.MINOR.PATCH>.md` names, if any.
pub fn version_of_file_name(name: &str) -> Option<Version> {
    Version::parse(name.strip_suffix(".md")?)
}

/// `S` is the maximum over the version files present; `O` bumps it by the largest impact among
/// pending docs (`fix` when none is pending, so `O` is always ahead of `S`). With no version
/// file, `S` is none and `O` is `start` (R-VER-001 to R-VER-003).
pub fn derive_line(
    version_files: impl IntoIterator<Item = Version>,
    pending: impl IntoIterator<Item = Impact>,
    start: Version,
) -> VersionLine {
    let sealed = version_files.into_iter().max();
    let impact = pending.into_iter().max().unwrap_or(Impact::Fix);
    VersionLine {
        sealed,
        open: bump(sealed, impact, start),
    }
}

/// R-VER-004: a configured mirror (for example the Cargo workspace version) that differs from
/// `S` is reported, never failed. Returns the warning text, or `None` when they agree or no
/// version is sealed yet.
pub fn mirror_divergence(line: &VersionLine, mirror: Option<&str>) -> Option<String> {
    let sealed = line.sealed?;
    let m = mirror?;
    (Version::parse(m) != Some(sealed)).then(|| {
        format!("version mirror {m} differs from the sealed version {sealed}; the global version is independent")
    })
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("{s} parses"))
    }

    #[test]
    fn r_ver_006__major_zero_shifts_one_level_down() {
        let start = v("0.1.0");
        assert_eq!(bump(Some(v("0.4.2")), Impact::Break, start), v("0.5.0"));
        assert_eq!(bump(Some(v("0.4.2")), Impact::Add, start), v("0.4.3"));
        assert_eq!(bump(Some(v("0.4.2")), Impact::Fix, start), v("0.4.3"));
        assert_eq!(bump(Some(v("1.4.2")), Impact::Break, start), v("2.0.0"));
        assert_eq!(bump(Some(v("1.4.2")), Impact::Add, start), v("1.5.0"));
        assert_eq!(bump(Some(v("1.4.2")), Impact::Fix, start), v("1.4.3"));
    }

    #[test]
    fn r_ver_003__fresh_codebase_opens_at_start() {
        assert_eq!(bump(None, Impact::Break, v("0.1.0")), v("0.1.0"));
    }

    #[test]
    fn r_ver_001__sealed_is_max_of_version_files() {
        let names = ["0.9.0.md", "0.10.0.md", "0.2.1.md", "notes.md", "1.0.md"];
        let files = names.iter().filter_map(|n| version_of_file_name(n));
        let line = derive_line(files, [Impact::Add, Impact::Fix], v("0.1.0"));
        assert_eq!(line.sealed, Some(v("0.10.0")));
        assert_eq!(line.open, v("0.10.1"), "add while major is 0 bumps patch");
    }

    #[test]
    fn r_ver_003__fresh_codebase_line_is_none_and_start() {
        let line = derive_line([], [Impact::Break], v("0.1.0"));
        assert_eq!(
            line,
            VersionLine {
                sealed: None,
                open: v("0.1.0")
            }
        );
    }

    #[test]
    fn r_ver_002__open_is_ahead_of_sealed_with_nothing_pending() {
        let line = derive_line([v("1.2.3")], [], v("0.1.0"));
        assert_eq!(line.open, v("1.2.4"));
    }

    #[test]
    fn r_ver_004__mirror_divergence_warns_only() {
        let line = derive_line([v("1.2.0")], [], v("0.1.0"));
        assert!(mirror_divergence(&line, Some("1.3.0")).is_some());
        assert!(mirror_divergence(&line, Some("1.2.0")).is_none());
        assert!(mirror_divergence(&line, None).is_none());
        let fresh = derive_line([], [], v("0.1.0"));
        assert!(mirror_divergence(&fresh, Some("9.9.9")).is_none());
    }

    #[test]
    fn d_42__two_part_value_is_not_a_version() {
        for bad in [
            "1.2",
            "1",
            "",
            "1.2.3.4",
            "1.02.3",
            "v1.2.3",
            "1.2.3-rc1",
            "-1.2.3",
            "1..3",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
        assert_eq!(v("10.0.0").to_string(), "10.0.0");
        assert!(v("1.10.0") > v("1.9.9"), "numeric, not textual order");
    }

    proptest! {
        #[test]
        fn r_ver_002__bump_is_monotonic(a in 0u64..50, b in 0u64..50, c in 0u64..50, i in 0usize..3) {
            let impact = [Impact::Fix, Impact::Add, Impact::Break][i];
            let s = Version::new(a, b, c);
            prop_assert!(bump(Some(s), impact, Version::new(0, 1, 0)) > s);
            prop_assert_eq!(Version::parse(&s.to_string()), Some(s));
        }
    }
}
