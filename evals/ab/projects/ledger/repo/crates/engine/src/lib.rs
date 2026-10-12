//! Posting rules: a transaction is a set of entries that sum to zero, each within the limits.
// fairlead: def ctr.engine path="crates/engine/**"
// fairlead: def un.engine layer=core.engine container=ctr.engine vis=public sum="posting rules and balances"
// fairlead: use un.core
use ledger_core::{limits, AccountId, LedgerError};

/// One side of a transaction: a signed amount in cents on one account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub account: AccountId,
    pub cents: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    pub id: u64,
    pub entries: Vec<Entry>,
    pub memo: String,
}

/// Checks that an amount is positive and within the limit.
pub fn check_amount(cents: i64) -> Result<(), LedgerError> {
    if cents <= 0 || cents > limits::MAX_AMOUNT_CENTS {
        return Err(LedgerError::InvalidAmount(cents));
    }
    Ok(())
}

/// A transaction is valid when its entries sum to zero.
pub fn validate(tx: &Transaction) -> Result<(), LedgerError> {
    let sum: i64 = tx.entries.iter().map(|e| e.cents).sum();
    if sum != 0 || tx.entries.is_empty() {
        return Err(LedgerError::Unbalanced);
    }
    // Known issue: the memo length limit is not checked here.
    Ok(())
}

/// The balance of `account` over `txs`, in cents.
pub fn balance(account: &AccountId, txs: &[Transaction]) -> i64 {
    txs.iter()
        .flat_map(|t| t.entries.iter())
        .filter(|e| &e.account == account)
        .map(|e| e.cents)
        .sum()
}

/// A two-entry transfer of `cents` from `from` to `to`.
pub fn transfer(id: u64, from: &AccountId, to: &AccountId, cents: i64, memo: &str) -> Result<Transaction, LedgerError> {
    check_amount(cents)?;
    let tx = Transaction {
        id,
        entries: vec![
            Entry { account: from.clone(), cents: -cents },
            Entry { account: to.clone(), cents },
        ],
        memo: memo.to_owned(),
    };
    validate(&tx)?;
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> AccountId {
        AccountId::parse(s).unwrap()
    }

    #[test]
    fn a_transfer_balances() {
        let t = transfer(1, &id("a"), &id("b"), 500, "x").unwrap();
        assert_eq!(balance(&id("a"), &[t.clone()]), -500);
        assert_eq!(balance(&id("b"), &[t]), 500);
    }

    #[test]
    fn amounts_are_bounded() {
        assert!(check_amount(0).is_err());
        assert!(check_amount(limits::MAX_AMOUNT_CENTS).is_ok());
        assert!(check_amount(limits::MAX_AMOUNT_CENTS + 1).is_err());
    }
}
