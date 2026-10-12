# Task: review a command-line tool's public API before it is frozen

Attached:
- `v1-api.md`, the document that lists the surface to be frozen;
- `15-cli.md`, the specification chapter for the commands;
- `surface.md`, the snapshots: each command's `--help` text, the JSON output shapes of `check` and
  `context`, and the table of finding codes.

After the freeze, nothing in this surface can be renamed, removed or retyped without a new
edition. Review it as a maintainer who will have to live with it for years. Report:

1. **Inconsistencies.** The document, the chapter and the snapshots disagree: a flag, verb, exit
   code, JSON key or code that is in one and missing from or different in another.
2. **Unclear flags.** A flag whose help text does not say what it does, or two flags that seem to
   overlap.
3. **Likely-regretted names.** A verb, flag, key or code name that is misleading, inconsistent with
   its siblings (spelling, tense, singular and plural, word order), or too narrow for what it will
   likely need to cover.

Judge only from the attached files. Do not run commands or open other files. Report only concrete
items you can point to; do not report wording you merely prefer.

Report each item on its own line, exactly in this form:

ITEM: <inconsistency|unclear|name>: <where (file and the flag, key or code)>: <the problem> -> <the fix you propose>

Then one final line: `LABEL: clean` when you report no item, otherwise `LABEL: issues`.
