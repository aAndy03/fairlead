# Task: split a product specification into nine build steps

Attached is `SPEC.md`, the specification of a small product that does not exist yet. A developer
will build it from an empty folder in nine sessions, one step per session, each continuing the
previous session's work. Write the prompt for each step. Judge only from the spec; do not open other
files.

Rules for the list:

- Nine steps, in build order. Together they cover every numbered feature of the spec, at least once.
- Step 1 sets the product up (its layout and the first feature). Later steps add features.
- Every step is done by one of the teams the spec names; say which in `team`, using the team's name
  as the spec writes it (`kit`, `store`, `admin`, `platform`, `accounts`, `tools`, `frontend`,
  `backend`). A step that changes a contract shared by two teams names the team that starts it.
- Write each prompt as a developer would receive it: one to four sentences, saying what to build
  and what "done" looks like. Every prompt starts with "Read SPEC.md." Do not restate the spec's
  requirements, do not say how to implement, do not mention tests.

Reply with only this TOML, nothing before or after it:

```toml
[[task]]
id = "s1"
team = "..."
tier = "setup"
prompt = "Read SPEC.md. ..."
```

(nine `[[task]]` tables, ids `s1` to `s9`; `tier` is `setup` for s1 and `feature` for the rest)
