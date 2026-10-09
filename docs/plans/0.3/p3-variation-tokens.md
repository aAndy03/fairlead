# P3: variation protocol and richer token checks

> Chapter [31-variation-protocol.md](../../31-variation-protocol.md) (all); [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) R-CHK-049; [21-environments-maturity-lifecycle.md](../../21-environments-maturity-lifecycle.md) R-ENV-037. Decisions D-04, D-12, D-21, D-28, D-49.

## Goal

A consumer has four legitimate moves for a reusable unit:
1. Use an existing axis value.
2. Request a variant.
3. Wrap the unit locally with an expiry.
4. Write a permanent adapter.

Illegitimate moves are detected: undeclared forks (off by default) and hardcoded values that should be tokens. These token checks go beyond 0.1's exact literal leaks.

## Design

- **Requests** are docs with `kind = "request"`, created by `new request` in the consumer scope. Fields: `need want because sum out`; the ID is a random draft ID.
- **Request status is derived, never stored:**
  - `open`: no doc answers it.
  - `answered`: a doc has `answers`.
  - `closed`: answered, no `awaiting` flag names it any more, and the consumer references have migrated.
- **Answers** are change docs in the home scope with `answers = [...]` and `type = variant | axis | rejected`.
- **Axes** are `def` keys with a closed value set. Variants are units with `variant-of=` (base scope only) and optional `native=`.
- **Wrappers** carry `flag awaiting to=<request> target=<version>`. **Adapters** carry `adapter=true` with `rejected-by=` or `wraps=<ext id>`.
- **`tokens::canon`** canonicalises colours: hex (3, 4, 6 and 8 digits), `rgb()` and `hsl()` map to one sRGB form; `oklch()` and `color()` are compared exactly after number normalisation. Lengths are unit-aware: `rem` uses `[tokens] root-px` (default 16), and unitless lengths in JS style objects are px.

## Tasks

**T1. Home scope and visibility.** Rules R-VAR-001, R-VAR-002, R-VAR-010 to R-VAR-012:
- One home scope per unit. A second declaration is `HN-D001`.
- Home changes only through `move`.
- `variant-of=` is legal only in the base's own scope.
- Visibility itself is P1-T8; here add the fix hint naming the lowest common ancestor.

Tests: one per rule.

**T2. Request layers.** R-VAR-020 to R-VAR-022:
- A variation of a unit in `[variation] request-layers`, or of one marked `request=required`, must go through a request.
- `wraps=` declares a local composition, which must contain `use <id>`.
- A consumer's own new concern is a new unit with `extends=`, never the primitive's ID.

Tests: one per rule.

**T3. Requests.** R-VAR-030 to R-VAR-033:
- `new request` takes the required fields.
- Status is derived.
- `var.request-stale` (HN-W001) fires after `[variation] stale-seals` seals. `context` lists stale requests.
- Request text is fenced as data.

Tests:
- `r_var_031__status_derived_open_answered_closed`
- `r_var_032__stale_request_listed`
- `r_var_033__request_text_fenced`

**T4. Answers.** R-VAR-040 to R-VAR-043:
- `answers=` and `type`.
- `affects` is generated from reverse `use` edges; check requires every affected scope, or `affects = "all"`.
- A contract-bearing change bumps the contract's `ver`.
- A rejected answer carries a graveyard entry whose fingerprint covers `want` (0.2 graveyard).
- Acceptance needs an `[[approval]]` from the home scope's owner.

Tests: one per rule.

**T5. Variants and axes.** R-VAR-050 to R-VAR-054:
- A variant inherits kind and layer, shares or siblings the container, and its files contain `use <base>` (evidence-checked).
- Too many variants gives `var.axis-suggest` (HN-W004, info).
- After acceptance, consumer `use`s migrate and `awaiting` is removed.
- `native=` spellings are aliases, unique per unit.
- Theme words are axis values, not envs.

Tests: one per rule.

**T6. Interim wrappers.** R-VAR-060 to R-VAR-064:
- `flag awaiting to= target=` is required on the wrapper region.
- Version-based ramp: info one version before `target`, warn when due next, error once `O >= target`. `exp=` is opt-in.
- An `awaiting` naming a missing, closed or rejected request is an error.
- Wrappers roll up into the exception ledger at seal, and sprawl is reported.

Tests: one per rule.

**T7. Adapters and externals.** R-VAR-070 to R-VAR-074:
- An adapter has `adapter=true` with `rejected-by=` or `wraps=`.
- `def ext.<name>` at the boundary declares the import specifier and path. Its files are vendor-owned and carry no directives.
- Consumers import only `import-as=`. The `ext` specifier outside the adapter is `var.adapter-bypass` (HN-W002, error); the check is a grep over import specifiers (P1-T11).
- A changed vendor hash is `var.vendor-changed`, the alias of `HN-G004` at warn, and needs a change doc touching the adapter.

Tests: one per rule.

**T8. Forks.** R-VAR-080 to R-VAR-084:
- A declared fork is `ovr rule.var-fork of= why= owner= until=`.
- Detection is off by default (`[variation] fork-detect = "off"`). When on, it starts at `observe`. It flags the same `sym` or basename in two scopes, or a symbol shadowing a declared `sym`, as `var.fork-suspect` (HN-W003).
- A pair can be dismissed with `dismiss=true pair=<hash>`; a third location re-flags it.
- Generated and vendor files are excluded. The `similarity` key is reserved, `off`.

Tests:
- `r_var_084__fork_detection_off_by_default`
- `r_var_082__dismissed_pair_refires_on_third_location`
- `r_var_083__generated_and_vendor_excluded`

**T9. Token contracts.** R-VAR-090, R-VAR-091, R-VAR-099. A token declares `css=` and `applies=`. Its value is read from authored CSS into the derived index and is never copied into the `def`. The 0.1 rules R-CHK-017 to R-CHK-022 are unchanged; this phase only adds to them. Test: `r_var_099__0_1_leak_results_unchanged` (the 0.1 leak goldens still pass).

**T10. Property contexts.** R-VAR-092, R-VAR-094:
- CSS declarations.
- JS/TS style objects (`backgroundColor` is read as `background-color`).
- Presentation attributes (`fill=`, `stroke=`).
- Tailwind arbitrary values (`bg-[#3B82F6]`) through a prefix-to-property table in the control file.

Unsupported contexts are skipped, not guessed. Detection runs only in `style-governed` globs. Tests: one per context.

**T11. Canonical comparison and near misses.** R-VAR-093, R-VAR-095, R-VAR-096:
- Compare colours and lengths after canonicalisation.
- The message carries the replacement from `use-css` or `use-tw`.
- A near miss (per-channel distance at most `[tokens] near`, default 2) is `var.token-near` (HN-T006, info).
- Distinctive constants are scanned only with `match=`; ambiguous values (`5`, `0`) never are.

Tests:
- proptest `r_var_093__hex_rgb_hsl_canonicalise_equal`
- `r_var_093__rem_equals_px_at_root_px`
- `r_var_095__near_miss_is_info_with_replacement`
- `r_var_096__ambiguous_values_never_scanned`

**T12. `var()` chains.** R-VAR-097, R-VAR-098:
- Resolve one level.
- A deeper chain is recorded `unresolved` with the chain text, never compared, and reported as `var.token-unresolved` (HN-T007, info).
- Several theme sources are listed per theme, not merged.
- A local override is `ovr <token> ... until=`. A same-named child token without `ovr` is `HN-R005`.

Tests: one per rule.

**T13. Codes.** Remove `HN-W001` to `W004`, `HN-T006` and `T007` from `PENDING_0_3`.
