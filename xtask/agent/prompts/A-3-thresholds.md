# Task: propose the 0.2 and 0.3 thresholds

Attached: the roadmap (`60-roadmap.md`; section 4 holds each milestone's exit criteria and
thresholds), the 0.1 benchmark baseline (`baseline.toml`), and two agent-run results: A-1 (an
eight-week simulated pilot; its threshold is at most 30% stale or expired exceptions) and A-2 (the
coverage-replay labelling; its threshold is at most 5% wrong flags).

For each threshold in roadmap sections 4.2 and 4.3, decide whether the evidence supports keeping
it, tightening it or loosening it. Base every proposal on a number in the attached files and say
which. Where the evidence is thin (one model, few samples), say so and prefer keeping the
threshold. Never loosen a threshold only because a result missed it: a missed threshold is
reported as missed.

Write the amendment between two marker lines, as Markdown the owner can paste into the roadmap:

BEGIN-DRAFT
| Threshold | Current | Proposed | Evidence | Reason |
|---|---|---|---|---|
...
END-DRAFT

Judge only from the attached files. Do not run commands or open other files. End with one line:
`LABEL: change` if any threshold changes, otherwise `LABEL: no-change`.
