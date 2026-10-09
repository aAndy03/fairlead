//! Root discovery and the pin check (R-CLI-004, R-CLI-005, R-ARCH-010..013, D-31, D-33).
//! Part 1: the pure pin types. Task 15 appends root lookup and I/O.
use semver::Version;
use std::cmp::Ordering;

/// Integer edition (D-45).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Edition(pub u32);

/// The edition this build reads and writes.
pub const SUPPORTED_EDITION: Edition = Edition(1);

/// Default managerial folder name (a config key, D-30).
pub const DEFAULT_FOLDER: &str = "fairlead";

/// `[settings] fairlead-version` plus the header edition. "0.1" is padded to 0.1.0 (C-12).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pin {
    pub version: Option<Version>,
    pub edition: Edition,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PinRefusal {
    pub install_line: String,
    pub reason: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PinVerdict {
    Ok,
    OkNewerBinary { info: String },
    Refused(PinRefusal),
}

/// Parse a pin such as `0.1`, `1` or `1.2.3`; `None` when it is not a version.
pub fn parse_pin_version(s: &str) -> Option<Version> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let padded = match t.matches('.').count() {
        0 => format!("{t}.0.0"),
        1 => format!("{t}.0"),
        _ => t.to_string(),
    };
    Version::parse(&padded).ok()
}

fn install_line(v: Option<&Version>) -> String {
    match v {
        Some(v) => format!("cargo install fairlead --version {v} --locked"),
        None => "cargo install fairlead --locked".to_string(),
    }
}

/// Pure (R-CLI-005, D-50(4)): another edition or an older binary refuses; a newer binary on the same
/// edition proceeds. Pre-release and build metadata of the binary are ignored.
pub fn pin_check(binary: &Version, supported: Edition, pin: &Pin) -> PinVerdict {
    if pin.edition != supported {
        return PinVerdict::Refused(PinRefusal {
            install_line: install_line(pin.version.as_ref()),
            reason: format!(
                "this binary supports edition {} but the codebase is pinned to edition {}",
                supported.0, pin.edition.0
            ),
        });
    }
    let Some(pinned) = &pin.version else {
        return PinVerdict::Ok;
    };
    let release = Version::new(binary.major, binary.minor, binary.patch);
    match release.cmp(pinned) {
        Ordering::Less => PinVerdict::Refused(PinRefusal {
            install_line: install_line(Some(pinned)),
            reason: format!("binary {binary} is older than the fairlead-version pin {pinned}"),
        }),
        Ordering::Equal => PinVerdict::Ok,
        Ordering::Greater => PinVerdict::OkNewerBinary {
            info: format!(
                "binary {binary} is newer than the fairlead-version pin {pinned} (same edition {}); proceeding",
                supported.0
            ),
        },
    }
}

#[cfg(test)]
mod pin_tests {
    use super::*;
    use semver::Version;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }
    fn pin(version: Option<&str>, edition: u32) -> Pin {
        Pin {
            version: version.map(v),
            edition: Edition(edition),
        }
    }

    #[test]
    fn r_cli_005__older_binary_is_refused_with_exact_install_line() {
        match pin_check(&v("0.1.0"), SUPPORTED_EDITION, &pin(Some("0.2.0"), 1)) {
            PinVerdict::Refused(r) => {
                assert_eq!(
                    r.install_line,
                    "cargo install fairlead --version 0.2.0 --locked"
                );
                assert!(r.reason.contains("older"), "{}", r.reason);
            }
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn r_cli_005__newer_binary_same_edition_proceeds_with_info_line() {
        match pin_check(&v("0.3.1"), SUPPORTED_EDITION, &pin(Some("0.2.0"), 1)) {
            PinVerdict::OkNewerBinary { info } => {
                assert!(info.contains("0.3.1") && info.contains("0.2.0"))
            }
            other => panic!("expected OkNewerBinary, got {other:?}"),
        }
    }

    #[test]
    fn r_cli_005__equal_version_and_missing_pin_are_ok() {
        assert_eq!(
            pin_check(&v("0.2.0"), SUPPORTED_EDITION, &pin(Some("0.2.0"), 1)),
            PinVerdict::Ok
        );
        assert_eq!(
            pin_check(&v("0.2.0"), SUPPORTED_EDITION, &pin(None, 1)),
            PinVerdict::Ok
        );
    }

    #[test]
    fn r_cli_005__other_edition_is_refused_whichever_side_is_newer() {
        for e in [0u32, 2] {
            match pin_check(&v("9.0.0"), SUPPORTED_EDITION, &pin(Some("0.1.0"), e)) {
                PinVerdict::Refused(r) => {
                    assert!(r.reason.contains(&format!("edition {e}")), "{}", r.reason);
                    assert_eq!(
                        r.install_line,
                        "cargo install fairlead --version 0.1.0 --locked"
                    );
                }
                other => panic!("expected refusal, got {other:?}"),
            }
        }
        match pin_check(&v("1.0.0"), SUPPORTED_EDITION, &pin(None, 2)) {
            PinVerdict::Refused(r) => assert_eq!(r.install_line, "cargo install fairlead --locked"),
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn r_cli_005__prerelease_binary_satisfies_its_release_pin() {
        assert_eq!(
            pin_check(
                &v("0.1.0-alpha.2"),
                SUPPORTED_EDITION,
                &pin(Some("0.1.0"), 1)
            ),
            PinVerdict::Ok
        );
    }

    #[test]
    fn r_cli_005__pin_version_is_padded_to_three_parts() {
        assert_eq!(parse_pin_version("0.1"), Some(v("0.1.0")));
        assert_eq!(parse_pin_version("1"), Some(v("1.0.0")));
        assert_eq!(parse_pin_version(" 1.2.3 "), Some(v("1.2.3")));
        assert_eq!(parse_pin_version("x.y"), None);
        assert_eq!(parse_pin_version(""), None);
    }

    #[test]
    fn r_arch_013__supported_edition_is_one() {
        assert_eq!(SUPPORTED_EDITION, Edition(1));
    }
}
