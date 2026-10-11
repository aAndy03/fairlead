+++
id = "d-cpn1nn8s"
kind = "change"
status = "ready"
impact = "add"
scope = "root"
title = "Editions, pins and upgrade"
covers = ["crates/fairlead-core/src/edition.rs", "crates/fairlead-core/src/upgrade.rs", "crates/fairlead/src/commands/upgrade.rs"]
created = "0.2.0"
+++

# Editions, pins and upgrade

Adds editions and pins: a binary refuses a tree pinned to another edition with the exact install line, and `fairlead upgrade` plans, drafts an upgrade change doc, waits for an owner approval of the plan hash, and applies as one resumable transaction.
