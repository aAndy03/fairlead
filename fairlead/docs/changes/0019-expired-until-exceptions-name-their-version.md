+++
id = "d-edy5c9es"
kind = "change"
status = "sealed"
impact = "fix"
scope = "root"
title = "expired until= exceptions name their version"
covers = ["crates/fairlead-core/src/check/builtin/exceptions.rs"]
created = "0.2.2"
sealed = "0.2.2"
body-hash = "b3:06mdr7dmf1fh6e97fdqs"
ordinal = 19
+++

# expired until= exceptions name their version

An exception that expired by `until=<version>` was reported as "expired on" with no date. The `HN-X001` message now says "expired when version <v> opened"; a date expiry still says "expired on <date>".
