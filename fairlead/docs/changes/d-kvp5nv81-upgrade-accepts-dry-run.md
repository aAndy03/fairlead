+++
id = "d-kvp5nv81"
kind = "change"
status = "ready"
impact = "fix"
sum = "upgrade --dry-run prints the plan, writes nothing and exits 0, like every other writer"
out = "shipped"
why = "the v1 API review found upgrade was the one writer without --dry-run"
scope = "root"
title = "upgrade accepts --dry-run"
covers = ["crates/fairlead/src/**"]
created = "0.3.1"
+++

# upgrade accepts --dry-run

Every command that writes files accepts `--dry-run`, which prints what it would do, writes nothing and exits 0; `upgrade` did not. It does now, even beside `--yes`. Without either flag `upgrade` still prints the plan and exits 4 ("not applied"), as before.
