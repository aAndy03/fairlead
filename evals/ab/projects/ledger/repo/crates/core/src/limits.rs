//! Every limit and default of the ledger, in one place.

/// The largest single transfer, deposit or withdrawal, in cents (1,000,000.00).
pub const MAX_AMOUNT_CENTS: i64 = 100_000_000;
/// The longest transaction memo, in characters.
pub const MAX_MEMO_LEN: usize = 140;
/// The longest account identifier, in characters.
pub const MAX_ACCOUNT_ID_LEN: usize = 32;
/// The currency of a new account when none is given.
pub const DEFAULT_CURRENCY: &str = "EUR";
