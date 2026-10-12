# P3: directives at any line, anchored to objects

> Outline goal 3; [12-directive-syntax.md](../../12-directive-syntax.md) R-SYN-019 (extents), R-SYN-021 (sites). Decisions D-12, D-63.

## Goal

An agent may write a directive above any line of any parsed file. It binds to the object below it, and the tracked item follows that object, not the line number, across edits and moves.

## Design

- **Binding.** A directive with no explicit extent binds to the first object whose span starts after the directive (skipping blank lines, attributes, decorators and doc comments). Its extent becomes the object's span. With an explicit `+N` or `+end`, the 0.1 extent rule applies unchanged.
- **Identity.** A site records the object's `ObjectId` (P1) and its `signature_hash`. Across edits the site is found again by `ObjectId`; when the object was renamed in the same change, by the signature hash among the file's objects; after `fairlead move`, by the move plan's path map.
- **Unbound.** A directive with nothing parseable below it in a parsed file is `HN-O009` (warn): the binding falls back to the 0.1 line extent.

## Tasks

**T1. Binding.** `anchor::bind(directive, objects) -> Option<ObjectId>` with the skipping rules above, per language. Tests: `r_anchor_001__binds_to_the_function_below`, `r_anchor_001__skips_attributes_and_doc_comments`.

**T2. Extents from objects.** A bound site's extent is the object's span; `use` lines inside it are attributed to the item (the 0.3 extent containment, E3-7, now exact). Test: `r_anchor_002__use_inside_the_bound_function`.

**T3. Identity across edits.** The registry records `object=<ObjectId>` on each bound site in the generated shards; on the next run a site whose line moved is found by its ID, and a renamed object by its signature hash. Tests: `r_anchor_003__lines_inserted_above_keep_the_binding`, `r_anchor_003__a_renamed_function_keeps_the_binding`.

**T4. Unbound (`HN-O009`).** Test: `r_anchor_004__a_directive_above_nothing_is_unbound`.

**T5. Hierarchy and variation over objects.** A unit's `sym` defaults to its bound object's name; `HN-H011` and `HN-H012` (0.3 evidence) use object identifiers instead of identifier hashes when the file is parsed. Test: `r_anchor_005__sym_defaults_to_the_bound_object`.

**T6. The anchor endurance test.** A scripted history of 50 edits (exit criterion 3) on a fixture of anchored directives; every binding survives. Done when it passes in CI.
