+++
id = "d-x32fmkx0"
kind = "change"
status = "ready"
impact = "add"
scope = "root"
title = "Hierarchy: ladders, containers, units and the use graph"
covers = ["crates/fairlead-core/src/**", "crates/fairlead-syntax/src/directive.rs", "crates/fairlead/src/cli.rs", "crates/fairlead/Cargo.toml"]
created = "0.2.3"
+++

# Hierarchy: ladders, containers, units and the use graph

Fairlead now understands how the pieces of a codebase fit together. The root control file declares ladders of ranked layers (`[layers.<ladder>]`, presets with `fairlead new ladder ui|core`), free layers, sanctioned cross-ladder flows and `[hier] level`, which switches the checks on step by step (0 to 2 in this phase). A `def` in code now declares a unit or a container next to its symbol; units name a layer and one container, containers nest with `parent=` and claim files with `path=`.

`fairlead check` reports, per level: overlapping or escaping container claims and parent cycles (level 0); units with no or several containers, double bindings, unknown layers, orphan and hollow units, and governed files bound to nothing, with a coverage ratio the baseline ratchets (level 1); `use` edges that point upward, cross ladders without a flow or break free-layer and entry-only rules, cycles between units and between containers (Cargo path dependencies included), visibility across scopes, `sym=` and import evidence, maturity and env reach (level 2). Every verdict on edges is a lower bound: declared edges only. Refactor suggestions (promote, split, demote) are written to `gen/hier.suggest` and never fail a run. `fairlead move --merge <ctr> <ctr>` merges two containers: it rewrites `container=` and `parent=`, leaves a tombstone with `moved-to`, and drafts a refactor change doc.
