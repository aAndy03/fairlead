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

#[cfg(test)]
mod tests {
    use super::*;

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
