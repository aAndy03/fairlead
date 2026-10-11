+++
id = "d-tf506nrd"
kind = "change"
status = "sealed"
impact = "fix"
scope = "root"
title = "Help lists exit codes, topics and field keys"
covers = ["crates/fairlead/src/cli.rs", "crates/fairlead/src/commands/approve.rs", "Cargo.toml", "crates/fairlead/Cargo.toml", "crates/fairlead-core/Cargo.toml", "crates/fairlead-syntax/Cargo.toml", "crates/fairlead/README.md", "crates/fairlead-core/README.md", "crates/fairlead-syntax/README.md"]
created = "0.2.1"
sealed = "0.2.1"
body-hash = "b3:85ys8wrdqsm49cyqkbs0"
ordinal = 12
+++

# Help lists exit codes, topics and field keys

`fairlead --help` lists the exit codes, the explain topics and how to start; `fairlead new --help` lists the `--field` keys of each kind. `fairlead approve --list` says `no approvals recorded` when there are none. Each crate has its own README on crates.io.
