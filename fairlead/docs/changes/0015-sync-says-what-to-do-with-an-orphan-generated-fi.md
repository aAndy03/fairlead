+++
id = "d-5ykdxj0m"
kind = "change"
status = "sealed"
impact = "fix"
scope = "root"
title = "sync says what to do with an orphan generated file"
covers = ["crates/fairlead-core/src/write/sync.rs"]
created = "0.2.2"
sealed = "0.2.2"
body-hash = "b3:w2jdh0g8d2farvyw1w76"
ordinal = 15
+++

# sync says what to do with an orphan generated file

The note for a generated file that no scope writes any more no longer refers to release 0.1: it says that sync leaves the file in place and that it can be deleted by hand.
