+++
id = "d-ppvsk9cz"
kind = "chore"
status = "draft"
impact = "fix"
sum = "evals/ is excluded from the scan, docs-first and the Cargo workspace"
out = "shipped"
why = "the evaluation projects carry their own fairlead folders and Cargo workspaces"
scope = "root"
title = "Keep the A-B-C evaluation out of fairlead's own scan and workspace"
covers = ["Cargo.toml", "fairlead/control.toml"]
created = "0.3.2"
+++

# Keep the A-B-C evaluation out of fairlead's own scan and workspace

The with-and-without-fairlead evaluation in evals/ab holds three small projects, each with its own fairlead folders and, for the Rust ones, its own Cargo workspace. fairlead's own scan and docs-first rule skip evals/, as they skip ablation/, and the root workspace excludes it.
