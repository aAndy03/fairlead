# Question register (all resolved)

> Status: draft · Milestone: mixed · Decisions: D-01 to D-50 (see Settled) · Related: decisions.md, conventions.md, 00-vision.md, 01-glossary.md, 10-scopes-and-inheritance.md, 11-files-and-classes.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 14-context.md, 15-cli.md, 20-versions-and-seal.md, 21-environments-maturity-lifecycle.md, 22-editions-and-upgrade.md, 23-git-integration.md, 30-hierarchy-and-engines.md, 31-variation-protocol.md, 32-portability-and-restructure.md, 33-retention-and-compaction.md, 40-agent-integration-and-trust.md, 50-architecture.md, 60-roadmap.md, examples/ts-react.md, examples/rust-workspace.md, examples/fullstack-five-years.md

## Purpose
This register records every question raised during specification. All of them were resolved on 2026-10-09 under [D-46](decisions.md), each by its stated leaning or by the decision that superseded it. Each row gives the resolution and the rule IDs that record it. New open questions are added here as open, with options and a leaning, and need owner acceptance before they are resolved.

**ID note.** This register uses `Q-AREA-n`. Chapter-local questions are named `Q<chapter>-<n>` (for example Q12-1). Each chapter lists its own under "Resolved questions" at its end; see the index below. The worked examples keep their questions as `Q-EXTS-n` (TypeScript) and `Q-EXRS-n` (Rust).

**Legend.** S1 to S4 and H1 to H8 are the source analyses. `G n` is the nth item under "Gaps & unhandled cases", `K n` the nth item under "KEY decisions", and `D n` an agent decision in that source. **Milestone** is where the resolved behaviour lands; `publish` means only public release. **Recorded in** gives rule IDs. The prefix names the chapter: SCOPE 10, FILE 11, SYN 12, CHK 13, CTX 14, CLI 15, VER 20, LC and ENV 21, EDN 22, GIT 23, HIER 30, VAR 31, PORT 32, RET 33, AGT 40, ARCH 50, ROAD and REL 60, VIS 00, GLOS 01. `§` is a section.

## Status
All 130 register questions are resolved; none is open.

| Area | Questions | Resolved | Open |
|---|---|---|---|
| SCOPE (scopes) | 10 | 10 | 0 |
| FILE (files) | 10 | 10 | 0 |
| SYN (syntax and CLI) | 14 | 14 | 0 |
| CHK (checks) | 8 | 8 | 0 |
| VER (versions, seal, editions) | 12 | 12 | 0 |
| LIFE (environments, lifecycle) | 10 | 10 | 0 |
| HIER (hierarchy) | 10 | 10 | 0 |
| VAR (variation) | 5 | 5 | 0 |
| CTX (context, retention) | 9 | 9 | 0 |
| GIT (git) | 6 | 6 | 0 |
| PORT (portability) | 11 | 11 | 0 |
| AGT (agents, trust) | 6 | 6 | 0 |
| TOOL (tooling) | 10 | 10 | 0 |
| PROD (product) | 9 | 9 | 0 |
| **Total** | **130** | **130** | **0** |

**Conflict still to settle.** Q-GIT-4 is answered two ways. [23-git-integration](23-git-integration.md) (R-GIT-052, R-GIT-073) puts one allocation lock in the common git directory. [32-portability-and-restructure](32-portability-and-restructure.md) (Q32-4, R-PORT-015) has per-worktree locks and no cross-worktree lock. Both are counted as resolved above. The owner must pick one, and the other chapter must follow.

Chapter-local resolved questions, one line per chapter file:

| Chapter file | Chapter-local resolved questions |
|---|---|
| 00-vision | 2 |
| 01-glossary | 5 |
| 10-scopes-and-inheritance | 7 |
| 11-files-and-classes | 5 |
| 12-directive-syntax | 10 |
| 13-checks-and-diagnostics | 10 |
| 14-context | 5 |
| 15-cli | 10 |
| 20-versions-and-seal | 7 |
| 21-environments-maturity-lifecycle | 5 |
| 22-editions-and-upgrade | 5 |
| 23-git-integration | 5 |
| 30-hierarchy-and-engines | 6 |
| 31-variation-protocol | 5 |
| 32-portability-and-restructure | 6 |
| 33-retention-and-compaction | 7 |
| 40-agent-integration-and-trust | 6 |
| 50-architecture | 9 |
| 60-roadmap | 7 |
| examples/ts-react | 4 |
| examples/rust-workspace | 7 |
| examples/fullstack-five-years | 0 |
| **Total** | **133** |

## Scopes (SCOPE)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-SCOPE-1 | Are child control files allowed in 0.1? | (a) root control only; (b) child controls limited to delegated keys; (c) full child controls | (a) for inherited keys: child files add facts only (`[fairlead]` header, local tables); `[delegate]` is 0.2. Superseded in part by D-11, D-33, D-34: child files also need the identity header and `[[allow]]`/`[[req]]` tables in 0.1 | R-SCOPE-051, R-SCOPE-052, R-SCOPE-056 | 0.1 | S1 §4; H1 K5; H2 D8 |
| Q-SCOPE-2 | Is membership declared, or read from manifests? | (a) folder declares scope; (b) read Cargo and pnpm workspace metadata as an oracle; (c) both, declaration wins | (c), as logged: the folder declares scope; manifests are an oracle (Cargo TOML only, D-50), never edited; the declaration wins | R-SCOPE-016 | 0.1 | H2 D9; H1 G16; S1 §6 |
| Q-SCOPE-3 | Which scope is home of a shared declaration file (`bridge.toml`, shared token file)? | (a) file in root folder, `home` field names owner; (b) declaration in home scope folder; (c) home inferred from impl site | (b): the declaration lives in the home scope's folder; `home=` must name an ancestor-or-self of every implementation site, else HOME-MISMATCH (HN-C013) | R-SCOPE-017 | 0.1 | H3 G6; D-21 |
| Q-SCOPE-4 | Are `extends` (inheritance) and `requires` (dependency) separate fields? | (a) two fields; (b) one field with a mode | (a): `extends` and `requires` are two separate fields | R-SCOPE-066 | 0.3 | H1 G17; S1 §8.1 |
| Q-SCOPE-5 | Which policy keys may a child change, and how is that written? | (a) root `delegate` table (S1 §4.1); (b) vocabulary compiled in, policies delegated | Superseded in part by D-02: vocabulary stays root-only and sealed in the control file, not compiled in. Policies are delegated through a root `[delegate]` table (0.2) | R-SCOPE-057 (cites R-SCOPE-037, R-SCOPE-051) | 0.1 / 0.2 | S1 §4.1; H1 G19; D-02 |
| Q-SCOPE-6 | Two `ovr` lines for one key in one scope, or in sibling scopes. | (a) error DUP-OVR; (b) order by position | (a): two overlapping `ovr` for one target and field in one scope are DUP-OVR (HN-X014), both inert; sibling scopes are independent, not an error | R-SCOPE-064 | 0.1 | H6 G4 |
| Q-SCOPE-7 | Sideways refs (sibling scopes): qualifier only, or qualifier plus `provides`? | (a) qualifier plus `provides` export; (b) qualifier only | (a), split: the qualifier works in 0.1; `provides` is enforced from 0.2 (NOT-PROVIDED, HN-R008) | R-SCOPE-024 (changed), R-SCOPE-029 | 0.1 / 0.2 | S1 §0 (H-c), §10 #2; H6 G9; S4 §8 #8 |
| Q-SCOPE-8 | Scope eligibility and depth in 0.1; is `new scope` a verb or `new <kind>`? | (a) every folder eligible; (b) declared scopes only, depth at most 3; (c) root plus globs | (b): declared scopes only; `fairlead new scope` is the verb; depth over 3 warns and does not refuse. Superseded in part by D-06 and D-38 (nested scopes specified, root-first build order) and by D-11 (any folder eligible) | R-SCOPE-010 (changed); 15 `new scope` row | 0.1 | H8 K1; S1 §6; S4 §6; H3 G11 |
| Q-SCOPE-9 | Non-member code, a member inside a member, and an ambiguous layout (`edge.rs` beside `edge/`). | (a) explicit owned-by rule from manifest; (b) orphan; ambiguous layout is an error | (a): an explicit `own` convention for non-owned code (0.2); an ambiguous layout (`edge.rs` beside `edge/`) is SCOPE-AMBIG (HN-C014), an error | R-SCOPE-019 (added), R-SCOPE-003 (changed) | 0.2 | H2 G2, G11, G12 |
| Q-SCOPE-10 | Framework conventions that break path-derived membership (`app/(app)` route groups, `*.gen.ts`, `tests/` crate roots). | (a) explicit convention list in control; (b) manifest hints only | (a): an explicit `[[convention]]` list in the control file (`transparent`, `class`) | R-SCOPE-018 | 0.1 | H1 G14; H2 G4 |

