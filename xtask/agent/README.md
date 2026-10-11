# Agent protocol runners (0.2)

Runners for the agent protocols of [docs/plans/agent-runbook.md](../../docs/plans/agent-runbook.md)
(decision D-59). Windows PowerShell 5.1 or later, `opencode` on the PATH. Each sample is a fresh
`opencode run` session in an empty folder under the system temp directory; answers are stored under
opaque IDs with the private `map.csv` beside them; the result goes to
`docs/plans/0.2/results/<date>-<protocol>.md`, labelled agent-run. Transcripts stay outside the
repository; only their hashes are recorded.

| Script | Protocol | Run it |
|---|---|---|
| `A-1.ps1` | eight-week pilot on three public repositories | no arguments needed: three suggested repositories and the release build; `-Check` shows what would run (clones the repositories) |
| `A-2.ps1` | label the coverage-replay CSV | after `cargo xtask coverage-replay`; `-Csv <csv> -Repo <path>` |
| `A-3.ps1` | threshold amendment draft | after A-1 and A-2; `-A1 <result.md> -A2 <result.md>` |
| `A-4.ps1` | changelog review | after the release seal, before the owner tags |
| `A-5.ps1` | golden review | the six 0.2 git-case goldens by default |

`selftest.ps1` parses every script and checks the shared helpers without running a model.
Agents never approve and never tag: a protocol that needs either stops for the owner.
