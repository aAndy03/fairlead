# 0.2 errata

Deviations from the 0.2 plans, in the order they were made. The chapters and decisions.md win over the plans; the plans are not edited by executors.

- E2-1: P1-T1. `ACTIVE_MILESTONE = M02` and `PENDING_0_2` (40 codes, with a second test that fails when a listed code becomes asserted, so the list only shrinks). The workspace version stays `0.1.0` until the 0.2 release: the `check` header carries the binary version, so a `-dev` bump mid-milestone would churn every golden. Edition-1 byte stability is `tests/editions.rs` (`r_edn_051__...`): every golden that `sync` accepts must regenerate its committed `gen/` files byte for byte (CRLF checkouts compared after line-ending normalisation, R-GIT-013), instead of a copied `tests/editions/1/` tree. Two 0.1 tests that used `HN-V001` as their "inactive code" example now use `HN-H001` (0.3).
- E2-2 (owner, 2026-10-10): agent-run protocols use `opencode-go/muse-spark-1.3-contributor` (agent-runbook, shared rule 4).
