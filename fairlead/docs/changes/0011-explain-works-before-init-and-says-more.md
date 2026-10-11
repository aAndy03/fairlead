+++
id = "d-rh0cb5hp"
kind = "change"
status = "sealed"
impact = "fix"
scope = "root"
title = "explain works before init and says more"
covers = ["crates/fairlead-core/src/explain.rs", "crates/fairlead-core/src/explain/topics.rs", "crates/fairlead/src/commands/explain.rs"]
created = "0.2.1"
sealed = "0.2.1"
body-hash = "b3:gzcmp5xdjv979raaxb5r"
ordinal = 11
+++

# explain works before init and says more

`fairlead explain` answers its static topics and any diagnostic code before `fairlead init`. For an undeclared `file.*` ID it says how a file-item is declared (a `[[file]]` table in a control file). The syntax card and the unknown-topic message now describe this release.
