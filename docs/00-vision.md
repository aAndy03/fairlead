# Vision

> Status: draft · Milestone: mixed · Decisions: D-01, D-03, D-06, D-07, D-10, D-23, D-26, D-27, D-28, D-30, D-32, D-34, D-37, D-39, D-42, D-43, D-46, D-47 · Related: 01-glossary.md, decisions.md, 50-architecture.md, 60-roadmap.md, 70-open-questions.md

## 1. Purpose

This chapter states the problem fairlead exists to solve, what fairlead is and is not, why it is built the way it is, and how it is positioned against existing tools. It also records the owner's original vision in plain prose so that every later chapter can be traced back to an intent. The product is named `fairlead` (D-58; first working name `harna`, see Appendix A).

## 2. The problem

A codebase that lives for years is edited by many people and, increasingly, by AI agents that start every session knowing nothing. Three failures recur.

1. **Context loss across sessions.** An agent that fixed a problem on Monday has no memory of why on Friday. It re-derives conventions from whatever files it happens to open, repeats rejected experiments, hardcodes values that were deliberately centralised, and invents a second way to do something the codebase already does.
2. **Static instruction files do not scale.** `CLAUDE.md`, `AGENTS.md`, `llms.txt`, Cursor rules and skills help, but they are prose that nobody checks. They go stale silently, they cannot say "this rule applies here but not there, and the exception is approved until March", and they cost the same tokens whether or not they are relevant to the task. Tools that fan one rule file out to many assistants copy the text, so the copies drift.
3. **Intent and code diverge, and nobody sees it happen.** A team declares "colours come from tokens". Six months later forty files carry hex literals. The declaration is still true as a document and false as a description of the code. The same happens to layering rules, retired features, and architecture decisions.

Humans suffer the same failures more slowly. Agents make them visible because they are fast, literal, and start cold every time.

## 3. What fairlead is

Fairlead is a command-line tool, written in Rust and published on crates.io, that acts as a versioned harness around a codebase. It does four things.

| # | Job | Mechanism |
|---|---|---|
| 1 | Hold the project's declared intent as documentation | Rules, tokens, constants, decisions, structure and roadmap live as Markdown with TOML frontmatter and `def` directives, plus TOML control files, in committed `fairlead/` folders. The human reviewer reads these documents; they are the truth ([10-scopes-and-inheritance.md](10-scopes-and-inheritance.md)). |
| 2 | Let code carry small references to that intent | Directives are comments (`// fairlead: use tok.brand`), so there is no compile cost and no language lock-in ([12-directive-syntax.md](12-directive-syntax.md)). |
| 3 | Check the code against the intent | `fairlead check` reports drift with stable rule IDs, an intent excerpt and a fix hint ([13-checks-and-diagnostics.md](13-checks-and-diagnostics.md)). |
| 4 | Give agents a compact, relevant brief | `fairlead context <path>` returns the effective rules, items, open drift and rejected ideas for a place in the tree, under a token budget ([14-context.md](14-context.md)). |

Fairlead does not hardcode folder, file, or document names. The project shapes its own vocabulary (kinds, layers, environments) in the control file, and fairlead enforces the shape the project declared.

### 3.1 What fairlead is not

| Not | Why |
|---|---|
| An agent, an LLM wrapper, or a prompt library | Fairlead has no model and makes no network calls. It informs, scaffolds, and fails checks. It cannot make an agent obey, and it never claims to. |
| A daemon, watcher or service | It runs only while a command runs (D-26). |
| A replacement for linters, type checkers, or tests | It checks declared intent, not general code quality. It is meant to sit beside ESLint, clippy, Semgrep and CI. |
| A proof that the codebase is consistent | A passing `check` means "the declared constraints that fairlead can verify hold". Undeclared drift is invisible. This sentence is repeated in the shipped reference docs. |
| A release manager | Release notes (Changesets-style tools), ownership (CODEOWNERS), deploy environments (CI/CD) and dependency bumps (Renovate-style tools) stay with their tools. |
| A self-updating tool | `fairlead upgrade` migrates files in the repository. It never replaces the binary. |
| A package manager for scopes | Portability exists to survive restructures and mounts, not to distribute scopes between organisations. |

