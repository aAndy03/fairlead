# Ledger

Ledger is a small double-entry bookkeeping library and command line, written in Rust with no
dependencies. Money is always integer cents.

## Layout

| Crate | What it is | Team |
|---|---|---|
| `crates/core` (`ledger-core`) | money, currencies, account ids, the limits and defaults, the error type and its codes | platform |
| `crates/engine` (`ledger-engine`) | posting rules: transactions, entries, balances, transfers | platform |
| `crates/service` (`ledger-service`) | the accounts service: an in-memory ledger with accounts, deposits, withdrawals, transfers and statements | accounts |
| `crates/cli` (`ledger-cli`, binary `ledger`) | runs ledger commands from a file or standard input | tools |
| `docs/errors.md` | the error codes and what they mean | platform |

The crates build on each other in that order: `core`, then `engine`, then `service`, then `cli`.
`CODEOWNERS` lists which team owns which crate.

## Running

```bash
cargo test
echo "open alice Alice
deposit alice 5000
balance alice" | cargo run -q -p ledger-cli
```

Commands: `open <id> <name> [currency]`, `deposit <id> <cents>`, `withdraw <id> <cents>`,
`transfer <from> <to> <cents> [memo]`, `balance <id>`, `statement <id>`.

## Features

- Accounts in EUR (the default), USD or GBP; account ids are lower-case letters, digits and dashes.
- Deposits and withdrawals go through a `cash` account, so every transaction balances.
- Transfers between accounts of the same currency, with a memo; amounts have an upper limit.
- Statements list an account's entries.
- Every error has a stable code (`E001` to `E008`) documented in `docs/errors.md`.

## Known issues

- Withdrawing exactly the whole balance of an account is refused; it should be allowed.
- Transaction memos have a maximum length, but it is not enforced.

## Ideas the teams have mentioned

- Closing an account (only when its balance is zero).
- Reversing a transaction with a new, opposite transaction.
- A CSV export of a statement from the command line.
- Monthly interest on savings accounts.
- Statements that show the running balance after each entry.
