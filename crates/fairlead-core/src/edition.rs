//! Editions (chapter 22): the static registry of editions this binary reads, and which one it
//! writes. 0.2 ships edition 1 only. The `test-edition` feature adds a synthetic edition 2 with
//! one mechanical change (the generated header key) and one newly required field, so the
//! `upgrade` machinery can be tested before a real edition 2 exists (owner question Q-1); it is
//! selected only when the caller passes `2` from the `FAIRLEAD_TEST_EDITION` variable.
use crate::discover::Edition;

/// One edition: its number, grammar name, generated-header key and the root keys it newly
/// requires (a value no migration may invent, R-EDN-034).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EditionSpec {
    pub number: u32,
    pub grammar: &'static str,
    /// Key of the edition in a generated file's header line.
    pub header_key: &'static str,
    /// `(table, key)` the root control file must set from this edition on.
    pub requires: &'static [(&'static str, &'static str)],
}

pub const EDITION_1: EditionSpec = EditionSpec {
    number: 1,
    grammar: "edition 1 (frozen at 0.1.0)",
    header_key: "edition",
    requires: &[],
};

#[cfg(feature = "test-edition")]
pub const EDITION_2: EditionSpec = EditionSpec {
    number: 2,
    grammar: "edition 2 (test only)",
    header_key: "ed",
    requires: &[("settings", "purpose")],
};

#[cfg(not(feature = "test-edition"))]
const REGISTRY: &[EditionSpec] = &[EDITION_1];
#[cfg(feature = "test-edition")]
const REGISTRY: &[EditionSpec] = &[EDITION_1, EDITION_2];

/// Every edition this binary knows, oldest first.
pub fn registry() -> &'static [EditionSpec] {
    REGISTRY
}

pub fn spec(e: Edition) -> Option<&'static EditionSpec> {
    REGISTRY.iter().find(|s| s.number == e.0)
}

/// The edition this binary reads and writes. `test_override` is the `FAIRLEAD_TEST_EDITION`
/// value, honoured only in a `test-edition` build.
pub fn current(test_override: Option<&str>) -> Edition {
    #[cfg(feature = "test-edition")]
    if let Some(n) = test_override.and_then(|v| v.trim().parse::<u32>().ok())
        && spec(Edition(n)).is_some()
    {
        return Edition(n);
    }
    let _ = test_override;
    Edition(1)
}

/// The edition `upgrade` may read besides the current one: the previous edition (R-EDN-010).
pub fn previous(current: Edition) -> Option<Edition> {
    current
        .0
        .checked_sub(1)
        .map(Edition)
        .filter(|e| spec(*e).is_some())
}

/// `1` or `1, 2`: what refusals name (R-EDN-012).
pub fn supported_list() -> String {
    REGISTRY
        .iter()
        .map(|s| s.number.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The header key of `edition` in generated files (`edition` unless an edition says otherwise).
pub fn header_key(e: u32) -> &'static str {
    spec(Edition(e)).map_or("edition", |s| s.header_key)
}

/// The three pins of the root control file (R-EDN-006).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Pins {
    pub edition: Option<u32>,
    pub version: Option<String>,
    pub range: Option<String>,
}

impl Pins {
    pub fn of(c: &crate::config::ControlFile) -> Pins {
        Pins {
            edition: c.fairlead.edition.map(|e| e.0),
            version: c.settings.fairlead_version.clone(),
            range: c.settings.fairlead_range.clone(),
        }
    }

    /// The pins a version file recorded at its seal.
    pub fn from_version_text(text: &str) -> Pins {
        let fm = text
            .strip_prefix("+++\n")
            .and_then(|r| r.split_once("\n+++"))
            .map_or("", |(f, _)| f);
        let t: toml::Table = fm.parse().unwrap_or_default();
        Pins {
            edition: t
                .get("edition")
                .and_then(toml::Value::as_integer)
                .and_then(|n| u32::try_from(n).ok()),
            version: t
                .get("fairlead-version")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            range: t
                .get("fairlead-range")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        }
    }

