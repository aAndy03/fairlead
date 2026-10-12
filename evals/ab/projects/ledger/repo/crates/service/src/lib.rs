//! The accounts service: an in-memory ledger with accounts, deposits, withdrawals and transfers.
// fairlead: def ctr.service path="crates/service/**"
// fairlead: def un.service layer=core.service container=ctr.service vis=public sum="accounts and their operations"
// fairlead: use un.core un.engine
use std::collections::BTreeMap;

use ledger_core::{limits, AccountId, Currency, LedgerError, Money};
use ledger_engine::{self as engine, Entry, Transaction};

/// The account money enters from and leaves to (deposits and withdrawals).
pub const CASH: &str = "cash";

#[derive(Clone, Debug)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub currency: Currency,
}

#[derive(Default)]
pub struct Ledger {
    accounts: BTreeMap<AccountId, Account>,
    txs: Vec<Transaction>,
}

impl Ledger {
    pub fn new() -> Ledger {
        Ledger::default()
    }

    pub fn open(&mut self, id: &str, name: &str, currency: Option<&str>) -> Result<AccountId, LedgerError> {
        let id = AccountId::parse(id)?;
        if self.accounts.contains_key(&id) {
            return Err(LedgerError::AccountExists(id.to_string()));
        }
        let currency = Currency::parse(currency.unwrap_or(limits::DEFAULT_CURRENCY))?;
        self.accounts.insert(id.clone(), Account { id: id.clone(), name: name.to_owned(), currency });
        Ok(id)
    }

    pub fn account(&self, id: &str) -> Result<&Account, LedgerError> {
        let key = AccountId::parse(id)?;
        self.accounts.get(&key).ok_or_else(|| LedgerError::UnknownAccount(id.to_owned()))
    }

    pub fn balance(&self, id: &str) -> Result<Money, LedgerError> {
        let acct = self.account(id)?;
        Ok(Money::new(engine::balance(&acct.id, &self.txs), acct.currency))
    }

    fn next_id(&self) -> u64 {
        self.txs.len() as u64 + 1
    }

    fn cash() -> AccountId {
        AccountId::parse(CASH).expect("the cash account id is valid")
    }

    pub fn deposit(&mut self, id: &str, cents: i64) -> Result<Money, LedgerError> {
        let to = self.account(id)?.id.clone();
        let tx = engine::transfer(self.next_id(), &Self::cash(), &to, cents, "deposit")?;
        self.txs.push(tx);
        self.balance(id)
    }

    pub fn withdraw(&mut self, id: &str, cents: i64) -> Result<Money, LedgerError> {
        let from = self.account(id)?.id.clone();
        let available = self.balance(id)?.cents;
        // Known issue: withdrawing exactly the whole balance is refused.
        if cents >= available {
            return Err(LedgerError::InsufficientFunds { needed: cents, available });
        }
        let tx = engine::transfer(self.next_id(), &from, &Self::cash(), cents, "withdrawal")?;
        self.txs.push(tx);
        self.balance(id)
    }

    pub fn transfer(&mut self, from: &str, to: &str, cents: i64, memo: &str) -> Result<u64, LedgerError> {
        let (a, b) = (self.account(from)?.clone(), self.account(to)?.clone());
        if a.currency != b.currency {
            return Err(LedgerError::CurrencyMismatch);
        }
        let available = self.balance(from)?.cents;
        if cents > available {
            return Err(LedgerError::InsufficientFunds { needed: cents, available });
        }
        let tx = engine::transfer(self.next_id(), &a.id, &b.id, cents, memo)?;
        let id = tx.id;
        self.txs.push(tx);
        Ok(id)
    }

    /// The entries of one account, oldest first: (transaction id, signed cents, memo).
    pub fn statement(&self, id: &str) -> Result<Vec<(u64, i64, String)>, LedgerError> {
        let acct = self.account(id)?;
        Ok(self
            .txs
            .iter()
            .flat_map(|t| t.entries.iter().map(move |e| (t, e)))
            .filter(|(_, e): &(&Transaction, &Entry)| e.account == acct.id)
            .map(|(t, e)| (t.id, e.cents, t.memo.clone()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_transfer_and_balance() {
        let mut l = Ledger::new();
        l.open("alice", "Alice", None).unwrap();
        l.open("bob", "Bob", Some(Currency::Eur.code())).unwrap();
        l.deposit("alice", 5_000).unwrap();
        l.transfer("alice", "bob", 1_200, "rent").unwrap();
        assert_eq!(l.balance("alice").unwrap().cents, 3_800);
        assert_eq!(l.balance("bob").unwrap().to_string(), "12.00 EUR");
        assert_eq!(l.statement("bob").unwrap(), vec![(2, 1_200, "rent".to_owned())]);
    }

    #[test]
    fn transfers_need_funds_and_one_currency() {
        let mut l = Ledger::new();
        l.open("a", "A", None).unwrap();
        l.open("u", "U", Some(Currency::Usd.code())).unwrap();
        assert_eq!(l.transfer("a", "u", 1, "x"), Err(LedgerError::CurrencyMismatch));
        l.open("b", "B", None).unwrap();
        assert!(matches!(l.transfer("a", "b", 1, "x"), Err(LedgerError::InsufficientFunds { .. })));
    }
}
