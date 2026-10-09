# Editions and Upgrade
> Status: draft · Milestone: mixed · Decisions: D-03, D-13, D-14, D-17, D-18, D-19, D-26, D-30, D-32, D-33, D-34, D-36, D-41, D-42, D-45, D-46, D-50 · Related: 11-files-and-classes.md, 12-directive-syntax.md, 15-cli.md, 20-versions-and-seal.md, 23-git-integration.md, 32-portability-and-restructure.md, 50-architecture.md

## Purpose
A codebase pins the grammar and generated-output format it was written under for years, while the tool that reads it keeps changing. This chapter separates the crate's own semver from the **edition** (the contract with the codebase), defines the byte-stability promise, compatibility behaviour for every binary/codebase combination, the `fairlead upgrade` flow as a docs-first change, mixed editions for sealed sub-roots, and how fairlead's own repository exercises all of this.

## Concepts

| Term | Definition |
|---|---|
| **Crate semver** | Version of the binary (`0.x` while alpha). About the tool: bugs, performance, new verbs. |
| **Edition** | An integer (`edition = 1`) in the root control file. The contract for directive grammar, rule semantics and generated-file format. Years were rejected (D-45): they imply a cadence. |
| **Pins** | `edition` (contract) in the frozen `[fairlead]` header table of `fairlead/control.toml` (D-33); `fairlead-version` (binary pin; a mismatch is refused per [CLI R-CLI-005](15-cli.md), D-50) and `fairlead-range` (compatible binary range, honoured by CI; renamed from `requires` so it cannot collide with the portable-scope `requires`, [syntax](12-directive-syntax.md) R-SYN-051) as `[settings]` keys of the same file ([syntax](12-directive-syntax.md) R-SYN-038). |
| **Byte-stable** | For one edition, the generated output of any binary that supports it is identical for identical inputs. |
| **Must-understand directive** | A directive whose verb is marked with `!` (`// fairlead: !use ...`, [syntax](12-directive-syntax.md) R-SYN-009); a binary that does not know it MUST stop (like a PNG critical chunk). |
| **Upgrade plan** | A generated change doc listing the exact mechanical edits to move one edition to the next. |

## Rules

### Pinning and the stability promise
- **R-EDN-001** [0.1] `init` MUST write the frozen `[fairlead]` header table of the root control file (`edition = 1`, `root`, `scope`, `marker`; schema frozen across editions, D-33) and the `[settings]` keys `fairlead-version` and `fairlead-range`. The edition is therefore read before any other key. Version 0.1 writes and reads `edition = 1`; the upgrade machinery below ships in 0.2.
- **R-EDN-002** [0.2] Within an edition, generated output MUST be byte-stable across binary minor and patch versions. Upgrading the binary alone MUST NOT cause diff churn.
- **R-EDN-003** [0.2] Format fixes, grammar changes and changed defaults MUST ship only with a new edition or an explicit `upgrade`. A changed default severity MUST NOT arrive via a binary update.
- **R-EDN-004** [0.2] Rules added in a minor release MUST ship at `off` (or `observe`) and be listed by `context` ("3 new rules available"). Bug fixes MAY change check results; baselines absorb them (D-28).
- **R-EDN-005** [0.2] There MUST be one edition per codebase root. Only sealed sub-roots (R-EDN-040) MAY differ.
- **R-EDN-006** [0.2] Pins MUST be monotonic: lowering `edition`, `fairlead-version` or `fairlead-range` MUST require an approved change doc, that is an `[[approval]]` table in `fairlead/approvals.toml` written by `fairlead approve` (D-34); otherwise `check` MUST report a relaxation.
- **R-EDN-007** [0.2] The marker (`fairlead`) and managerial folder name MUST be control-file keys, never hard-coded. An `upgrade` plan MAY include a rename step that rewrites every directive prefix and folder name (D-30).

### Compatibility policy

