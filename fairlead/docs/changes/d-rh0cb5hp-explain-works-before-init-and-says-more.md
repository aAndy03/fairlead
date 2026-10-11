+++
id = "d-rh0cb5hp"
kind = "change"
status = "ready"
impact = "fix"
scope = "root"
title = "explain works before init and says more"
covers = ["crates/fairlead-core/src/explain.rs", "crates/fairlead-core/src/explain/topics.rs", "crates/fairlead/src/commands/explain.rs"]
created = "0.2.1"
+++

# explain works before init and says more

`fairlead explain` answers its static topics and any diagnostic code before `fairlead init`. For an undeclared `file.*` ID it says how a file-item is declared (a `[[file]]` table in a control file). The syntax card and the unknown-topic message now describe this release.
