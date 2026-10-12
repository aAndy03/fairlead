+++
id = "d-c6xtq0ry"
kind = "change"
status = "sealed"
impact = "add"
scope = "root"
title = "Context queries and agent tools"
covers = ["crates/**", "xtask/**", "tools/**", ".github/workflows/**"]
created = "0.2.3"
sealed = "0.3.0"
body-hash = "b3:3pnrq7tx58h41k9nmrt4"
ordinal = 24
+++

# Context queries and agent tools

`fairlead context` now accepts a task in plain words ("add a progress bar"): it ranks the scopes whose names, summaries, tags, doc titles, graveyard entries and tombstones match, briefs the best one (or the one pinned with `--scope`), and lists the matching items, with graveyard entries first. It never guesses: no match gives the root brief and says so. Where the hierarchy is in use, a path's brief also names its units, their containers, the containers' public surface and the interim wrappers that are about to expire. `--dense` prints a shorter, dictionary-aliased brief for very large trees.

A new `fairlead-mcp` binary lets AI agents call `check`, `context` and `explain` as MCP tools, read-only, over stdio; it runs the `fairlead` binary, so the answers are identical, and the `fairlead` binary itself stays free of any MCP or network code. `authority.verify-signatures = true` makes `check --ci` verify the signatures of every authority record, `ovr` lines included. `cargo xtask query-recall` measures how often task queries find the expected scope, and a weekly CI job tries the tree-sitter TypeScript grammar on the test corpus so its adoption can be decided later.
