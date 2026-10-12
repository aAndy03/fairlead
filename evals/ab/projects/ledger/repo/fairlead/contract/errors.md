+++
title = "Error codes"
directives = [
  'def err.e001 sum="InvalidAccountId: bad account id"',
  'def err.e002 sum="UnknownAccount: no account with that id"',
  'def err.e003 sum="AccountExists: id already open"',
  'def err.e004 sum="UnknownCurrency: not EUR, USD or GBP"',
  'def err.e005 sum="CurrencyMismatch: a transfer between currencies"',
  'def err.e006 sum="InvalidAmount: zero, negative or above the limit"',
  'def err.e007 sum="InsufficientFunds: would go below zero"',
  'def err.e008 sum="Unbalanced: entries do not sum to zero"',
]
+++

The error contract. `ledger_core::LedgerError::code` and `docs/errors.md` must list the same codes;
a new error takes the next free code and is declared here. Codes are never reused or renumbered.