## 4. The wedge

D-01 sets the product's lead claim. It is deliberately narrow.

> Fairlead leads with **per-key inheritance with an explicit override policy**, **expiring waivers**, and an **exception ledger**. Drift reports and token budgets are table stakes.

Stated as a user-visible promise: in a tree of nested scopes, every deviation from a parent's rule is either impossible (sealed), or a visible, owned, expiring, recorded exception. "What is an exception in the larger context is a principle in its own context" is expressed in syntax, not hidden in config.

### 4.1 Prior art, honestly

Tags follow the prior-art survey: **V** = verified on a primary page, **S** = search snippet or third-party only, **UNVERIFIED** = not confirmed.

The combination "declared intent plus drift plus agent context" is already occupied. Archkeel covers a declared contract, baseline-aware validation and an agent-oriented report slice (V, README and 0.8.0 release) and was active into October 2026 (S). Fairlead must not claim novelty there.

| Prior art | What it already does | Where fairlead differs |
|---|---|---|
| **Archkeel** (V) | `architecture-contract.json`; `validate --baseline` with `--accept-new`; agent slice (`report --only architecture --json`); installable agent skill; recursive child contracts. | Intent is human-reviewed documents, not an import contract. Per-key override policy with sealed keys, expiring waivers and a ledger (none documented). Rules beyond imports (tokens, constants, ADRs, structure). 0.1 is AST-free, Archkeel parses Python, Dart and TypeScript. Fairlead adopts its honesty line: a pass proves supported declared constraints, not complete architecture quality or agent obedience. |
| **Archgate** (V, partial) | ADRs with `files` globs and companion TypeScript check files; `archgate check` exit codes; Claude Code and Cursor plugins. | Rules are declarative data, not TypeScript. Waivers with reason and expiry, supersede/deprecate lifecycle and machine-readable output are not described there. |
| **OpenSpec** (V, partial) | `specs/` as truth, `changes/<name>/` proposal plus deltas, archive on completion, many assistants. | Nested inheritance, drift or staleness checks, a warn-level coverage check, budgeted context. Fairlead adopts ADDED/MODIFIED/REMOVED delta vocabulary where useful (MODIFIED and REMOVED: S). |
| **Turborepo and Nx** (V) | Package `extends` with an explicit append marker and per-task opt-out; `turbo boundaries` allow/deny per tag; Nx tags with `depConstraints`. | Intent is a document tree agents load under a budget, not config inside build tooling. Exceptions are ledger entries with expiry. Lifecycle and maturity are ranked axes. |
| **Nested AGENTS.md / CLAUDE.md chains, Copilot path instructions, Cursor rules, Kiro steering; Ruler and rulesync** (V) | Lazy loading by directory, activation modes, one source fanned out to many tools. Ruler has a CI drift check on generated copies. | Fairlead writes a pointer, not copies. Child overrides are declared, not silently appended. Drift is checked against code. Overflow is an explicit dropped list, unlike Codex's silent 32 KiB cut. |
| **agentslint** (V, 0.1.0 Jul 2026) | Referential checks on AGENTS.md: dead paths, dead commands. | Fairlead checks intent against code, not one file's references. |
| **Mechanism donors** | tsconfig `extends`, Cargo `workspace = true` and pnpm `catalog:` (V): opt-in inheritance by key. `.editorconfig` `root = true` and `unset` (V): scope stop and local exception. Rust `#[expect(lint, reason)]` (V): stale-waiver detection. ArchUnit freezing (V): shrinking baseline ratchet. Kyverno Audit-to-Enforce ramp and exception objects (S). Bazel visibility (V): provider-declared consumers. Backstage `partOf` vs `dependsOn` (V): containment separate from dependency. MADR `rejected` status (V): closest graveyard analogue. Changesets per-change files (V): collision-free draft IDs. CODEOWNERS (V): required review. | Fairlead combines them under one declared policy per key. None of the sampled tools seals keys or keeps an expiring exception ledger. |

