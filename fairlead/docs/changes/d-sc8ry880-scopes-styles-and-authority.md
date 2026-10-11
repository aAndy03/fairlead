+++
id = "d-sc8ry880"
kind = "change"
status = "ready"
impact = "add"
scope = "root"
title = "Scopes, styles and authority"
covers = ["crates/fairlead-core/src/config/**", "crates/fairlead-core/src/registry/**", "crates/fairlead-core/src/extract/**", "crates/fairlead-core/src/check/builtin/cascade.rs", "crates/fairlead-core/src/check/builtin/authority.rs", "crates/fairlead-core/src/check/builtin/vendor.rs", "crates/fairlead-core/src/check/builtin/exceptions.rs", "crates/fairlead-core/src/context/**"]
created = "0.2.0"
+++

# Scopes, styles and authority

Adds root delegation of keys a child may tighten, `own` conventions, provided items, `[[attach]]` directives, the CSS cascade checks, `FOREIGN-LOST`, agent approvals that count for nothing on floor-adjacent targets, stale escalations, version-based `ovr until=`, and `context --since` with open hotfix debts and escalations.
