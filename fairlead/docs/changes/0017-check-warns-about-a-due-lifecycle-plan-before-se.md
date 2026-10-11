+++
id = "d-96s5fqdv"
kind = "change"
status = "sealed"
impact = "fix"
scope = "root"
title = "check warns about a due lifecycle plan before seal refuses"
covers = ["crates/fairlead-core/src/check/builtin/plans.rs", "crates/fairlead-core/src/explain.rs", "crates/fairlead/src/commands/explain.rs"]
created = "0.2.2"
sealed = "0.2.2"
body-hash = "b3:5farv8mh6b7z9j867xw3"
ordinal = 17
+++

# check warns about a due lifecycle plan before seal refuses

A lifecycle plan that is due blocks the next seal. When no reference to the item is left, `fairlead check` said nothing about it until the seal refused. It now warns (`HN-L001`) at the flag with the three ways to finish the plan, and `fairlead explain <id>` lists the check findings that name the item, with their fix. The exit code of `check` does not change.
