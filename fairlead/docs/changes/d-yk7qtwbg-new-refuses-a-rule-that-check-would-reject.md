+++
id = "d-yk7qtwbg"
kind = "change"
status = "ready"
impact = "fix"
scope = "root"
title = "new refuses a rule that check would reject"
covers = ["crates/fairlead-core/src/pipeline.rs", "crates/fairlead-core/src/check/builtin/paths.rs"]
created = "0.2.1"
+++

# new refuses a rule that check would reject

`fairlead new` checks the control file it is about to write the way `check` would read it. An invalid value, such as a `forbid` regex that does not compile, or a new rule whose glob matches no file, is refused with the reason and nothing is written. Before, such a rule was written and then every command failed until the file was repaired by hand.
