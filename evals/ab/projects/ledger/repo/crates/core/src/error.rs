//! The ledger's error contract. Each variant has a stable code, documented in docs/errors.md.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerError {
    InvalidAccountId(String),
    UnknownAccount(String),
    AccountExists(String),
    UnknownCurrency(String),
    CurrencyMismatch,
    InvalidAmount(i64),
    InsufficientFunds { needed: i64, available: i64 },
    Unbalanced,
}

impl LedgerError {
    /// The stable code of the error, as documented.
    pub fn code(&self) -> &'static str {
        match self {
            LedgerError::InvalidAccountId(_) => "E001",
            LedgerError::UnknownAccount(_) => "E002",
            LedgerError::AccountExists(_) => "E003",
            LedgerError::UnknownCurrency(_) => "E004",
            LedgerError::CurrencyMismatch => "E005",
            LedgerError::InvalidAmount(_) => "E006",
            LedgerError::InsufficientFunds { .. } => "E007",
            LedgerError::Unbalanced => "E008",
        }
    }
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::InvalidAccountId(s) => write!(f, "{}: invalid account id {s:?}", self.code()),
            LedgerError::UnknownAccount(s) => write!(f, "{}: no account {s}", self.code()),
            LedgerError::AccountExists(s) => write!(f, "{}: account {s} already exists", self.code()),
            LedgerError::UnknownCurrency(s) => write!(f, "{}: unknown currency {s}", self.code()),
            LedgerError::CurrencyMismatch => write!(f, "{}: currencies differ", self.code()),
            LedgerError::InvalidAmount(a) => write!(f, "{}: invalid amount {a}", self.code()),
            LedgerError::InsufficientFunds { needed, available } => {
                write!(f, "{}: needs {needed} cents, has {available}", self.code())
            }
            LedgerError::Unbalanced => write!(f, "{}: entries do not sum to zero", self.code()),
        }
    }
}

impl std::error::Error for LedgerError {}
