# Checks and diagnostics
> Status: draft · Milestone: mixed · Decisions: D-01, D-02, D-03, D-12, D-15, D-19, D-21, D-23, D-26, D-28, D-34, D-36, D-37, D-38, D-41, D-43, D-44, D-46, D-48, D-49, D-50, D-51 · Related: 10-scopes-and-inheritance.md, 11-files-and-classes.md, 12-directive-syntax.md, 14-context.md, 15-cli.md, 40-agent-integration-and-trust.md

## Purpose
`fairlead check` compares what the code says against what the declarations say and reports the difference. It is the enforcement half of the product: deterministic, read-only, fast enough for a pre-commit hook, and quiet enough that agents read the output. A pass proves the *declared* constraints hold, not that the codebase is consistent.

## Concepts
- **Check**: a built-in, compiled-in verification with a stable diagnostic code (`HN-<letter><nnn>`).
- **Rule**: a declaration (`rule.<name>`) in a control file that configures a check type (`leak`, `pattern`) or tunes a built-in. Rules carry severity, stage, scope and waivability.
- **Severity**: `error`, `warn`, `info` (what the finding is). **Stage**: `observe`, `warn`, `enforce` (how hard it bites; the ramp, D-28).
- **Floor rule**: a check compiled into the binary that cannot be waived, baselined, lowered or staged below enforce (D-03).
- **Waiver**: an `ovr` directive on a `waivable` rule; effective and recorded in the ledger (D-02).
- **Baseline**: a committed, sorted list of finding fingerprints that are tolerated until fixed.
- **Fingerprint**: stable identity of a finding, independent of line numbers.
- **As-of date**: the date against which `exp=` is evaluated; the only time input to `check`.

## Rules

### What `check` does
- **R-CHK-001 [0.1]** `check` MUST (1) resolve the root and scope chain, (2) parse every control file and directive, (3) build the ID registry, (4) evaluate built-in checks and declared rules, (5) apply waivers, baseline and stage, (6) print diagnostics and a summary, (7) exit with the codes of [CLI](15-cli.md) R-CLI-003. It MUST NOT write to the working tree. It MAY write the disposable cache `.fairlead/`.
- **R-CHK-002 [0.1]** Global checks (dangling, duplicate, shadowing) MUST be computed on the whole registry even when a `[path]` argument limits what is reported. A path-scoped run MUST equal the full run filtered to that path.
- **R-CHK-003 [0.1]** A repo with zero declared rules and zero IDs MUST print `HN-N001 info 0 rules declared, 0 IDs: nothing verified` and still exit 0. A silent green on an empty declaration set is a trust failure. `--require-declarations` turns it into exit 1.
- **R-CHK-004 [0.1]** `check` never executes project code, never contacts the network and never reads outside the root (D-26, D-27).

### Rule model
- **R-CHK-005 [0.1]** Rules are declarations: `[rules.<name>]` tables in a control file (ID `rule.<name>`). Keys: `check` (`leak` or `pattern`), `sev`, `stage`, `glob`, `exclude`, `waivable`, `why` (one line, shown as the *intent excerpt*), plus type keys (`tokens` for `leak`; `forbid` regex and `msg` for `pattern`).
- **R-CHK-006 [0.1]** Rules are `sealed` by default: a descendant scope may only tighten (raise `sev`/`stage`, widen `glob`; it may never lower them or add `exclude`). A rule with `waivable = true` is `needs-exception`: a descendant may `ovr` it with `why`, `owner`, `exp` (D-02).
- **R-CHK-007 [0.1]** A rule applies to files under the declaring scope's folder that match `glob` and not `exclude`, except files belonging to a descendant scope that has not inherited it (see [scopes](10-scopes-and-inheritance.md)). A `glob` is relative to the declaring scope.
- **R-CHK-008 [0.1]** Floor rules are listed by code in the table below. Their `sev`/`stage` MUST NOT be edited; an `ovr` targeting one is `HN-X003` and inert; a baseline MUST NOT contain them.
- **R-CHK-009 [0.1]** A new user rule scaffolded by `fairlead new rule` defaults to `stage = "observe"`; built-in structural checks default to `enforce`; `HN-T001` defaults to `observe` until a rule enables it.
- **R-CHK-010 [0.1]** Authors SHOULD keep the starting rule set at 10 or fewer, each with an owner and a one-line `why`. A rule whose findings are more than 20% waived is reported `HN-X008` for review.

Stage x severity decides what the user sees and whether the run fails:

| Stage | error finding | warn finding | info finding | Affects exit code |
|---|---|---|---|---|
| `observe` | counted in summary; printed only with `--show-observe`; in JSON/SARIF | same | same | no |
| `warn` | printed | printed | printed (no `-v` in 0.1) | only with `--deny-warnings` |
| `enforce` | printed, fails | printed | printed (no `-v` in 0.1) | error findings: exit 1 |

### Deterministic guarantees
- **R-CHK-011 [0.1]** For the same tree content, binary, `--as-of` and flags, the output MUST be byte-identical: sorted by (path, line, col, code), LF-only, repo-relative `/` paths, no timestamps, no absolute paths, no hostnames, no locale dependence, no thread-order dependence.
- **R-CHK-012 [0.1]** Output MUST be identical on Linux and Windows checkouts of the same commit, with LF or CRLF (hashing and parsing normalise to LF; BOM and CR stripped).
- **R-CHK-013 [0.1]** The cache MUST NOT change results: `--no-cache` and a warm cache give identical bytes. A corrupt or version-skewed cache is discarded and rebuilt silently.
- **R-CHK-014 [0.1]** The only time input is `--as-of YYYY-MM-DD` (default: UTC today). The header line prints it. CI SHOULD pass the commit date so reruns are reproducible. Git is never consulted for the default.
- **R-CHK-015 [0.1]** `check`, `context` and `explain` MUST leave `git status` unchanged. This is a release test (see [roadmap](60-roadmap.md)).
- **R-CHK-016 [0.1]** Hot-path goals (to be benchmarked, [architecture](50-architecture.md)): 100k-file Windows tree, warm no-change run in seconds, cold run in a minute, `--files-from` run in about a second.

