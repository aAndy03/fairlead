# P2: constants

> Outline goal 2 (constants are tracked) and goal 5 (ownership); the 0.1 literal-leak check (R-CHK-017 to R-CHK-022) and its 0.3 extensions (R-VAR-090 to R-VAR-099), generalised. Decisions D-04, D-43, D-63.

## Goal

Every declared constant is an object with a home scope, and fairlead reports drift, redundancy, repeated literals, unused constants and use outside the allowed scopes.

## Design

- **What is a constant.** Rust `const` and `static`; TS/JS exported `const` with a literal initialiser (P7); a CSS custom property (already read by the token readers); a TOML or JSON config value under a declared path (P7); and a `def const.<name>` directive anywhere, which binds to the object below it (P3) or carries `value=`.
- **Value.** Canonicalised with `tokens::canon` for colours and lengths (0.3), plus numbers (integers and floats normalised), strings (exact), and booleans. A constant whose initialiser is not a literal has no value and takes part only in "unused" and "ownership".
- **Home.** The scope whose files declare it. Visibility follows the 0.3 rules: `vis=subtree` by default, `vis=public`, and `[scope] provides` for sideways use.
- **Uses.** A use is an identifier match of the constant's name in another object of a parsed file (P1 objects, whole identifiers), or a `use const.<name>` directive. Import evidence (0.3) narrows ambiguous names to the imported module.

## Tasks

**T1. Kind `const`.** Built-in like `ext` (no kind table needed), shape code; `def const.<name> value= applies= vis= sym=`. A code `def const.x` above a parsed constant takes its name and value from the object when absent. Test: `r_const_001__a_const_directive_binds_the_constant_below`.

**T2. Constant extraction.** From the P1 objects of every parsed file into `consts: BTreeMap<ConstKey, Vec<ConstSite>>` (name, value, path, span, scope). Test: `r_const_002__rust_const_and_static_are_constants`.

**T3. Drift (`HN-O002`).** The same constant name with different values in two places visible to each other (same scope chain, or provided). The message names both values and places; the hint proposes one home. Test: `r_const_003__same_name_different_values_is_drift`.

**T4. Redundancy (`HN-O003`).** The same non-ambiguous value (R-VAR-096: never `0`, bare small numbers or empty strings) declared under two names in one scope chain. Test: `r_const_004__same_value_two_names_is_redundant`.

**T5. Repeated literals (`HN-O004`).** A literal in code equal to a declared constant's value, in a property or argument context the constant `applies` to; with no `applies`, only `match=`-style distinctive values (URLs, long numbers, strings of 8 or more characters). This generalises 0.1's leak check: tokens keep `HN-T001`, other constants get `HN-O004`. Tests: `r_const_005__a_literal_repeating_a_constant`, `r_const_005__ambiguous_values_never_scanned`.

**T6. Unused constants (`HN-O005`).** A declared constant with no use in any parsed file and not exported from a crate or package root. Info at observe: a lower bound, because dynamic access is invisible (D-40). Test: `r_const_006__an_unused_constant_is_info`.

**T7. Ownership (`HN-O006`).** A use of a constant from outside its visibility (the 0.3 `vis`/`provides` rules, now over real uses instead of `use` directives). Test: `r_const_007__use_outside_visibility_is_ownership`.

**T8. Constants in `context` and `explain`.** The brief of a path lists the constants it declares and the ones it uses from elsewhere (L1 budget); `explain const.<name>` lists its value, home and every use. Tests: `r_const_008__brief_lists_constants`, trycmd `explain-const`.

**T9. The seeded constants fixture.** `cargo xtask const-fixture --seed N` writes a repository with known drift, redundancy and repeated literals and scores `check` against the planted list (exit criterion 2). Done when the score is recorded.
