# P8: governed config editing and agent mode

> Outline "New surface" (`fairlead config`, `[agent-mode]`) and "Agent mode"; owner answer Q4-2 (`check --ci` does not fail on agent-mode approvals by default). [40-agent-integration-and-trust.md](../../40-agent-integration-and-trust.md) R-AGT-026 (the approve terminal gate), [10-scopes-and-inheritance.md](../../10-scopes-and-inheritance.md) (policies, delegation). Decisions D-02, D-03, D-34, D-62.

## Goal

Control files are edited through a governed command, so a change is a plan with its policy judged before it is written. Teams that run fairlead with agents only can turn on a bounded agent mode instead of a human approving each exception.

## Design

- **`fairlead config <op>`.** Operations: `rule <name> glob=... | stage=... | sev=... | retire`, `pin raise <key> <version>`, `table add <table> key=value...`, `retain <kind> ttl=...`. Each is a plan (diff of the control file, `toml_edit`, comments kept); a loosening (a lower severity or stage, a wider glob for an allow, a retired rule, a lowered pin) needs an `[[approval]]` like a loosening move (0.3); a tightening applies with `--yes`.
- **Agent mode.** `[agent-mode] scopes = [...] until = "<version>" approver = "agent-mode"` in the root control file, valid only with an `[[approval]]` of that table by a human owner (the last human act it needs). Inside its window, `fairlead approve --as agent-mode` works without the terminal gate for `ovr`s, answers (R-VAR-043) and loosening plans of the named scopes. Floor rules never yield; sealed keys outside the named scopes still need an owner; the window ends at `until` or when the table is removed.
- **Visibility.** Every agent-mode approval is recorded with `approver = "agent-mode"`; `seal` lists them in the version file, `context` lists those of the scope, `check --ci` lists them as `HN-Y001` (info by default; a setting `[agent-mode] fail-on-ci = true` makes them errors on protected branches, Q4-2). An approval attempted outside the window is `HN-Y002` and refused.

## Tasks

**T1. `fairlead config` verb.** Parsing of the operations, the plan, `--dry-run`, `--yes`. trycmd: `config-rule-stage`, `config-plan-only`.

**T2. Loosening judgement.** Reuse the 0.2 relaxation logic (pins, stages) and the 0.3 loosening move approval (`approve move-<fp>` becomes `approve plan-<fp>` for config plans, with the old name kept as an alias). Tests: `r_config_001__a_loosening_needs_an_approval`, `r_config_001__a_tightening_applies`.

**T3. Edits keep the file.** `toml_edit` keeps comments, order and formatting; a round-trip of every golden control file through a no-op edit is byte-identical. Test: `r_config_002__a_no_op_edit_is_byte_identical`.

**T4. `[agent-mode]` parsing and its owner approval.** The table, its approval fingerprint (the table's canonical text), and the check that it is approved by a human handle (R-AGT-028: not an agent handle). Test: `r_agent_001__agent_mode_needs_an_owner_approval`.

**T5. Approvals inside the window.** `fairlead approve --as agent-mode <target>` without the terminal gate, only for targets in the named scopes and below `until`; floor rules and foreign sealed keys refused (`HN-Y002`). Tests: `r_agent_002__an_ovr_in_scope_is_approved`, `r_agent_002__a_floor_rule_never_yields`, `r_agent_002__outside_the_window_is_refused`.

**T6. Listing (`HN-Y001`).** `seal` writes `agent-approvals = [...]` into the version file; `context` adds a line per approval of the scope; `check --ci` reports each as `HN-Y001`. Test: `r_agent_003__every_agent_approval_is_listed`.

**T7. Turning it off.** Removing the table (itself a governed edit, no approval needed: it tightens) ends the window; `fairlead approve --revoke-agent-mode` revokes every agent-mode approval at once, and their exceptions become unapproved findings again. Test: `r_agent_004__revoking_restores_the_findings`.

**T8. The bounds test.** Exit criterion 5: a scripted session that tries every bound (scope, version, floor rule, sealed key, revocation). Done when it passes.
