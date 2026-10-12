# v1 API surface (draft)

> Status: draft, written in 0.3 (P7-T7). The surface freezes at the end of 0.4 (D-64); until then a change updates the snapshot in `tests/v1_api/` together with an errata entry, so every change stays deliberate (D-53, R-ARCH-003). Agent protocol B-4 reviews this page before the freeze.

The v1 API is what a user, a script or an agent may rely on across every 1.x release. Anything not listed here may change in a minor release.

## 1. CLI verbs and documented flags

Ten verbs: `init`, `check`, `sync`, `new`, `context`, `explain`, `approve`, `move`, `seal`, `upgrade` ([15-cli.md](15-cli.md)). Their documented flags are exactly the ones `fairlead <verb> --help` prints; the snapshots are `tests/v1_api/help.txt` and `tests/v1_api/help-<verb>.txt`. Global flags (`--root`, `--as-of`, `--format`, `--no-cache`, `--threads`) are in `help.txt`.

Also part of the surface: the `fairlead-mcp` server's three tools (`check`, `context`, `explain`), their argument names and its three `fairlead://` resources (E3-35). The server follows the CLI's output; it adds no behaviour of its own.

## 2. Exit codes

| Code | Meaning |
|---|---|
| 0 | success, nothing blocking |
| 1 | findings at the blocking severity |
| 2 | usage error, or mandatory context that does not fit the budget |
| 3 | generated files are stale |
| 4 | refused: a plan was printed but not applied, or a gate refused it |
| 5 | internal error, or the verdict cannot be determined |

When several apply, the precedence is 5, 2, 4, 1, 3, 0 (`outcome::ExitCode`).

## 3. Directive grammar, per edition

Edition 1 ([12-directive-syntax.md](12-directive-syntax.md)): six verbs (`def`, `use`, `ovr`, `flag`, `env`, `end`), the ID grammar `[scope:]kind.name`, the comment leaders per language, the extents `+N` and `+end`, and the known keys per verb (`directive::known_keys`). Keys added in 0.3 (hierarchy, variation, tokens) are edition 1 keys: an older binary warns on them and keeps them (R-EDN-021). A later edition changes the grammar only through `fairlead upgrade`.

## 4. Output shapes

- `check --format json` and `context --format json`: the key paths and value types in `tests/v1_api/check.shape.txt` and `tests/v1_api/context.shape.txt`. Keys may be added in a minor release; none is removed or retyped.
- `check --format sarif`: SARIF 2.1.0, validated by `cargo xtask sarif-validate` against `fixtures/sarif/schema/`.
- `check --format terse` and `context` (terse): one finding or brief entry per line; the first token of a finding line is its code. The `--dense` form is defined in `render::dense` (R-CTX-034).

## 5. Generated-file formats

Every generated file starts with a header line naming its scope and edition ([11-files-and-classes.md](11-files-and-classes.md)). The kinds: registry shards `gen/reg/<kind>.reg`, `gen/exceptions`, `gen/members`, `gen/baseline.txt`, `gen/vendored.lock`, `gen/alias`, `gen/scope.lock` (with the 0.3 `chain`, `edition`, `req=`, `snap` and `pin` lines), `gen/hier.suggest`, `gen/requires`, and the changelog aggregate. Ledger year files (`ledger/hist/<year>.hl`) are append-only and hash-chained ([33-retention-and-compaction.md](33-retention-and-compaction.md)).

## 6. Diagnostic and rule IDs

Every `HN-` code with its name, default severity, default stage, milestone, floor and waivable flags: `tests/v1_api/codes.txt` (generated from `check::all_specs`). A code is never reused for another meaning; a retired code stays reserved. Named built-in rules: `rule.maturity-gap`, `rule.env-leakage`, `rule.hier-rank`, `rule.hier-cycle`, `rule.var-fork`.

## 7. What is not frozen

Human-format output wording, hint texts, the order of brief entries within a priority, cache files under `.fairlead/`, `xtask` subcommands, and anything marked experimental in its chapter.
