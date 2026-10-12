# How fairlead works so far: assessment of 2026-10-12

> Agent-run evidence, small. 79 sessions: the pilot (54, tasks 1 to 3, one run) and the stopped
> full run (25, task 1 only, runs 1 to 3), all re-scored with the corrected scorer. The full run was
> stopped by the owner at 25 of 486 sessions. Model `opencode-go/muse-spark-1.3-contributor`,
> fairlead 0.3.1. No decision threshold of PROTOCOL.md applies: they need the full run.

## The numbers

| Track | Arm | Sessions | Task done | Nothing broken | New violations per session | Tokens per session | Minutes | fairlead commands |
|---|---|---|---|---|---|---|---|---|
| maintenance | A, no guidance | 15 | 100% | 100% | 0.40 | 26,157 | 1.1 | 0 |
| maintenance | B, hand-written AGENTS.md | 11 | 91% | 100% | 0.09 | 36,065 | 2.3 | 0 |
| maintenance | C, fairlead | 14 | 100% | 100% | 0.29 | 33,661 | 1.4 | 6.2 |
| from scratch | A | 11 | 100% | 100% | 0.00 | 40,611 | 2.2 | 0 |
| from scratch | B | 14 | 100% | 100% | 0.14 | 42,137 | 2.5 | 0 |
| from scratch | C | 14 | 93% | 100% | 0.00 | 57,831 | 3.7 | 5.4 |

## What fairlead did in the 28 arm-C sessions

- **The agents used it as told.** The agent block made every C agent run fairlead: 60 `context`,
  53 `check`, 35 `explain` calls in 28 sessions.
- **It found almost nothing.** `check` reported an error in 3 of 28 sessions, and every one of those
  errors was fairlead's own bookkeeping (`HN-G001`, generated files out of date; `HN-R001`, a
  dangling ID), not a broken product rule. No token leak, pattern rule, layer or boundary finding
  fired, because the agents did not break those rules in the first three tasks, with or without
  fairlead.
- **From scratch, fairlead stayed empty.** In all 8 from-scratch C chains the agents ran fairlead
  every session but declared nothing in it: no rule, no directive, no doc. The spec's requirements
  were never written into fairlead, so its checks had nothing to check. Every fairlead command there
  was pure cost.
- **In maintenance, the agents rarely extended it.** Across 8 chains, C agents added 5 directives in
  total for the code they wrote.
- **Where violations happened, fairlead had no rule for them:** an edit in another team's crate
  (fairlead 0.3 does not check edits against owners), a command added on one side of a contract
  (bridges compare files and symbols, not wire names), an app importing another app (Shelf's rule
  catches it, but it happened in arm A).

## Assessment

1. **No benefit shown, a cost shown.** On the first tasks, a capable model keeps these small
   projects' rules almost perfectly on its own, so there is nothing for fairlead to catch. Fairlead
   cost 29% more tokens than no guidance in maintenance and 42% more from scratch (37% more than a
   hand-written AGENTS.md from scratch; about equal to it in maintenance).
2. **A good AGENTS.md is the strongest competitor.** Arm B kept the rules as well as or better than
   C, at similar or lower cost, with no tool at all.
3. **Fairlead does not bootstrap itself.** Its checks are only as good as the declared rules, and
   agents do not declare rules on their own, even when told the repository uses fairlead and given a
   spec full of requirements. Writing the rules down is the expensive part, and it was left undone.
4. **Its own bookkeeping shows up as friction:** the only errors it raised were about its generated
   files.
5. **The thesis is still untested, not refuted.** Fairlead claims to stop drift over long chains, in
   codebases too large to read whole, and across teams. Tasks 1 to 3 on projects of a few hundred
   lines cannot show that; the stopped full run would have reached tasks 4 to 9 only.

## What this suggests

- Do not build 0.4 as planned: its features (objects, constants, anchors, query rules) all deepen
  checking of declared rules, and the evidence says the problem is upstream: rules are not declared,
  and on small tasks nothing breaks.
- If fairlead continues, test the claim where it lives before building more: one larger project an
  agent cannot read whole, long task chains across teams, and a cheaper arm D (fairlead as a CI
  check only, with no agent loop), so benefit and cost can be separated.
- Consider making rule capture the product: the from-scratch result shows the gap is between a
  written requirement and a checkable rule; that step is where an agent needs help.

## Limits

Tasks 1 to 3 only; 11 to 15 sessions per cell; one model; projects built by the maintainer knowing
what fairlead checks; the reviewer judged a third of "task done" results. The raw logs are outside
the repository; the scored sessions are in `2026-10-12-pilot/` and `2026-10-12-full/`.
