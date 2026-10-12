# P6: restructuring by fairlead, and automatic follow-up

> Outline goals 6 and 7; [32-portability-and-restructure.md](../../32-portability-and-restructure.md) (the 0.2 `move`); [15-cli.md](../../15-cli.md) R-CLI-002, R-CLI-040, R-CLI-043. D-50's "no rewriting tool" covered 0.1 to 0.3; 0.4 lifts it for references fairlead can see (D-63).

## Goal

A rename or move of an ID, a file or a scope rewrites the references, shows the plan first, and applies it as one transaction. After any writing command, fairlead runs the follow-up a person would.

## Design

- **What is rewritten.** Fairlead's own text (as since 0.2), plus code references it can see in parsed files: Rust `use` paths and `mod` declarations for moved files, TS/JS relative import specifiers (P7), Markdown links and code references in docs, and every directive naming a renamed ID. Anything it cannot see (dynamic imports, string-built paths) is listed in the plan as "not rewritten", never guessed (D-40).
- **Verbs.** `fairlead move` (0.2) gains `--rewrite` (default on once P6 lands; `--no-rewrite` keeps the 0.3 behaviour), and `fairlead rename <ID> <new ID>` is a new verb for items (the outline's "rename of an ID").
- **Follow-up.** After `new`, `move`, `rename`, `seal` and `config`, the command runs `sync` for generated files and prints a short "what changed" note (files written, findings that appeared or disappeared). `--no-follow-up` skips it; `--dry-run` writes nothing at all.

## Tasks

**T1. Rewrite plans.** `restructure::rewrite::plan(objects, from, to) -> RewritePlan { edits, not_rewritten }`, pure; edits are exact byte ranges from P1 spans. Test: `r_rewrite_001__plan_lists_exact_edits_and_unseen_references`.

**T2. Rust references.** `use` paths, `mod` declarations, `crate::` paths after a file move. Test: `r_rewrite_002__moving_a_rust_module_rewrites_its_uses`.

**T3. Directive and doc references.** Every `use`, `ovr`, `flag`, `wraps`, `variant-of` and doc link naming a renamed ID. Test: `r_rewrite_003__renaming_an_id_rewrites_every_directive`.

**T4. `fairlead rename`.** The verb, plan first, a tombstone with `replaced-by` for the old ID (0.2 lifecycle), one transaction. trycmd: `rename-plan`, `rename-apply`.

**T5. `move --rewrite`.** Wire T1 to T3 into the 0.2 and 0.3 move planners (scope moves, cut, copy, merge). Test: `r_rewrite_004__a_scope_move_rewrites_imports`.

**T6. Automatic follow-up.** As in the design; the note is a few terse lines. Tests: `r_follow_001__new_runs_sync`, `r_follow_001__dry_run_writes_nothing`.

**T7. The rewrite exit test.** Exit criterion 4 on three goldens (`hier-core`, `hier-tauri`, `ts-react`): rename and move, `check` unchanged, a second run is a no-op. Done when it passes in CI.