## Files (FILE)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-FILE-1 | May `init` write outside `fairlead/` (`.gitignore`, `.git/config`, agent files)? | (a) never; print the line; (b) `--yes` appends a marked line; (c) always write | (a), with (b) only behind `--yes`: `init` writes outside `fairlead/` only with `--yes`, and only the R-CLI-015 lines; never `.git/config` or manifests | R-FILE-026 (added), R-FILE-023 (changed) | 0.1 | H3 G2; S4 §5; H7 G6 |
| Q-FILE-2 | Ownership and merge rules for the marker block in `AGENTS.md` and `CLAUDE.md`, including nested-scope blocks. | (a) fairlead-owned pointer block with drift check; (b) full copy; (c) symlink | (a): a fairlead-owned pointer block, identical in every target and scope; drift is HN-A001 | R-FILE-027 | 0.1 | H1 G24; S4 §5 |
| Q-FILE-3 | Control keyset and minimal TOML schema (`kind`, `scope`, `policy`, `retain`, `flagword`, `layer`), and the unknown-key policy. The `[fairlead]` header keys are frozen by D-33; the rest is open. | (a) fixed schema per milestone; (b) open schema, unknown keys warn (D-17) | (b): open schema; unknown keys warn (D-17); minimal 0.1 schema in 12; keys from later milestones warn as unknown in older builds | R-SYN-039 (changed; schema table in 12) | 0.1 | H1 G1; H3 G2; S4 §1; S3 §1; D-05 |
| Q-FILE-4 | Vendor-owned lock: is a `*.gen.ts`-style output a generated file that fairlead writes, or an external generated file? The lock's format and location are settled (D-32; `fairlead/vendored.lock`). | (a) generated, fairlead writes it; (b) external-generated, its `// fairlead:` text ignored (11 R-FILE-009) | Class follows the writer, so both apply: fairlead-written files are generated; files written by another tool are external-generated | R-FILE-014 (added; cites R-FILE-009) | 0.2 | H1 G2; H3 G18; 11 R-FILE-005, R-FILE-009 |
| Q-FILE-5 | Foreign regions and vendor CLI rewrites (`shadcn init`, `--overwrite`): survival and diagnostics. The syntax is settled (D-35). | (a) foreign regions indexed, with OVR-MISSING and FOREIGN-LOST; (b) forbid vendor CLI edits | (a), split: foreign regions are indexed; OVR-MISSING in 0.1, FOREIGN-LOST reconstruction in 0.2; vendor CLI edits are detected, not forbidden | R-FILE-043 (changed), R-FILE-045 (added) | 0.1 / 0.2 | H1 G3, G4; S1 §5.1, §10 #31 |
| Q-FILE-6 | Extractor precision for CSS-in-JS, Tailwind v4 (`@theme`, `inline`, `@source`, `utility`, palette namespace) and JS theme objects. | (a) custom kinds per extractor; (b) declared safelist; (c) defer | (a): custom file kinds per extractor with documented precision; Tailwind v4 caveat checks move to 0.2 | R-FILE-035 (retagged 0.2), R-FILE-056 (added) | 0.2 | S1 G5; H1 G5, G6, G25 |
| Q-FILE-7 | Literal-leak scan policy per file type (SVG, fixtures, brand colours), and opt-in with a minimum length. | (a) per-kind globs; (b) opt-in per declaration with a minimum length | (b): opt-in per declaration through `applies=`, with `min-len` (default 4) | R-FILE-039 (added); R-CHK-017 | 0.1 | H8 G5 |
| Q-FILE-8 | Build-script outputs (`build.rs`, `OUT_DIR`) and the inputs of generated hashes. | (a) build outputs are generated with a declared generator; hashes exclude build environment | (a): build outputs are external-generated with a declared generator; hashes exclude the build environment and OUT_DIR | R-FILE-054 (changed), R-FILE-055 (added) | 0.2 | H2 G5, G6; D-27 |
| Q-FILE-9 | Legacy non-UTF-8 text (cp1250, ISO-8859-2) inside scopes. | (a) skip with a warning; (b) declared encoding per kind | (a): skipped with a warning (ENCODING-SKIP, HN-I003) | R-FILE-015 (added), R-SYN-030 (changed) | 0.1 | H4 G3 |
| Q-FILE-11 | Semantic merge conflicts inside an authored control file. | (a) left to git and `check`; (b) keyed merge | (a): no merge driver and no keyed merge; left to git and `check` | R-FILE-028 (added) | 0.2 | S2 G4 |

## Syntax and CLI (SYN)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-SYN-1 | Write confirmation: `--dry-run` with `--yes` (S4, H3) or plan by default with `--apply` (S2 `upgrade`)? | (a) `--dry-run` and `--yes` on every writer; (b) plan by default, `--apply` to write | (a), split: `--dry-run` on every writer; `init`, `upgrade`, `seal` and `move --adopt` are plan-first (exit 4 without `--yes`); `sync` and `new` apply directly; no `--apply` | R-CLI-043 (added), R-CLI-002, R-CLI-003 | 0.1 | S4 §6; S2 §6.3; H3 Y0 |
| Q-SYN-3 | Flag grammar has no `target=` (D-16 requires since and target); S2 adds `by-date=`. | (a) authored `target=<version>` (`MAJOR.MINOR.PATCH`), `since` recorded by seal; (b) target only in control | (a): authored `target=<version>` on `flag`, `since` recorded by seal, `by-date` opt-in. Superseded in part by D-49: `until=` is not accepted on `flag` | R-SYN-048 (added); flag rows in 21, 30, 31 corrected | 0.2 | D-16; S4 §1; S2 §5.1 |
| Q-SYN-4 | Alias left by a rename (`alias old=new`) is not one of the six verbs. Lifetime undefined. | (a) alias recorded in the tombstone; (b) alias line in source | (a): recorded in the tombstone with no source line; it lives as long as the tombstone (forever) | R-SYN-042 (added) | 0.2 | S4 §8 #6; S1 §7; H3 G12 |
| Q-SYN-5 | Version pin on `use` (checked by the D-29 version check). | (a) `pin=3` key; (b) `@3` suffix | (a): a `pin=<N>` key on `use`, compared with the bridge `ver` | R-SYN-049 (added), R-SYN-015 (changed) | 0.3 | H3 bridge; D-29 |
| Q-SYN-6 | Events (`evt.*`), `Channel` arguments, error shapes, and the wire-spelling rule (snake_case fn, camelCase keys). | (a) new `proto=` kinds, spelling declared per bridge; (b) out of scope | (a): events and shapes are user kinds with a `proto` word; spelling is declared per bridge with `spell=` | R-SYN-043 (added); R-HIER-060 | 0.3 | H3 G29; S3 §2.4 |
| Q-SYN-8 | `move` rewrite rules. The milestone is settled (0.2, D-38); the rewrite rules are not: `covers`, `generate_handler!`, `mod` and `use` paths, TS imports, CSS `@import`, path aliases. | (a) rewrite declarations and membership only, report code sites; (b) rewrite code | (a): `move` rewrites fairlead text only (directives, `covers`, paths, membership, ID refs); code sites are reported, not rewritten | R-CLI-044 (added), R-CLI-040 | 0.2 | D-25, D-38; S4 §6; H1 G15; H2 G20; H3 G12 |
| Q-SYN-9 | Escalation and request syntax. S1 proposes an eighth class (`ask`), S4 a control-only `req`, and D-18 allows six verbs. | (a) a `[[req]]` table in the child control file, written by `fairlead new escalation`, no source line; (b) a `def` of a request kind | (a): a `[[req]]` table in the child control file, written by `fairlead new escalation`; no eighth verb. Milestone 0.1 per R-SYN-041 [0.1] | R-SYN-041 (changed), R-CLI-030 | 0.1 | S1 §4.2; S4 §1, §8 #18; D-02; H1 G18; H3 G28 |
| Q-SYN-10 | Per-host placement rules (JSX children, Markdown, TOML, CSS) and whitespace tolerance; bare `//` in JSX text. | (a) per-host table in reference docs; (b) comment-only forms | (a): a per-host placement table (in `ref/syntax.md`) and whitespace tolerance | R-SYN-044 (added), R-SYN-006 (changed) | 0.1 | H1 G12; S4 §1 |
| Q-SYN-11 | Attachment for multi-line Rust items and expressions (S4 §1 rules 1 to 5). | (a) S4 rules as written, confirmed by fixtures; (b) line-only | (a): S4 attachment rules kept, plus attribute skipping and signature-plus-body extents, confirmed by golden fixtures | R-SYN-045, R-SYN-046 (added) | 0.1 | H2 G18; S4 §1, §8 #11 |
| Q-SYN-12 | Placement of `!` for must-understand directives, and the `x-` key prefix. | (a) `!` before the verb; (b) `!` on the key | (a): `!` goes before the verb; `x-` keys are user extension keys | R-SYN-009 (changed) | 0.2 | D-17; S2 §6.2 |
| Q-SYN-13 | Is `approve` a verb or folded into `new`? | (a) keep `approve`; (b) fold into `new` | (a): `approve` stays a verb; it is not folded into `new` | R-CLI-045 (added) | 0.1 | D-25 (open there); D-06; D-34 |
| Q-SYN-14 | `sync` write-set (generated shards, cache, tombstones for user deletions). Part is settled by D-38: `check` writes nothing, and `sync --baseline` writes the baseline. | (a) S4 §6 as written; (b) `sync` writes generated files only | (a): `sync` writes the scope index shards, the baseline (`--baseline`) and the cache; from 0.2 also tombstones for user deletions; `check` writes nothing | R-CLI-046 (added), R-CLI-025, R-CLI-027 | 0.1 | H3 G2; S4 §4, §6; D-38 |
| Q-SYN-15 | Kind prefix grammar (S4: 2 to 4 lowercase letters) against readable kind labels (H3 used `tauri-command`). | (a) prefix is the kind, label is text in control; (b) relax the grammar | (a): the prefix is the kind (2 to 8 letters); the label is `title`/`proto` text in control | R-SYN-024 (changed) | 0.1 | S4 §1; H3 bridge |
| Q-SYN-17 | Append marker for list overrides. D-12 requires an explicit marker; S1 has children append by default. | (a) `ovr ... op=append`; (b) a `+=` line | (b): the append marker is `k+=a,b`; no `op=append` | R-SYN-013 (changed) | 0.2 | D-12; S1 §2.1; H6 G1 |

