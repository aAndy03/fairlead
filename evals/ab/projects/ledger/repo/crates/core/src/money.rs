//! Money is integer cents in one currency; never a float.
use crate::LedgerError;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Currency {
    Eur,
    Usd,
    Gbp,
}

impl Currency {
    pub fn parse(s: &str) -> Result<Currency, LedgerError> {
        match s {
            "EUR" => Ok(Currency::Eur),
            "USD" => Ok(Currency::Usd),
            "GBP" => Ok(Currency::Gbp),
            other => Err(LedgerError::UnknownCurrency(other.to_owned())),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Currency::Eur => "EUR",
            Currency::Usd => "USD",
            Currency::Gbp => "GBP",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Money {
    pub cents: i64,
    pub currency: Currency,
}

impl Money {
    pub fn new(cents: i64, currency: Currency) -> Money {
        Money { cents, currency }
    }
}

impl std::fmt::Display for Money {
    /// `1234 EUR` displays as `12.34 EUR`; `-5 EUR` as `-0.05 EUR`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign = if self.cents < 0 { "-" } else { "" };
        let abs = self.cents.unsigned_abs();
        write!(f, "{sign}{}.{:02} {}", abs / 100, abs % 100, self.currency.code())
    }
}
