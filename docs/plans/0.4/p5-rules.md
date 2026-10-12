# P5: rules over objects (`query` and `require`)

> Outline "New surface" (`check = "query"`, `check = "require"`); [13-checks-and-diagnostics.md](../../13-checks-and-diagnostics.md) rule declarations (R-CHK-030 to R-CHK-035); owner question Q4-6. Decisions D-26, D-63.

## Goal

People and agents write their own rules over objects in a control file. The bundled rules (0.1 to 0.3) become examples of what anyone can write. The language is small, declarative and readable, with no scripting and no network.

## Design

- **Shape.** A rule is `[rules.<name>] check = "query"` with a `select` (which objects), an optional `where` (predicates) and an `expect` (`none`, or a count), plus the usual `sev`, `stage`, `glob`, `why`. `check = "require"` turns it around: every selected object must have a matching `has` (a related object or attribute).
- **Selectors.** `kind` (`fn`, `type`, `const`, `import`, ...), `scope`, `layer`, `container`, `vis`, `name` (glob), `path` (glob), `exported`.
- **Predicates.** `imports` (from scope, path or module), `uses` (an ID or object), `has-doc`, `has-test` (a test object naming it, by the 0.3 auto-entry rules), `lines` (greater than N), `params` (greater than N).
- **Two forms.** Structured TOML tables, and a one-line string for simple cases, `"fn in app imports finance -> none"`; both compile to the same AST. The owner picks the primary form after T1 (Q4-6).
- **Evaluation.** Pure, over the P1 objects and the P4 graph; findings carry the rule's name like 0.1 rules, so baselines, waivers and stages work unchanged.

## Tasks

**T1. The language.** Grammar of both forms in `docs/34-content-model.md`, with ten worked examples covering the outline's ("no object in `app` imports from `finance`", "every public function in `core` has a doc comment"). Done when the owner picks the primary form (Q4-6).

**T2. Parser and checker.** `rules::query::parse` with errors as `HN-Q003` at the control file (error, enforce): unknown selector, unknown predicate, type mismatch. Tests: `r_query_001__both_forms_compile_alike`, `r_query_001__an_unknown_predicate_is_invalid`.

**T3. Evaluator for `query` (`HN-Q001`).** Tests: `r_query_002__app_importing_finance_is_reported`, `r_query_002__expect_count`.

**T4. Evaluator for `require` (`HN-Q002`).** Tests: `r_query_003__a_public_fn_without_doc`, `r_query_003__has_test_uses_auto_entries`.

**T5. Rules in the registry.** Query rules are items like 0.1 rules: inherited, overridden with `ovr`, waived, baselined, tightened by children (R-SCOPE rules apply unchanged). Test: `r_query_004__a_child_tightens_a_query_rule`.

**T6. Rewrite three bundled checks as examples.** The 0.3 visibility, rank and fork checks expressed as query rules in `docs/examples/`; a test runs both on the same fixture and compares findings (the bundled checks stay; the examples prove the language). Test: `r_query_005__examples_match_the_bundled_checks`.

**T7. `explain` and `new rule`.** `explain rule.<name>` shows the compiled query; `fairlead new rule --check query` scaffolds one. trycmd: `new-query-rule`.

**T8. Cost.** A query rule evaluates only over the objects its `select` names (an index by kind and scope); the bench records the cost of the ten examples on the 100k corpus. Done when within exit criterion 1.