The verdict: the wedge survives only through two features no verified tool shows: (1) per-key inheritance with an explicit override policy and sealed keys; (2) waivers with expiry plus an exception ledger. This claim is bounded by the survey's sample. Snippet-only tools (tsarch, ArchSpec, bitloops, vibgrate, packmind) were not weighed, and a digest-and-delta context protocol has no prior art found, so it is novel and untested. Tags carried over unchanged: Structurizr `!include`, the C4 model, dependency-cruiser `--ignore-known`, Semgrep inline ignores, Design Tokens alias syntax, Repomix compress options, Kyverno details (S), Changesets file path (S), spec-kit constitution path (S) are UNVERIFIED or S and are not relied on anywhere in the spec. Before the first public release every such citation is verified on a primary page or removed (R-VIS-008, [60-roadmap.md](60-roadmap.md) R-REL-041); a tag is not changed here until the verification has been done and dated.

There are also live crates in the same concept space: `charter` (LLM codebase-context generator), `corbel` (MCP call-graph server for coding agents), `clew` (git-native project tracker for agents) and `dowel` (architecture linter, placeholder). Fairlead's positioning must be stated against them at publish time (R-REL-041).

### 4.2 Risk to the wedge

A literal-leak check alone is replicable with ripgrep and CI. Substitutes (Semgrep or ast-grep rules, dependency-cruiser or Nx boundaries, ESLint rules, nested CLAUDE.md or Cursor glob rules) may already cover much of what 0.1 checks. The literal-leak check, using declared token values plus a property context, is nevertheless part of 0.1 (D-43): it is the owner's original style-token example and the 0.1 proof that declared intent can be checked against code. The defensible part is the loop: declaration, then check, then agent context, then a recorded exception when reality disagrees. [60-roadmap.md](60-roadmap.md) turns this into falsifiable criteria and lists the conditions under which the project should stop.

## 5. Core principles

| # | Principle | Consequence in the spec |
|---|---|---|
| 1 | **Documentation is the truth.** | Intent lives in documents a human reviews. Code carries IDs, not copies of rules. The managerial folder is committed and visible. |
| 2 | **Docs first.** | A change is described in the scope that owns it, then implemented. Fairlead verifies path coverage, not temporal order, and says so (D-15). |
| 3 | **Intent flows down, facts flow up.** | Authored intent is resolved at read time down the scope chain and never copied (D-10). Registries, ledger counts and summaries roll up only at `seal` (0.2). |
| 4 | **Locality and declared exceptions.** | A child scope may deviate, but only through a declared `ovr` or an ancestor-authored `allow`, depending on the key's policy (D-02). The deviation is relative: a principle in its own scope, an entry in the ancestor's ledger. |
| 5 | **Syntax over tools.** | The CLI verb set is small and fixed (D-25). Variety comes from user-defined kinds and a six-verb directive grammar (D-18). |
| 6 | **Deterministic.** | Same inputs, same bytes, on every machine. Time enters a check only through `--as-of` (default UTC today, printed in the output header). No hash-order output. |
| 7 | **No background processes.** | Nothing runs between commands (D-26). Staleness and retention are evaluated when a command runs, using recorded timestamps. |
| 8 | **Token efficiency.** | Terse `ID k=v` lines, tiers L0 to L3, budgeted packing, ancestor deduplication (D-23). A syntax card an agent must read stays short. |
| 9 | **Principles, not absolutes.** | Severity ramps observe, warn, enforce; baselines let a rule land in a legacy repo; waivers expire; waiver sprawl is reported (D-28). |
| 10 | **Authority is not self-granted.** | Floor rules compiled into the binary cannot be waived; the managerial folder needs CODEOWNERS review; `fairlead approve` writes `[[approval]]` records to `fairlead/approvals.toml` and refuses without a terminal or under CI, so an unattended agent cannot approve itself (D-03, D-34). |
| 11 | **Readable after 100 years.** | Plain text formats (TOML for authored input, Markdown, terse lines for generated files, D-32), committed generated files that are sorted and sharded, summaries kept forever, tombstones for removed IDs, version-based retention with a calendar floor (D-39). Raw history stays in git; the tree keeps summaries and pointers (D-24). |
| 12 | **Directive text is data.** | Anything fairlead prints from a repository file is emitted as fenced data with provenance, never as an instruction to the agent (D-18, [40-agent-integration-and-trust.md](40-agent-integration-and-trust.md)). |

