+++
id = "d-vm20ca0a"
kind = "change"
status = "draft"
impact = "fix"
sum = "of= on a unit is reported, and with fork-detect a renamed local copy of another scope's unit is a fork suspect"
out = "shipped"
why = "agents copied a unit under a new name with of= and check accepted it"
scope = "root"
title = "A renamed local copy of another scope's unit is reported"
covers = ["crates/fairlead-core/src/check/builtin/variation.rs"]
created = "0.3.1"
+++

# A renamed local copy of another scope's unit is reported

of= names the base of a design token; on a unit it is now reported, with a pointer to explain variation. When fork detection is on, a unit that uses a unit of another scope on the same layer and shares the last part of its name (ghost-button and button), without wraps= or variant-of=, is reported as a possible local copy. Composition of different units on one layer is not reported.
