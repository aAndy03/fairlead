# P9: the 0.3 gaps, exit and the v1 API freeze

> [../1.0-gaps.md](../1.0-gaps.md) (rows targeted at 0.4); [../../v1-api.md](../../v1-api.md); [60-roadmap.md](../../60-roadmap.md) R-REL-004, R-REL-034; decisions D-53, D-64.

## Goal

Close the 0.3 gaps scheduled for 0.4, prove the exit criteria, run the agent field test, and freeze the v1 API surface.

## Tasks

**T1. MCP on `rmcp` (G-1).** After the owner allows it (Q4-5): replace the stdio JSON-RPC layer of `fairlead-mcp` with `rmcp` pinned to an exact minor; the tools, their arguments and the resources do not change (the 0.3 tests run unchanged). Publish the crate if Q4-7 says so.

**T2. Mount inheritance (G-2).** `[[mount]] inherit = [...]`: inside a sub-root, the listed outer items resolve as an inherited layer read from the outer root's registry shards (read-only; the outer root still writes nothing inside). Test: `r_port_031__inherit_reaches_only_the_listed_items`.

**T3. Attach provenance and `req` scaffolds (G-6), portable doc snapshot (G-7).** `move --attach` records `[scope] attached-from = "<repo>@<commit>"` when given `--from <repo>@<commit>` and offers a `req` scaffold per ungranted override; a portable lock adds the L0 line of each required doc. Tests: `r_port_042__attach_records_provenance_and_offers_reqs`, `r_port_022__portable_lock_holds_doc_l0_lines`.

**T4. Variation remainders (G-8 to G-10).** A contract-bearing answer must bump the bridge's `ver=`; `sync` writes the computed `affects` into a draft answer; a vendor change needs a change doc touching the adapter (`HN-G004` hint and a docs-first finding). Tests: `r_var_041__contract_change_bumps_ver`, `r_var_041__sync_writes_affects`, `r_var_073__vendor_change_needs_an_adapter_doc`.

**T5. Retention remainders (G-12, G-13) and the hierarchy remainders (G-14, G-16).** Compaction of ADRs, handoffs, brainstorms and graveyard bodies (doc kinds `adr`, `handoff`, `brainstorm`); "compacted in <yyyy>Qn" for a folded record (rollup lines gain the folded draft IDs' digest so a reference can be matched); `explain` of a forgotten unit shows its last-touch seal and last known entry; `HN-E006`/`HN-E001` use the object-anchored edges of P3. Tests: one per gap.

**T6. The bench gate.** Exit criterion 1 measured on the 100k corpus with every 0.4 feature on; numbers recorded in the errata.

**T7. The pre-registered measurements.** Exit criteria 2 (constants fixture), 3 (anchor endurance), 4 (rewrite) and 5 (agent-mode bounds) run in CI; their results recorded.

**T8. Agent field test.** As for 0.2: a fresh agent on a real repository, with only `--help`, `explain` and the MCP tools, adopts fairlead including objects, constants and one query rule; every finding of the test is fixed or recorded (exit criterion 7).

**T9. The v1 API freeze.** First resolve the 0.3 B-4 items (1.0 gap G-19: fix each or record why it stands). Finalise `docs/v1-api.md` with the 0.4 surface (`map`, `rename`, `config`, the `query`/`require` grammar, `[content]`, `[agent-mode]`, the new codes and the `map` JSON shape); agent protocol B-4 reviews it; then the snapshot test's "a missing file is written" becomes "a missing file fails", and any change after the freeze needs an edition or a recorded 1.0 gap decision (D-53).

**T10. Regression and codes.** Every 0.1 to 0.3 suite passes unchanged, `PENDING_0_4` is empty, and `docs/plans/1.0-gaps.md` is updated with what 0.4 leaves for 0.5 to 0.9 (exit criterion 6).
