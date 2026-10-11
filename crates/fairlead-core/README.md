# fairlead-core

The engine of [fairlead](https://crates.io/crates/fairlead): root discovery, the file walk,
extraction with its cache, the registry of declared items and scopes, the checks, context briefs,
and the transactional writers behind `init`, `sync`, `new`, `seal`, `move` and `upgrade`.

It reads the repository and git state; it never contacts the network and reads the clock
only where a seal records its time. The `fairlead` command-line
tool is a thin layer over `pipeline` (load, resolve, check, context, explain) and `write` (plans
and transactions).

Most users want the command-line tool: `cargo install fairlead --locked`. Depend on this crate to
embed fairlead's checks or briefs in another tool; for the directive grammar alone, use
[`fairlead-syntax`](https://crates.io/crates/fairlead-syntax), which has no I/O at all.

Pre-1.0: this API is not frozen. Licence: GPL-3.0-or-later (0.1.0 was MIT).
