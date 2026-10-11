+++
id = "d-gya1avh7"
kind = "change"
status = "ready"
impact = "fix"
scope = "root"
title = "a [[file]] path that matches no file is reported"
covers = ["crates/fairlead-core/src/check/builtin/paths.rs", "Cargo.toml", "crates/fairlead/Cargo.toml", "crates/fairlead-core/Cargo.toml", "crates/fairlead-syntax/Cargo.toml"]
created = "0.2.2"
+++

# a [[file]] path that matches no file is reported

A `[[file]]` table whose scope-relative `path` matches no file in its scope is now reported as `HN-M001`, the warning already used for a glob that matches nothing. Before, a wrong path was accepted silently.
