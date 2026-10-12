+++
id = "d-v3m7t0k9"
kind = "change"
status = "sealed"
impact = "add"
scope = "root"
title = "Variation protocol and richer token checks"
covers = ["crates/fairlead-core/src/**", "crates/fairlead/src/**", "crates/fairlead-syntax/src/**"]
created = "0.2.3"
sealed = "0.3.0"
body-hash = "b3:m5r1169gpwhym2ctgnwt"
ordinal = 29
+++

# Variation protocol and richer token checks

A consumer of a reusable unit now has four checked moves. It can use an existing axis value, file a request (`fairlead new request <id>` in its own scope), wrap the unit locally while it waits (`flag awaiting to=<request> target=<version>`, which warns as the target nears and errors once it is reached), or keep a permanent adapter linked to the rejection or to an external library. The home scope answers with a change doc (`answers`, `type`, `affects`), the home owner accepts it with `fairlead approve <draft>`, and a request's status (open, answered, closed, rejected) is derived, never stored. Variants must stay in the base's scope, kind, layer and container and use their base; many variants suggest an axis; units in `[variation] request-layers` are varied only through a request. Importing an external library's specifier outside its adapter is an error. Fork detection (`[variation] fork-detect`) is off by default and flags the same `sym` or file name declared in two scopes; a fork is declared or dismissed with `ovr rule.var-fork`. `context` lists open requests with their text fenced as data.

Token checks compare canonical values: hex, `rgb()` and `hsl()` colours are one form, `rem` equals `px` at `[tokens] root-px`, and JS style objects' unitless lengths are pixels. Length properties are now scanned, Tailwind arbitrary values gain more prefixes and a `[tokens.tailwind]` table, a colour within `[tokens] near` of a token is an info near miss, a token's `match=` constant is found in string literals, `0` and bare numbers are never compared, and one level of `var()` is followed (a deeper chain is reported unresolved).