## 6. Non-goals

1. Making an agent do anything. Fairlead cannot enforce obedience, only report divergence.
2. Understanding code semantics in 0.1. The core is AST-free (D-27); import graphs via tree-sitter or `syn` are later.
3. Verifying deployment state. Environment declarations are cross-checked against build manifests for existence and never evaluated (D-16).
4. Natural-language contradiction detection. Doc contradictions are tracked only when declared (D-22).
5. Authenticating authors. Without signed commits, owner identity is unauthenticated; fairlead relies on CODEOWNERS review, and `approve` is terminal-gated record-keeping, not authentication (D-03, D-16, D-34).
6. Telemetry. Waiver growth is measured from git history, not reported by the tool (R-VIS-007).
7. YAML. Authored input is TOML or Markdown; generated files and `context` output are terse `ID k=v` lines (D-05, D-32).
8. Replacing `Cargo.toml` or `package.json` versions. The intent version, once it exists (0.2), is `MAJOR.MINOR.PATCH`, independent of package versions, and optionally mirrored (D-14, D-42).

## 7. Personas

| Persona | Wants | Touches | Pain fairlead removes |
|---|---|---|---|
| **Human developer** | To change code without breaking conventions they did not write. | `check`, `explain`, `new`; reads `fairlead/` docs. | Folklore. "Why is this rule here, and who may break it?" is answered by `explain`. |
| **AI agent** | A small, correct brief for the files it is about to edit; scaffolds so it never hand-writes config. | `context`, `new`, `check`, `explain`. Never `approve`. | Cold starts, rule hallucination, repeating rejected ideas. |
| **Reviewer** | To see what a change deviates from, and whether a human agreed. | The diff of `fairlead/`, the exception ledger, `approve` records, CODEOWNERS rules. | Deviations hidden in code. New `ovr` lines are visible, owned and expiring. |
| **Owner / maintainer** | To own the vocabulary and the policy; to keep the system readable for years. | The root control file, `allow` grants, `seal`, `upgrade`, retention settings. | Rule sprawl, waiver sprawl, orphaned structure, silent rewrites of intent. |

## 8. A day with fairlead

Scenario: web app in a monorepo with a root scope and a child scope `web`. Mara is the owner of `web`. An agent is asked to add a compact density to the data table. Output formats are illustrative; exact shapes are fixed in [14-context.md](14-context.md). The scenario uses the whole model: a step or line that needs a later milestone is marked [0.2] or [0.3]; in 0.1 the day is the same without those lines.

**09:00 - session start.** The agent runs `fairlead context apps/web/src/tables --budget 700` and receives about 150 tokens.

```text
@tables d=aq5c2e7k ed=1 ^root:dq3f2xbr ^web:7bq4x2ec
purpose: "data tables for admin views"
inh: ^root dq3f2xbr 6err | ^web 7bq4x2ec 2err
rules: rule.no-raw-hex "tokens not hex" (web)
uses: tok.surface cmp.button cmp.data-table
drift: 2 rule.no-raw-hex RowStyle.tsx:44,81 | waivers 0
grave: grv.0004 "autosave on blur" rejected, do-not-retry     [0.2]
=d 4kq5w2ma next: fairlead explain rule.no-raw-hex
```

