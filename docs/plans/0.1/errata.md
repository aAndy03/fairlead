# 0.1 implementation errata

Deviations of the actual implementation from the phase plans, recorded as they happen (master plan, execution protocol 3).

| # | Phase | Deviation | Reason |
|---|---|---|---|
| E-1 | P1 | No pre-created stub modules, stub test targets or placeholder workflows; modules, `[[test]]` targets and workflows arrive with the phase that implements them. | Avoids dead files; the contract's module paths are still followed. |
| E-2 | P1 | `indexing_slicing = "deny"` is set by `#![deny(clippy::indexing_slicing)]` in `fairlead-syntax` only, not workspace-wide. | Cargo forbids overriding `workspace.lints` per crate; core indexes under review. |
| E-3 | P1 | `license = "MIT"` (D-58) replaces the placeholder `license-file`; `publish = false` stays until the 0.1 exit criteria hold. | Owner decision. |
| E-4 | P1 | `.cargo/config.toml` sets `build.jobs = 2`. | Owner asked for light cargo use. |
| E-5 | all | The plans' "run the test to see it fail" steps are collapsed: tests and implementation land together and the test run is the check. | Time; red phase gives no information once the API exists. |

## Implementation notes (P5a, after E-5)

- E-6: rule items now carry `check`, `tokens`, `forbid`, `msg`, `waivable`, `why`, `since` fields (P4 only had `sev`, `stage`, `glob`, `exclude`); `ovr` on a rule may touch only `sev`, `stage`, `glob`, `exclude` (C-32). `check = "builtin"` is accepted by the config parser (A-24).
- E-7: `RuleSet::applies` goes through `Registry::glob_matches` (A-27); rule defaults are `sev=error stage=observe` for every kind (A-52).
- E-8: `HN-G001` diffs bodies only (the header hash always differs when the body does); the first three differing lines are shown.
- (Superseded) Safe stop point: P5a tasks 1-17 are done (Check trait, syntax, refs, tokens, leak, pattern rules, generated). Next: T18 exceptions, T19 controls, T20 scopes, T21 vendor, T22 paths, T23 agents, T24 notices, T25-T29 apply/summarize/run, then P5b render, P6-P9.

## Verification audit (2026-10-09, before P5a-T18)

- E-9: P2, P3 and P4 have every planned test name; the workspace tests pass. P1 was incomplete: `fairlead/control.toml` (P1-T12) was missing and the xtask tests did not carry the `r_arch_123__` names; both fixed. `fuzz.yml`, `bench.yml`, `dogfood.yml`, `release.yml` and the PR template stay deferred to the phases that fill them (E-1).
- E-10: `cargo clippy -D warnings` and `cargo fmt --check` were failing at `05be41e` (32 lint errors from P5a-T12..T17). Fixed; helpers that only P5a-T25..T29 consume carry a temporary `#[allow(dead_code, reason = ...)]`, removed as they are wired. The `--budget` `Deadline` carries the local clock allow that clippy.toml requires.

## P5a-T18 onwards

- E-11: `ControlsCheck` (T19) does not pass `ControlFile.violations` through: `config::control_diagnostics` already carries them into the load diagnostics (A-11), so passing them again would report each `HN-C003` twice.
- E-12: `ScopesCheck` (T20) reads shards with `generate::members::parse_members` (A-36), not the plan's `<id> parent= dir=` parser: a `self.` line gives a scope's recorded parent (`up=`), a `scope.<child>` line records the shard's owner as the child's parent. Test fixtures use that format and their pattern rules carry `msg`.
- E-13: `write::init::pointer_block` (P7-T7) and its three tests land before P7 so `HN-A001` (T23) can import it (A-25, A-64). `write/mod.rs` holds only `pub mod init;` until P7-T0.
- E-14: Relaxation lines (T27) follow A-52, not the plan's per-kind defaults: a line is written when a scope's effective rule has a lower `stage` or `sev` than the rule as declared in its home, formatted `rules.<name> stage=<x> in <scope> (declared <y>)`; valid `[[allow]]` grants are listed as planned. `CheckReport.absent_scopes` (A-45) maps to exit 5, other unknown scopes to exit 2.
- E-15: `check::run` (T29) calls `generate` without a version (A-42) and builds the baseline path with `builtin::baseline_rel` (the `apply::baseline_path` of the plan is dropped). End-to-end tests use `testkit::Built::synced()`, which puts the output of `generate` on disk; without it every expected generated file is `HN-G001` and the run exits 3.
- E-16: The T30 code-literal audit looks for a quoted code (`"HN-`), not the bare prefix, so doc comments may name codes; it also covers `testkit.rs` and asserts no `STUB(P5)` is left in `literals.rs` and `secrets.rs` (A-14).
- Safe stop point (2026-10-09): P5a complete (T1-T30). Next: P5b T13-T19 (render), then P6, P7, P8, P9.
- E-17: P5b render (T13-T18): `Renderer` has only `format` and `check` until T19 adds `context`/`explain` (their P6/P7 types do not exist yet). Snapshots are exact-string `assert_eq!` (no `insta` dependency, as P5a CI-14). The human block and `render::diagnostic_lines` (A-20) show `Finding.excerpt` / `Diagnostic.text` as `| <text>` with a `| ^^^` caret line of the same char count (A-16); JSON keeps the C-38 key set (no excerpt). SARIF uses the A-29 key `fingerprint/v1`. `CheckReport` and `ReportHeader` derive `Clone, PartialEq, Eq, Debug`.
- E-18: `ci.yml` and `msrv.yml` no longer run on every push to `main` (owner request, Actions minutes): they run on pull requests, weekly (Monday 04:00 UTC) and on demand; superseded runs are cancelled. Every commit is still checked locally (fmt, clippy -D warnings, workspace tests) before it is pushed.
