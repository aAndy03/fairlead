# Task: write nine maintenance tasks for a software project

Attached is the `README.md` of a small project. You are a product lead writing the next nine tasks
that developers will be given, one after another, on this project. Judge only from the README; do
not open other files.

Rules for the list:

- Nine tasks, in the order they will be done. Each later task may build on the earlier ones.
- Every task is done by exactly one of the teams the README names. Say which team in `team`, using
  the team's name exactly as the README writes it (for example `store`, `kit`, `accounts`).
- Three are **simple** (a fix or a small addition inside one area), three **medium** (a change
  across two areas or files of the team), three **complex** (a feature that needs something
  another team owns, or crosses layers or both sides of the project). Mix the tiers; do not put all
  simple tasks first.
- Use the README's "Known issues" for at least two tasks, and its "Ideas" for others; you may add
  your own reasonable tasks too.
- Write each prompt as a developer would receive it: one to four sentences, saying what to do and
  what "done" looks like from a user's point of view. Do not tell the developer how to implement
  it, which files to touch, or any coding rules; do not mention tests.

Reply with only this TOML, nothing before or after it:

```toml
[[task]]
id = "t1"
team = "..."
tier = "simple"
prompt = "..."
```

(nine `[[task]]` tables, ids `t1` to `t9`)
