+++
id = "d-b5pwp89v"
kind = "change"
status = "sealed"
impact = "fix"
scope = "root"
title = "check --for-agent suggests showing hidden findings first"
covers = ["crates/fairlead-core/src/render/terse.rs"]
created = "0.2.2"
sealed = "0.2.2"
body-hash = "b3:ckgwrwapkwyy5xn8bnhb"
ordinal = 18
+++

# check --for-agent suggests showing hidden findings first

When no finding listed by `fairlead check --for-agent` fails the run and observe-stage findings are hidden, the `next:` line is now `fairlead check --show-observe` instead of an explain of the first warning.