### Built-in checks for 0.1
Codes are stable identifiers (never reused). Syntax codes `HN-S*` are in [syntax](12-directive-syntax.md). `F` = floor (not waivable, not baselined). A row tagged [0.2] ships in 0.2 and is inactive in 0.1; untagged rows are 0.1.

| Code | Name | Default sev / stage | Floor | Waivable | What it reports |
|---|---|---|---|---|---|
| HN-R001 | dangling reference | error / enforce | | no | `use`, `ovr`, `flag to=`, code `def` of an ID with no declaration; nearest 3 IDs by edit distance, and the right ID if only the kind prefix is wrong; a `def` that exists only in a draft, ready or abandoned change doc does not satisfy a reference (20 R-VER-060) |
| HN-R002 | unused declaration | warn / observe | | yes | doc `def` with no `use` or code site; opt-in per kind (`unused = "warn"`); advisory only, dynamic references exist |
| HN-R004 | alias or removed-ID reference (`ALIAS-REF`) [0.2] | warn / warn; error when the removed ID has no alias | | no | a `use`, `ovr` or `flag to=` naming a tombstoned ID; with a `replaced-by` alias the fix hint names the successor and the finding stays until the reference is updated (D-50, 12 R-SYN-042, 21 R-LC-011, R-LC-022). `HN-R003` is withdrawn (D-51) and merged into this code |
| HN-R006 | deprecated or replaced ID reference (`DEPRECATED-REF`) [0.2] | warn until the due version, error from it | | no | a reference to an ID that is `deprecated` or replaced and still declared (21 R-LC-008); existing references are baselined and counted, a new one is reported as new |
| HN-R005 | shadowing without `ovr` | error / enforce | | no | child redefines an inherited ID (D-12) |
| HN-D001 | duplicate ID | error / enforce | F | no | two doc `def`s of one ID in one scope, including two parallel branches that each declared it, found on the merged tree (20 R-VER-044); the same ID in an ancestor and a descendant is `HN-R005` |
| HN-D003 | tombstoned-ID reuse (`ID-RESERVED`) [0.2] | error / enforce | | no | a `def` of a removed ID, which is reserved forever (21 R-LC-021); a reference to a removed ID with no alias is the error case of `HN-R004` |
| HN-D002 | too many sites | error / enforce | | no | more code `def` sites than the kind's `impl` allows |
| HN-T001 | literal token leak | error / observe | | yes | declared token value used literally in an applicable property context |
| HN-U001 | pattern-rule finding (`PATTERN`) | the rule's `sev` / the rule's `stage` | | per rule (`waivable`) | a match of a rule with `check = "pattern"` (`forbid` regex, R-CHK-005); the message carries the rule ID and its `msg`. The `HN-U` family is reserved for user-declared check types; `leak` rules report `HN-T001` |
| HN-G001 | stale generated file | error / warn | | no | on-disk generated file differs from what `sync` would write; exit 3 at stage warn or enforce |
| HN-G002 | hand-edited generated file | error / enforce | | no | header hash does not match body (D-19) |
| HN-X001 | expired exception | error / enforce | F | no | `ovr` with `exp` before as-of; the waiver is ineffective and the underlying finding returns |
| HN-X002 | unapproved exception | error / warn | | no | `needs-exception` `ovr` with no matching `fairlead approve` record |
| HN-X003 | exception on floor rule | error / enforce | F | no | `ovr` targets a floor rule; inert |
| HN-X004 | stale exception | warn / warn | | no | `ovr` whose violation no longer exists (like `#[expect]`) |
| HN-X005 | inert exception | warn / warn | | no | `ovr` on a `sealed` key with no `[[allow]]`; hint `fairlead new escalation` |
| HN-X006 | exception horizon | error / enforce | | no | `exp` beyond `max-exp` (default 90 days) |
| HN-X007 | waiver budget | warn / warn | | no | scope has more active waivers than `[policies.waiver] budget` |
| HN-X008 | rule waiver rate | info / warn | | no | more than 20% of a rule's findings are waived |
| HN-X009 | ramp stalled | info / warn | | no | a rule has sat in `observe`/`warn` past the nudge points (60, 120 days since `since`) |
| HN-B001 | baseline entry fixed | info / warn | | no | tolerated finding is gone; prune with `sync --baseline` |
| HN-B002 | baseline tampered | error / enforce | F | no | baseline hash mismatch (hand-edited) or contains a floor finding |
| HN-A001 | agent block missing or stale | error / warn | | yes | the marker block in an `[agents] targets` file is absent or differs |
| HN-A002 | CODEOWNERS gap | error / warn | | yes | a managerial folder is not covered by a CODEOWNERS entry |
| HN-C001..C006 | control file | error / enforce | C001,C003,C004 F | no | unparseable, unknown key (warn), non-delegable key (in 0.1 a child control file may not set the keys of the G2 group, [scopes](10-scopes-and-inheritance.md) R-SCOPE-056; `[delegate]` lifts this from 0.2), root problems (incl. `ROOT-DUP`, C004), folder mismatch, pin mismatch |
| HN-C007 | duplicate scope id (`DUP-SCOPE-ID`) | error / enforce | | no | two control files carry one scope id |
| HN-C008 | managerial folder not registered (`SCOPE-UNREGISTERED`) | error / warn | | no | folder missing from its parent's membership shard; `sync` registers it only on request |
| HN-C009 | registered scope folder missing (`SCOPE-VANISHED`) | error / enforce | | no | demote with a tombstone instead of deleting |
| HN-C011 | line class not allowed in this file class (`CLASS-LINE`) | error / enforce | | no | see [files](11-files-and-classes.md) |
| HN-C012 | file extends cycle (`EXT-CYCLE`) | error / enforce | | no | the extends chain must be acyclic |
| HN-N002 | scope eligible for promotion (`SCOPE-ELIGIBLE`) | info | | no | hint only; promotion is never automatic |
| HN-R007 | ambiguous unqualified reference (`AMBIG`) | error / enforce | | no | qualify as `scope:kind.name`; the target must be in `provides` (enforced from 0.2, see HN-R008; 0.1 requires only the qualifier) |
| HN-S002 | unknown or invalid kind or ID | error / enforce | | no | the kind is not declared ("undeclared kind"), or the ID or kind is lexically invalid ("invalid ID"); syntax code, see [syntax](12-directive-syntax.md) R-SYN-034 |
| HN-S016 | incomplete `ovr` (`OVR-INCOMPLETE`) or malformed directive body | error / enforce | | no | an `ovr` missing a required key (a missing `why` only when the target policy is `needs-exception` or `sealed`), or any other malformed body: wrong arity, stray characters, an extent in frontmatter; the directive is inert; syntax code, see [syntax](12-directive-syntax.md) |
| HN-X010 | exception target changed or vanished (`EXC-STALE`) | warn / warn | | no | never auto-deleted; `sync` proposes keep, local def or drop |
| HN-X011 | exception sprawl (`EXC-SPRAWL`) | warn / warn | | no | a rule with too many live exceptions; re-declare or retire it |
| HN-T002 | value differs from contract without `ovr` (`OVR-MISSING`) | error / enforce | | no | token value changed against its base |
| HN-T003 | contract token missing from CSS (`TOK-MISSING`) | error / enforce | | no | contract and stylesheet disagree |
| HN-T004 | undeclared CSS token outside a foreign region (`TOK-UNDECLARED`) | error / enforce | | no | declare it, or move it into `def foreign` |
| HN-T005 | derived token not re-declared in an overriding theme (`TOK-DERIVED`) | error / enforce | | no | a derivation declared in the token contract; Tailwind v4 caveat, see [files](11-files-and-classes.md) R-FILE-035(b) (R-CHK-046) |
| HN-T010 | derivation found by graph (`TOK-DERIVED-GRAPH`) [0.2] | error / enforce | | no | a token whose value is `var(--other)` in the CSS but is not declared as derived in the contract, and is not re-declared in an overriding theme (R-CHK-046) |
| HN-G004 | vendor-owned file changed (`VENDOR-CHANGED`) | warn / warn | | no | accept with `sync --accept-vendor` |
| HN-G005 | edit to a vendor-owned file (`VENDOR-EDIT`) | error / enforce | | no | put variants in a fairlead-owned wrapper unit |
| HN-G006 | foreign anchor lost (`FOREIGN-LOST`) [0.2] | warn / warn | | no | reconstructed from the ledger where possible; ships in 0.2 |
| HN-F001 | secret-like value | error / enforce | F | no | key/token pattern in a managerial file, a directive value, or the `sum`/`out`/`why`/`alt` fields of records ([retention](33-retention-and-compaction.md) R-RET-006); declare secrets by env-var name only |
| HN-P001 | path hygiene | error / enforce | F | no | reserved Windows names, case-only collisions, non-NFC names |
| HN-C013 | home mismatch (`HOME-MISMATCH`) | error / enforce | | no | `home=` names a scope that is not an ancestor-or-self of an impl site |
| HN-C014 | scope folder beside a sibling module file (`SCOPE-AMBIG`) [0.2] | error / enforce | | no | no `own` convention declared for it |
| HN-R008 | item not provided (`NOT-PROVIDED`) [0.2] | error / enforce | | no | a qualified sideways reference to an item that is not in the target scope's `provides` |
| HN-X014 | overlapping exceptions (`DUP-OVR`) | error / enforce | | no | two `ovr`s of one target in one scope; both are inert |
| HN-X015 | unsigned authority record (`H-AUTH-UNSIGNED`) [0.2] | warn / warn; error per scope | | no | only with `authority.signed-records = true` ([lifecycle](21-environments-maturity-lifecycle.md) R-LC-040) |
| HN-X016 | exception beyond the version cap (`EXC-CAP`) [0.2] | error / enforce | | no | `until` above the cap version ([versions](20-versions-and-seal.md) R-VER-075); the date cap of 0.1 is `HN-X006` |
| HN-N001, HN-I001, HN-I002 | notices | info | | no | nothing verified; file skipped; unknown comment syntax |
| HN-I003 | non-UTF-8 file skipped (`ENCODING-SKIP`) | warn / warn | | no | the file is not scanned and is listed; never silently green |