## Checks and diagnostics (CHK)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-CHK-1 | Diagnostic code namespace (S4 `HN-` codes against short `E-` and `W-` names) and default severities. | (a) HN codes with stable IDs; (b) short names | (a): `HN-` codes with stable IDs; numbering frozen; the prefix is fixed, independent of `marker` | R-CHK-042 (added) | 0.1 | S4 §1, §8; H3 (codes "mine") |
| Q-CHK-3 | Severity of unverifiable refs (dynamic `invoke(name)`, `W-UNVERIFIABLE`). | (a) warn; (b) error when strict | (a): `warn` (HN-H024), with the scope ramp able to raise it to error (0.3) | R-CHK-043 (added) | 0.3 | H3 G24; S3 §3 |
| Q-CHK-4 | Move-only and rename-only changes: exempt from coverage, or covered by the restructure doc that `move` writes? | (a) generated restructure doc covers them; (b) exempt globs | (a): covered by the restructure doc that `move` writes | R-CHK-044 (added) | 0.2 | S2 §3, §8 #39; H3 G12; D-15 |
| Q-CHK-5 | Covers granularity (file, item, declaration), the unit for orphan checks, and shared hotspot files claimed by many docs. | (a) files only in 0.2, IDs in 0.3; (b) files and IDs together | (a): the coverage unit is the file in 0.2; IDs in `covers` count only from 0.3; a shared hotspot file is covered by any one doc | R-CHK-045 (added) | 0.2 | H3 G3; S2 §7.1; S3 §6 |
| Q-CHK-6 | Zero-declaration output. | (a) explicit "0 rules declared, nothing verified", never a silent green; (b) a non-zero exit | (a): `HN-N001` "nothing verified" with exit 0; `--require-declarations` makes it exit 1 | R-CHK-003 (unchanged) | 0.1 | H8 G1 |
| Q-CHK-7 | Derived-token graph and an E-DERIVED check (`--sidebar: var(--background)`). | (a) derive graph once the token contract ships; (b) skip derived tokens | (a): the derived-token graph is built once the token contract ships; HN-T005 uses it from 0.2 | R-CHK-046 (added) | 0.2 | H1 G7 |
| Q-CHK-8 | Cascade layers and import order as override semantics (`@layer`, unlayered child CSS, reorder by `shadcn init`). | (a) order check plus layer model; (b) forbid bare cascade overrides | (a): order check (HN-T008) and forbidden bare cascade overrides (HN-T009), both 0.2 | R-CHK-047 (added) | 0.2 | H1 G9, G10; S1 §5.1 |
| Q-CHK-9 | Class-name-to-token references and computed names (`bg-${tone}`). | (a) declared safelist; (b) approximate and warn | (a): computed class names are analysable only through a declared token safelist (`classes=`), 0.2 | R-CHK-048 (added) | 0.2 | H1 G11; S1 §5 |

## Versions, seal and editions (VER)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-VER-1 | Created stamp. D-13 gives new docs a once-only, immutable stamp. 11 R-FILE-010 writes it at creation as `created`; 20 R-VER-012 writes a different once-only stamp (`sealed`, `body-hash`) at seal. No per-file version counter or header may be written into a doc (D-13); the `created = "<version>"` identity key is allowed (D-42). | (a) `created` only; (b) `created` at creation plus a once-only `body-hash` at seal, no version counter; (c) a mutable per-file version counter, which contradicts D-13 | (b): `created` at creation (0.1); once-only `sealed` and `body-hash` at seal (0.2); no version counter (D-13) | R-VER-012 (edited), R-VER-013 | 0.1 / 0.2 | D-13; 11 R-FILE-010; 20 R-VER-012, R-VER-013; S2 §1.2 E, K1; H4 K5; H1 K4 |
| Q-VER-3 | Partial seal (`seal --scope A`) against all-or-nothing seals (D-14), and the refusal while a ready ancestor doc covers A. | (a) no partial seal in 0.2; (b) partial seal under rules | (a): no partial seal in 0.2; `seal --scope` is withdrawn | R-VER-056 (withdrawn, D-14) | 0.2 | S2 §2.3; D-14 |
| Q-VER-4 | ID claims and leases for new declarations on parallel branches. Duplicates appear only on the merged tree. | (a) no claims; CI check on the merged tree; (b) claim file per ID; (c) content-derived IDs for declarations | (a): no claims or leases; duplicate IDs are found by CI on the merged tree (HN-D001) | R-VER-044 (added) | 0.2 | H3 G22; H8 G4; S2 §7.2 |
| Q-VER-5 | Mixed editions: refuse or warn when an old binary meets a newer config or an older pin; monotonicity. | (a) S2 §6.2 table (refuse an older binary, warn for a pin); (b) warn only | (a): an older binary refuses a newer edition or pin; a lowered pin is a reported relaxation. The "warn for a pin" part is superseded in part by D-50(4): a pin is refused | R-EDN-014 (added), R-EDN-006, R-EDN-013 | 0.2 | H3 G21; H8 G6; S2 §6.2; D-17 |
| Q-VER-6 | Edition upgrade: semantic preview and rollback. The plan lists mechanical edits only. | (a) mechanical plan, with git revert as rollback; (b) semantic diff; (c) no upgrade in 0.2 | (a): mechanical plan with git as rollback; no semantic diff in 0.2 | R-EDN-038 (added), R-EDN-035 | 0.2 | H3 G20; S2 §6.3 |
| Q-VER-7 | Waiver expiry at an edition change: expiry cannot be filled in automatically, and `ttl=never` must not stand in for it. Under D-37 an expiry is `exp=<date>` or `until=<version>`. | (a) refuse; a human sets the expiry; (b) default expiry | (a): upgrade never fills an expiry; `never` is not a valid expiry; a human sets it | R-VER-074 (added), R-EDN-034 (edited) | 0.2 | H3 G19; D-02; D-37; S4 §4 |
| Q-VER-8 | Do unresolved escalations block seal? | (a) block; (b) warn at seal, error after K versions | (b): warn at seal; error after K seals (`version.escalation-grace`); a `blocking` escalation blocks at once | R-VER-058 (added) | 0.2 | H5 G10; S1 §4.2 |
| Q-VER-9 | Abandoned drafts left orphaned after squash; sweeping at release. | (a) sweep report at seal; (b) auto-abandon after K versions | (a): seal writes a sweep report of orphaned or carried drafts; no auto-abandon | R-VER-045 (added) | 0.2 | H8 G3; S2 §8 #40 |
| Q-VER-10 | Maintenance and hotfix lines: seal only on the integration line, or per line. Backports. | (a) seal on integration line only; hotfix lines reseal on merge-back; (b) per-line version counters | (a): seal on the integration line only; maintenance seals are unintegrated and reseal on merge-back; no per-line counters | R-GIT-035 (rewritten), R-VER-055 (edited) | 0.2 | S2 §8 #11, #42; D-14 |
| Q-VER-11 | Waiver lifetime cap. H8 proposes 90 days. | (a) version-based cap; (b) date cap with `--as-of` | (a) from 0.2 (`exceptions.max-minors`); the 0.1 cap is a date (`max-days` 90) | R-VER-075 (added) | 0.2 | H8 K4; D-37 |
| Q-VER-12 | Mounted sub-root: edition or rollup-schema mismatch with the parent. | (a) per-boundary check, mismatch is an upgrade diagnostic; (b) fail | (a): a per-boundary check; a supported but different sub-root edition is an upgrade diagnostic (HN-V014) | R-EDN-045 (added; 0.3) | 0.3 | S1 §8.2, §10 #21; S1 G3 |
| Q-VER-13 | Alias renumbering at seal, and ledger snapshots: only the touch-points are specified. | (a) define the snapshot and alias write set inside seal; (b) defer | (a): the alias map, checkpoint snapshot and ordinal write set are defined inside `seal` | R-VER-046 (added), R-VER-059 (added), R-VER-050 step 4 (edited) | 0.2 | S1 G6; S1 §7 |

