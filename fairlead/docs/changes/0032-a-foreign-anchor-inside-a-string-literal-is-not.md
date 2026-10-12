+++
id = "d-crr9mnfs"
kind = "change"
status = "sealed"
impact = "fix"
sum = "the lost-anchor check reads the committed copy with the directive extractor, not by text search"
out = "shipped"
why = "fairlead's own check reported a lost foreign region for a test string that only moved one line"
scope = "root"
title = "A foreign anchor inside a string literal is not reported lost"
covers = ["crates/fairlead-core/src/**"]
created = "0.3.1"
sealed = "0.3.1"
body-hash = "b3:8y5r8rw3tw5r434kf75q"
ordinal = 32
+++

# A foreign anchor inside a string literal is not reported lost

`HN-G006` reports a foreign region whose `def foreign.<tool>` anchor comments a tool removed. It found the anchors in the committed copy of a file by searching the text, so an anchor written inside a string literal (a test fixture, for example) counted there but not in the current file, which is read by the real extractor. Any edit to such a file then reported the region as lost. The committed copy is now read by the same extractor, so both sides agree.
