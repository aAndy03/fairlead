# Agent instructions

Ledger is a Rust workspace in layers. Each crate belongs to a team (see `CODEOWNERS`). Follow these
rules in every change.

## Layers
- The crates build in one direction: `core` <- `engine` <- `service` <- `cli`. A crate never depends
  on one to its right: `core` depends on no other crate, `engine` only on `core`, `service` on `core`
  and `engine`.
- The `cli` talks to the `service` (and may use `core` types); it never uses `ledger-engine`
  directly, not in `Cargo.toml` and not in code.

## Money
- Money is integer cents (`i64`). Never use `f32` or `f64` anywhere in the crates.

## Limits and defaults
- Every limit and default lives in `ledger_core::limits` (`MAX_AMOUNT_CENTS`, `MAX_MEMO_LEN`,
  `MAX_ACCOUNT_ID_LEN`, `DEFAULT_CURRENCY`). Add new ones there. Never repeat their values
  (`100_000_000`, `140`, `"EUR"`) in `engine`, `service` or `cli`; currency codes come from
  `Currency`.

## Errors
- Every `LedgerError` variant has a stable code in `LedgerError::code` and a row in
  `docs/errors.md`. A new variant takes the next free code (`E009`, ...) and gets its row in the same
  change. Never reuse or renumber a code.

## Ownership
- Change only the crates of the team the task is for. If you need something from another team's
  crate, keep your change in your crate and write a short note under `docs/requests/` saying what
  you need from them.

## Before you finish
- Run `cargo test` and keep it passing; add tests for what you change.