The rows HN-C007 to HN-G006 allocate codes to the mnemonic diagnostics of chapters 10 and 11; the numbering is frozen (R-CHK-042).

- **R-CHK-042 [0.1]** Diagnostic codes are `HN-<letter><nnn>`, stable and never reused or renumbered. The `HN-` prefix is fixed and does not follow the `marker` key, so machine ids survive a rename (D-30, D-47). The default severity and stage of every code is the one in the tables of this chapter; the mnemonic names (`DUP-SCOPE-ID`, `H-VER-GAP`, ...) are display aliases only. The numbering of every row in this chapter is frozen from 0.1.

Later (not 0.1): HN-A003 instruction-like text in authored fields (heuristic, info) [0.2]; HN-A004 and HN-X012 [0.1] and HN-X013 [0.2] (owner identity and approver, R-CHK-053, R-CHK-054); HN-T008 and HN-T009 (cascade layers, below) [0.2]; HN-H024 unverifiable reference [0.3]; HN-R004, HN-R006 and HN-D003 (references to retired IDs and ID reuse, R-CHK-059) [0.2]; HN-T010 [0.2]; HN-H025 [0.3]; HN-E001 env-gate violation [0.2]; HN-K001 missing summary at seal [0.3] ([retention](33-retention-and-compaction.md) R-RET-022); HN-G003 ledger checkpoint continuity after a union merge [0.2] and ledger hash chain [0.3] (23 R-GIT-015); reachability and variation verdicts: see the rows above.

### Codes for chapters 20-23, 30, 31, 33
These mnemonic diagnostics are cited by name in the chapters named. Each has one HN code here; the numbering is frozen (R-CHK-042). Severity is the default and the rollout ramp applies as for any rule. None is a floor rule; waiver rules are those of the citing chapter.

