//! The only clock access in the crate (R-CHK-014, contract invariant 9).
use crate::error::Error;
use std::fmt;

/// The reference date of a run (`--as-of`). Date-dependent results and cache entries carry it.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AsOf(pub jiff::civil::Date);

impl AsOf {
    /// Strict `YYYY-MM-DD`; any other shape is a usage error.
    pub fn parse(s: &str) -> Result<Self, Error> {
        let b = s.as_bytes();
        let shape_ok = b.len() == 10
            && b.iter().enumerate().all(|(i, c)| {
                if i == 4 || i == 7 {
                    *c == b'-'
                } else {
                    c.is_ascii_digit()
                }
            });
        if !shape_ok {
            return Err(Error::Usage(format!(
                "--as-of expects YYYY-MM-DD, got {s:?}"
            )));
        }
        s.parse::<jiff::civil::Date>()
            .map(AsOf)
            .map_err(|e| Error::Usage(format!("invalid --as-of date {s:?}: {e}")))
    }

    /// UTC today. The ONLY wall-clock read in fairlead-core.
    /// Verify: `Timestamp::now`, `Timestamp::to_zoned` and `TimeZone::UTC` exist in jiff 0.2 with `std`.
    pub fn today_utc() -> Self {
        // fairlead: ovr rule.no-clock why="the one clock read: AsOf::today_utc (R-CHK-014)" owner=@aAndy03 until=0.4.0
        let zoned = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC);
        AsOf(zoned.date())
    }
}

impl fmt::Display for AsOf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_chk_014__parse_accepts_iso_dates_only() {
        let a = AsOf::parse("2026-10-09").unwrap();
        assert_eq!(a.to_string(), "2026-10-09");
        assert_eq!(a.0.year(), 2026);
        assert_eq!(a.0.month(), 10);
        assert_eq!(a.0.day(), 9);
        for bad in [
            "20261009",
            "2026-1-9",
            "2026-13-01",
            "2026-02-30",
            "2027-02-29",
            "",
            "today",
            "2026-10-09T00:00:00Z",
            " 2026-10-09",
        ] {
            let err = AsOf::parse(bad).unwrap_err();
            assert_eq!(err.class(), crate::error::ErrorClass::Usage, "{bad:?}");
        }
        assert!(AsOf::parse("2028-02-29").is_ok(), "leap day");
    }

    #[test]
    fn r_chk_014__as_of_orders_by_date() {
        assert!(AsOf::parse("2026-10-09").unwrap() < AsOf::parse("2026-10-10").unwrap());
        assert_eq!(
            AsOf::parse("2026-10-09").unwrap(),
            AsOf::parse("2026-10-09").unwrap()
        );
    }
}
