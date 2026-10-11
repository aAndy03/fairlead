# Task: review a golden repository's expected outputs against the rules

Attached: `golden.md` (a scripted git history in `golden.toml`, each case's command, expected exit
code and expected output file) and the specification chapters whose rules the history exercises.

Go through the cases in order. For each expected output, compare it line by line with what the
rules require at that point of the history:

- the exit code (the chapters and `golden.toml` name what each exit means);
- every finding: its code, severity, path and line, and whether the rule says it must, may or must
  not be reported there;
- every rule-required finding that is missing (a silent pass where a rule demands a report);
- the version numbers, ordinals and written files a `seal` prints;
- the fix hint of each finding: does it name a step that actually resolves it?

Count a difference only when an output contradicts a rule or omits what a rule requires. Wording,
line order fixed by sorting, and counts such as `files=` are not differences. Judge only from the
attached files. Do not run commands or open other files.

Report each difference on its own line, exactly in this form:

ITEM: <expected file> line <n>: <what the rule requires (rule ID)> vs <what the output shows>

Then one final line: `LABEL: clean` when you report no item, otherwise `LABEL: issues`.