| Code | Name | Default sev / stage | Milestone | Rule |
|---|---|---|---|---|
| HN-V001 | H-VER-COLLISION | error / enforce | 0.2 | 20 R-VER-055; 23 R-GIT-030 |
| HN-V002 | H-VER-GAP | warn / warn | 0.2 | 20 R-VER-001 |
| HN-V003 | H-SEAL-TAMPER | error / enforce | 0.2 | 20 R-VER-054 |
| HN-V004 | H-SYNC-ANCESTOR | info, then warn, then error at seal | 0.2 | 20 R-VER-020 |
| HN-V005 | H-TX-INCOMPLETE | error / enforce | 0.2 | 20 R-VER-051; 23 R-GIT-051 |
| HN-V006 | H-ID-COLLISION | error / enforce | 0.2 | 20 R-VER-042; 23 edge 29 |
| HN-V007 | H-ORD-DUP | error / enforce | 0.2 | 20 R-VER-042 |
| HN-V008 | H-DOC-BROAD | warn / warn | 0.2 | 20 R-VER-037 |
| HN-V009 | H-DOC-EMPTY | info / warn | 0.2 | 20 edge 12 (R-VER-031) |
| HN-V010 | H-GIT-SHALLOW | warn / warn | 0.2 | 23 R-GIT-040 |
| HN-V011 | H-UPG-PARTIAL | error / enforce | 0.2 | 22 R-EDN-032 |
| HN-V012 | H-REQ-CHANGED | info / warn | 0.3 | 23 edge 33 |
| HN-V013 | H-GIT-SPARSE | warn / warn | 0.1 | 23 R-GIT-007 |
| HN-V014 | H-UPG-BOUNDARY | info / warn | 0.3 | 22 R-EDN-045 |
| HN-V015 | H-HOTFIX-DEBT | warn while open, error after the deadline | 0.2 | 20 R-VER-047 |
| HN-V016 | H-ESC-STALE | warn at seal; error after `version.escalation-grace` seals or when `blocking` | 0.2 | 20 R-VER-058 |
| HN-E002 | H-ENV-UNDECLARED | warn / warn | 0.2 | 21 R-ENV-031 |
| HN-E003 | H-ENV-UNBOUND | info / warn | 0.2 | 21 R-ENV-033 |
| HN-E004 | H-VOC-UNKNOWN | error / enforce | 0.2 | 21 R-ENV-003 |
| HN-E005 | H-MAT-MISSING | warn / warn; error at enforce | 0.2 | 21 R-ENV-005 |
| HN-E006 | H-MAT-ORDER | error / enforce | 0.2 | 21 R-ENV-011 |
| HN-E007 | H-MAT-REGRESS | warn / warn | 0.2 | 21 R-ENV-012 |
| HN-L001 | H-LC-STALE-PLAN | warn / warn | 0.2 | 21 R-LC-006 |
| HN-L002 | H-GRAVE-MATCH | warn / warn; error per scope for `rejected` | 0.2 | 21 R-LC-033 |
| HN-H001 | hier.single-container | error / enforce | 0.3 | 30 R-HIER-010 |
| HN-H002 | hier.parent-cycle | error / enforce | 0.3 | 30 R-HIER-011 |
| HN-H003 | hier.orphan-file | error / enforce | 0.3 | 30 R-HIER-051 |
| HN-H004 | hier.orphan-unit | error / enforce | 0.3 | 30 R-HIER-050, R-HIER-102 |
| HN-H005 | hier.hollow | warn / warn | 0.3 | 30 R-HIER-052 |
| HN-H006 | hier.sym-ambiguous | warn / warn | 0.3 | 30 R-HIER-030 |
| HN-H007 | hier.rank | error / enforce | 0.3 | 30 R-HIER-002, R-HIER-020 |
| HN-H008 | hier.dag | error / enforce | 0.3 | 30 R-HIER-021 |
| HN-H009 | hier.cycle | error / enforce | 0.3 | 30 R-HIER-022 |
| HN-H010 | hier.visibility | error / enforce | 0.3 | 30 R-HIER-026; 31 R-VAR-010 |
| HN-H011 | hier.uses-noevidence | warn / warn | 0.3 | 30 R-HIER-031 |
| HN-H012 | hier.uses-undeclared | warn / warn | 0.3 | 30 R-HIER-031 |
| HN-H013 | hier.promote | info / observe | 0.3 | 30 R-HIER-071 |
| HN-H014 | hier.demote | info / observe | 0.3 | 30 R-HIER-071 |
| HN-H015 | hier.split | info / observe | 0.3 | 30 R-HIER-071 |
| HN-H016 | hier.merge | info / observe | 0.3 | 30 R-HIER-071 |
| HN-H017 | hier.maturity | warn / warn | 0.3 | 30 R-HIER-073 |
| HN-H018 | hier.stale | warn / warn | 0.3 | 30 R-HIER-041 |
| HN-H019 | hier.dormant | info / warn | 0.3 | 30 R-HIER-043 |
| HN-H020 | hier.forgotten | warn / warn | 0.3 | 30 R-HIER-043 |
| HN-H021 | hier.bridge-end-missing | error / enforce | 0.3 | 30 R-HIER-062 |
| HN-H022 | hier.bridge-pin | error / enforce | 0.3 | 30 R-HIER-063 |
| HN-H023 | hier.double-binding | error / enforce | 0.3 | 30 R-HIER-014 |
| HN-H025 | hier.bridge-wire | error / enforce | 0.3 | 30 R-HIER-063 (the wire literal differs between the two ends; a missing end is HN-H021) |
| HN-H024 | hier.unverifiable | warn / ramp (R-CHK-043) | 0.3 | 30 R-HIER-063 (dynamic wire name) |
| HN-D004 | doc.undocumented-unit | warn / ramp (D-15) | 0.3 | 30 R-HIER-090 |
| HN-D005 | doc.dangling | warn / ramp (D-15) | 0.3 | 30 R-HIER-090 |
| HN-D006 | doc.uncovered | warn / ramp (D-15) | 0.2 (paths); 0.3 (units) | 20 R-VER-032, R-VER-034; 30 R-HIER-091 |
| HN-W001 | var.request-stale | warn / warn | 0.3 | 31 R-VAR-032 |
| HN-W002 | var.adapter-bypass | error / enforce | 0.3 | 31 R-VAR-072 |
| HN-W003 | var.fork-suspect | warn / observe | 0.3 | 31 R-VAR-081 (ships off, R-VAR-084; name heuristics only, no similarity) |
| HN-W004 | var.axis-suggest | info / observe | 0.3 | 31 R-VAR-051 |
| HN-T006 | var.token-near | info / warn | 0.3 | 31 R-VAR-095 (R-CHK-049) |
| HN-T007 | var.token-unresolved | info / observe | 0.3 | 31 R-VAR-098 |
| HN-T008 | css.layer-order (`LAYER-ORDER`) | error / enforce | 0.2 | 11 R-FILE-035(c); R-CHK-047 |
| HN-T009 | css.bare-override (`BARE-OVERRIDE`) | error / enforce | 0.2 | 11 R-FILE-035(c); R-CHK-047 |
| HN-A004 | owner not covered by CODEOWNERS (`OWNER-UNCOVERED`) | warn / warn | 0.1 | R-CHK-053 |
| HN-M001 | GLOB-DEAD | warn / warn | 0.1 | 32 R-PORT-002 |
| HN-M002 | SCOPE-REPARENTED | error / enforce when the new view loosens; warn when it only tightens | 0.1 | 32 R-PORT-003 |
| HN-M003 | SCOPE-ID-CHANGED | error / enforce | 0.1 | 32 R-PORT-008 |
| HN-M004 | DUP-CLAIM | error / enforce | 0.3 | 32 R-PORT-025 |
| HN-M005 | UNSATISFIED | error / enforce | 0.3 | 32 R-PORT-041, R-PORT-047 |
| HN-M006 | NOT-CHECKED | warn / warn | 0.3 | 32 R-PORT-052 |
| HN-M007 | DANGLING | error / enforce | 0.2 | 32 R-PORT-010, R-PORT-045 |
| HN-M008 | MOVE-INCOMPLETE | error / enforce | 0.2 | 32 R-PORT-005 |
| HN-M009 | PKG-HINT | info / warn | 0.3 | 32 R-PORT-054 |
| HN-M010 | PROJECTION-OUTSIDE | error / enforce | 0.3 | 32 R-PORT-036 |
| HN-M011 | DETACHED | info / warn | 0.2 | 32 R-PORT-013 |
| HN-M012 | LOCK-STALE | warn / warn | 0.3 | 32 R-PORT-046 |
| HN-X012 | unknown owner on `ovr` (`OWNER-UNKNOWN`) | error / enforce | 0.1 | R-CHK-053; 12 R-SYN-011 |
| HN-X013 | agent approver on a human-only approval (`APPROVER-AGENT`) | error / enforce | 0.2 | R-CHK-054; 40 R-AGT-029 |

