+++
id = "d-2bhm0jjk"
kind = "change"
status = "sealed"
impact = "fix"
sum = "an interim wrapper near its target version warns (HN-W001) and fails only once the target is reached (HN-X001)"
out = "shipped"
why = "the advance notice used a code that is always an error, so the run failed early"
scope = "root"
title = "A wrapper whose target is near warns instead of failing"
covers = ["crates/fairlead-core/src/check/builtin/variation.rs"]
created = "0.3.1"
sealed = "0.3.1"
body-hash = "b3:99p1hp0x59h9qpn8xt9n"
ordinal = 31
+++

# A wrapper whose target is near warns instead of failing

A wrapper flagged awaiting a request was meant to give notice before its target version: info two minors ahead, a warning one minor ahead, and an error once the target is reached. The notice used the same code as the error, which is always an error, so check failed as soon as the target came near. The notice is now HN-W001 (info or warn) and only the target itself fails the run.
