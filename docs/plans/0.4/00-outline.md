# fairlead 0.4 outline: the content model

> Decision [D-63](../../decisions.md). Status: outline; the phased plan is [00-master.md](00-master.md) (written 2026-10-12, after the 0.3 build). The v1 API freeze is at the end of 0.4 (D-64), so 0.4 adds its surface before the freeze, within edition 1.

## The idea

Up to 0.3, fairlead sees mostly text: its own directives, globs, regex rules and a few built-in readers (CSS tokens, imports by regex, bridge literals). In 0.4 the content of every supported file becomes **objects**: functions, types, constants, imports and exports, Markdown headings, TOML and JSON keys, CSS custom properties. Fairlead tracks them, maps them, and lets people and agents write rules over them. The bundled rules become examples of what anyone can write.

## Goals, in the owner's order

1. **Fast first.** Nothing is parsed unless a rule, a map or a directive needs it. Objects are cached by the file's content hash, so a warm run re-parses only changed files (the 0.2 cache already reads exactly one file after a one-file change). Parsing runs in parallel. Budgets, measured by the bench harness on the 100k-file corpus: a warm `check` with object rules within 1.2x of the 0.3 time; a cold run within 2x.
2. **Constants are tracked.** Every declared constant (Rust `const`/`static`, TS/JS exported `const`, CSS custom property, TOML/JSON config value, and a `def const.<name>` directive anywhere) is an object with a home scope. Fairlead reports:
   - the same name with different values in two places (drift);
   - the same value declared twice under different names (redundancy);
   - a literal in code that repeats a declared constant's value (the 0.1 token-leak check, generalised to every constant);
   - a constant nobody uses;
   - a constant used outside the scopes allowed to see it (ownership).
3. **Directives at any line.** An agent may write a fairlead directive above any line of any supported file. It binds to the object below it: the function, the constant, the block. The tracked item then follows that object, not the line number, across edits and moves.
4. **No drift, no redundancy.** Besides constants: duplicated functions or blocks above a size threshold (reported, never auto-merged); docs whose headings or code samples name objects that no longer exist.
5. **Ownership and modularity.** Every object has a home scope. Rules can say who may use what (the 0.3 layers and provides, applied to real imports). `map` suggests module splits where a scope's objects form separate clusters.
6. **Restructuring by fairlead.** A rename or move of an ID, file or scope rewrites the references (lifting D-50's "no rewriting tool" limit, which covered 0.1 to 0.3), shows the plan first, and applies it as one transaction like every other writer.
7. **More automatic follow-up.** After any writing command, fairlead runs the follow-up a person would: `sync` for generated files, reference rewriting after a move, and a short "what changed" note. Commands that write still show their plan, and `--dry-run` still writes nothing.

## New surface

| Name | What it does |
|---|---|
| `fairlead map [path or ID]` | The object graph: who declares, uses, imports and owns what. Text for people, JSON for agents and tools. |
| `fairlead config ...` | Governed config editing (D-62): re-glob or retire a rule, raise a pin, add a table. Each is a plan; loosening needs an approval. |
| kind `const` | A declared constant, bound to the symbol below the directive. |
| `check = "query"` | A rule over objects, for example "no object in `app` imports from `finance`" or "every public function in `core` has a doc comment". |
| `check = "require"` | A positive check: something must exist (a test per public function, a changelog line per release). |
| `[content]` | Which languages to parse, object limits, cache settings. |
| `[agent-mode]` | The optional agent mode below. |

The rule language for `query` is designed in 0.4's first phase. It must be small, declarative and readable in a control file, with no scripting and no network.

## Agent mode (optional, off by default)

Some teams run fairlead with agents only and want no human approval step. Agent mode allows that, but it weakens a guarantee fairlead has kept since 0.1: an agent cannot approve its own exception (D-03). So it is bounded:
- Only an owner turns it on, through an approved control change: the last human act it needs.
- It covers only the scopes and the version range the owner names, and expires with that range.
- Floor rules never yield, and items sealed outside those scopes still need an owner.
- Every approval it makes is recorded as `approver = "agent-mode"`. `seal`, `context` and `check --ci` list these approvals, and a CI setting can fail on them for protected branches.
- An owner can turn it off, and revoke its approvals, at any time.

## Parsing

Languages follow the repositories fairlead is used on: Rust, TypeScript and JavaScript, CSS, Markdown, TOML and JSON first. Parsers are tree-sitter grammars pinned to exact versions (risk 9 of the roadmap) or the existing pure Rust parsers (TOML, Markdown). The MSRV rises to 1.90 with tree-sitter (R-REL-006), announced in the 0.4 changelog. A file whose language has no parser keeps today's line-based behaviour.

## Order of work (draft)

1. The object model, the content cache and the parser for Rust; benchmarks before anything else.
2. Constants: the `const` kind, drift, duplicates, repeated literals, unused constants.
3. `map`, text and JSON, including through the MCP server.
4. The `query` and `require` rule types.
5. Reference rewriting on rename and move; automatic follow-up.
6. TypeScript, CSS, Markdown, TOML and JSON parsers.
7. `config` (governed editing) and agent mode.
8. Field test by an agent on a real repository, as for 0.2.

## Owner answers (D-64)

- Q4-1: duplicated code is reported only in 0.4; no proposed merge.
- Q4-2: `check --ci` does not fail on agent-mode approvals by default; failing on them is a setting.
- Q4-3: languages after Rust: TypeScript, then CSS, then Markdown, TOML and JSON.
- The v1 API freeze is at the end of 0.4: 0.4's last phase writes `docs/v1-api.md` and freezes it.
