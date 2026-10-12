# Pilot, 2026-10-12: findings

> **Pilot, not evidence.** 54 sessions: 3 projects × first 3 tasks × arms A, B, C × 1 run, in both
> tracks. Its purpose is to find faults in the projects, tasks, scorer and runner (PROTOCOL.md).
> Model `opencode-go/muse-spark-1.3-contributor`, fairlead 0.3.1, seed 1. Files: `plan.json`,
> `sessions.jsonl` (as scored live), `sessions-rescored.jsonl` (the same sessions scored again with
> the corrected scorer and hidden tests by `rescore.ps1`; the numbers below come from it).

## Numbers (re-scored)

| Track | Arm | Task done | Nothing broken | New violations per session | Drift at task 3 | Tokens per session | Minutes | fairlead commands |
|---|---|---|---|---|---|---|---|---|
| maintenance | A | 9/9 | 9/9 | 0.33 | 0.7 | 28,194 | 1.1 | 0 |
| maintenance | B | 8/9 | 9/9 | 0.11 | 0.0 | 34,794 | 1.4 | 0 |
| maintenance | C | 9/9 | 9/9 | 0.22 | 0.3 | 37,872 | 1.6 | 6.9 |
| from scratch | A | 9/9 | 9/9 | 0.00 | 0.0 | 41,685 | 2.2 | 0 |
| from scratch | B | 9/9 | 9/9 | 0.00 | 0.0 | 43,133 | 1.9 | 0 |
| from scratch | C | 9/9 | 9/9 | 0.00 | 0.0 | 54,833 | 2.3 | 5.3 |

The eight violations, all real:

- Two ownership breaks (A and C): Ledger t1 was assigned to the platform team, but the code is in the accounts team's crate; A and C edited it, while B declined and left a request (so B's "task done" is 8/9).
- One ownership break (B): Relay t3, a back-end task, where B also edited the front end.
- Two contract breaks (A and C): Relay t3, `notes.rename` added to the back end and the contract, not the front end. The task said the front end would follow ("so the front end can offer rename next").
- One boundary break (A): Shelf t3, the admin app imported from the store app.

## What the pilot shows

- **The first three tasks do not separate the arms.** Nearly every session did its task and kept
  every rule, in every arm. The tasks that could show a difference are the later, harder ones
  (cross-team, contract and variant tasks), and drift needs the whole chain of nine. This is the
  pilot's main lesson: only the full run can answer the question.
- **Fairlead's cost shows already.** Arm C used about 35% more tokens than A in both tracks, 9%
  more than B in maintenance, and 27% more than B from scratch; it ran 5 to 7 fairlead commands per
  session.
- **Fairlead did not stop the one ownership break it could have**, Ledger t1: arm C edited another
  team's crate. The project declares owners only in CODEOWNERS, which fairlead 0.3 does not check
  against edits.

## Faults found and fixed before the full run

| Fault | Effect on the live scores | Fix |
|---|---|---|
| Agents committed `target/`; the scorer ignored it after the session but not before | up to 478 "files outside the team" in from-scratch Ledger, A and B | build folders are skipped on both sides |
| Five hidden tests demanded more than the spec says (id format, `titleProblem` returning `null`, a numeric id, spaces as an empty title, the rounding of 10% off) | "task done" wrongly false for all arms in four from-scratch steps | each test now checks only what the spec states; checked against the pilot's own trees and the reference code |
| The contract check read one fixed file per side and one notation | every from-scratch Relay session counted as a contract break | each side is a glob; a name counts in a quoted string, backticks, a heading or a table cell; Rust test modules and file names are ignored |
| Test code counted as product code (`Some("USD")` in a test) | 4 false "repeated limit" violations | test folders and Rust test modules are outside the forbid rules |
| A setup step's partial contract counted as a break | 1 false contract break | contracts are not scored on the setup step, like ownership |
| "Nothing broken" re-counted a task the agent rightly left undone in every later session | M2 false for B in Ledger | M2 now uses only earlier tasks whose hidden tests passed when they were done |
| The results folder was named `fairlead-ab`, so arm A and B agents saw "fairlead" in their working path | none: no A or B session ran, read or mentioned fairlead (checked in all 31 logs) | the folder is now `ab-eval` |
| A git warning stopped the runner | the first start lost one session | native warnings no longer stop the run |

## Threats noticed

- One run per task; a single different choice changes a row.
- The task writer assigned Ledger t1 to a team that does not own the code, and phrased Relay t3 so
  the contract would be one-sided. Both are kept, as written (PROTOCOL.md).
- The reviewer judged 18 of the 54 "task done" results; it reads a diff with every fairlead trace
  removed, but it is one model.

## Hidden tests for the full run

Written after the pilot, against the task text and the spec only, for every task whose feature the
existing code or the spec names: Shelf t7, s4, s8, s9; Ledger t7, s4, s5, s8, s9; Relay t4, t7, s4
(both sides), s5, s6, s7, s8. Each was run on the reference code (where it fails exactly on the
known issues and the features not built yet) and, for Relay s5 to s8, on a throwaway reference
implementation of the commands, where it passes. Where the spec leaves a reply format open
(`notes.by-tag`, `notes.search`, the spelling of `pinned`, the CSV header), the tests accept every
reading. The other tasks are judged by the blind reviewer.

## Found during the full run

- The scorer read every file in the working tree, including files the project's `.gitignore`
  excludes, so a `Cargo.lock` written by `cargo test` counted as a change, and in Relay as an
  ownership break (first seen in `maint-relay-C-r1-t1`). The working tree is now what
  `git ls-files -co --exclude-standard` lists. The full run is re-scored with `rescore.ps1` when it
  ends; its live numbers before that are provisional.
