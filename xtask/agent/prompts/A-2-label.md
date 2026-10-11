# Task: label one coverage flag

You review one file that a tool flagged. The tool's rule: every file changed since the last
release must be covered by an open (draft or ready) change doc, a short Markdown note that says
what changed and why, matched through the doc's `covers` globs. The attached context shows the
file's change in one commit, the change docs present at that commit, and the `[docs-first]` table
(its `exempt` globs are files the tool must never flag).

Decide whether the flag was **right** or **wrong**.

- **right**: the change is one a maintainer would want described in a change doc, and no change doc
  present describes it.
- **wrong**: any of these holds:
  - a change doc present already describes this change in substance, and only its `covers` globs
    miss the path;
  - the file is mechanical: a lockfile, generated output, vendored code, formatting or whitespace
    only, or a version-number bump that a release tool writes;
  - the file should be exempt (a glob in `exempt` should match it, or obviously belongs there).

Judge only from the attached context. Do not run commands or open other files.

Answer with two or three sentences of reasoning, then one final line exactly in this form:

LABEL: right

or

LABEL: wrong

If it is wrong because the file should be exempt, name the glob you would add on a line
`EXEMPT: <glob>` before the label.
