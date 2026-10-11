# fairlead-syntax

The directive grammar of [fairlead](https://crates.io/crates/fairlead), as a pure library: comment
leaders per language, the `def`, `use`, `ovr`, `env`, `flag` and `end` directives, IDs, Markdown
frontmatter, and the syntax diagnostics. No file I/O, no git, no threads, no clock.

```rust
use fairlead_syntax::{extract, ExtractConfig};

let ex = extract(
    b"// fairlead: use tok.primary\nfn f() {}\n",
    "rs",
    &ExtractConfig::default(),
);
assert_eq!(ex.directives.len(), 1);
```

Whether `tok.primary` is declared, and what it means, is decided later by
[`fairlead-core`](https://crates.io/crates/fairlead-core), which resolves IDs across the codebase.
`fairlead explain syntax` prints the grammar on one screen.

Pre-1.0: the grammar is versioned by edition; this crate implements edition 1. Licence:
GPL-3.0-or-later (0.1.0 was MIT).
