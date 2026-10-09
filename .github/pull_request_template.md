## Summary

<!-- What changes and why. Cite the rule IDs (R-xxx-nnn) and decisions (D-nn) that the change implements. -->

## Checks

- [ ] `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` pass.
- [ ] `cargo test --workspace --locked` passes on Linux, Windows and macOS (R-ARCH-110).
- [ ] A bug fix adds its regression fixture in an earlier commit than the fix (R-ARCH-152).
- [ ] A new diagnostic code is asserted by a test; `tests/codes.rs` checks this (R-ARCH-151).
- [ ] A new dependency has its row in 50-architecture Section 11 before it is added (R-ARCH-120).
