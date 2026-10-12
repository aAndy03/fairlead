# fairlead

fairlead keeps a codebase consistent for AI agents and human developers across sessions and years:
declared intent lives in the repository as documentation, small comment directives tie code to it,
`fairlead check` reports drift, and `fairlead context` gives an agent a compact, token-budgeted brief.

Status: pre-alpha. The specification is in [docs/](docs/README.md); the 0.1 implementation plan is in
[docs/plans/0.1/](docs/plans/0.1/00-master.md).

## Using it with an AI agent (MCP)

Each release ships a second program, `fairlead-mcp`, next to `fairlead`. It gives an agent three
read-only tools: `check`, `context` and `explain`. It runs on your machine only: it listens on no
port and makes no network calls. You do not start or stop it yourself: register it once, and the
agent's client starts it with each session and stops it when the session ends.

With Claude Code, from your project folder:

```bash
claude mcp add fairlead -- fairlead-mcp
```

Remove it with `claude mcp remove fairlead`. `fairlead-mcp --help` shows the same instructions. Keep
both programs in the same folder, or on your `PATH`.

## Licence

Copyright (C) 2026 Andy.

From 0.2.0, fairlead is free software under the GNU General Public License, version 3 or (at your
option) any later version (`GPL-3.0-or-later`); the full text is in [LICENSE](LICENSE). It comes
with no warranty, to the extent permitted by law.

Release 0.1.0, already published, was licensed under MIT and stays available under MIT; the change
applies to 0.2.0 and later (decision D-60).