Aliases, not new codes:
- `H-GEN-STALE` (cited in [versions](20-versions-and-seal.md) edge 17 and [git](23-git-integration.md) R-GIT-014) is HN-G002, a hand-edited generated file, not HN-G001.
- `var.token-hardcoded` ([variation](31-variation-protocol.md) R-VAR-091) is HN-T001; chapter 31 adds the property-context rules.
- `var.vendor-changed` ([variation](31-variation-protocol.md) R-VAR-073) is HN-G004; its severity is the one of the HN-G004 row (warn), the alias has none of its own (R-CHK-050).

- **R-CHK-043 [0.3]** A reference that cannot be verified statically (a dynamic `invoke(name)`, a computed wire name) is reported as `HN-H024` with severity `warn` and never guessed by regex. A scope MAY raise it to `error` through the stage ramp (R-CHK-033); raising it is a tightening and free.

### Literal token leaks (HN-T001)
- **R-CHK-017 [0.1]** Detection is declared value plus property context, never raw-value matching (D-21). A token `def` opts in with `applies=<properties>`; without it the token is never leak-checked ([files](11-files-and-classes.md) R-FILE-039).
- **R-CHK-018 [0.1]** The declared value is `value=` on the doc `def`, or, for `sym=--name` tokens, the value found for that custom property at the code `def` site (authored CSS is the source of truth, D-04).
- **R-CHK-019 [0.1]** Property context is recognised per file type, textually: CSS `prop: value`; JS/TS object keys (`backgroundColor` mapped to `background-color`) and JSX attributes; Tailwind arbitrary values (`bg-[#2563eb]`, prefix mapped to its property); SVG `fill`/`stroke`. A literal with no recognised applicable property is NOT reported (a hex in a comment is not a leak).
- **R-CHK-020 [0.1]** Comparison normalises: hex 3/4/6/8 digits to lower 6/8, `rgb()`/`rgba()` integer forms to hex, lengths to a canonical number+unit, whitespace collapsed; other colour spaces compare as normalised strings. A value shorter than `min-len` (default 4 characters) is never checked, so `0`, `1`, `100%` cannot flood reports.
- **R-CHK-021 [0.1]** The def site's own value, `def foreign` regions, `exclude`d globs and `skip`ped files are exempt. Computed names (`bg-${tone}`) are not analysable and are not reported.
- **R-CHK-022 [0.1]** The fix hint names the exact replacement (`var(--primary)`, `bg-primary`) when the kind provides a projection pattern, else the token ID.

- **R-CHK-046 [0.1]** The derived-token graph (a token whose value is `var(--other)`) is derived from the token contract once it ships. `HN-T005` checks from 0.1 the derivations that the token contract declares (a derived token must be re-declared in every theme that overrides its base). Graph-based detection of derivations the contract does not declare is the separate code `HN-T010`, 0.2. The other Tailwind v4 caveat checks ([files](11-files-and-classes.md) R-FILE-035 (a), (c) to (f)) and `HN-G006` ship in 0.2; `HN-T002` is 0.1.
- **R-CHK-047 [0.2]** Cascade layers and import order are override semantics. `check` MUST validate the declared `@layer` order statement and the stylesheet import order (`HN-T008`), and MUST forbid a bare override, that is an unlayered or out-of-order child rule that overrides a token or utility without an `ovr` (`HN-T009`). "Later file wins" is never assumed (R-FILE-035(c)).
- **R-CHK-048 [0.2]** Class-name references to tokens that are computed (`bg-${tone}`) are made analysable only by a declared safelist: a token `def` MAY carry `classes=<pattern,...>`, and a class matching the list counts as a use of the token and is leak-checked as a literal in its property context. Without a safelist computed names stay unreported (R-CHK-021).
- **R-CHK-049 [0.1]** Literal checks are exact after normalisation (R-CHK-020); near colours and `rem` against `px` equivalence are not checked in 0.1. The property-context rules R-CHK-017 to R-CHK-022 are 0.1 (D-43). The richer variation rules of [variation](31-variation-protocol.md) R-VAR-090 to R-VAR-097, including `HN-T006`, stay [0.3] and only add to these.
- **R-CHK-050 [0.1]** A changed vendor-owned file (`HN-G004`) has severity `warn`; the alias `var.vendor-changed` has the same severity.