| # | Situation | Behaviour | Rule |
|---|---|---|---|
| 1 | Binary newer, same edition | Fully works | R-EDN-010 |
| 2 | Binary newer by one edition | Refused by the pin check ([CLI R-CLI-005](15-cli.md)) with the install line; only `upgrade` reads the older edition, to migrate it; output format is never mixed | R-EDN-010, R-EDN-011 |
| 3 | Binary supports neither (codebase too old) | Refuse; print the exact bridging version to install | R-EDN-012 |
| 4 | Binary older than the codebase edition | Refuse all commands with the refusal code of [CLI R-CLI-003](15-cli.md) | R-EDN-013 |
| 5 | Binary older than `fairlead-version` pin, same edition | Refused by the pin check ([CLI R-CLI-005](15-cli.md)); no override flag exists in this chapter | R-EDN-014 |
| 6 | Unknown directive **class/verb** (`// fairlead: frobnicate ...`) | Error `HN-S001` (unknown verb, [syntax](12-directive-syntax.md) R-SYN-009); a typo never passes silently | R-EDN-020 |
| 7 | Unknown **key** on a known verb | Warn; preserved on rewrite | R-EDN-021 |
| 8 | `!`-marked directive the binary does not know | Hard fail | R-EDN-022 |
| 9 | Keys prefixed `x-` | Always ignored, always preserved | R-EDN-023 |
| 10 | Unknown frontmatter keys in docs | Preserved on every rewrite | R-EDN-024 |

- **R-EDN-010** [0.2] A binary MUST treat every edition it parses identically to the table above. A binary parses the current edition. Parsing the previous edition happens only inside `upgrade`. There is no read-only window for it: [CLI R-CLI-005](15-cli.md) refuses any edition mismatch, and a codebase older than the binary is also told the bridging version to install (R-EDN-012).
- **R-EDN-011** [0.2] On an older-edition codebase, writers (`sync`, `new`, `move`, `seal`) MUST refuse until `upgrade` has been applied (`upgrade --yes`, [CLI](15-cli.md)).
- **R-EDN-012** [0.2] The refusal message MUST contain the exact installation line (for example `cargo install fairlead --version X --locked`).
- **R-EDN-013** [0.2] Refusals for a codebase newer than the binary, and every exit code, are those defined in [CLI](15-cli.md) (R-CLI-003, R-CLI-005). This chapter defines no exit codes.
- **R-EDN-014** [0.2] An older binary MUST refuse a codebase pinned to a newer edition, and any binary older than the `fairlead-version` pin of the same edition, with the refusal of [CLI R-CLI-005](15-cli.md). A pin that has been lowered is not a refusal: `check` reports it as a relaxation (R-EDN-006) and warns, and the approval requirement decides.
- **R-EDN-020** [0.2] Unknown classes MUST be errors (D-17).
- **R-EDN-021** [0.2] Unknown keys MUST produce a warning (D-17) and MUST round-trip unchanged.
- **R-EDN-022** [0.2] The `!` prefix MUST be grammar-level and MUST NOT depend on the verb being known.
- **R-EDN-025** [0.2] The binary MUST embed the reference docs and edition grammar for supported editions so that an offline sandbox does not need network access; only `upgrade --fetch` (explicit, foreground, [CLI R-CLI-009](15-cli.md)) uses the network, and then only to retrieve edition migration data, never a new binary. There is no background or automatic update check (D-26).

### Upgrade (plan and apply as a doc-first change)
- **R-EDN-030** [0.2] An upgrade MUST be two steps. `fairlead upgrade` without `--yes` MUST be plan-only: it prints the plan and exits as not applied ([CLI R-CLI-002](15-cli.md)). `fairlead upgrade --yes`, when no approved upgrade doc matches the current plan hash, MUST write only a draft change doc `d-<draft>-upgrade-edition-N-to-M.md` (`kind = "upgrade"`, `impact = "break"`, `created = "<version>"`) in the root scope, edit no other file and exit as not applied (refused, awaiting approval). The doc lists: exact mechanical edits per file, counts, new rules with dry-run violation counts (auto-baselined to `observe`), and a plan hash.
- **R-EDN-031** [0.2] `fairlead upgrade --yes` MUST apply the edits only if an upgrade doc for this plan is `ready` and carries an `[[approval]]` recorded by `fairlead approve` (D-34), and the plan hash still matches the tree; otherwise it MUST report "plan stale, re-plan" or "approval missing". It MUST require that every file in the write-set equals HEAD (`--allow-dirty` overrides).
- **R-EDN-032** [0.2] Apply MUST stage all edits in `.fairlead/tx/`, apply by atomic rename in a fixed order with the control file (carrying the new `edition`) renamed last, then run `check`. An interrupted apply MUST be resumable by re-running (idempotent edits); a half-migrated tree MUST be reported as `H-UPG-PARTIAL`.
- **R-EDN-033** [0.2] The upgrade doc's only stamp is `created = "<version>"`, an identity key and not a version counter (D-13, [versions](20-versions-and-seal.md) R-VER-012); no file receives a version header. It is sealed like any change doc. Reference-class and generated files are regenerated from the new binary; authored files change only by the listed mechanical edits.
- **R-EDN-034** [0.2] An upgrade MUST NOT invent a semantic value. A field an edition newly requires that cannot be derived mechanically MUST appear in the plan as `needs-human`; `upgrade --yes` MUST refuse until it is filled. This includes any exception expiry: an edition MUST NOT fill in `until=` or `exp=` automatically and `never` is not a valid expiry ([versions](20-versions-and-seal.md) R-VER-074).
- **R-EDN-035** [0.2] Rollback is the clean-tree precondition plus git: the plan lists the write-set, the pre-apply tree is HEAD. Fairlead MUST NOT implement a separate undo log.
- **R-EDN-036** [0.2] The plan MUST be reviewable by CODEOWNERS like any managerial-folder change (D-03); an agent MUST NOT be able to apply any upgrade without the approval of R-EDN-031, and MUST NOT lower a pin without the approval of R-EDN-006.
- **R-EDN-037** [0.2] The edit set of an upgrade MUST be deterministic: the same tree and the same binary MUST produce the same plan hash.
- **R-EDN-038** [0.2] The plan lists mechanical edits only. There is no semantic preview of changed rule results in 0.2 beyond the dry-run violation counts of R-EDN-030; rollback is git (R-EDN-035).
- **R-EDN-039** [0.2] An upgrade moves exactly one edition (N to N+1). Crossing several editions MUST be run stepwise, each step its own upgrade doc, sealed like any change doc; a plan for N to N+2 MUST be refused with the list of steps.

