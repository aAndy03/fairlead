+++
id = "d-en7jxym1"
kind = "change"
status = "ready"
impact = "fix"
scope = "root"
title = "explain finds an item declared in a child scope"
covers = ["crates/fairlead-core/src/explain.rs"]
created = "0.2.2"
+++

# explain finds an item declared in a child scope

`fairlead explain <kind.name>` now finds an item declared in a child scope without the `scope:` qualifier when only one scope declares that name. When several do, it lists the qualified IDs to choose from.