## Environments and lifecycle (LIFE)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-LIFE-1 | Revive path for a tombstoned ID. | (a) none: new ID plus graveyard; (b) owner-approved revive doc recorded by `fairlead approve`; (c) `ovr` on the tombstone with an expiry | (b): revive only through an approved `revive` doc, which takes a new version-suffixed ID; the old ID stays reserved | R-LC-024 (added) | 0.2 | D-16, D-03; S2 §5.2; H3 G9, K2 |
| Q-LIFE-2 | Rename, merge and split of IDs: verb, alias and reference handling. | (a) `move --id` with alias (S4); (b) manual `replaced-by` plus W-REF-RETIRED until cleared; (c) a restructure change kind | (b), with `move --id` withdrawn (superseded in part by D-50(1)): rename, merge and split use `flag replace` with a permanent `replaced-by` alias in the tombstone; references through the alias are reported (HN-R006) until updated | R-LC-011 (added), R-LC-020, R-LC-022; 15 R-CLI-040 | 0.2 | H3 G11, K4; S4 §8 #6; S1 §7 |
| Q-LIFE-3 | Graveyard fingerprint: what it hashes, and whether tombstones also match by path. | (a) content fingerprint; (b) S2 match terms; (c) both, plus ID and paths | (c): a content fingerprint computed once at seal from removed files and stored, plus match terms, the tombstone ID and `paths` | R-LC-020 (edited), R-LC-031 (edited), R-LC-033 (edited) | 0.2 | D-16; S2 §5.3; H3 K2, G8 |
| Q-LIFE-4 | Severity of references to deprecated, replaced or removed IDs before the due version. | (a) warn until due, error at due (S2 §4.2); (b) error from the day of deprecation (S2 §5.1) | (a): warn until the due version, error from it; removed IDs always error (R-LC-008 rewritten; S2 §5.1 superseded) | R-LC-008 (rewritten), R-ENV-024 | 0.2 | S2 §4.2 vs §5.1; S4 §8 #26; H3 Y2 |
| Q-LIFE-5 | Flag slot taxonomy (planned, edit, dep, rm) and the env and stage ordering. | (a) S2 state-versus-plan split; (b) S4 flat user words | (a): the state-versus-plan split is kept; environment and maturity are separate axes | R-LC-001 (edited), R-ENV-004 | 0.2 | S4 G1; S2 §5.1; D-16 |
| Q-LIFE-6 | Native deprecation attributes (`#[deprecated]`, `remove_in`) against fairlead flags. | (a) fairlead flags authoritative, native attributes cross-checked; (b) native attributes win | (a): fairlead flags are authoritative; native attributes are cross-checked | R-LC-010 (edited) | 0.2 | H2 G16 |
| Q-LIFE-7 | Env axis for test-only code (`#[cfg(test)]`) and the owner of build-time env selection. | (a) default env is test when detectable, otherwise warn; CI build matrix selects env; (b) exclusive features | (a): test code defaults to env `test` when the root declares it and it is detectable, else warn; fairlead selects no build env | R-ENV-036 (added) | 0.2 | H2 G3, G15; S2 §4.1 |
| Q-LIFE-8 | Theme variants (light and dark) are not environments. Which axis holds them? | (a) separate variant axis; (b) use env | (a): theme variants (light, dark, brand) are a separate variation axis, not environments | R-ENV-037 (added), Concepts note | 0.2 | H1 G22 |
| Q-LIFE-9 | Feature unification: a dev probe enabled in one crate puts dev code in a prod build. Reachability needs resolved features. | (a) accepted limit under D-16, report declared edges; (b) resolve features through cargo metadata | (a): an accepted limit; fairlead reads declared edges only and never resolves features | R-ENV-030 (edited) | 0.2 / 0.3 | H2 G13 |
| Q-LIFE-10 | Migration tool for long-lived branches still on old ID schemes. | (a) none in 0.2; (b) batch `move --id` | (a): no migration tool; `move --id` is not part of 0.1 to 0.3 (D-50) | R-LC-012 (added) | 0.3 | H5 G12 |

## Hierarchy (HIER)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-HIER-1 | Import extraction in 0.1: regex over TS `import` and Rust `use crate::`, reported as a lower bound, or defer to tree-sitter or syn. | (a) regex lower bound; (b) defer | (a), as logged: a regex import pass (TS `import`, Rust `use crate::`) is optional lower-bound evidence from 0.3; nothing is extracted in 0.1 or 0.2 (partly superseded by D-27 and D-40) | R-HIER-033 (added), R-HIER-032 | 0.3 | H3 K5; H1 G13; D-27 |
| Q-HIER-2 | Rank rule scope: `use` edges only, or Cargo edges too. Same-rank policy (`allow`, `container`, `forbid`). | (a) `use` edges only; (b) also Cargo edges | (b): Cargo workspace edges are also read as container edges; the same-rank default is `allow` | R-HIER-027 (added) | 0.3 | S3 §1, §3.1; H2 D4 |
| Q-HIER-3 | Sideways dependency policy (`provides`) and layer-rank rules: one rule or two. | (a) two: `provides` gates visibility, ranks gate direction; (b) unified | (a): two rules; `provides` and `vis` gate visibility, ranks gate direction | R-HIER-028 (added) | 0.3 | S1 G4; S3 §7 |
| Q-HIER-4 | Data source for "forgotten": seal-ledger digests or git history. | (a) seal-ledger digests; (b) git history | (a): dormancy reads seal-ledger digests, never git | R-HIER-045 (added), R-HIER-043 | 0.3 | H3 G25; S3 §3.1 |
| Q-HIER-5 | Entry declarations and whether draft docs suppress W-STALE. | (a) entries declared with kinds, drafts suppress stale; (b) no suppression | (a): entries are declared with kinds; unsealed change docs suppress `hier.stale` | R-HIER-046 (added), R-HIER-040 | 0.3 | H3 G25; S3 §8 #12, #36; D-20 |
| Q-HIER-6 | Upward runtime flows: port unit plus `implements` only, or an `allow-up` exception with expiry. | (a) port and `implements`; `allow-up` only with an expiry | (a), tightened: a port unit plus `implements`; otherwise `ovr rule.hier-rank` with an expiry; no `allow-up` | R-HIER-024 (edited), R-HIER-023 | 0.3 | S3 §8 #10; D-20 |
| Q-HIER-7 | Containment declaration: how a unit names its single container. D-40 declares units and containers with `def`; the open part is the form that names the container. | (a) TOML declaration; (b) directive key `in=` | Superseded by D-40: containment is named by `container=` or `parent=` on `def`; no TOML table and no `in=` | R-HIER-013 (added) | 0.3 | S3 §2.1; D-20; D-40 |
| Q-HIER-8 | Container path claims against marker binding, and the overlap rule. | (a) path globs for containers; markers bind within a file; both on one unit is an error | (a): containers bind by `path=`; units bind by `path=` or a code `def`, never both (`hier.double-binding`, HN-H023) | R-HIER-014 (added), R-HIER-010, edge case 8 | 0.3 | S3 §2.1, §8 #5, #7 |
| Q-HIER-9 | Marking generated bindings (ts-rs, specta) so rank and fork scans skip them. | (a) `generated-from` marker in the file; (b) declared file list | Superseded by D-35: `gen=` and the foreign and vendor-owned classes apply; there is no `generated-from` file marker | R-HIER-066 (edited) | 0.3 | S3 §8 #20; H3 G18 |
| Q-HIER-10 | Guardrails for `sym=` evidence (length, uniqueness, comment stripping). | (a) S3 §3.2 rules; (b) drop `sym` | (a): `sym=` guardrails kept (length, uniqueness, comment stripping), plus a whole-identifier match | R-HIER-030, R-HIER-031 (edited) | 0.3 | S3 §3.2; D-20 |

## Variation (VAR)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-VAR-1 | Fork detection: similarity (minhash) or declared-only `not-fork-of` with a stored pair hash. | (a) declared only; (b) similarity | (a): declared-only fork detection plus name heuristics; similarity deferred; a stored pair hash records dismissals; the scan ships off (observe first) | R-VAR-081, R-VAR-082 (edited), R-VAR-084 (added) | 0.3 | S3 G4; S3 §8 #25 |
| Q-VAR-2 | Wrapper naming and lint reconciliation (two `Button` names). | (a) lint rule plus naming convention; (b) convention only | (a): lint rule plus naming convention | R-VAR-074 (added) | 0.3 | H1 G4 |
| Q-VAR-3 | Variant naming across `cva`, Tailwind `@custom-variant` and fairlead `variant-of`. | (a) fairlead IDs aliasing native names; (b) native names only | (a): fairlead IDs alias native variant names through `native=` | R-VAR-053 (added) | 0.3 | H1 G25 |
| Q-VAR-4 | Who accepts a variation request for a home scope (human or agent), and who enforces expiry of interim wrappers. | (a) home owner under CODEOWNERS (D-03); (b) any author | (a): the home owner under CODEOWNERS accepts (through `approve`); `check` enforces wrapper expiry | R-VAR-043 (edited) | 0.3 | S3 G7; S3 §8 #26; D-03, D-21 |
| Q-VAR-5 | Enforcement mode of home-scope visibility (`subtree` or `public`) against outside consumers. | (a) error at enforce, warn before; (b) warn only | (a): a violation warns before enforce and errors at enforce | R-VAR-010 (edited) | 0.3 | S3 §4.6; D-21 |

