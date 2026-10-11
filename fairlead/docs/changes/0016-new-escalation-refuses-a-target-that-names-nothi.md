+++
id = "d-7apnej6s"
kind = "change"
status = "sealed"
impact = "fix"
scope = "root"
title = "new escalation refuses a target that names nothing"
covers = ["crates/fairlead-core/src/write/new.rs", "crates/fairlead/src/cli.rs"]
created = "0.2.2"
sealed = "0.2.2"
body-hash = "b3:vztv1xesdkkyeajkzx5q"
ordinal = 16
+++

# new escalation refuses a target that names nothing

`fairlead new escalation <name>` writes the name as the item being escalated. A name with no dot (a label such as `allow-dbg-demo`), or a `rule.*` or other item ID that no scope in reach declares, is now refused, because no ancestor could ever answer such a request. Vocabulary keys such as `layer-ranks.widget` are accepted as before. `fairlead new --help` says the name is the item escalated.
