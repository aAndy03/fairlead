# Documentation conventions

> Status: draft · Applies to: every file in `docs/` · Decisions: D-46

The docs are the truth: the human reviewer reads them first, AI agents second. Dense, concrete, no filler, no marketing. No product code (pseudo-syntax and directive examples are fine).

## Header
Every chapter starts with:
```
# <Title>
> Status: draft · Milestone: <0.1|0.2|0.3|mixed> · Decisions: D-xx, D-yy · Related: <file names>
```

## Structure (omit sections that do not apply)
1. **Purpose** (2-4 sentences)
2. **Concepts** (definitions; use the terms in [01-glossary](01-glossary.md) exactly)
3. **Rules** — normative, numbered `R-<AREA>-<nnn>` (AREA = SCOPE, FILE, SYN, CHK, CLI, CTX, VER, ENV, LC, EDN, GIT, HIER, VAR, RET, PORT, AGT, ARCH, ROAD, VIS ...). Use MUST / SHOULD / MAY. Tag each rule with its milestone `[0.1]`, `[0.2]`, `[0.3]`. One rule = one testable statement. Rule IDs are stable: never renumber; retire a rule by marking it `(withdrawn, D-xx)`; add new rules with the next free number.
4. **Examples** — fenced blocks tagged `fairlead` (directives), `toml`, or `text` (trees). Directives obey [12-directive-syntax](12-directive-syntax.md): comment carrier, six verbs `def use ovr flag env end`, IDs `[scope:]kind.name`, TOML frontmatter delimited by `+++`.
5. **Edge cases** — table `# | case | behaviour | rule`.
6. **Resolved questions** — one line each: `Q<chapter>-<n> (resolved <date>): <resolution>; see R-xxx`. Open questions, when they exist, are listed first under **Open questions** as `Q<chapter>-<n>` with options and leaning.

## Rules of maintenance
- [decisions](decisions.md) wins over any chapter. A decision changes only by a new D-xx entry that names what it amends.
- Questions: chapter-local `Q<chapter>-<n>` live at the end of the chapter (files 00, 01, 50 and 60 use `Q-VIS-n`, `Q-GLOS-n`, `Q-ARCH-n` and `Q-ROAD-n`); the global register [70-open-questions](70-open-questions.md) uses `Q-AREA-n`. Under D-46 a question with a stated leaning is resolved as that leaning; resolved questions are folded into normative rules.
- Exit codes are defined only in [15-cli](15-cli.md). Diagnostic codes only in [13-checks-and-diagnostics](13-checks-and-diagnostics.md). File layout only in [11-files-and-classes](11-files-and-classes.md). Control-file schema index only in [12-directive-syntax](12-directive-syntax.md). Other chapters cite them.
- Cross-refer by file name and rule ID. Keep chapters at roughly 1,500 to 5,000 words.
- Names: product `fairlead`, folder `fairlead/`, marker `fairlead:` (D-58); diagnostic codes keep the `HN-` prefix.

## File set
00-vision · 01-glossary · conventions · decisions · 10-scopes-and-inheritance · 11-files-and-classes · 12-directive-syntax · 13-checks-and-diagnostics · 14-context · 15-cli · 20-versions-and-seal · 21-environments-maturity-lifecycle · 22-editions-and-upgrade · 23-git-integration · 30-hierarchy-and-engines · 31-variation-protocol · 32-portability-and-restructure · 33-retention-and-compaction · 40-agent-integration-and-trust · 50-architecture · 60-roadmap · 70-open-questions · examples/{ts-react, rust-workspace, fullstack-five-years} · README
