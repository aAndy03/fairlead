# 0.1 owner runbook: the human-only runs

> **Status: not required.** The owner accepted 0.1 as built on 2026-10-10 ([D-59](../../decisions.md)). The runs below are waived, not passed, and nothing may claim their results. From 0.2 the same kinds of evidence are gathered by AI agents ([../agent-runbook.md](../agent-runbook.md)). This file is kept as a record of what 0.1 would have needed.

In PowerShell, quote handles: `--as '@aAndy03'` (a bare `@name` is splatting and arrives empty).

These steps finish the 0.1 exit criteria. Agents cannot do them for you (A-68). Record each run in `docs/plans/0.1/results/<date>-<name>.md`. Note the model and its settings, the date, the commit hash and anything unusual. The thresholds are frozen in [ablation-protocol.md](ablation-protocol.md), and a changed rule invalidates earlier runs.

Suggested order (cheapest first): 1, 2, 3, 4, 5.

## 1. Benchmark baseline (E6), about 15 minutes, mostly waiting

1. On GitHub, go to Actions, then **bench**, then **Run workflow** on `main`.
2. When it finishes, download the artifact with the results. Each OS has a `results.toml` with `cold_s`, `warm_s`, `changed_s`, `files_from_s` and `peak_rss_mb`.
3. If any value is over its limit (cold 60 s, warm 5 s, changed 1 s, 500 MB), the gate failed. Open an issue and stop here.
4. Otherwise, copy the Linux values into `bench/baseline.toml` as plain `key = value` lines below the comments. Commit them. From then on a 25% regression fails the workflow.

To run it locally instead (Windows, slower), run the following commands, then time `fairlead check` in that folder yourself:

```bash
cargo build --release -p fairlead
```

```bash
cargo xtask bench-corpus generate --files 100000 --out target/bench/corpus
```

## 2. Syntax-card trial (E8), about 1 to 2 hours

Goal: fresh agent sessions, given only the syntax card, write 100 directives. At most 5 may be invalid.

1. Print the card and save it:
   ```bash
   cargo run -p fairlead -- explain syntax
   ```
2. Make 100 small prompts that ask for one directive each. Spread them over hosts (TS, TSX, Rust, CSS, Markdown, TOML) and verbs (`def`, `use`, `ovr` with `why`/`owner`/`exp`, `flag`, regions with `+end`). Example: "Here is a TSX file. Add a fairlead directive saying this component uses the token tok.color.primary. Reply with the whole file."
3. Run each prompt in a **new** session whose only context is the card. No repo access, and don't correct its answers.
4. Save each answer as `trial/syntax-card/NNN.<ext>` (001 to 100), with the real file extension, outside the repo or git-ignored.
5. Score it:
   ```bash
   cargo xtask syntax-card-score trial/syntax-card
   ```
   It prints `samples=100 invalid=N` and exits 0 if N is 5 or less.

## 3. Adoption trial (E8), 15 minutes per person

Find someone who has never read the spec. Give them the binary and an unfamiliar repo. Time them from install to the first green `fairlead check`. They must run `init`, then write a baseline, then reach a green check. Record the minutes and which docs they opened. The limit is 15 minutes.

## 4. Controlled test, E2 (wedge effect), the big one: 180 sessions

20 tasks × 3 arms × 3 runs. Budget a few days. A script that drives your agent CLI helps.

**Prepare (once):**
1. Generate arm B's file:
   ```bash
   cargo xtask ablation-arm-b --out ablation/arms/b-agents.md
   ```
2. For each task, make a clean copy of its fixture (`tests/golden/<fixture>/repo`) and run `git init`, then commit.
3. Shuffle all 180 (task, arm, run) triples with a seed you write down. Give each one a random opaque id such as `k3f9`. Keep the id-to-(arm, task) map in a private file, `map.csv`, with the header `diff_id,arm,task`. Do not look at it while scoring.

**Each session** (fresh, same model and settings throughout; record them):
- **Arm A:** the task prompt from `ablation/tasks.toml` only, with `fairlead/` hidden or deleted from the copy.
- **Arm B:** same as A, plus `ablation/arms/b-agents.md` placed as `AGENTS.md` in the copy.
- **Arm C:** the fixture as is. Run `fairlead context <the path the task touches>` and paste its output at the start of the session.
- When the agent finishes, save `git diff` as `diffs/<id>.diff`.

**Score blind, then unblind:**
```bash
cargo xtask ablation-score --diffs diffs --out scores.csv
```
```bash
cargo xtask ablation-report --scores scores.csv --map map.csv
```
The report prints the mean hardcoded values per arm and track, the C-vs-A reduction and the verdict: `pass` at 50% or more, `weak` under 20%. Arm B is printed but never gated. Write down the threats listed in the protocol (model drift, fixture ease, agents reading `fairlead/` directly).

## 5. The rest of the protocol

- **E3 exception hygiene:** 12 deviation tasks on a three-scope fixture, arms A and C, classified (a) to (d). Needs a fixture built for it; ask an agent to build one from the protocol text first.
- **Precision:** 200 seeded-random findings from three real repos, judged blind. At most 2% may be false.
- **Bytes per token:** run `fairlead context` on a few paths, count tokens with the target model's tokenizer, and record bytes/token in `60-roadmap.md` section 11.

When results land, an agent can tabulate them, but the judgement calls stay yours.