### Stale and generated files
- **R-CHK-023 [0.1]** `check` computes in memory what `sync` would write and compares bytes. Differences are `HN-G001` with the first three differing lines; the fix is `fairlead sync`. Ancestor roll-ups are written only at `seal` [0.2]; in 0.1 the generated files are the baseline and scope index shards. Generated-file and baseline headers carry the edition only (`# scope <id> edition=<e> body=<hash>`, `# baseline edition=<e> body=<hash>`); the binary version is never stamped into a generated file, so a binary upgrade within an edition never makes a file stale (D-55).
- **R-CHK-024 [0.1]** Every generated file carries a header hash of its body (blake3, D-50; sha256 is not used anywhere). A mismatch is `HN-G002`; `sync` refuses to overwrite it without `--force` (see [cli](15-cli.md)). A file containing git conflict markers is treated as regenerable.

### Exceptions, waivers, sprawl
- **R-CHK-025 [0.1]** An `ovr` is *effective* iff it is well formed, its target key policy permits it (R-SYN-011), it has not expired at as-of, and (when `[policies.waiver] approval = "required"`, the default for `needs-exception`) a matching approval exists. Otherwise the underlying finding is reported with state `waiver-ineffective`.
- **R-CHK-026 [0.1]** An approval binds to a fingerprint of (target ID, scope id, `why`, `owner`, `exp`), not to the file path, so moving a file does not invalidate it and re-dating a waiver does.
- **R-CHK-027 [0.1]** A second approval of the same (target, scope) is a renewal and is counted. Renewals are shown in the summary and in `explain`; the third renewal SHOULD require the root owner (`[policies.waiver] max-renewals`, default 2).
- **R-CHK-028 [0.1]** Every run MUST print the waiver summary: active count per scope, how many expire within 30 days, budget use, and the list of *relaxations* in force (rules whose `sev`/`stage` is below the edition default, every `[[allow]]`). Erosion is visible on every run, not found in an audit.
- **R-CHK-029 [0.1]** Waiver sprawl is reported, never auto-fixed: budget (`HN-X007`), waiver rate per rule (`HN-X008`), renewals, oldest active waiver. Nothing is auto-deleted or auto-renewed.

### Baselines and the observe, warn, enforce ramp
- **R-CHK-030 [0.1]** `init` and `sync --baseline` record existing findings in the baseline (default `fairlead/gen/baseline.txt`, one terse `<fp> code=<code> path=<path>` line each, sorted, LF, header `# baseline edition=<e> body=<hash>` with the edition only, D-55). `check` never writes it (D-38). Baselined findings are shown only as a count and never fail the run.
- **R-CHK-031 [0.1]** The fingerprint is blake3 over (code, rule id, scope id, repo-relative path, whitespace-collapsed offending text, occurrence index in the file), truncated to 12 base32 characters. Editing elsewhere in the file does not change it.
- **R-CHK-032 [0.1]** The baseline only shrinks. `sync --baseline` on an existing baseline prunes fixed entries and refuses to add; adding needs `sync --baseline --accept-new --why "<text>"`, which marks each added line `acc=<as-of>` and counts them in the summary. Floor findings are never accepted, and findings of the authority family (`HN-X*`, `HN-A*`, `HN-C*`) are accepted only when an `approve` record covers the added lines (R-CHK-052).
- **R-CHK-033 [0.1]** A rule's `stage` ratchets observe, warn, enforce. Raising it is free anywhere; lowering it is a relaxation (R-CHK-028) and in a descendant needs `[[allow]]`. `since=` records when the stage was set; `HN-X009` nudges at 60 and 120 days.
- **R-CHK-034 [0.2]** Docs-first coverage (each changed file covered by a change doc) ships `observe`, then `warn`, per-scope opt-in `enforce` (D-15). It needs the git diff and is not a 0.1 check.
- **R-CHK-044 [0.2]** Move-only and rename-only changes are not exempt from coverage. `move` writes a generated restructure change doc whose `covers` lists the moved paths, and that doc covers them (R-VER-032). An exempt glob is not needed for them.
- **R-CHK-045 [0.2]** The coverage unit is the file: a changed file is covered when it matches the `covers` globs of at least one non-abandoned doc, so a shared hotspot file claimed by many docs is covered by any one of them. IDs in `covers` are recorded and validated but do not count toward coverage in 0.2; ID-level coverage and the unit-level orphan check arrive in 0.3 (`HN-D006` for units).

