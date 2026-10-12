# P7: more languages

> Outline "Parsing"; owner answer Q4-3 (TypeScript, then CSS, then Markdown, TOML and JSON). [30-hierarchy-and-engines.md](../../30-hierarchy-and-engines.md) R-HIER-033 (import evidence), R-HIER-035; [31-variation-protocol.md](../../31-variation-protocol.md) R-VAR-090 to R-VAR-099.

## Goal

The object model of P1 covers the languages of the repositories fairlead is used on, in the owner's order. Each language replaces a line-based reader only where its parse is strictly better, and the line-based reader stays as the fallback.

## Tasks

**T1. TypeScript and JavaScript.** Functions, classes, interfaces, type aliases, exported `const` with literal initialisers, `import`/`export` (static and re-exports), JSX components (a function returning JSX is a component object). Import evidence (0.3) uses the parsed imports when present; `invoke(...)` calls are objects for the bridge checks (R-HIER-063). Tests: `content__ts_objects`, `content__tsx_components`, `r_hier_033__parsed_imports_replace_regex_when_present`.

**T2. CSS.** Rules, custom properties, `@layer` and `@theme` blocks as objects; the 0.1 token readers and the 0.2 cascade facts read from them when parsed. Tests: `content__css_custom_properties`, `r_var_099__0_1_leak_results_unchanged` (stays green).

**T3. Markdown.** Headings (with their level and anchor), fenced code blocks (with their language), links. Feeds P4's stale doc references and the task-query index (doc headings, 0.3 E3-33). Test: `content__markdown_headings_and_code`.

**T4. TOML and JSON.** Keys as objects with dotted paths and literal values, through the pure Rust parsers already in the tree; constants from declared config paths (P2: `[content] config-constants = ["Cargo.toml#package.version", "package.json#version"]`). Tests: `content__toml_keys`, `r_const_009__a_config_value_is_a_constant`.

**T5. Language demand and limits.** `[content] languages` lists which parsers run; `max-file` and `max-objects` cap work per file (over the cap: `HN-O001`, line-based). Test: `r_content_002__a_file_over_the_cap_falls_back`.

**T6. Benchmarks per language.** The bench harness records cold and warm parse cost per language on the 100k corpus; exit criterion 1 holds with every language on. Done when recorded.
