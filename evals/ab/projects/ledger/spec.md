# Ledger: product specification

Ledger is double-entry bookkeeping in Rust: a library in layers and a command line, with no
external dependencies. `cargo test` runs every test.

## Layout and owners

A Cargo workspace with four crates:

| Crate (path) | Contents | Owner |
|---|---|---|
| `ledger-core` (`crates/core`) | money, currencies, account ids, limits and defaults, the error type and its codes | platform team |
| `ledger-engine` (`crates/engine`) | posting rules: transactions, entries, balances, transfers | platform team |
| `ledger-service` (`crates/service`) | the accounts service: an in-memory ledger | accounts team |
| `ledger-cli` (`crates/cli`, binary `ledger`) | runs ledger commands from a file or standard input | tools team |
| `docs/errors.md` | the error codes and their meanings | platform team |

A `CODEOWNERS` file records the owners.

## Requirements (they hold for the whole life of the product)

1. **Layers.** `core` depends on no other crate; `engine` only on `core`; `service` on `core` and
   `engine`; `cli` on `service` and `core`, never on `engine`.
2. **Money.** Money is integer cents (`i64`); no `f32` or `f64` anywhere.
3. **Limits.** Every limit and default lives in `ledger_core::limits` (at least `MAX_AMOUNT_CENTS =
   100_000_000`, `MAX_MEMO_LEN = 140`, `MAX_ACCOUNT_ID_LEN = 32`, `DEFAULT_CURRENCY = "EUR"`). Other
   crates never repeat these values or currency codes.
4. **Errors.** `ledger_core::LedgerError` has a stable code per variant (`code()` returns `"E001"`,
   `"E002"`, ...). Every code is listed in `docs/errors.md`, and every listed code exists. Codes are
   never reused or renumbered.
5. **Ownership.** Each change is made by one team and touches only its crates.
6. **Tests.** Every feature has tests, and `cargo test` passes after every change.

## Public names (tests call these)

- `ledger_core`: `AccountId::parse(&str)`, `Currency::{Eur, Usd, Gbp}`, `Currency::parse`,
  `Currency::code`, `Money { cents, currency }` displayed as `"12.34 EUR"`, `LedgerError` and
  `LedgerError::code`, `limits::*`.
- `ledger_service::Ledger`: `new()`, `open(id, name, currency: Option<&str>) -> Result<AccountId>`,
  `deposit(id, cents) -> Result<Money>`, `withdraw(id, cents) -> Result<Money>`,
  `transfer(from, to, cents, memo) -> Result<u64>`, `balance(id) -> Result<Money>`,
  `statement(id) -> Result<Vec<(u64, i64, String)>>`.
- The `ledger` binary reads one command per line: `open <id> <name> [currency]`,
  `deposit <id> <cents>`, `withdraw <id> <cents>`, `transfer <from> <to> <cents> [memo]`,
  `balance <id>` (prints `<id>: <money>`), `statement <id>`. An error prints its code and the run
  exits 1.

## Features

1. Core types: money, currencies, account ids, limits, errors with codes and `docs/errors.md`.
2. The engine: balanced transactions, transfers, balances, amount limits.
3. The service: accounts, deposits and withdrawals through a `cash` account, transfers in one
   currency, statements. Withdrawing exactly the whole balance is allowed.
4. The command line.
5. Memo length enforced (`MAX_MEMO_LEN`).
6. Closing an account, only when its balance is zero; a closed account accepts nothing.
7. Reversing a transaction with a new, opposite transaction.
8. A CSV export of a statement from the command line (`export <id>`).
9. Statements that show the running balance after each entry.
