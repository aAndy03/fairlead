+++
id = "d-b1rb2eeg"
kind = "change"
status = "sealed"
impact = "add"
scope = "root"
title = "check says what is hidden and what does not fail"
covers = ["crates/fairlead-core/src/render/mod.rs", "crates/fairlead-core/src/render/human.rs", "crates/fairlead-core/src/render/terse.rs"]
created = "0.2.1"
sealed = "0.2.1"
body-hash = "b3:8hyy4cpcyy85dt1ty61e"
ordinal = 10
+++

# check says what is hidden and what does not fail

`fairlead check --for-agent` names hidden observe-stage findings with the command that shows them, and ends with a `next:` step. The human and agent outputs say when an error is at stage warn and does not fail the run. Terse, JSON and SARIF output are unchanged.
