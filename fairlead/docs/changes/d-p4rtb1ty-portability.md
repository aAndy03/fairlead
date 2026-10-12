+++
id = "d-p4rtb1ty"
kind = "change"
status = "ready"
impact = "add"
scope = "root"
title = "Portability: requires, locks, mounts and moving scopes"
covers = ["crates/fairlead-core/src/**", "crates/fairlead/src/**"]
created = "0.2.3"
+++

# Portability: requires, locks, mounts and moving scopes

A scope can now say what it depends on. `[scope] requires` (or `portable = true`) makes `sync` write `gen/requires`, built from the scope's `use` and `extends` lines plus what it lists, and the scope's lock pins a digest of each required item; a portable scope also keeps a snapshot of them. `check` says when a required item changed since the lock, when one is no longer provided (`UNSATISFIED`), and when two scopes require each other. Doc IDs can be written `<scope>:d-<draft>`, and `context` always writes them that way. A claim restated in a child scope needs `contradicts=`.

A second root inside the tree is a mounted sub-root once the root registers it with `[[mount]]`: the outer root never reads or writes inside it, sees only its scope list, edition and pin, warns when its scope ids collide (use `as=`), and reports when a submodule mount moved since the lock or is not checked out. `fairlead move --adopt <scope>` opens a mount after an approval; `--cut` moves a scope and records where it came from; `--copy` makes a copy under a new scope id; `--attach` joins an arrived scope to its new parents after listing what is satisfied, changed or missing. `check` also hints when a publishable crate would ship the managerial folder or when a path dependency's version disagrees, and reports scopes a sparse checkout left out.
