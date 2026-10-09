# fairlead — specification (draft, alpha 0.1.0 plan)

> Status: draft · All open questions were resolved on 2026-10-09 (D-46). The product is named `fairlead` (D-58); the diagnostic-code prefix `HN-` is fixed (R-CHK-042). Maintenance rules are in [conventions](conventions.md). Name history: [00-vision](00-vision.md) Appendix A.

Fairlead is a Rust CLI that keeps a codebase consistent for AI agents and human developers across sessions and years. Intent (rules, structure, tokens, hierarchy, ADRs) lives in the repo as documentation that is the truth; small comment directives tie code to it; `fairlead check` reports drift; `fairlead context` gives agents a compact, token-budgeted brief. **No code exists yet. These documents are the product definition.**

## How to read
- Start with [00-vision](00-vision.md), then [decisions](decisions.md): every settled choice, D-01 to D-57 (D-08 and D-09 are not used). A decision always wins over a chapter.
- Rules are numbered `R-<AREA>-<nnn>` and tagged with a milestone: **[0.1]**, **[0.2]**, **[0.3]**. Chapter-local questions are `Q<chapter>-<n>`, all resolved and listed at the end of each chapter. The global register is [70-open-questions](70-open-questions.md) (`Q-AREA-n`), also all resolved.
- Terms are defined once in [01-glossary](01-glossary.md).
- How these files are written and changed is in [conventions](conventions.md).

## Milestones (D-06, amended D-38/D-43)
| | Scope |
|---|---|
| **0.1** | nested scopes (root-only build order first), inheritance, `ovr`/`allow`/approval, `def`/`use`, `check` (read-only; incl. literal-leak check and `--changed`, D-43), `context`, `init`, `new`, `explain`, `sync` (+ `--baseline`), `approve`, baseline |
| **0.2** | seal and versions, environments/maturity/lifecycle/graveyard, editions and `upgrade`, git integration, docs-first coverage, `move` (D-38, including `--detach`) |
| **0.3** | layer ranks and hierarchy, reachability, variation protocol, portability (`move --attach`, `--adopt`, mounted sub-roots, lock and digest pins), retention/compaction, TS<->Rust bridge |

## Chapters
| File | Subject | Milestone |
|---|---|---|
| [00-vision](00-vision.md) | problem, wedge vs prior art, principles, non-goals, naming | — |
| [01-glossary](01-glossary.md) | every term, with its defining chapter | — |
| [decisions](decisions.md) | decision log D-01..D-57 | — |
| [conventions](conventions.md) | authoring rules, required structure and maintenance rules for every file in `docs/` | — |
| [10-scopes-and-inheritance](10-scopes-and-inheritance.md) | scope tree, resolution, override policies, exceptions, control files | 0.1 |
| [11-files-and-classes](11-files-and-classes.md) | five file classes, managerial-folder layout, styles/tokens, vendor-owned files | 0.1 |
| [12-directive-syntax](12-directive-syntax.md) | grammar, six verbs, IDs, control-file schema index | 0.1 |
| [13-checks-and-diagnostics](13-checks-and-diagnostics.md) | rules, severity, baselines, diagnostic codes, output formats | 0.1 |
| [14-context](14-context.md) | `fairlead context`, tiers, budgets, terse format | 0.1 (minimal) |
| [15-cli](15-cli.md) | the verb set, flags, exit codes, `init` | 0.1 |
| [20-versions-and-seal](20-versions-and-seal.md) | versions, `seal`, docs-first coverage | 0.2 |
| [21-environments-maturity-lifecycle](21-environments-maturity-lifecycle.md) | three axes, lifecycle flags, tombstones, graveyard | 0.2 |
| [22-editions-and-upgrade](22-editions-and-upgrade.md) | editions, compatibility, `upgrade` | 0.2 |
| [23-git-integration](23-git-integration.md) | git-case matrix, regenerate-instead-of-merge | 0.2 |
| [30-hierarchy-and-engines](30-hierarchy-and-engines.md) | atomic design for UI and backend, containers, engines, reachability | 0.3 |
| [31-variation-protocol](31-variation-protocol.md) | variants live at the primitive's home; hardcoded-token detection | 0.3 (leak check 0.1) |
| [32-portability-and-restructure](32-portability-and-restructure.md) | move/copy/mount, requires/provides/lock | 0.2 (`move`) / 0.3 |
| [33-retention-and-compaction](33-retention-and-compaction.md) | hot/warm/cold, summaries, hash-chained ledger | 0.2 / 0.3 |
| [40-agent-integration-and-trust](40-agent-integration-and-trust.md) | how agents consume fairlead, authority and trust model | 0.1 / 0.2 / 0.3 |
| [50-architecture](50-architecture.md) | internal architecture, crates, performance, testing (no code) | mixed |
| [60-roadmap](60-roadmap.md) | milestones, exit criteria, success metrics, validation plan | — |
| [70-open-questions](70-open-questions.md) | register of every specification question; all resolved 2026-10-09 (D-46); new open questions are added there | — |
| [examples/ts-react](examples/ts-react.md) | worked TypeScript/React example | 0.1-0.3 |
| [examples/rust-workspace](examples/rust-workspace.md) | worked Rust workspace example | 0.1-0.3 |
| [examples/fullstack-five-years](examples/fullstack-five-years.md) | five-year fullstack scenario | 0.1-0.3 |
| [plans/0.1/00-master](plans/0.1/00-master.md) | implementation plan for 0.1: master plan, contract ([01](plans/0.1/01-contract.md)), amendments ([02](plans/0.1/02-contract-amendments.md)), nine phase plans (p1 to p9) | 0.1 |

## Known limits of this draft
- Written by orchestrated analysis passes and then conformed to the decision log. The chapters have not yet been read end-to-end by a human; treat [decisions](decisions.md) as authoritative where text disagrees.
- Real-repository facts in the examples come from fetched summaries and carry `UNVERIFIED` tags where unconfirmed.
- Token-size figures in [14-context](14-context.md) are estimates, not measurements.
- The worktree lock is answered two ways: Q-GIT-4 in [23-git-integration](23-git-integration.md) and Q32-4 in [32-portability-and-restructure](32-portability-and-restructure.md) disagree. The owner has to pick one (see [70-open-questions](70-open-questions.md), Status).
