//! Ledger core: money, identifiers, limits and the error contract. Depends on nothing.
// fairlead: def ctr.core path="crates/core/**"
// fairlead: def un.core layer=core.core container=ctr.core vis=public sum="money, ids, limits, errors"
pub mod error;
pub mod limits;
pub mod money;

pub use error::LedgerError;
pub use money::{Currency, Money};

/// An account identifier: lower-case letters, digits and dashes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(String);

impl AccountId {
    pub fn parse(s: &str) -> Result<AccountId, LedgerError> {
        let ok = !s.is_empty()
            && s.len() <= limits::MAX_ACCOUNT_ID_LEN
            && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if ok {
            Ok(AccountId(s.to_owned()))
        } else {
            Err(LedgerError::InvalidAccountId(s.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