## Context and retention (CTX)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-CTX-1 | Default `context` budget, token estimator and ratio. | (a) `ceil(bytes/4)` heuristic labelled as an estimate, ratio in config; (b) exact counts behind a flag | (a): `ceil(bytes/4)` labelled an estimate, ratio in config; exact counts opt-in (`--exact-tokens`); default budgets 500 (start) and 700 (path) | R-CTX-043 (edited), R-CTX-044 (added); `[context]` table in 12 | 0.1 | S4 §3, G3; H7 G5, K5; H5 G13 |
| Q-CTX-2 | Which task queries work in 0.1, given that BM25 is later (D-23)? | (a) path and ID queries only; a task query answers "no match" rather than guessing; (b) keyword list | (a): only path and ID queries in 0.1; free text exits 2 with "no match" and never guesses | R-CTX-004 | 0.1 | H3 G4; D-23; S4 §3 |
| Q-CTX-3 | Reasoning and memory outside fairlead are lost between sessions. | (a) handoff docs (`new handoff`), residual risk accepted; (b) capture transcripts | (a): `new handoff` and a `handoff:` line in the session brief from 0.1; residual risk accepted | R-CTX-008 (added), R-CTX-001; `new handoff` row in R-CLI-030 | 0.1 | H3 G5; H5 G8; S4 §6 |
| Q-CTX-4 | `--since` keyed by version or by commit. Digests are not keyed by commit, so deltas go wrong across branches. | (a) version or rev primary, digest only as an accelerator; recompute on the merged tree | (a): `--since` takes a version or rev as primary; digests are an accelerator, recomputed on the merged tree | R-CTX-054 (added) | 0.1 | H3 G23; S4 §3; D-23 |
| Q-CTX-5 | Budget overflow: dropped list and continuation token. | (a) `+N more` line plus `explain` (S4); (b) continuation token | (a): a `+N more` line plus `explain`; no continuation token | R-CTX-046 (added) | 0.1 | H6 G6; S4 §3 |
| Q-CTX-6 | Cost of Merkle digests and the ledger at 100k files; benchmark targets are assumptions. | (a) benchmark before fixing targets; (b) fixed targets now | (a): performance figures are goals until the 0.1 benchmark runs, including Merkle digests at 100k files | R-CTX-072 (added), R-CTX-070 | 0.1 | S2 G2; S4 §7; H6 G7 |
| Q-CTX-7 | BM25 recall on very short tasks, and whether authors must write `kw=` tags. | (a) measure, then require `kw` where recall fails; (b) drop BM25 | (a): measure BM25 recall, then require `kw=` where it fails | R-CTX-063 (added) | 0.3 | S4 G5 |
| Q-CTX-8 | Purge and legal erasure of records already in git history (`sync --purge`). | (a) purge keeps only id, date and digest; (b) out of scope | (a): purge keeps only id, date and digest | R-RET-043 | 0.3 | S4 G6; S4 §4 |
| Q-CTX-9 | Summary fields: the 240-character cap and redaction of secrets. D-39 makes `sum:` mandatory at creation and `out:` filled at close; the authors write both. | (a) cap plus refusal on secret patterns; (b) cap only | (a): 240-character cap plus refusal on secret patterns (HN-F001); seal refuses on hits in record fields | R-RET-006 (added); 20 seal preflight | 0.2 | H3 G8; S4 §4; D-39 |

