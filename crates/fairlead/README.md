# fairlead

Repository-local rules and context for AI agents and human developers. Declared intent lives in
the repository as Markdown docs and small comment directives; `fairlead check` reports drift between
code and that intent; `fairlead context` gives an agent a compact, token-budgeted brief of the part
of the codebase it is about to touch. Everything is offline and deterministic: no network, no model,
no telemetry.

This crate is the command-line tool. It is not a library: to build on fairlead, depend on
[`fairlead-core`](https://crates.io/crates/fairlead-core) (the engine) or
[`fairlead-syntax`](https://crates.io/crates/fairlead-syntax) (the directive grammar).

## Install

```sh
cargo install fairlead --locked
```

## First steps

```sh
fairlead explain syntax        # the directive grammar, one screen; works before init
fairlead init --dry-run        # shows what init would write
fairlead init --yes            # writes fairlead/control.toml, AGENTS.md block, .gitattributes
fairlead check                 # reports drift; exit 0 when clean
fairlead context src/main.rs   # the brief an agent reads before editing that file
```

Tie code to intent with a comment directive, for example `// fairlead: use tok.primary`, after
declaring the kind (`fairlead new --help` lists what each kind takes). `fairlead explain <code>`
explains any finding; `fairlead --help` lists the explain topics and the exit codes.

## Versions and change docs

From 0.2, every change is described by a change doc (`fairlead new change "<title>"`), and
`fairlead seal` turns the ready docs into a version: a version file, ledger records and an optional
generated changelog. `fairlead move` restructures files and scopes with a list of what changes;
`fairlead upgrade` moves a codebase to a new grammar edition.

## Status and licence

Pre-1.0: the command-line surface and output formats freeze at the end of 0.3. From 0.2.0 fairlead
is licensed GPL-3.0-or-later; release 0.1.0 was MIT and stays MIT.