### Mixed editions
- **R-EDN-040** [0.2] A sealed sub-root (vendored, subtree, mounted, submodule; see [portability](32-portability-and-restructure.md)) MAY declare its own edition. The binary MUST read it under that edition's parser.
- **R-EDN-041** [0.2] Inheritance across the sealed boundary MUST be off. The parent sees the child only through its `provides` digest (an edition-neutral summary); the child sees the parent only through declared `imports`.
- **R-EDN-042** [0.2] Non-sealed child scopes MUST share the root edition (a root-only sealed key).
- **R-EDN-043** [0.2] `upgrade --scope <sealed-root>` MUST upgrade that sub-root independently. A parent seal records `child@version` but writes nothing inside the sub-root.
- **R-EDN-044** [0.2] If the binary supports neither edition of a sub-root, it MUST report that sub-root as unreadable and continue with the rest, failing the parent's check under the pin-mismatch refusal of [CLI R-CLI-005](15-cli.md).
- **R-EDN-045** [0.3] For a mounted sub-root whose edition or rollup schema differs from the parent's but is supported, the check MUST be per boundary: the mismatch is reported once, at the boundary, as an upgrade diagnostic (info, with the `upgrade --scope <sealed-root>` hint), and neither side's check fails because of it. A sub-root the binary cannot read at all remains R-EDN-044.

### Dogfooding
- **R-EDN-050** [0.2] The fairlead repository MUST be a fairlead codebase pinned to the edition it ships; its CI MUST run `check` with the binary built from the same commit and with the last released binary.
- **R-EDN-051** [0.2] For every supported edition the repository MUST keep a fixture codebase and golden generated output; a test MUST fail on any byte difference (byte-stability promise).
- **R-EDN-052** [0.2] Each edition bump MUST include a fixture upgrade from the previous edition, with plan, apply, `check` and a second apply that is a no-op.
- **R-EDN-053** [0.2] The spec in `docs/` is the source for the embedded reference docs; docs and grammar changes ship together.

## Examples

Pins in the root control file:
```toml
[fairlead]                       # frozen header, parsed first (D-33)
edition = 1
root = true
scope = "root"
marker = "fairlead"

[settings]                    # root body keys, after the frozen header (12 R-SYN-038)
folder = "fairlead"
fairlead-version = "0.3.2"       # binary pin; mismatch refused (R-CLI-005)
fairlead-range = ">=0.3, <0.5"  # honoured by CI
```

Upgrade plan doc (generated):
```toml
+++
id = "d-3m8q1xzr"
kind = "upgrade"
status = "draft"
impact = "break"
from-edition = 1
to-edition = 2
plan-hash = "b3:77c0..."
scope = "root"
created = "1.4.0"
+++
```
```text
edits:    412 directive lines in 188 files (mechanical)
          reference files regenerated: 9
          generated shards re-sharded: 31
needs-human: 3 (field `owner` required on ovr in scopes: pipelines, ui, tools)
new rules (dry run, baselined to observe): R-SYN-020 (14), R-CHK-031 (0)
apply:    fairlead upgrade --yes   (requires the approved upgrade doc, a clean write-set and a matching plan hash)
```