## Git (GIT)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-GIT-1 | Authorship trailers (`Fairlead-Version`, change trailers) are omitted by agents, so commit authorship is unknown. | (a) trailer optional, never relied on (S2 §2.2); (b) required in CI | (a): trailers are optional and never required by `check` or CI | R-GIT-005 (edited) | 0.2 | H5 G9; S2 §2.2 |
| Q-GIT-2 | Rename detection (about 50% similarity) loses `--follow` history on large splits, so history-based audits misfire. | (a) audits report "unavailable" on low similarity; the restructure doc lists both paths; (b) none | (a): history audits report "unavailable" on low similarity; the restructure doc lists both paths | R-GIT-075 (edited) | 0.2 | H3 G14; S2 §3 (rename row) |
| Q-GIT-3 | Sparse checkouts and partial clones: scopes outside the checkout make `check` pass vacuously. | (a) report out-of-set scopes as unknown, never green; (b) ignore | (a): scopes outside a sparse or partial checkout are unknown, never green (HN-V013) | R-GIT-007 (added) | 0.1 | H4 G2 |
| Q-GIT-4 | Concurrent `move` and `seal` from two worktrees: which lock covers both. | (a) lock in the common git dir for seal and move; (b) per worktree (S2) | (a), as recorded in 23: one allocation lock in the common git directory for `seal`, `move` and `upgrade`. **Conflict:** 32 (Q32-4, R-PORT-015) records per-worktree locks and no cross-worktree lock. Unreconciled; see Status | R-GIT-052, R-GIT-073 (23); R-PORT-015 (32, conflicts) | 0.2 | H4 G5; S2 §3 (worktrees) |
| Q-GIT-5 | Reverting the commit that introduced an ID flips it between unused and dangling. | (a) `kind: revert of=<id>` doc (S2 §8 #9), tombstone state follows content; (b) none | (a): a revert of a sealed ID needs a `revert` doc; the tombstone state follows content | R-GIT-036 (added) | 0.2 | H8 G2; S2 §3, §8 #9 |
| Q-GIT-6 | Non-git workspaces and Jujutsu: coverage, versioning and merge assume git. | (a) declare unsupported; `check` and `context` only; (b) support jj | (a): non-git VCS and jj are unsupported beyond `check` and `context` | R-GIT-006 (edited) | 0.2 | H5 G7; H8 G11; S2 §3 |

## Portability (PORT)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-PORT-1 | Extracted root: copy or inherit inherited rules, scope identity on extraction, archival of historic `covers`. | (a) new root with lock-inherit; (b) copy with provenance; (c) mounted sub-root with `adopt` (D-22) | (a): the extracted scope becomes a new root; inherited rules come through a pinned lock, not a copy; sealed `covers` are kept as history | R-PORT-045 (added) | 0.3 | H3 G16, G17; S1 §8.2; D-22 |
| Q-PORT-2 | Lock staleness with no transport between repos. A breaking backend change loses the in-repo consumer check, and cross-repo consumers are invisible. | (a) explicit pull at a pinned version plus a CI check; (b) daemon (refused by D-26) | (a): explicit pull at a pinned version plus a CI check; no daemon | R-PORT-046 (added) | 0.3 | H3 G16, G17; S3 G5; D-22, D-26 |
| Q-PORT-3 | Root-level `shared/` imports break silently at extraction. `requires` is not computable in 0.1. | (a) import extraction (Q-HIER-1) and fail on unresolved imports; (b) manual `requires` | Superseded in part by D-27 and D-40: outward imports found by the regex pass are `requires` candidates, reported `UNSATISFIED` at attach (a lower bound) | R-PORT-047 (added); R-HIER-033 | 0.3 | H3 G15; S1 §8.1 |
| Q-PORT-4 | `exposure` is not linked to `AppManifest` (`capabilities/*.json`, `build.rs`). | (a) parse manifests for declared exposure; (b) intent only, unverified | (b): `exposure` is intent only, reported as `exposure unverified`; manifests are not parsed | R-HIER-067 (added; placed in 30 bridges) | 0.3 | H3 G29 |
| Q-PORT-5 | Cross-repo transactions and moves: no transaction spans two `.fairlead/` caches. | (a) out of scope; each repo seals alone; (b) two-phase | (a): out of scope; each repo seals alone | R-PORT-043 (edited) | 0.3 | H4 G1 |
| Q-PORT-6 | Detach and attach: what a detached scope materialises. | (a) synthetic detached `origin=snapshot` layer until adopted (S1 §8.2); (b) copy everything | (a): a synthetic `origin=snapshot` layer (R-PORT-041). `move --detach` snapshots the inherited view into `gen/scope.lock` and refuses with exit 4 unless the scope is `portable` or inherits nothing (R-CLI-047); DETACHED (HN-M011) | R-PORT-013 (0.2), R-PORT-041; 15 R-CLI-047 | 0.2 | H6 G5; S1 §8.2; D-22 |
| Q-PORT-7 | Canonical casing for scope paths and IDs on NTFS and APFS. | (a) lowercase-normalised paths; a case-only rename is a move; (b) case-sensitive | (a): lowercase-normalised paths and IDs; a case-only rename is a move | R-PORT-007 (edited) | 0.1 | H7 G1; S2 §8 #43 |
| Q-PORT-8 | Windows long paths (over 260 characters), network drives and antivirus cost are unmeasured. | (a) test Windows from the first commit; (b) defer | (a): Windows is tested from the first commit | R-PORT-014 (added) | 0.1 | H5 G11; S2 G6; H7 K4 |
| Q-PORT-9 | Scope-ID uniqueness across mounts (`as=` prefix) and the namespace rule for copies (`derived-from`). | (a) namespace on import, `as=` for mounts; (b) reject duplicates | (a): namespace on import and `as=` on mounts; a bare duplicate is DUP-SCOPE-ID (HN-C007) | R-PORT-035 (added) | 0.3 | S4 G4; H8 G8; S1 §7, §10 #23 |
| Q-PORT-10 | Mounted roots: `:root` projections clash when two roots generate them. | (a) projections only inside each root's own scope; (b) forbid mounting | (a): `:root` projections only inside each root's own scope (HN-M010) | R-PORT-036 (added) | 0.3 | H1 G26 |
| Q-PORT-11 | Published packages consumed by another repo, beyond `requires` and `provides` digests. | (a) out of scope in 0.3; (b) registry integration | (a): out of scope in 0.3; digests only | R-PORT-055 (added) | 0.3 | S2 G1 |

## Agents and trust (AGT)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-AGT-1 | Authority behind `rejected`, `ovr` and tombstone records when commits are not signed. Owner and agents may share one git author. | (a) CODEOWNERS plus `fairlead approve` (D-03); (b) signed commits verified by fairlead; (c) owner-only path hook | (a) now: CODEOWNERS plus `approve`. (b) later: the general rule is 0.3 (R-AGT-027), with an opt-in `authority.signed-records` from 0.2 (R-LC-040) | R-AGT-026 [0.1], R-AGT-027 [0.3]; 21 R-LC-040 [0.2] | 0.1 / 0.2 / 0.3 | H3 K1, G7; H4 K3; D-03, D-16 |
| Q-AGT-2 | Owner identity on `ovr`: free text, or a verified identity list in control. | (a) identity list in control, checked against CODEOWNERS; (b) free text | (a): `owner=` must be a handle in `owners`, checked against CODEOWNERS; free text is OWNER-UNKNOWN (HN-X012) and the `ovr` is ineffective | R-SYN-011; R-AGT-028 (added), R-CHK-053 (added) | 0.1 | D-02, D-03; S1 §3.2; H3 Y2 |
| Q-AGT-3 | Approver when the ancestor owner is an agent, and team roles. | (a) human approver required for floor-adjacent keys; (b) team roles declared in control | (a): a human approver is required for floor-adjacent keys; an `owners` entry may be `kind = "agent"` (0.2); APPROVER-AGENT is HN-X013 | R-AGT-029 (added), R-CHK-054 (added) | 0.2 | H4 G4, K3; D-03 |
| Q-AGT-4 | Escalation resolver and unseal authority for sealed keys. | (a) ancestor owner resolves through `approve`; (b) root only | (a): the nearest governing ancestor owner resolves escalations through `approve`; root only for non-delegable keys | R-AGT-030 (added) | 0.2 | H6 G2; S1 §4.2; D-02 |
| Q-AGT-6 | Human edits without change docs: coordination beyond the warn severity. | (a) warn plus PR template; (b) block the merge | (a): warn ramp plus PR template text (`explain pr-template`); no fairlead merge block | R-AGT-031 (added) | 0.2 | H3 G6; D-15 |
| Q-AGT-7 | Secrets already in git history and inside summaries: detection scope. | (a) going forward only, purge stays a human rewrite; (b) secret scanner in `check` | (a): going forward only; purge stays a human rewrite; summaries are covered by the floor secret scan (see Q-CTX-9) | R-AGT-025 [0.1] | 0.1 | H5 G4; S4 G6 |

## Tooling and crates (TOOL)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-TOOL-1 | Cache format and key: binary mmap (S4 §7) or JSON (H7). The key must cover fairlead version, grammar, config hash and `--as-of`. | (a) JSON in 0.1 until profiled, key covers all four; (b) binary now | (a): JSON cache in 0.1 until profiled; key covers fairlead version, grammar, config hash (incl. marker) and `--as-of` where date-dependent | 50 R-ARCH-073, R-ARCH-077 | 0.1 | H7 D3, G2; S4 §7; S2 G5 |
| Q-TOOL-2 | Which manifests are parsed in 0.1: Cargo (TOML parse for features and members), `package.json`, capability JSON, `build.rs`. | (a) Cargo TOML parse only; (b) none | (a): only Cargo TOML manifests are parsed in 0.1, read-only; `cargo` is never invoked; no YAML (D-50(3)) | 50 R-ARCH-005, R-ARCH-006 | 0.1 | H3 G10; S2 §4.1; S1 §8.1 |
| Q-TOOL-3 | tree-sitter-typescript compatibility with tree-sitter 0.27 (unverified). | (a) test before committing to TS parsing; (b) regex only | (a): tree-sitter TS parsing is gated on a CI compatibility test; the regex pass runs until then | 30 R-HIER-035 (added) | 0.3 | H7 G8 |
| Q-TOOL-4 | Tauri config assumes a co-located frontend (`frontendDist`, `beforeBuildCommand`). `tauri-specta` output is an unowned generated file. | (a) declare the external artifact as an input; mark bindings `generated-from`; (b) ignore | (a), in the D-35 form: external build inputs are `def ext.<name> role=input`; bindings use `gen=`, not `generated-from` | 30 R-HIER-068 (added), R-HIER-066 | 0.3 | H3 G18; S3 §8 #20 |
| Q-TOOL-5 | Per-worker git handles (`git2::Repository` is not `Sync`). | (a) open one repository per worker; (b) single-threaded git | (a): one `git2` repository per worker, opened from the codebase root | 50 R-ARCH-081 | 0.1 | H7 G4 |
| Q-TOOL-6 | Lock behaviour on network drives (advisory `fd-lock`). | (a) pid lock per S2 and document network drives as unsupported; (b) advisory lock | (a): a pid lock file `.fairlead/lock` (pid, host, start time), not an OS advisory lock; network drives are unsupported | 50 R-ARCH-142, R-ARCH-146 | 0.1 | H7 G3; S2 §3 |
| Q-TOOL-7 | SARIF output against the 2.1.0 schema (the `serde-sarif` revision is unverified). | (a) CI validates output against the official schema; (b) trust the crate | (a): SARIF 2.1.0 output is validated against the official schema in CI; milestone 0.1 (chapters 50 and 60 corrected from 0.2) | 50 R-ARCH-054; 13 R-CHK-036; 60 §3.1 | 0.1 | H7 G7 |
| Q-TOOL-8 | MCP server (`rmcp`): timing and release churn. | (a) after 0.1, pinned and retested on each release; (b) skip | (a): after 0.1 (the 0.3 line); `rmcp` pinned to an exact minor and re-tested each release | 50 R-ARCH-004; 60 §3.3 | after 0.1 | H7 G9, D7 |
| Q-TOOL-9 | Update checks need the network (P7). | (a) explicit command only, never in background (D-26); (b) automatic check | (a): updates only through the explicit `upgrade --fetch` (0.2); isolated `net` module; never in the background | 50 R-ARCH-124; 22 R-EDN-025 | 0.2 | H3 G21; D-26 |
| Q-TOOL-10 | `cargo publish` of crates containing `fairlead/`, and path-plus-version dependency edges that must agree. | (a) exclude `fairlead/` in `[package]`; (b) include | (a): exclude the managerial folder in `[package]`; path-plus-version edges must agree; a missing exclude is PKG-HINT (HN-M009) | 32 R-PORT-054 (added) | 0.3 | H2 G8, G9 |

## Product and naming (PROD)
| ID | Question | Options | Resolution | Recorded in | Milestone | Src |
|---|---|---|---|---|---|---|
| Q-PROD-1 | Owner ratification of the inferred decisions D-10 to D-30. decisions.md marks them "inferred, owner may overrule". | (a) ratify all; (b) ratify the 0.1 subset first, leave the rest marked open | Ratify all: the owner approved every inferred decision (D-10 to D-45) under D-46 | D-46; 00 §11 | 0.1 | decisions.md header and section titles |
| Q-PROD-2 | Final name: `fairlead` is temporary (D-30). `fairlead` is preferred, with `gusset` and `lashing` as candidates. Trademarks, GitHub noise and `harnext` are not checked. | (a) rename to `fairlead` before publish; (b) keep `fairlead` | (a): the target name is `fairlead`, after a trademark check (D-47); the docs keep `fairlead` until then. The trademark check is not recorded as done | D-47; 00 Appendix A.3, R-VIS-006; 60 R-REL-040, R-REL-043 | publish | D-30; H7 K1, G10 |
| Q-PROD-3 | Is the literal-leak check part of 0.1? D-06 names the 0.1 features but not the checks inside `check`. 60 §3.1 and R-ROAD-003 to R-ROAD-005 assume literal-leak checks in 0.1; this register's first draft did not. | (a) keep literal leaks out of 0.1, as the first draft leaned; (b) keep them in 0.1, as 60 assumes | Yes (D-43): the literal-leak check is part of 0.1; 60 follows option (b) | D-43; 00 §4.2; 60 §3.1, R-ROAD-016 | 0.1 | D-06; H8 D1; 60 §3.1 |
| Q-PROD-4 | Measurable success criteria for the 0.1 exit test (about 20 tasks showing agents change behaviour after `context`). | (a) write the criteria before building the test; (b) decide after | (a): measurable 0.1 exit criteria are written and pre-registered before the test is built | 60 §4.1 (E1 to E10), R-ROAD-014 | 0.1 exit | D-06 |
| Q-PROD-5 | Dogfooding is Rust-only, so no TypeScript path is exercised. | (a) add a TypeScript fixture repo to 0.1; (b) rely on the H1 example only | (a): a TypeScript/React fixture is mandatory in the exit test, the precision corpus and replay | 60 R-ROAD-015, E9, §7.1; 50 R-ARCH-153 | 0.1 exit | H8 G7 |
| Q-PROD-6 | Prior-art citations are unverified (Structurizr, C4, Changesets path, spec-kit, Kyverno, Repomix). | (a) verify before publishing; (b) cite as-is | (a): verify prior-art citations before publishing, or remove them | 00 §4.1, R-VIS-008; 60 R-REL-041 | docs | H6 G11 |
| Q-PROD-7 | "No telemetry": waiver growth is measured from git history (`git log -S "fairlead: ovr"`), not by the tool. | (a) accept and document; (b) add telemetry (against the stance) | (a): accepted and documented; no telemetry; waiver growth is measured from git history | 00 R-VIS-007, non-goal 6; 60 §7.3 | none | H8 G9 |
| Q-PROD-8 | P10, "edge cases over years", must become numbered cases before 0.2. | (a) numbered cases in each chapter; (b) one edge-case chapter | (a): a numbered edge-case table in each chapter; no separate chapter | 00 R-VIS-009, P10 | 0.2 | H8 G10 |
| Q-PROD-9 | Do 0.1 `use` lines exist? D-06 lists `def` and `use` in 0.1. H8 D2 moves `ref` to 0.2. | (a) D-06 as written; (b) H8 cut, `use` in 0.2 | (a): `use` lines exist in 0.1, as D-06 lists them | 60 §3.1 | 0.1 scope | D-06; H8 D2 |

## Index of chapter-local questions
Chapter-local questions are listed in their chapter, not repeated here. Each row points to the section that holds them. Chapters 00, 01, 50 and 60 and the examples use their own prefixes (Q-VIS, Q-GLOS, Q-ARCH, Q-ROAD, Q-EXTS, Q-EXRS) rather than `Q<chapter>-<n>`. Several chapters also list register items (for example Q-SCOPE-1) in their resolved lists; those are counted in the tables above, not as chapter-local.

| Chapter | Where | Chapter-local IDs |
|---|---|---|
| [00-vision](00-vision.md) | Resolved questions (§11) | Q-VIS-1 to Q-VIS-2 |
| [01-glossary](01-glossary.md) | Resolved questions | Q-GLOS-1 to Q-GLOS-5 |
| [10-scopes-and-inheritance](10-scopes-and-inheritance.md) | Resolved questions | Q10-1 to Q10-7 |
| [11-files-and-classes](11-files-and-classes.md) | Resolved questions | Q11-1 to Q11-5 |
| [12-directive-syntax](12-directive-syntax.md) | Resolved questions | Q12-1 to Q12-10 |
| [13-checks-and-diagnostics](13-checks-and-diagnostics.md) | Resolved questions | Q13-1 to Q13-10 |
| [14-context](14-context.md) | Resolved questions | Q14-1 to Q14-5 |
| [15-cli](15-cli.md) | Resolved questions | Q15-1 to Q15-10 |
| [20-versions-and-seal](20-versions-and-seal.md) | Resolved questions | Q20-1 to Q20-7 |
| [21-environments-maturity-lifecycle](21-environments-maturity-lifecycle.md) | Resolved questions | Q21-1 to Q21-5 |
| [22-editions-and-upgrade](22-editions-and-upgrade.md) | Resolved questions | Q22-1 to Q22-5 |
| [23-git-integration](23-git-integration.md) | Resolved questions | Q23-1 to Q23-5 |
| [30-hierarchy-and-engines](30-hierarchy-and-engines.md) | Resolved questions | Q30-1 to Q30-6 |
| [31-variation-protocol](31-variation-protocol.md) | Resolved questions | Q31-1 to Q31-5 |
| [32-portability-and-restructure](32-portability-and-restructure.md) | Resolved questions | Q32-1 to Q32-6 |
| [33-retention-and-compaction](33-retention-and-compaction.md) | Resolved questions | Q33-1 to Q33-7 |
| [40-agent-integration-and-trust](40-agent-integration-and-trust.md) | Resolved questions | Q40-1 to Q40-6 |
| [50-architecture](50-architecture.md) | Resolved questions (§18) | Q-ARCH-1 to Q-ARCH-9 |
| [60-roadmap](60-roadmap.md) | Resolved questions (§12) | Q-ROAD-1 to Q-ROAD-7 |
| [examples/ts-react](examples/ts-react.md) | Resolved questions | Q-EXTS-1 to Q-EXTS-4 |
| [examples/rust-workspace](examples/rust-workspace.md) | Resolved questions | Q-EXRS-1 to Q-EXRS-7 |
| [examples/fullstack-five-years](examples/fullstack-five-years.md) | Resolved questions (register items only) | none |

## Settled
Items below are settled by [decisions](decisions.md) and are not open. Status **owner** means owner-approved in decisions.md (the owner-approved list, the reconciliation section D-31 to D-45, or the acceptance section D-46 to D-50). **owner-approved (D-46)** marks items that decisions.md had marked as inferred (D-10 to D-30 and statements derived from them); the owner approved these on 2026-10-09 under D-46. A source proposal that a decision overrides is named on the same line. Questions moved here from the open lists keep their former IDs in brackets.

| Item | Settled by | Status |
|---|---|---|
| Root lookup: `--root`, then `FAIRLEAD_ROOT`, then an upward search that accepts only folders whose control file has `[fairlead] root = true`; the nearest one wins; no root is an error [was Q-SCOPE-11] | D-31 (amends D-11) | owner |
| Control-file `[fairlead]` header (keys `edition`, `root`, `scope`, `marker`), frozen across editions and read first; no separate pin file; the edition is the pin [was Q-FILE-12] | D-33 | owner |
| Format split: authored input (control file, `+++` frontmatter, approvals, allow grants) is TOML; generated files, ledger shards and `context` output are terse `ID k=v` lines [was Q-FILE-10; format part of Q-FILE-4] | D-32 (amends D-05) | owner |
| Override expiry: `exp=YYYY-MM-DD` in 0.1, checked against `--as-of`; `until=<version>` (`MAJOR.MINOR.PATCH`, D-42) is the default from 0.2; dates stay allowed [was Q-SYN-2] | D-37 (amends D-14) | owner |
| Allow grants are `[[allow]]` tables in the granting ancestor's control file, not a seventh verb; a tightening `ovr` on a root-ordered scale needs no grant | D-34 | owner |
| Approvals are `[[approval]]` tables in `fairlead/approvals.toml`, written by `fairlead approve`; terminal-gated, not authentication | D-34 | owner |
| Foreign regions are `def foreign.<name>` closed by `end`; no dedicated verb [syntax part of Q-FILE-5] | D-35 | owner |
| Generated items are declared with a `gen=` key on `def`; there is no separate `gen` line | D-35 | owner |
| Reference files carry a machine-readable header and prose, and never directives [was Q-SYN-16] | D-36 (amends D-19) | owner |
| Entries, units, containers and bridges are declared with `def`: in code comments, in doc or control files, or in marked Markdown registry blocks [was Q-SYN-7; declaration part of Q-HIER-7] | D-40 | owner |
| Exit codes are defined only in [15-cli.md](15-cli.md); other chapters cite it and define none [was Q-CHK-2]. Exit 5 is an internal error or a result that cannot be determined; a held lock is refusal (4) [was Q-ARCH-8] | D-41; D-44 | owner |
| Retention clock: version-based TTL with a calendar floor (90 days by default); `out:` filled at close; hash-chain checkpoints [was Q-CTX-10] | D-39 (amends D-24) | owner |
| `move` is 0.2; the baseline is written by `fairlead sync --baseline`; `check` is read-only; hook installation is offered by `init` and never default-on [was Q-AGT-5; milestone part of Q-SYN-8; check part of Q-SYN-14] | D-38 | owner |
| 0.1 build order: root-only first, then nested scopes; the spec covers both | D-38 | owner |
| No per-file version headers; file history from git and `explain` | D-13 | owner-approved (D-46); stamp form: Q-VER-1 |
| Created stamp: a once-only, immutable `created` key at creation; never rewritten; not a version header | D-13 | owner-approved (D-46); seal-time stamp and form: Q-VER-1 |
| Scope identity is a stored ID, not a path | D-11 | owner-approved (D-46) |
| Unparseable control file fails closed (S1 §10 fallback proposal dropped) | D-11 | owner-approved (D-46) |
| Scopes are eligible, not mandatory; lazy creation; demotion or deletion leaves a tombstone | D-11 | owner-approved (D-46) |
| Child redefinition of an inherited ID without `ovr` is an error (S4 §8 #7 "warn HN-R005" dropped) | D-12 | owner-approved (D-46) |
| Per-field override: replace by default; list append needs an explicit marker (S1 §2.1 "child appends" dropped; marker syntax is Q-SYN-17) | D-12 | owner-approved (D-46) |
| Policies are `open`, `needs-exception`, `sealed`; rules default to `sealed`, `waivable` makes them `needs-exception` (S1 §3.1 `exception` and its default dropped) | D-02 | owner |
| Override carries reason, owner and expiry; effective and recorded; sealed override inert until `allow` | D-02 | owner |
| Vocabulary keys are root-only and sealed; a child attempt is an escalation proposal | D-02 | owner |
| Floor rules compiled into the binary; not waivable | D-03 | owner |
| CODEOWNERS review of the managerial folder; `fairlead approve` records approvals | D-03 | owner |
| Authored CSS holds values; fairlead holds the token contract; fairlead never merges file contents | D-04 | owner |
| Vendor-owned file class and wrapper units | D-04 | owner |
| No YAML; Markdown bodies and `+++` frontmatter (H1, H3 and S2 YAML drafts dropped); the authored and generated split is D-32 above | D-05 | owner |
| Feature milestones (0.1, 0.2, 0.3 lists) | D-06 | owner |
| Docs live in `docs/` of this repo | D-07 | owner |
| Authored intent resolved at read time, never copied; only generated and reference files written | D-10 | owner-approved (D-46) |
| Facts roll up only at seal; 0.1 generated files are the baseline and scope index shards | D-10 | owner-approved (D-46) |
| Sealed version is derived, never stored, and the version file is written last; the format is `MAJOR.MINOR.PATCH` (D-42) | D-14 (format: D-42) | owner-approved (D-46) |
| Global version is `MAJOR.MINOR.PATCH` (the owner's `1.0.2` form), independent of Cargo and npm; integers dropped [was Q-VER-14] | D-42 (amends D-14) | owner |
| Open version is the sealed version bumped by the largest declared impact: breaking -> major, feature (`add`) -> minor, fix or chore -> patch; while MAJOR is 0 each step shifts down one level (breaking -> minor); `created = "<version>"` is identity, not a counter [was Q-VER-2; Q20-3, Q20-7] | D-42 (amends D-14) | owner |
| Seal runs on the integration line by default; branch seals allowed for solo work with collision repair | D-14 | owner-approved (D-46) |
| Draft IDs are random; ordinals assigned at seal, per scope | D-14 | owner-approved (D-46) |
| Generated-file conflicts resolved by regeneration; no custom merge driver | D-14 | owner-approved (D-46) |
| Fairlead never commits unless `--commit` | D-14 | owner-approved (D-46) |
| Docs-first is path coverage, not order; warn by default; exempt globs; chore docs; ordering only through optional hooks | D-15 | owner-approved (D-46) |
| Environment, maturity and lifecycle are three axes with root-owned user words | D-16 | owner-approved (D-46) |
| Cargo features, cfg and TS env flags are cross-checked for existence and never evaluated | D-16 | owner-approved (D-46) |
| Lifecycle operations deprecate, replace, edit, remove; removed IDs reserved forever; tombstones in the scope folder | D-16 | owner-approved (D-46) |
| Graveyard keeps rejected features with a fingerprint (definition is Q-LIFE-3) | D-16 | owner-approved (D-46) |
| Editions are byte-stable; new rules ship off within an edition | D-17 | owner-approved (D-46) |
| Unknown directive class is an error; unknown key a warning; `!` is must-understand | D-17 | owner-approved (D-46) |
| One edition per codebase root; a sealed sub-root may carry its own | D-17 | owner-approved (D-46) |
| Directives are comments only, never attributes or doc comments | D-18 | owner-approved (D-46) |
| Six verbs (`def use ovr flag env end`); no eighth class (S1 `ask` as a class dropped; S3 `unit`, `container`, `entry`, `bridge`, `layer` and H3 `impl`, `call`, `register`, `shape` dropped as verbs) | D-18 | owner-approved (D-46) |
| IDs are `[scope:]kind.name`, path-independent; kinds user-defined in control | D-18 | owner-approved (D-46) |
| Directive text is data, never an instruction to an agent | D-18 | owner-approved (D-46) |
| File classes: authored, generated, reference, vendor-owned, append-only ledger | D-19 (reference contents: D-36) | owner-approved (D-46) |
| Generated files committed, deterministic, sorted, sharded per scope; hand-edited lines flagged (H8's computed-on-demand option dropped) | D-19 | owner-approved (D-46) |
| Ladders are user data; ranks compare inside one ladder; containment separate from `uses`; `uses` as comments aggregated upward | D-20 | owner-approved (D-46) |
| Coverage is an adoption ramp L0 to L3; entry kinds; orphan and stale-and-dormant verdicts; upward flows via port and `implements` | D-20 | owner-approved (D-46) |
| Variation: visibility `subtree` or `public`; four outcomes; request mandatory for primitive variation | D-21 | owner-approved (D-46) |
| Hardcoded-token detection by declared values and `applies=` context | D-21 | owner-approved (D-46) |
| Mounted root is a sealed sub-root, opened only by explicit `adopt`; digest-only lock unless `portable` | D-22 | owner-approved (D-46) |
| Doc IDs namespaced by scope; `contradicts` declared on ID'd claims | D-22 | owner-approved (D-46) |
| Terse `ID k=v` lines for generated files and `context`; tiers L0 to L3; Merkle digests; `--since`; priority `--budget` | D-23 | owner-approved (D-46); BM25 later (Q-CTX-7) |
| Retention runs inside seal using the seal's recorded time; TTL by version; decisions, graveyard, tombstones kept forever | D-24 | owner-approved (D-46); clock detail: D-39 |
| Authored `sum` and `out` mandatory on retained records; fairlead runs no LLM (`out` is filled at close, D-39) | D-24 | owner-approved (D-46) |
| CLI verb set includes `approve`; `status` folded into `context`; `compact` folded into `seal` | D-25 | owner-approved (D-46); approve kept (Q-SYN-13) |
| No background processes | D-26 | owner-approved (D-46) |
| Rust; AST-free 0.1; `ignore` with `require_git(false)` and `git_global(false)`; git2 for reads (D-48); no bincode; MSRV 1.88 | D-27 | owner-approved (D-46) |
| Scope is folder-based; module paths are aliases; generated items declared at the generator with `gen=` (D-35 replaces the `gen` line) | D-27 | owner-approved (D-46) |
| Baselines, observe-warn-enforce ramp, expiring waivers, waiver sprawl reported | D-28 | owner-approved (D-46) |
| Bridges declare params, not structs; checks are presence, wire literal and version pin; payload equality out of scope | D-29 | owner-approved (D-46) |
| Name is temporary; marker and folder name come from config | D-30, decisions header | owner-approved (D-46); final name: D-47 |
| Wedge: per-key inheritance, explicit override policy, expiring waivers, exception ledger | D-01 | owner |
| Version format: the global version is MAJOR.MINOR.PATCH, not an integer; `until=<version>` takes this format; the open version is derived, not stored (bump rules: Open version row) | D-42 | owner |
| Literal-leak check is in 0.1: a hardcoded value where a declared token exists is found from declared token values and the `applies=` property context; `check --changed` is 0.1 (Q-PROD-3: yes) | D-43 | owner |
| Exit 5 is an internal error or a result that cannot be determined (for example, unreadable git state when a command needs it); defined only in 15-cli.md (extends D-41) | D-44 | owner |
| Edition is an integer (`edition = 1`) in the `[fairlead]` header, not a year; D-14's `versions/N.md` wording is superseded by D-42 | D-45 | owner |
| Owner acceptance: every inferred decision (D-10 to D-45) and every open question is accepted by its stated leaning; a superseded leaning is resolved as "superseded by D-xx"; new questions are added as open and need owner acceptance | D-46 | owner |
| Target name `fairlead`, applied after a trademark check (not yet recorded as done); the docs keep `fairlead` until then; the rename is mechanical (marker and folder are config keys) | D-47 (Q-PROD-2) | owner |
| Git access split: reads (status, diff, history, rename detection) use git2; `seal --commit` and `seal --tag` run the user's `git` executable; no other verb commits, stages or pushes (amends D-27). Gap: `check --ci` signature checks (`git verify-commit`, R-CLI-010, R-LC-040) are not listed here | D-48 | owner |
| One key per concept: `ovr` expiry is `exp=<date>` (0.1) or `until=<version>` (0.2 and later); `flag` targets use `target=<version>` and never take `until=` (extends D-37) | D-49 | owner |
| No `move --id` in 0.1 to 0.3: renames and merges use `replaced-by` with a permanent tombstone alias, and references through the alias are reported until updated; the content hash is blake3 (sha256 appears nowhere); only Cargo TOML is parsed in 0.1 (no YAML, no `cargo metadata`); `fairlead-version` is a refusing pin | D-50 | owner |

**Accepted limits** (consequences of decisions, not open questions):

| Limit | Source | Under |
|---|---|---|
| Markers inside string literals and fixtures are parsed; `skip` globs and the `fairlead\:` escape apply | S4 §8 #1; H5 G1 | D-27 |
| Docs and tokens are checked for presence and coverage, not for truth | H5 G2; S1 G1 | D-15, D-22 |
| `--no-verify` bypasses optional hooks; CI is the gate | H5 G3 | D-15 |
| Pipeline order and branching not checked; evidence-grep is textual; macro and re-export output invisible; symbol binding is line-based | S3 G1, G2; H2 G7, G19; S4 G2 | D-27, D-20 |
| Dynamic references and reachability depend on declared entries and edges; no code graph in 0.1 | S3 G3; H3 G25; H5 G5 | D-20 |
| Environment gating and Cargo feature unification are not provable without a build | S2 G7; H2 G13, G14 | D-16 |
| Squash merges erase commit order, so docs-first is coverage-only | H5 G6; H3 G1 | D-15 |
| Field-level payload equality not checked | H3 G29 | D-29 |
| Graveyard matching is lexical; synonyms need `kw` terms | S2 G8 | D-16 |
| Coverage of a human edit is a warning, not a block | H3 G6 | D-15 |
