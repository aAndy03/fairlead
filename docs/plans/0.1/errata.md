# 0.1 implementation errata

Deviations of the actual implementation from the phase plans, recorded as they happen (master plan, execution protocol 3).

| # | Phase | Deviation | Reason |
|---|---|---|---|
| E-1 | P1 | No pre-created stub modules, stub test targets or placeholder workflows; modules, `[[test]]` targets and workflows arrive with the phase that implements them. | Avoids dead files; the contract's module paths are still followed. |
| E-2 | P1 | `indexing_slicing = "deny"` is set by `#![deny(clippy::indexing_slicing)]` in `fairlead-syntax` only, not workspace-wide. | Cargo forbids overriding `workspace.lints` per crate; core indexes under review. |
| E-3 | P1 | `license = "MIT"` (D-58) replaces the placeholder `license-file`; `publish = false` stays until the 0.1 exit criteria hold. | Owner decision. |
| E-4 | P1 | `.cargo/config.toml` sets `build.jobs = 2`. | Owner asked for light cargo use. |
| E-5 | all | The plans' "run the test to see it fail" steps are collapsed: tests and implementation land together and the test run is the check. | Time; red phase gives no information once the API exists. |
