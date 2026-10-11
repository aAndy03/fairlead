# Task: review a generated changelog before a release is tagged

Attached: `CHANGELOG.md`, generated from the project's version files, and `sealed-docs.md`, every
sealed change doc of the project (TOML frontmatter between `+++` lines, then the text).

Check, and report each problem you find:

1. Every sealed doc appears in the changelog under the version in its `sealed` key, with its title.
2. The impact word on each changelog line (`break`, `add`, `fix`) matches the doc's `impact`.
3. A `break` is a real incompatibility for users (a licence change, a removed or renamed command,
   flag, file format or behaviour); an `add` or `fix` that hides one is a problem.
4. No line is internal-only noise that a user of the release cannot act on (refactors, test-only
   or CI-only work presented as user-facing changes).
5. Titles are clear to a user who has not read the code.

Judge only from the attached files. Do not run commands or open other files.

Report each problem on its own line, exactly in this form:

ITEM: <version> <doc id or line>: <the problem and the fix you suggest>

Then one final line: `LABEL: clean` when you report no item, otherwise `LABEL: issues`.
