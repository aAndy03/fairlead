+++
id = "d-edy5c9es"
kind = "change"
status = "ready"
impact = "fix"
scope = "root"
title = "expired until= exceptions name their version"
covers = ["crates/fairlead-core/src/check/builtin/exceptions.rs"]
created = "0.2.2"
+++

# expired until= exceptions name their version

An exception that expired by `until=<version>` was reported as "expired on" with no date. The `HN-X001` message now says "expired when version <v> opened"; a date expiry still says "expired on <date>".