Must-understand directive:
```fairlead
// fairlead: !use engine.verify
```
(The `!` directly precedes the verb, after the single space that follows the colon; see [directive syntax](12-directive-syntax.md) R-SYN-009. An older binary that does not know the verb stops.)

Version refusal:
```text
fairlead: this codebase is pinned to edition 2; this binary supports editions 1.
       install: cargo install fairlead --version 0.5.0 --locked   (refusal, [CLI R-CLI-005](15-cli.md))
```

## Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | `cargo install` of a newer minor changes nothing in the diff | Byte-stability within an edition | R-EDN-002 |
| 2 | New binary adds a rule; CI turns red | Cannot happen: new rules ship `off`/`observe` | R-EDN-004 |
| 3 | Pin file lowered to dodge new rules | Relaxation reported; needs approval | R-EDN-006 |
| 4 | Interrupted `upgrade --yes` | Re-run is idempotent; `H-UPG-PARTIAL` if left half-done | R-EDN-032 |
| 5 | Upgrade needs a value no one can derive (new required field) | Plan item `needs-human`; apply refuses | R-EDN-034 |
| 6 | Upgrade plan stale after more commits | "plan stale, re-plan"; apply refuses | R-EDN-031 |
| 7 | Vendored subtree on edition 1 inside an edition 2 repo | Read under its own edition; no inheritance across | R-EDN-040, R-EDN-041 |
| 8 | Bisect lands on a commit pinned to an unsupported edition | Refusal with the bridging version ([CLI R-CLI-005](15-cli.md)) | R-EDN-012, R-EDN-013 |
| 9 | Old and new binary on the same repo on different machines | Older binary refuses the verb ([CLI R-CLI-005](15-cli.md)); no read-only window | R-EDN-014 |
| 10 | Typo `// fairlead: dcl ...` | Error, never silent | R-EDN-020 |
| 11 | Directive with `x-` key from a team plug-in | Ignored, preserved | R-EDN-023 |
| 12 | Product rename | Marker and folder are config keys; upgrade plan rewrites them | R-EDN-007 |
| 13 | Edition downgrade attempted by an agent | Needs an approved change doc | R-EDN-006 |
| 14 | Sealed sub-root edition unsupported | Sub-root reported unreadable; parent check fails under the [CLI R-CLI-005](15-cli.md) refusal | R-EDN-044 |
| 15 | Cloud sandbox with no network | Embedded grammar/reference docs; no fetch needed | R-EDN-025 |
| 16 | Agent runs `upgrade --yes` straight away | Only a draft doc is written; apply needs the approved doc | R-EDN-030, R-EDN-031 |
| 17 | Codebase two editions behind | Plan refused; stepwise upgrades | R-EDN-039 |
| 18 | Upgrade would need a waiver expiry | `needs-human`; no default, `never` invalid | R-EDN-034 |
| 19 | Mounted sub-root on a different supported edition | Single upgrade diagnostic at the boundary | R-EDN-045 |
| 20 | Want to know if a newer edition exists | `upgrade --fetch`, explicit only | R-EDN-025 |

## Resolved questions
- Q22-1 (resolved 2026-10-09): pins are the frozen `[fairlead]` header of `fairlead/control.toml` (D-33); no separate pin file; see R-EDN-001
- Q22-2 (resolved 2026-10-09): the previous edition is parsed only inside `upgrade`; no read-only window; see R-EDN-010, R-EDN-014
- Q22-3 (resolved 2026-10-09): upgrades are stepwise, one edition per sealed doc; see R-EDN-039
- Q22-4 (resolved 2026-10-09): network use only through explicit `upgrade --fetch`; see R-EDN-025
- Q22-5 (resolved 2026-10-09): two steps: `upgrade --yes` writes the draft doc, a later `upgrade --yes` applies it once approved; see R-EDN-030, R-EDN-031, R-EDN-036
- Q-VER-5 (resolved 2026-10-09): an older binary refuses a newer edition or pin; a lowered pin is reported as a relaxation; see R-EDN-006, R-EDN-014
- Q-VER-6 (resolved 2026-10-09): mechanical plan with git as rollback; semantic preview not in 0.2; see R-EDN-038
- Q-VER-7 (resolved 2026-10-09): expiry is never filled in by an upgrade; a human sets it; see R-EDN-034
- Q-VER-12 (resolved 2026-10-09): per-boundary check for a mounted sub-root, a mismatch is an upgrade diagnostic; see R-EDN-045