### Diagnostic format
- **R-CHK-035 [0.1]** Every diagnostic carries: stable `code`; `severity`; `stage`; `path:line:col`; the related ID or rule; a message; the **intent excerpt** (the rule's `why`, <= 80 chars, shown as data in quotes); exactly one **fix hint** (<= 100 chars, imperative, naming a fairlead command or concrete edit); the fingerprint (JSON/SARIF only); and state (`new`, `baselined`, `waived`, `waiver-ineffective`).
- **R-CHK-036 [0.1]** Formats: `--format human` (default on a TTY), `terse` (default when piped), `json`, `sarif`. `terse` is one line per finding. `json` lists every finding unless `--max N` is given, and then reports `omitted` (R-CHK-038). `json` is a stable schema with `fairlead`, `edition`, `as_of`, `findings[]`, `summary`. `sarif` is 2.1.0: `ruleId` = code, level (error to `error`, warn to `warning`, info to `note`), `partialFingerprints` = fingerprint, `suppressions` for baselined (`external`) and waived (`inSource`), intent in `properties`; `sarif` is complete unless `--max N` is given.
- **R-CHK-037 [0.1]** Text from the repository (`why`, notes, names) is emitted as data: control characters, ANSI escapes and bidi overrides stripped, length-capped, never executed or interpreted (see [trust](40-agent-integration-and-trust.md)).
- **R-CHK-038 [0.1]** Human and terse output show at most 20 display entries after same-file collapse (repeated findings of one code in one file are one entry: `Panel.tsx:44,81,90`), ordered floor first, then by severity, then path. The header line and the summary line with total counts come first. Truncation is explicit and counts findings: `+37 more -> fairlead check --rule HN-T001 --max 0`. Info findings take part in the cap; there is no `-v` flag in 0.1, and `--quiet` hides info findings except `HN-N001`. JSON and SARIF list every finding unless `--max N` is given explicitly, and then report `omitted`; a truncated machine list is never the default (R-CHK-040).
- **R-CHK-039 [0.1]** `--for-agent` implies `terse`, no excerpts, intent trimmed to 60 chars, hard cap 2 KB; if the cap bites it keeps floor and error findings and prints the omitted count.
- **R-CHK-040 [0.1]** Exit codes are defined only in [CLI](15-cli.md) R-CLI-003. For `check`: 1 at least one non-baselined, non-waived `error` at stage `enforce` (or any `warn` finding with `--deny-warnings`); 3 a stale generated file (`HN-G001`) at stage `warn` or `enforce`; 2 an unresolved scope, unparseable control file or syntax failure; 4 refusal (pin mismatch); 5 an internal error or a result that cannot be determined, including a scope absent from the tree or a sparse checkout (R-CLI-003, D-44, D-56).

### Token cost of diagnostics
Estimated as ceil(bytes / 4); to be re-measured with the target model's tokenizer.

| Format | Bytes per finding | ~Tokens | 20 findings | Intended reader |
|---|---|---|---|---|
| terse / `--for-agent` | 140-220 | 35-55 | 700-1,100 | agents, hooks |
| human | 300-450 | 75-110 | 1,500-2,200 | people |
| json | 500-700 | 125-175 | 2,500-3,500 | tools |
| sarif | 900-1,200 | 225-300 | 4,500-6,000 | CI upload only |

- **R-CHK-041 [0.1]** Agents SHOULD consume `terse`/`--for-agent` only; JSON and SARIF MUST NOT be piped into agent context by any fairlead-installed hook.

### Precision, authority and ramp policy
- **R-CHK-051 [0.1]** The 0.1 exit gate for precision is at most 2% of 200 blind-reviewed findings judged false, and zero false dangling-reference reports by test ([roadmap](60-roadmap.md) R-ROAD-003).
- **R-CHK-052 [0.1]** Growing the baseline needs `sync --baseline --accept-new --why`, reviewed through CODEOWNERS on `fairlead/gen/`. For findings of the authority family (`HN-X*`, `HN-A*`, `HN-C*`) it additionally needs an `approve` record by a handle in `owners`.
- **R-CHK-053 [0.1]** The `owner=` of an `ovr` MUST name a handle in `owners` ([syntax](12-directive-syntax.md) R-SYN-011). In 0.1 `owners` is a root-only list in the root control file; from 0.2 a declaring scope MAY add owners for its own subtree, and the handles valid for an `ovr` are those of the declaring scope and its ancestors; free text is `HN-X012` and the waiver is ineffective. Each handle in `owners` SHOULD be covered by a CODEOWNERS entry on the managerial folder; a handle with no entry is `HN-A004` (warn). Verification against CODEOWNERS is textual and offline.
- **R-CHK-054 [0.2]** An `owners` entry MAY be declared `kind = "agent"`. An approval, `[[allow]]` grant or baseline acceptance by an agent handle on a floor-adjacent key (a rule's `sev`/`stage`, `[policies]`, the authority-family codes) is `HN-X013` and ineffective; a human handle is required there.
- **R-CHK-055 [0.1]** `HN-X002` (unapproved exception) defaults to `error` severity at stage `warn`. A waiver without an approval is already ineffective (R-CHK-025), so the underlying finding fails the run; `HN-X002` explains why. A root MAY raise its stage to `enforce`.
- **R-CHK-056 [0.1]** On the first scan by `init`, `HN-R001` and `HN-D001` default to stage `enforce`, with existing findings recorded in the baseline (R-CHK-030). There is no grace period by date.


- **R-CHK-057 [0.1]** A scope that is absent from the working tree (sparse checkout, partial clone) is reported `HN-V013` and `check` exits 5, cannot-determine (D-56, 23 R-GIT-007, [CLI](15-cli.md) R-CLI-003). An unresolved scope (an unparseable control file) stays exit 2. A non-UTF-8 file is listed under `HN-I003` and skipped.
- **R-CHK-058 [0.2]** `HN-D001` is also computed on the merged tree by `check --ci` ([CLI](15-cli.md) flags); there are no claim or lease files (20 R-VER-044). `check --ci` runs `git verify-commit` only when `authority.signed-records = true` (R-LC-040), through the git executable of D-48; otherwise `check` never runs a subprocess.

- **R-CHK-059 [0.2]** References to retired IDs are reported by three distinct codes: `HN-D003` (a `def` of a tombstoned ID, error), `HN-R004` (a reference through a tombstone alias, warn, or to a removed ID with no alias, error), and `HN-R006` (a reference to a deprecated or replaced ID that is still declared: warn until its due version, error from the due version on; 21 R-LC-008). Each names the successor when one is recorded. `HN-R003` is not used.
- **R-CHK-060 [0.1]** Findings of a user rule with `check = "pattern"` are `HN-U001`. Their severity and stage come from the rule (R-CHK-005, R-CHK-009 default `observe`), they are waivable only if the rule is `waivable`, and the fingerprint includes the rule ID (R-CHK-031).

## Examples
`terse` (one line per finding; fenced text):
```text
fairlead check 0.1.0 edition=1 as-of=2026-10-08 scopes=3 files=1204 rules=7
errors 2 warn 1 info 0 | baselined 412 | waived 3 | observe 9 | accepted 0
HN-T001 error apps/web/src/Panel.tsx:44:18 literal #2563eb is tok.primary (color) | why: "colours come from tokens" | fix: use var(--primary)
HN-R001 error apps/web/src/Sync.tsx:7:3 unknown id cmp.sync-buton | fix: did you mean cmp.sync-button
HN-X004 warn apps/web/src/Legacy.tsx:12 ovr rule.no-raw-hex has no violation left | fix: delete the ovr line
waivers: web 3/25 (1 expires <=30d) | relaxations: rules.no-raw-hex stage=observe (default warn), allow web rules.no-raw-hex.stage exp 2027-03-31
```
`human`:
```text
error HN-T001  apps/web/src/Panel.tsx:44:18
  color: "#2563eb"
         ^^^^^^^^ duplicates tok.primary (applies: color)
  intent: rule.no-raw-hex "colours come from tokens" (web)
  fix:    use var(--primary)     explain: fairlead explain HN-T001
```
A waiver and its approval:
```fairlead
// fairlead: ovr rule.no-raw-hex why="legacy email template" owner=@design exp=2026-12-31
```
```toml
# fairlead/approvals.toml  (written by `fairlead approve`, reviewed via CODEOWNERS)
[[approval]]
target = "rule.no-raw-hex"
scope = "web"
fp = "k3m9q2x7a1bz"
approver = "@design"
date = "2026-10-08"
renewal = 0
```

## Edge cases
| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | Repo with no declarations | `HN-N001` "nothing verified", exit 0 (1 with `--require-declarations`) | R-CHK-003 |
| 2 | Agent lowers a rule from `error` to `off`/`observe` to go green | Allowed only at the declaring scope by its owners; printed under `relaxations:` every run; in a descendant it needs `[[allow]]` | R-CHK-028, R-CHK-033 |
| 3 | Agent runs `sync --baseline` to hide new findings | Refuses to add; `sync --baseline --accept-new --why` marks and counts; floor findings never accepted | R-CHK-032 |
| 4 | Baseline hand-edited | Hash mismatch: `HN-B002`, floor | R-CHK-008 |
| 5 | Agent deletes a human `ovr` during cleanup | Violation resurfaces as its own finding; no silent loss | R-CHK-025 |
| 6 | Agent renews every waiver with a new date | New fingerprint needs a new approval; renewals counted; third renewal needs the root owner | R-CHK-026, R-CHK-027 |
| 7 | Waiver expires at midnight UTC between local run and CI | Output differs only by `as-of`; CI passes the commit date | R-CHK-014 |
| 8 | `ovr` for a floor rule (secrets, duplicate IDs) | `HN-X003`, inert | R-CHK-008 |
| 9 | Declared hex appears in a comment, test fixture, brand asset | Comment: no property context, not reported. Fixture/brand: `skip`/`exclude` or an approved `ovr` | R-CHK-019, R-CHK-021 |
| 10 | Tailwind computed class `bg-${tone}`; `var()` consumers | Not analysable, not reported; unused-ID is opt-in per kind because CSS `var()` and utilities hide uses | R-CHK-021, R-CHK-005 |
| 11 | Secret pasted into a doc or token `value=` | `HN-F001`, never baselined or waived; history purge is a human action | R-CHK-008 |
| 12 | Legacy repo yields 4,000 findings on `init` | Baselined; only new findings fail | R-CHK-030 |
| 13 | 50 findings in one run | 20 shown, totals first, explicit `+N more`; `--for-agent` 2 KB cap | R-CHK-038, R-CHK-039 |
| 14 | Two agents add the same ID on parallel branches | Invisible on either branch; `HN-D001` on the merged tree, so CI MUST check the merge result | R-CHK-001 |
| 15 | Child control file unparseable | That subtree unresolved, exit 2; sibling scopes still checked and reported | R-SYN-033 |
| 16 | Rule with more than 20% of hits waived | `HN-X008` review prompt; the waiver is not revoked | R-CHK-010 |
| 17 | Path argument outside the root | exit 2 | R-CHK-001 |
| 18 | Same finding in 3 scopes' globs | Reported once, at the deepest declaring scope, fingerprint independent of scope chain order | R-CHK-031 |
| 19 | `ovr owner=someone` where the handle is not in `owners` | `HN-X012`, waiver ineffective | R-CHK-053 |
| 20 | Dynamic `invoke(name)` in a bridge | `HN-H024` warn; never guessed [0.3] | R-CHK-043 |
| 21 | Rename-only commit under docs-first coverage | Covered by the generated restructure doc of `move` [0.2] | R-CHK-044 |
| 22 | Agent handle approves a waiver on a floor-adjacent key | `HN-X013`, ineffective [0.2] | R-CHK-054 |

## Resolved questions
- Q13-1 (resolved 2026-10-09): `--as-of` defaults to UTC today and CI passes the commit date; see R-CHK-014
- Q13-2 (resolved 2026-10-09): baseline growth by `sync --baseline --accept-new --why` only, plus an `approve` record for the authority family; see R-CHK-032, R-CHK-052
- Q13-3 (resolved 2026-10-09): exact-after-normalisation only in 0.1; semantic fit stays out of scope; see R-CHK-049
- Q13-4 (resolved 2026-10-09): `HN-R001`/`HN-D001` start at `enforce` with a baseline; see R-CHK-056
- Q13-5 (resolved 2026-10-09): the `HN-` prefix is fixed and does not follow `marker`; see R-CHK-042
- Q13-6 (resolved 2026-10-09): `HN-X002` is `error` at stage `warn`; the unapproved waiver is already ineffective; see R-CHK-055
- Q13-7 (resolved 2026-10-09): the precision target is the 0.1 exit gate; see R-CHK-051
- Q13-8 (resolved 2026-10-09): the provisional codes are confirmed and the numbering is frozen; see R-CHK-042
- Q13-9 (resolved 2026-10-09): the property-context rules are 0.1 (D-43); the richer variation rules and `HN-T006` stay 0.3; see R-CHK-049
- Q13-10 (resolved 2026-10-09): `HN-G004` stays `warn`; chapter 31 follows; see R-CHK-050
- Q-CHK-1 (resolved 2026-10-09): `HN-` codes with stable ids; see R-CHK-042
- Q-CHK-3 (resolved 2026-10-09): unverifiable references are `warn` with the scope ramp; see R-CHK-043
- Q-CHK-4 (resolved 2026-10-09): the generated restructure doc of `move` covers move-only changes; see R-CHK-044
- Q-CHK-5 (resolved 2026-10-09): coverage by files in 0.2, IDs in 0.3; see R-CHK-045
- Q-CHK-6 (resolved 2026-10-09): zero declarations print `HN-N001`, exit 0, never a silent green; see R-CHK-003
- Q-CHK-7 (resolved 2026-10-09): the derived-token graph is derived once the token contract ships; see R-CHK-046
- Q-CHK-8 (resolved 2026-10-09): order check plus layer model, bare cascade overrides forbidden; see R-CHK-047
- Q-CHK-9 (resolved 2026-10-09): a declared safelist makes computed class names analysable; see R-CHK-048
