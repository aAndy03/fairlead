# Error codes

Every `LedgerError` has a stable code. Scripts and support staff rely on them, so a code is never
reused or renumbered; a new error gets the next free code and a row here.

| Code | Variant | Meaning |
|---|---|---|
| E001 | InvalidAccountId | the account id is empty, too long or has characters other than a-z, 0-9 and dashes |
| E002 | UnknownAccount | no account has that id |
| E003 | AccountExists | an account with that id is already open |
| E004 | UnknownCurrency | the currency is not EUR, USD or GBP |
| E005 | CurrencyMismatch | the two accounts of a transfer use different currencies |
| E006 | InvalidAmount | the amount is zero, negative or above the limit |
| E007 | InsufficientFunds | the account would go below zero |
| E008 | Unbalanced | a transaction's entries do not sum to zero |
