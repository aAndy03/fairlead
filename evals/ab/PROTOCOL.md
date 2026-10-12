# With and without fairlead: the A/B/C protocol (pre-registered)

> Status: pre-registered on 2026-10-12, before any project, task, hidden test or scorer of this
> folder existed. Owner: maintainer, at the owner's request. Changes after the first measured run are
> dated amendments appended at the end; an amendment invalidates the runs before it. A missed
> threshold is reported as missed (agent-runbook rule 8). This protocol supersedes, and absorbs,
> the 0.1 E2 ablation (`docs/plans/0.1/ablation-protocol.md`), which was pre-registered but never run.

## The question

Does an AI agent that works with fairlead leave a codebase in better shape than the same agent
without it, and what does fairlead cost? "Better" is measured from outside fairlead; "cost" is
measured in time, tokens, money and agent effort. Both are reported, whichever way they point.

## Arms

Every session uses the same model, the same settings and the same task prompt. The only difference
is what the repository contains.

| Arm | Repository | Binary on PATH | Extra prompt line |
|---|---|---|---|
| **A** no guidance | the project with every fairlead file and directive comment removed | none | none |
| **B** hand-written guidance | as A, plus an `AGENTS.md` that states the same rules in plain prose, written by hand once per project | none | none |
| **C** fairlead | the project as adopted: `fairlead/` folders, directive comments, the agent block `fairlead init` writes into `AGENTS.md` | `fairlead` | none |

No arm gets "use fairlead" in its prompt: arm C learns it from its `AGENTS.md`, as a real adopted
repository teaches it (D-66), and arm B gets the equivalent prose. An instruction only one arm
receives would measure the instruction, not the tool.

The question that matters is C against B ("does fairlead beat a good AGENTS.md?"). C against A is
reported too ("does it beat nothing?").

## Projects

Three projects, built for this protocol, each with simple and complex parts and with rules a careless
change would break:

1. **Shelf**: a TypeScript UI kit (design tokens, shared components owned by a `kit` team) and two
   apps (`store`, `admin`) owned by other teams.
2. **Ledger**: a Rust workspace in layers (`core` < `engine` < `service` < `cli`), with shared
   constants and a documented error contract.
3. **Relay**: a TypeScript front end and a Rust back end joined by a command contract that both
   sides must keep in step.

Each project has, outside the agent's sight: hidden tests per task, and an independent rule scorer
configuration. Neither is ever copied into a session before the session ends.

## Tasks

- Written by a separate agent session that sees only each project's `README.md` (the builder of the
  projects is not the task writer; agent-runbook rule 1). Nine per project, ordered as a chain: each
  session starts from the repository the previous task left in the same run.
- Mix, required of the task writer: three simple (a fix or a small addition inside one area), three
  medium (a change across two areas), three complex (a feature that crosses ownership or layers, or
  needs something another team owns). At least two tasks per project where fairlead should be
  neutral (plain bug fixes), so its overhead shows.
- Hidden tests are written after the tasks, by the maintainer, against the task text only.

## Measures (per session, scored after the session ends)

Scored by `cargo xtask ab-score`, which does not call fairlead or link its crates.

| ID | Measure | How |
|---|---|---|
| M1 | Task done | the task's hidden tests pass |
| M2 | Nothing broken | the hidden tests of every earlier task in the chain still pass |
| M3 | Rule violations | count by the independent scorer, per category: hardcoded values that have a declared constant or token, imports across forbidden layers or teams, edits to files another team owns, copies of another team's component, contract drift between two sides |
| M4 | Scope | files changed outside the areas the task names |
| C1 | Time | wall-clock seconds of the session |
| C2 | Tokens | input + output + reasoning tokens, from the agent's event log |
| C3 | Money | the agent's reported cost |
| C4 | Effort | tool calls; turns |
| C5 | Ceremony | files the agent wrote that are not code or tests (docs, requests, change docs); reported, not penalised |
| C6 | Friction (C only) | `fairlead` commands run; findings the agent argued with or waived |

## Decisions (full run only; the pilot decides nothing)

Fixed now, before any data:

- **Fairlead helps** when, over all sessions, arm C's mean M3 per session is at most 50% of arm A's
  **and** at most 75% of arm B's, **and** arm C's M1 rate is not more than 10 points below either.
- **No better than a good AGENTS.md** when arm C's mean M3 is at least 90% of arm B's. Reported in
  those words.
- **Fairlead hurts** when arm C's M1 or M2 rate is more than 10 points below arm B's.
- **Expensive** when arm C's mean C2 is more than 50% above arm B's. Reported whatever M3 says.
- **Long-run signal**: for each arm, M3 by chain position (task 1 to 9). Fairlead's claim is about
  drift accumulating; a flatter slope for C than for A and B is the evidence. Reported as a chart
  and a fitted slope, with no threshold (nine positions are too few for one).

## Size, runs and statistics

- **Pilot**: 3 projects × first 3 tasks × 3 arms × 1 run = 27 sessions. Purpose: find bugs in the
  projects, tasks, scorer and runner. Its numbers are published and labelled "pilot, not evidence".
- **Full run**: 3 projects × 9 tasks × 3 arms × 3 runs = 243 sessions. Run order randomised with a
  recorded seed; the three arms of one task and run index are interleaved, so model drift during the
  day spreads over all arms.
- Reported: mean, minimum and maximum per arm; per project; per task tier (simple, medium,
  complex). A difference smaller than the spread between runs of one arm is reported as "no
  difference shown".

## Settings recorded per session

Model ID, opencode version, date and time, the commit of this repository, the fairlead version
(arm C), the seed, the arm, the project, the task, the run index, the chain position. Raw event
logs stay outside the repository; their SHA-256 is recorded.

Model: `opencode-go/muse-spark-1.3-contributor` (agent-runbook rule 4), for every session.

## What this cannot show (stated with every result)

- **Time**: a chain of nine tasks stands in for months of work, not years.
- **Size**: the projects are small (hundreds to a few thousand lines), not 50,000, a million or more.
- **One model**: another model, or a human with an agent, may behave differently.
- **Built for the test**: the projects were written knowing what fairlead checks. The task writer
  never sees the rules, the neutral tasks exist to show overhead, and the scorer is independent, but
  the projects' rules are still the kind fairlead was designed for.
- **Arm B's quality**: arm B is one hand-written `AGENTS.md`; a better one could do better.
- **Small samples**: three runs per task gives a direction, not a proof.

## Amendments

(none)