    /// Version-file lines recording these pins.
    pub fn version_lines(&self) -> String {
        let mut out = String::new();
        if let Some(e) = self.edition {
            out.push_str(&format!("edition = {e}\n"));
        }
        if let Some(v) = &self.version {
            out.push_str(&format!("fairlead-version = {v:?}\n"));
        }
        if let Some(r) = &self.range {
            out.push_str(&format!("fairlead-range = {r:?}\n"));
        }
        out
    }

    /// The approval fingerprint of these pins (R-EDN-006): `fairlead approve pin`.
    pub fn fingerprint(&self) -> crate::hash::Fingerprint {
        crate::hash::Fingerprint::from_hash(&crate::hash::hash_raw(
            format!(
                "pin edition={:?} version={:?} range={:?}",
                self.edition, self.version, self.range
            )
            .as_bytes(),
        ))
    }
}

/// The lower bound of a range such as `>=0.1, <0.2`.
fn range_floor(r: &str) -> Option<semver::Version> {
    r.split(',')
        .map(str::trim)
        .find_map(|p| p.strip_prefix(">="))
        .and_then(crate::discover::parse_pin_version)
}

/// Each pin of `now` that is lower than in `sealed` (R-EDN-006), as relaxation text.
pub fn lowered(now: &Pins, sealed: &Pins) -> Vec<String> {
    let mut out = Vec::new();
    if let (Some(a), Some(b)) = (now.edition, sealed.edition)
        && a < b
    {
        out.push(format!("pin edition={a} (sealed {b})"));
    }
    let v = |s: &Option<String>| s.as_deref().and_then(crate::discover::parse_pin_version);
    if let (Some(a), Some(b)) = (v(&now.version), v(&sealed.version))
        && a < b
    {
        out.push(format!(
            "pin fairlead-version={} (sealed {})",
            now.version.as_deref().unwrap_or(""),
            sealed.version.as_deref().unwrap_or("")
        ));
    }
    let f = |s: &Option<String>| s.as_deref().and_then(range_floor);
    if let (Some(a), Some(b)) = (f(&now.range), f(&sealed.range))
        && a < b
    {
        out.push(format!(
            "pin fairlead-range={} (sealed {})",
            now.range.as_deref().unwrap_or(""),
            sealed.range.as_deref().unwrap_or("")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_edn_006__only_a_lower_pin_is_a_relaxation() {
        let pins = |e: u32, v: &str, r: &str| Pins {
            edition: Some(e),
            version: Some(v.into()),
            range: Some(r.into()),
        };
        let sealed = pins(1, "0.2", ">=0.2, <0.3");
        assert!(
            lowered(&pins(1, "0.3", ">=0.3, <0.4"), &sealed).is_empty(),
            "raising is free"
        );
        assert_eq!(
            lowered(&pins(1, "0.1", ">=0.1, <0.2"), &sealed),
            [
                "pin fairlead-version=0.1 (sealed 0.2)",
                "pin fairlead-range=>=0.1, <0.2 (sealed >=0.2, <0.3)"
            ]
        );
        let text = format!("+++\nversion = \"1.0.0\"\n{}+++\n", sealed.version_lines());
        assert_eq!(Pins::from_version_text(&text), sealed);
        assert_ne!(
            sealed.fingerprint(),
            pins(1, "0.1", ">=0.2, <0.3").fingerprint()
        );
    }

    #[test]
    fn r_edn_001__edition_1_is_the_default_and_frozen() {
        assert_eq!(current(None), Edition(1));
        assert_eq!(registry()[0], EDITION_1);
        assert_eq!(header_key(1), "edition");
        assert!(previous(Edition(1)).is_none());
    }

    #[cfg(not(feature = "test-edition"))]
    #[test]
    fn r_edn_010__a_release_build_ignores_the_test_override() {
        assert_eq!(current(Some("2")), Edition(1));
        assert_eq!(supported_list(), "1");
    }

    #[cfg(feature = "test-edition")]
    #[test]
    fn r_edn_010__the_test_edition_is_selected_only_by_the_override() {
        assert_eq!(current(Some("2")), Edition(2));
        assert_eq!(current(Some("9")), Edition(1));
        assert_eq!(previous(Edition(2)), Some(Edition(1)));
        assert_eq!(supported_list(), "1, 2");
    }
}
