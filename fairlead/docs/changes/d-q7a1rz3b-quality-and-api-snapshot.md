+++
id = "d-q7a1rz3b"
kind = "change"
status = "draft"
impact = "fix"
scope = "root"
title = "0.3 quality checks and the v1 API snapshot"
covers = ["crates/**", "tests/**", "xtask/**", "docs/**", ".github/workflows/**"]
created = "0.2.3"
+++

# 0.3 quality checks and the v1 API snapshot

Three new example repositories exercise the hierarchy end to end: a UI component library, a Rust backend workspace and a Tauri app with a TypeScript-to-Rust bridge; they show rank violations, cycles, visibility errors, an entry-only layer, a stale unit and every bridge break. Scripted tests run the whole variation protocol and a real git submodule mount. A five-year simulation of monthly seals checks that compaction is byte-identical on every machine, that a second compaction does nothing, and that an agent's brief does not grow with history; it found and fixed two compaction bugs. `docs/v1-api.md` lists the surface that will freeze at the end of 0.4, and a snapshot test fails on any unplanned change to it.
