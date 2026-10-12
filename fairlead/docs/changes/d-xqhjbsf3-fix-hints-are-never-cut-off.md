+++
id = "d-xqhjbsf3"
kind = "change"
status = "draft"
impact = "fix"
sum = "fix hints fit 100 characters, so terse output never cuts one off; the hierarchy lower-bound note moved to the message"
out = "shipped"
why = "a golden review found hints ending mid-word, among them the R-HIER-034 notice"
scope = "root"
title = "Fix hints are never cut off"
covers = ["crates/fairlead-core/src/**"]
created = "0.3.1"
+++

# Fix hints are never cut off

A fix hint is at most 100 characters (R-CHK-035), and output caps it there. Sixteen hints were longer, so their ends were lost. The worst case was the hierarchy checks: they appended "a lower bound: declared edges only" to every rank, cycle and stale-unit hint, and the cap always cut it off. Every hint now fits. That note now ends the finding's message. A graveyard match states the entry's state and reason in its message instead of its hint. Unit tests now refuse any hint over the limit.