**09:05 - docs first [0.2 for change docs, 0.3 for requests].** The task needs a new density value on a primitive that lives in the `ui` scope, so the agent files a variation request and a change doc with `fairlead new` instead of copying the button. The scaffold fills the frontmatter slots; the agent writes only the prose.

```toml
+++
kind = "change"
id = "d-7k3m9qxa"
scope = "web"
covers = ["apps/web/src/tables/**"]
status = "draft"
created = "1.4.0"
+++
```

**10:30 - implementation.** The chart library positions elements with inline styles, which `rule.no-inline-style` forbids. The rule is marked `waivable`, so the agent may override it, but must say why, who owns it (a handle in the scope's `owners`), and when it ends.

```fairlead
// fairlead: use cmp.data-table tok.surface
// fairlead: ovr rule.no-inline-style why="chart lib positions via style attr" owner=@mara exp=2027-01-31
const el = <Chart style={{ position: "absolute" }} />
```

**10:45 - check.** `fairlead check apps/web` fails on `RowStyle.tsx:44`: a hex literal equal to `tok.surface` (the literal-leak check, D-43), with the intent excerpt and the fix `use var(--surface)`. The agent fixes it. The new `ovr` is recorded in the exception ledger; it is effective once Mara has approved it, and `explain rule.no-inline-style` shows `effective=waived in web (why, owner, exp); root says error`.

**11:00 - session end.** The agent runs `fairlead sync` and `fairlead check --for-agent`. Fairlead never commits; the commit is the agent's git. The agent cannot run `fairlead approve`, which refuses without a terminal and under CI. Before leaving, it runs `fairlead new handoff` and writes a one-line `sum`; the next session's brief prints it.

**Next morning - review.** The reviewer sees the diff in `fairlead/` and the ledger line for the new waiver, which CODEOWNERS requires Mara to review. Mara runs `fairlead approve` on it. Three months later `fairlead check --as-of` reports the waiver as expired (`HN-X001`, a floor check): the waiver is ineffective, the underlying finding returns and fails the run, and the owner must renew it with a new approval or fix the code.

**Months later.** A different agent runs `fairlead context apps/web/src/settings` before adding autosave to the settings form; the graveyard line is printed [0.2], so the idea is not retried without a stated reason. A free-text query such as `fairlead context "add autosave to settings form"` exits 2 until task matching exists [0.3]; fairlead never guesses.

## 9. The owner's original vision

This section condenses the owner's twelve proposals into prose. It is the origin of the spec, not a statement of what each release delivers; the right-hand column of the table in 9.2 shows where each proposal is specified and where the decisions deviated.

### 9.1 In plain prose

Fairlead is a dynamic, versioned harness for AI agents and developers who work on one codebase across many sessions and years. It is installed once as a Rust tool. It does not demand a dependency in the project, and it never hardcodes the names of the project's folders, files or documents.

A project holds its intent as documentation. Any folder may carry its own managerial folder, which becomes the "global root" for that subtree while still extending the managerial folder of the parent scope, all the way up to the codebase root, "the root of all roots". If a folder `A` has `A1`, `A2`, `A3`, then `A` extends the root, and each of `A1`..`A3` extends `A` and the root. A shared stylesheet of variables at the root may be extended by a stylesheet in `A`, and a leaf such as a sidebar may keep only local files, or grow its own file that is then extended by component styles. A local file may contradict what it extends, but only as a declared exception: what is an exception in the larger context is a principle in its own context, and the owner wants that handled by syntax. Documents are scoped the same way. Restructuring a tree must be handled mostly automatically by a few tools, not one tool per case. Open cases the owner named: a root that becomes a child of a larger codebase, and a folder that is cut or copied into another codebase.

The root managerial folder contains a control file, written by a human or agent, that governs the generated and reference files of every descendant. Destructive edits are acceptable because git guards history. Lines of text have classes just as files do, and some changes propagate up while others propagate down.

Changes are made under a numbered version, with a global version and local versions per folder or file, tied to git and robust to every git workflow, including work done only inside a child folder. Environments (dev, prod, anything the user names) and maturity (alpha, beta, stable) are user-defined words, with some things existing only in some environments. Features have lifecycles: to be deprecated, replaced, edited, removed. Rejected ideas are kept as summaries so that a future agent does not retry them.

Design follows atomic design as a mindset for the whole stack, not only the user interface: every feature belongs to the system, containers group logic and nest arbitrarily, a pipeline or engine exists for everything, and a single "sync" button may call engines (verify, propagate progress, purify, validate connection, handle edge cases) each living in reusable containers. Work flows brainstorm, then written documentation of the change, then implementation. If one part of the codebase needs a variation of a shared component, the variation is added where the primitive lives and imported where needed; it is never hardcoded locally.

Fairlead must update itself gracefully: minor and major releases must not break codebases pinned to older versions. Context and audit must stay token-efficient at scale. Even after a hundred years of edits the folders must stay readable by humans and agents, some information has a retention period (a year, or the life of a feature, or until deleted), and summaries remain forever. Throughout, the owner asks for minimal redundancy, consistency across time, modularity, scalability, and edge cases of real human and agent behaviour handled over years.

### 9.2 Where each proposal is specified

| # | Proposal | Specified in | Deviation from the proposal as written |
|---|---|---|---|
| P1 | Nested managerial folders, extension, exceptions, restructure, mounting | [10](10-scopes-and-inheritance.md), [32](32-portability-and-restructure.md) | Eligible, not mandatory, in every folder (D-11). Authored files are never merged; fairlead checks the extends contract (D-04). |
| P2 | Control file, line classes, propagation | [10](10-scopes-and-inheritance.md), [11](11-files-and-classes.md), [12](12-directive-syntax.md) | Line direction is a fixed property of the verb, not per-line. Files have five classes (D-19). |
| P3 | Versioning tied to git | [20](20-versions-and-seal.md), [23](23-git-integration.md) | No per-file version headers (D-13). The global version is `MAJOR.MINOR.PATCH` (D-42); versions are per scope and per ID'd contract item; file history is derived from git. New docs get a once-only `created` stamp (D-13). |
| P4 | Environments and maturity | [21](21-environments-maturity-lifecycle.md) | Declarations are cross-checked, never evaluated (D-16). |
| P5 | Flags and lifecycle | [21](21-environments-maturity-lifecycle.md) | Lifecycle states are fixed; words may be aliased. |
| P6 | Atomic design, docs-first | [30](30-hierarchy-and-engines.md), [13](13-checks-and-diagnostics.md) | Coverage check, warn by default, not temporal enforcement (D-15). |
| P7 | Fairlead self-update | [22](22-editions-and-upgrade.md) | `upgrade` migrates files, never the binary. |
| P8 | Context and retention at scale | [14](14-context.md), [33](33-retention-and-compaction.md) | `sum:` is authored at creation and `out:` at close (D-39); no model inside fairlead (D-24). |
| P9 | Graveyard | [21](21-environments-maturity-lifecycle.md), [33](33-retention-and-compaction.md) | Lexical matching and a content fingerprint in 0.2; synonyms and rewrites evade it (21 R-LC-039). |
| P10 | Human and agent edge cases | all chapters; [examples/](examples/) | Each chapter carries a numbered edge-case table (R-VIS-009). |
| P11 | Variation protocol | [31](31-variation-protocol.md) | Four outcomes; mandatory request for primitive variations (D-21). |
| P12 | Hierarchy of components, containers, engines | [30](30-hierarchy-and-engines.md) | Ladders are user data; presets via `fairlead new ladder`; adoption ramp, not totality (D-20). |

## 10. Rules

| ID | Rule | Milestone |
|---|---|---|
| R-VIS-001 | Fairlead MUST NOT claim, in its CLI output, reference docs, README or crate description, that a passing `check` proves codebase consistency. The shipped text MUST say it proves declared, verifiable constraints only. | [0.1] |
| R-VIS-002 | Fairlead MUST NOT run any process between commands (no daemon, watcher, scheduled task). | [0.1] |
| R-VIS-003 | Fairlead MUST NOT make network calls in any command, except `upgrade --fetch` (0.2), which retrieves edition migration data only and never a binary ([15-cli.md](15-cli.md), R-CLI-009). Exact token counts through a vendor API, if ever added, MUST be an explicit opt-in flag. | [0.1] |
| R-VIS-004 | The product name and managerial folder name MUST come from configuration (`marker`, folder name), never from string literals scattered in code, so that a rename is a configuration and packaging change. | [0.1] |
| R-VIS-005 | The public pitch leads with the wedge (D-01): per-key inheritance with an explicit override policy, expiring waivers, an exception ledger. Public copy MUST NOT claim more than the measured effect of the exception-hygiene experiment ([60-roadmap.md](60-roadmap.md) R-ROAD-009). If that experiment fails, the pitch changes to the declaration-check-context loop (60 edge case 1). | [0.1] |
| R-VIS-006 | Nothing is published (crates.io, prebuilt binaries, announcement) until Gate G1 has passed (60 R-ROAD-011) and the pre-publish checklist is complete (60 R-REL-040 to R-REL-046): trademark check of the target name, prior-art verification, crate name claim. The first public release is the 0.1 alpha, under the target name `fairlead` unless the trademark check eliminates it (D-47). | [0.1] |
| R-VIS-007 | Fairlead MUST NOT collect or send telemetry, crash reports or usage data, and has no update check. Waiver growth and adoption are measured from the git history of pilot repositories (`git log -S "fairlead: ovr"`), never reported by the tool. The README and `explain` state this. | [0.1] |
| R-VIS-008 | Every prior-art citation in 4.1 tagged S or UNVERIFIED MUST be verified on a primary page or removed before the first public release. A citation that stays unverified MUST NOT appear in public copy. The survey is refreshed at each milestone (60 R-REL-032). | [0.1] |
| R-VIS-009 | Every normative chapter (10 to 40) MUST carry a numbered edge-case table (columns: number, case, behaviour, rule) covering human and agent behaviour over years; there is no separate edge-case chapter (P10). | [0.1] |

## 11. Resolved questions

All questions were accepted by their stated leaning on 2026-10-09 (D-46).

- Q-VIS-1 (resolved 2026-10-09): the public lead is the wedge (D-01), validated by the exception-hygiene experiment before the first public announcement; see R-VIS-005
- Q-VIS-2 (resolved 2026-10-09): the product is public only after the 0.1 ablation criterion is met and the target name is applied (Appendix A); 0.1 is published as an alpha; see R-VIS-006
- Q-PROD-1 (resolved 2026-10-09): all inferred decisions D-10 to D-45 are ratified by the owner (D-46); see decisions.md
- Q-PROD-2 (resolved 2026-10-09): the target name is `fairlead` after a trademark check (D-47); the docs keep `fairlead` until then; see Appendix A, R-VIS-006
- Q-PROD-6 (resolved 2026-10-09): prior-art citations are verified before publishing; see R-VIS-008, [60-roadmap.md](60-roadmap.md) R-REL-041
- Q-PROD-7 (resolved 2026-10-09): no telemetry; waiver growth is measured from git history; see R-VIS-007
- Q-PROD-8 (resolved 2026-10-09): numbered edge cases in each chapter, no separate chapter; see R-VIS-009

## Appendix A. Naming

Checked 2026-10-08 against the crates.io API, GitHub search and docs.rs. Trademark checks were not done for any name and are a hard gate before publishing (60 R-REL-040). Whether crates.io rejects near-duplicate names is UNVERIFIED.

### A.1 Current name

`fairlead`, `fairlead-cli` and `fairlead-core` are unclaimed on crates.io (HTTP 404). GitHub search for "fairlead" returns 222 results; the top ones are `MarkGillespie/fairleadck-blender` (29 stars) and `fairleadsproject/fairleads` (6 stars). The nearest live product in the same space is `harnext`, described on one vendor page as an MIT AI coding agent installed via npm; no repository or npm listing was found (UNVERIFIED). A programming language named `harn` also has `harn-*` crates. Noise is moderate, and an AI-agent product with a near-identical name is the real collision risk.

### A.2 Candidates

All twelve are unclaimed on crates.io at the check date. GitHub columns give result count and the top repository with its stars.

| Name | GitHub results; top repo | Meaning and notes |
|---|---|---|
| **fairlead** (preferred) | 27; sirrobot01/fairlead (2) | A fitting that guides a line so it does not chafe. Lowest noise of all candidates. |
| gusset | 82; latchkey-dev/gusset (5) | Plate that joins members. Unusual spelling. |
| lashing | 62; top repos have 0 | Binds a load so nothing shifts. Generic word. |
| purlin | 69; pku-dasys/purlin (12) | Roof beam. Several small projects share it. |
| clinch | 302; BradWBeer/clinch (272) | Medium collision. |
| batten | 195; dockersecuritytools/batten (69) | Security-tool collision. |
| ashlar | 140; labsyspharm/ashlar (180) | Fits atomic design; a scientific tool shares the name. |
| bowline | 58; maccman/bowline (632) | Medium-high collision. |
| oriel | 455; wojciech-graj/oriel (57) | Common given name; a college. |
| sheave | 133; t-matsudate/sheave (41) | Confusable with "sheaf". |
| transom | 123; HKUST-KnowComp/TransOMCS (70) | Several projects. |
| statute | 1.1k; chrisryugj/korean-law-mcp (2.6k) | Heavy legal-tech noise. |

Rejected: `cornice` (an existing framework with 389 stars), `dado`, `brail`, `jamb`, `stanchion`, `pilaster`. Taken on crates.io: plumb, plinth, lintel, corbel, trellis, ratchet, keel, datum, tenon, cairn, mortise, joist, truss, rubric, bulkhead, keelson, quoin, newel, gnomon, scribe, stele, tessera, sextant, shackle, spline, bitt, cleat, thimble, lanyard, tiller, hitch, mullion, soffit, halyard, bollard, shroud, canon, ordo, tenet, charter, kerf, dowel, plumbline, shim, brace, furl, rafter, clew.

### A.3 Recommendation and rename mechanics

The target name is `fairlead` (D-30, D-47), applied before any public publish and only after a trademark check; second choices are `gusset` and `lashing`. If the check eliminates a candidate, the next one is checked the same way. The rename MUST be cheap (R-VIS-004): the directive marker (`marker` in the `[fairlead]` header, D-33) and the managerial folder name are configuration keys, so changing them is an edit to that header plus a mechanical rewrite performed by `fairlead upgrade --rename-marker X --rename-folder Y` (see [22-editions-and-upgrade.md](22-editions-and-upgrade.md), [15-cli.md](15-cli.md) R-CLI-042). Before the first publish, availability of the three crate names (`<name>`, `<name>-core`, `<name>-syntax`) is re-checked, and they are claimed by publishing the first working alpha, not empty placeholders; the policy and the full checklist are in [60-roadmap.md](60-roadmap.md) (R-REL-021, R-REL-040 to R-REL-046). Until the rename, all specification text keeps `fairlead` and `fairlead/`.
