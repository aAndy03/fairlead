# Worked example: TypeScript / React frontend

> Status: draft · Milestone: mixed · Decisions: D-02, D-03, D-04, D-05, D-10, D-11, D-12, D-13, D-14, D-16, D-18, D-19, D-20, D-21, D-23, D-31, D-32, D-33, D-34, D-35, D-37, D-40, D-46, D-49 · Related: 10-scopes-and-inheritance.md, 11-files-and-classes.md, 12-directive-syntax.md, 13-checks-and-diagnostics.md, 14-context.md, 15-cli.md, 21-environments-maturity-lifecycle.md, 30-hierarchy-and-engines.md, 31-variation-protocol.md

## Purpose

A Next.js-style React app (`apps/console`) consumes a shared shadcn/Tailwind v4 primitive package (`packages/ui`). The example shows where managerial folders sit, how styles are authored and checked against a token contract, how a child overrides a parent under each override policy, and how a variant request stays visible as a dangling `use` until its home scope declares it.

Tags follow D-06 and D-38: `[0.1]`, `[0.2]`, `[0.3]`. Where a line needs two milestones, the first tags its syntax and the second its check. The open questions of the first draft are resolved in the last section.

## Real-world basis (UNVERIFIED)

Checked through WebFetch summaries. Some text came back as model extraction, so exact values are indicative. Re-check before reuse.

- `shadcn-ui/ui` `apps/v4/components.json`: `tailwind.css: app/globals.css`, `aliases.ui: @/registry/new-york-v4/ui`. UNVERIFIED.
- `shadcn-ui/ui` `apps/v4/app/globals.css`: four `@import`s, 25 `@source` lines, `@theme inline` bridge, `:root` and `.dark` blocks. UNVERIFIED.
- `shadcn-ui/ui` `templates/next-app/app/globals.css`: one line, `@import "tailwindcss";`. UNVERIFIED.
- `satnaing/shadcn-admin` `src/styles/theme.css`: `:root` defines all eight `--sidebar*` tokens as `var()` references; `.dark` defines none. UNVERIFIED.
- shadcn docs: `shadcn add --overwrite` is the only documented overwrite switch, default false; conflict behaviour is not stated. UNVERIFIED.

## Tree

```text
acme/
├─ fairlead/                                    root managerial folder (control.toml: [fairlead] root = true)
│  ├─ control.toml                           [fairlead], [settings], [kinds.K], [policies], [rules.R], [layers.ui], [env] and [maturity] [0.2], [[allow]]
│  ├─ approvals.toml                         [[approval]] records written by fairlead approve (D-34) [0.1]
│  ├─ contract/tokens.md                     token contract: def tok.* directives in frontmatter [0.1]
│  ├─ gen/reg/tok.reg                        derived token index, terse ID k=v lines, sorted [0.1]
│  ├─ gen/baseline.txt                       rollout baseline, written by sync --baseline (D-38) [0.1]
│  ├─ docs/d-7f3a9c21-token-policy.md        ADR; ordinal assigned at seal [0.2]
│  └─ versions/1.6.0.md                      sealed version file; the highest one is the sealed version [0.2]
├─ packages/ui/                              scope ui; parent root
│  ├─ fairlead/control.toml                     [fairlead] scope = "ui"; [[vendor]] for the shadcn files
│  ├─ fairlead/vendored.lock                    path and blake3 hash of each vendor-owned file, terse lines [0.1]
│  ├─ fairlead/docs/d-d4e1b7c2.md               answer change doc (type = "variant"), draft [0.2]
│  ├─ src/atoms/button.tsx                   fairlead-owned wrapper: cmp.button
│  ├─ src/components/ui/button.tsx           vendor-owned (shadcn); never edited
│  ├─ src/components/ui/sidebar.tsx          vendor-owned
│  └─ src/lib/utils.ts                       vendor-owned (cn)
└─ apps/console/                             scope console; parent root
   ├─ components.json                        vendor-owned
   ├─ app/globals.css                        vendor-owned (shadcn CLI); fairlead never writes it
   ├─ app/layout.tsx                         authored; imports globals.css, then src/styles/theme.css
   ├─ fairlead/control.toml                     [fairlead] scope = "console"; [[vendor]] for the shadcn files; no policy
   ├─ fairlead/vendored.lock                    hashes for components.json and app/globals.css [0.1]
   ├─ fairlead/tombstones/2026.toml             [[tomb]] records, reserved forever [0.2]
   ├─ src/styles/theme.css                   authored token overrides and a foreign region
   ├─ src/organisms/app-sidebar.tsx          organism
   ├─ src/organisms/search-beta.tsx          dev-only organism [0.2]
   ├─ src/organisms/sidebar/sidebar.css      leaf: no fairlead folder, resolves to console
   ├─ src/molecules/nav-group.tsx            molecule
   ├─ src/molecules/search-field.tsx         replacement for the tombstoned search box
   ├─ src/molecules/legacy-nav.tsx           deprecated molecule [0.2]
   ├─ src/templates/authenticated-layout.tsx template
   ├─ app/(app)/users/page.tsx               route group: not a scope, no fairlead folder
   └─ src/features/users/                    scope users; parent console
      ├─ fairlead/control.toml                  [fairlead] scope = "users"
      ├─ fairlead/docs/d-7k3m9qxa.md            request doc (kind = "request") [0.3]
      ├─ components/users-primary-buttons.tsx  molecule; needs a variant not yet declared
      ├─ components/users-table.tsx          organism
      └─ data/users-query.ts                 engine (container)
```

Managerial folders sit in four places: the root, `packages/ui`, `apps/console`, and `apps/console/src/features/users`. `organisms/sidebar/` and `app/(app)/` have none and resolve to their nearest ancestor scope (D-11). Route groups need no framework list because they carry no folder; a `[[convention]]` with `transparent = true` (R-SCOPE-018) is the explicit alternative. Vendor-owned files are declared with `[[vendor]]` (R-FILE-040), hash-checked and never written by fairlead (D-04, D-19). Their metadata lives in `vendored.lock`, a generated file in terse lines (D-32), because an annotation inside a vendor file disappears on `shadcn add --overwrite`.

## Root control and contract

```toml
# acme/fairlead/control.toml
[fairlead]                                  # frozen header, parsed first (D-33)
edition = 1
root    = true                           # root marker; nearest wins on lookup (D-31)
scope   = "root"                         # stored identity, not the path (D-11)
marker  = "fairlead"                        # config key (D-30)

[settings]                               # root-only body keys (R-SYN-038)
folder        = "fairlead"
fairlead-version = "0.1.0"                  # binary pin; an older binary refuses with exit 4 (R-CLI-005)
purpose       = "Console and shared UI: shadcn primitives, Tailwind v4 tokens."
owners        = ["@ui-platform", "@console-ui", "@design"]   # owners is a root-only list in 0.1; every ovr owner must be listed here (D-51, R-AGT-028)

[kinds.tok]                              # user-defined kinds, root-only vocabulary (D-02, D-18)
title = "design token"
shape = "doc"
[kinds.cmp]
title  = "component"
shape  = "either"
impl   = 1                               # one code def site per component (HN-D002)
layers = ["ui.*"]                        # ladder binding [0.3] (30)
[kinds.var]
title = "variant"
shape = "either"
[kinds.page]
title = "route page"
shape = "code"

[policies]                               # override policy per key pattern, declared at the root (D-02)
"tok.color.background" = "open"          # nearest scope wins
"tok.color.sidebar.bg" = "open"
"tok.radius.base"      = "needs-exception"   # ovr needs why, owner and exp; approved; ledgered
"tok.color.primary"    = "sealed"            # ovr is inert unless an ancestor grants it
[policies.waiver]
budget   = 25
approval = "required"                    # every effective waiver, including the sealed grant, needs fairlead approve

[rules.dangling-use]                     # tunes HN-R001, which no waiver can reach
why = "a use must resolve to a declared item"

[rules.no-raw-color]                     # literal-leak rule (HN-T001, 0.1); rules are sealed by default
check    = "leak"
tokens   = ["tok.*"]
sev      = "error"
stage    = "observe"
glob     = ["**/*.css", "**/*.tsx"]
waivable = true                          # needs-exception (D-02)
why      = "colours come from tokens"

[env]                                    # [0.2] root-owned, in order (21)
order = ["dev", "staging", "prod"]

[maturity]                               # [0.2] user words mapped to integer ranks (21)
alpha  = 0
beta   = 1
stable = 2

[layers.ui]                              # ladder ranks [0.3]; root-only (30 R-HIER-001)
atom     = 0
molecule = 1
organism = 2
template = 3
page     = 4

[[allow]]                                # grant that unlocks a sealed key for one scope and its subtree (D-34, R-SCOPE-036)
target = "tok.color.primary"
for    = ["console"]
why    = "console may retune the brand colour"
exp    = "2027-12-31"
```

```fairlead
# acme/fairlead/contract/tokens.md  [0.1]  (the contract is a doc; its bodies are def directives, R-SYN-050)
+++
title = "Token contract"
directives = [
  'def tok.color.background sym=--background applies=background-color,color kw=utility sum="page background"',
  'def tok.color.sidebar.bg sym=--sidebar applies=background-color kw=utility sum="sidebar surface"',
  'def tok.color.primary sym=--primary applies=color,background-color,border-color policy=sealed sum="brand colour; retuned only through a grant"',
  'def tok.radius.base sym=--radius policy=needs-exception sum="base corner radius"',
  'def tok.layout.sidebar.width sym=--sidebar-width sum="sidebar rail width"',
]
+++
```

Values stay in authored CSS (D-04); the contract holds names, applicable properties and policy. `sym=` names the custom property, so the value is read from the CSS site (R-CHK-018). `applies=` opts a token into the literal-leak scan (R-FILE-039). `kw=utility` marks tokens bridged by `@theme inline`; that bridge check is 11 R-FILE-035(a), 0.2. A token that is declared only in `:root`, such as `tok.radius.base`, is matched statically: every theme block that declares its base must match it (R-FILE-036). Policy lives only at the root. Children write `ovr` lines and no policy (D-02).

## Directive lines

Grammar: `<comment-token> fairlead: <verb> <id> [key=value ...]`. Only six verbs exist: `def`, `use`, `ovr`, `flag`, `env` and `end`. Values with spaces are quoted. Comment tokens are `//` in TS and TSX, `/* */` in CSS, and `{/* */}` between JSX children. IDs are `[scope:]kind.name`; a bare ID resolves through the scope chain, and `ui:` names the sibling scope of `packages/ui`. Kinds, policies, ranks and environment order are TOML tables (D-05, D-32), so they are not directive lines. A unit's environment and maturity are keys of its `def` (21 R-ENV-010).

### packages/ui/src/atoms/button.tsx [0.1 syntax; layer= 0.3]

```fairlead
// fairlead: def cmp.button layer=ui.atom
```

The vendored primitive is declared by a `[[vendor]]` table in `packages/ui/fairlead/control.toml` and is never edited. Variants live in this wrapper (R-FILE-040, R-FILE-041).

```toml
# packages/ui/fairlead/control.toml
[fairlead]
scope = "ui"

[[vendor]]                               # shadcn output, hash-checked (R-FILE-040)
path = "src/components/ui/**"
tool = "shadcn"

[[vendor]]
path = "src/lib/utils.ts"
tool = "shadcn"
```

### packages/ui/fairlead/docs/d-d4e1b7c2.md [0.2 for the draft change doc; `answers` and `type` 0.3]

```fairlead
+++
id         = "d-d4e1b7c2"                # draft ID: 8 base32 characters (R-VER-040, R-VER-049)
kind       = "change"
scope      = "ui"
type       = "variant"                   # answers a request (R-VAR-040)
status     = "draft"                     # not sealed: the declaration below is not effective yet
impact     = "add"
answers    = ["d-7k3m9qxa"]
created    = "1.6.0"                     # once-only stamp: the open version at creation from 0.2, the as-of date in 0.1 (R-VER-012, D-57)
directives = [
  'def var.button.destructive-soft of=cmp.button sum="soft destructive variant of cmp.button"',
]
+++
```

Body: "Add a soft destructive variant to cmp.button. Requested by console:users."

### apps/console/src/features/users/fairlead/docs/d-7k3m9qxa.md [0.3]

```fairlead
+++
kind    = "request"
id      = "d-7k3m9qxa"
scope   = "users"
need    = "ui:cmp.button"
want    = "soft destructive variant of cmp.button"
because = "delete actions in user lists"
sum     = "Soft destructive variant for users table actions"
out     = ""                            # filled when the request closes (D-39)
created = "1.6.0"
+++
```

### apps/console/src/features/users/components/users-primary-buttons.tsx [0.1 def and use; layer= 0.3; the variant is 0.2 and 0.3]

```fairlead
// fairlead: def cmp.users-primary-buttons layer=ui.molecule
// fairlead: use ui:cmp.button
// fairlead: use ui:var.button.destructive-soft
```

### apps/console/src/organisms/app-sidebar.tsx [0.1; layer= and role= 0.3]

```fairlead
// fairlead: def cmp.app-sidebar layer=ui.organism role=chrome
// fairlead: use cmp.nav-group
// fairlead: use tok.color.sidebar.bg
```

### apps/console/src/molecules/nav-group.tsx [0.1; layer= 0.3]

```fairlead
// fairlead: def cmp.nav-group layer=ui.molecule
```

### apps/console/src/templates/authenticated-layout.tsx [0.1; layer= 0.3]

```fairlead
// fairlead: def cmp.authenticated-layout layer=ui.template
// fairlead: use cmp.app-sidebar
```

### apps/console/app/(app)/users/page.tsx [0.1; layer= 0.3]

```fairlead
// fairlead: def page.users layer=ui.page
// fairlead: use cmp.authenticated-layout
```

### apps/console/src/organisms/search-beta.tsx [0.2]

```fairlead
// fairlead: def cmp.search-beta layer=ui.organism in=dev mat=alpha
```

`in=dev` and `mat=alpha` are keys of the `def`, declared by the kind (21 R-ENV-010). Fairlead checks that `dev` exists in the root order and never selects a build; the CI matrix owns which builds run (21 R-ENV-036).

### apps/console/src/molecules/legacy-nav.tsx [0.1 syntax; checks 0.2]

```fairlead
// fairlead: def cmp.legacy-nav layer=ui.molecule
// fairlead: flag replace cmp.legacy-nav to=cmp.nav-group target=1.7.0
```

`since` is recorded by seal, not written by the author (R-SYN-048).

### apps/console/src/molecules/search-field.tsx [0.1]

```fairlead
// fairlead: def cmp.search-field layer=ui.molecule
```

### apps/console/src/organisms/sidebar/sidebar.css [0.1]

```fairlead
/* fairlead: use tok.layout.sidebar.width */
.sidebar-rail { width: var(--sidebar-width); }
```

### apps/console/src/styles/theme.css [0.1 for ovr with exp=; until= 0.2; foreign region 0.1 (D-35)]

```fairlead
/* fairlead: ovr tok.radius.base op=loosen value=0.25rem why="dense admin tables, DR-14" owner=@console-ui exp=2027-06-30 */
:root { --radius: 0.25rem; }

/* fairlead: ovr tok.color.sidebar.bg op=set value="oklch(0.96 0.01 260)" why="console sidebar tone" */
:root { --sidebar: oklch(0.96 0.01 260); }
.dark { --sidebar: oklch(0.2 0.02 260); }

/* fairlead: ovr tok.color.primary op=set value="oklch(0.55 0.2 264)" why="console brand colour" owner=@console-ui exp=2027-06-30 */
:root { --primary: oklch(0.55 0.2 264); }
.dark { --primary: oklch(0.7 0.15 264); }

/* fairlead: def foreign.theme-generator tool=theme-generator +end */
:root { --chart-1: oklch(0.7 0.1 150); }
/* fairlead: end */
```

The `ovr` for `tok.radius.base` is the needs-exception case: effective once approved, with reason, owner and a date expiry (`exp=`, 0.1; from 0.2 the version form `until=<version>` is the default, D-37). The `ovr` for `tok.color.sidebar.bg` is the open case: nearest scope wins, and it needs only `why`. The `ovr` for `tok.color.primary` is the sealed case: the root `[[allow]]` makes it effective, and the approval gate still applies because the root sets `approval = "required"`. One `ovr` covers every theme in the contract, so the `.dark` block is mandatory.

```toml
# acme/fairlead/approvals.toml  (written by fairlead approve; reviewed via CODEOWNERS)
[[approval]]
target   = "tok.radius.base"
scope    = "console"
fp       = "k3m9q2x7a1bz"
approver = "@console-ui"
date     = "2026-10-08"
renewal  = 0
```

The record is written to the root `approvals.toml` shown in the tree (D-34); the `ovr` it approves stays in `apps/console`.

### apps/console/fairlead/tombstones/2026.toml [0.2]

```toml
[[tomb]]                                 # written by seal when cmp.legacy-search is removed (R-LC-020)
id      = "cmp.legacy-search"
removed = "1.4.0"
by      = "cmp.search-field"
why     = "old search box; no policy carried over"
```

A tombstone is never a directive. The ID stays reserved: a later `def` with the same ID is an error (R-LC-021).

## Check and context output (illustrative)

`fairlead check` run at the root after the lines above. Codes come from [checks](13-checks-and-diagnostics.md).

```text
$ fairlead check
fairlead check 0.1.0 edition=1 as-of=2026-10-09 scopes=4
errors 3 warn 0 info 0 | baselined 3 | waived 1 | observe 0 | accepted 0
HN-X002 error apps/console/src/styles/theme.css:8 ovr tok.color.primary has no approval; state=waiver-ineffective | fix: fairlead approve <fp>
HN-R001 error apps/console/src/features/users/components/users-primary-buttons.tsx:3 ui:var.button.destructive-soft: a def in draft d-d4e1b7c2 does not satisfy a use (R-VER-060) | fix: seal d-d4e1b7c2, or remove the use
HN-D003 error apps/console/src/molecules/search-field.tsx:1 cmp.legacy-search reuses a tombstoned ID (removed 1.4.0) | fix: give the component a new ID
waivers: console 1 active, 0 expire <=30d | relaxations: [[allow]] tok.color.primary for console (exp 2027-12-31)
exit 1
```

Three cases are not in the output because the listing does not trigger them. Without the `[[allow]]` entry the `tok.color.primary` override is inert and reports HN-X005 (warn). A `def tok.radius.base` in `users` without an `ovr` reports HN-R005 (D-12). A hand edit to `packages/ui/src/components/ui/button.tsx` reports HN-G005 (VENDOR-EDIT); a regeneration by the shadcn CLI reports HN-G004 (VENDOR-CHANGED, warn), and a stale generated shard reports HN-G001 with exit 3.

`fairlead context apps/console/src/features/users` (L1 scope card; format `ID k=v` per D-23 and [context](14-context.md)):

```text
$ fairlead context apps/console/src/features/users
@users d=7bq4x2ec ed=1 ^console:c4q7w2bd ^root:dq3f2xbr
inh: ^root dq3f2xbr | ^console c4q7w2bd
rules: rule.dangling-use sealed | rule.no-raw-color needs-exception (waivable)
tok.radius.base needs-exception (ovr console: "dense admin tables, DR-14" exp=2027-06-30)
tok.color.primary sealed (grant console; its subtree includes users)
uses: ui:cmp.button resolved=packages/ui/src/atoms/button.tsx
uses: ui:var.button.destructive-soft DANGLING draft=d-d4e1b7c2
=d 9ab1c2de next: fairlead explain ui:var.button.destructive-soft
```

The L1 card is [0.1]. `exp=` is [0.1]; a version expiry (`until=`) needs seals [0.2]. `layer=` needs the ladder [0.3]. `draft=` needs change docs [0.2]; `DANGLING` on a request-backed use needs the variation protocol [0.3]. Budgets are the 0.1 defaults: 500 tokens for the brief and 700 for a path query (R-CTX-044); `--since` is [0.2] (R-CTX-051).

Generated files and the baseline open with an edition-only header, so a binary upgrade within edition 1 never makes them stale (R-FILE-003, D-55). The shard below belongs to `apps/console`; the baseline is the root's `fairlead/gen/baseline.txt`:

```text
# scope console edition=1 body=t4kq2mx7bd3e5fvh6cnw
tok.color.primary sym=--primary light=oklch(0.55 0.2 264) dark=oklch(0.7 0.15 264) src=src/styles/theme.css

# baseline edition=1 body=w3hb6ncq2xtd5mkr7e4f
7fq3a2kcxe9p code=HN-T001 path=apps/console/src/features/users/components/users-table.tsx
```

The baseline keeps one terse line per tolerated finding (three here, one shown); baselined findings never fail the run (R-CHK-030).

Milestones: HN-X002, HN-R001 and the baseline are [0.1]; the baseline is written by `fairlead sync --baseline` (D-38). HN-D003 needs tombstones [0.2]. Draft status needs change docs [0.2]; a draft `def` counts only once its doc is sealed (R-VER-060). The request doc and the variant answer follow the variation protocol [0.3] (31 R-VAR-040).

## What this example shows

1. Managerial folders are optional. Four scopes exist; the sidebar leaf and the route group have none and resolve upward (D-11).
2. Policy is declared once, at the root. Children write `ovr` only. Open, needs-exception and sealed give three different results in one run (D-02).
3. `allow` is a `[[allow]]` table in the granting scope, not a verb. Approval is an `[[approval]]` record in `fairlead/approvals.toml`, written by `fairlead approve`. With `approval = "required"` the sealed grant needs approval too, and CODEOWNERS review of the root folder is the gate (D-03, D-34).
4. Styles: authored CSS holds the values, the contract and index check them, vendor-owned shadcn files are never edited, and variants live in a fairlead wrapper. The harness stylesheet is imported from authored `app/layout.tsx`, not from vendor-owned `app/globals.css` (D-04).
5. A dangling `use` keeps the need visible. A `def` in a draft change doc does not satisfy it (20 R-VER-060; HN-R001), and the request and its answer follow the variation protocol (31 R-VAR-031, R-VAR-040; D-21 [0.3]).
6. No per-file version headers. `versions/<version>.md` is written only at seal (D-13, D-14); the `created` stamp is written once, at creation: the as-of date in 0.1, the open version from 0.2 (R-VER-012, D-57).
7. Removed IDs leave a `[[tomb]]` record in the scope folder and stay reserved (D-16; R-LC-020).

## Resolved questions

- Q-EXTS-1 (resolved 2026-10-09): import order is checked as the stylesheet order and `@layer` statement by HN-T008 (0.2), and a shadcn rewrite that drops foreign text is HN-G006 (0.2); until then the example imports from authored `app/layout.tsx` and the question does not arise; see R-CHK-047, R-FILE-035(c), R-FILE-043.
- Q-EXTS-2 (resolved 2026-10-09): contract-declared derivations are HN-T005 from 0.1 and graph-detected ones are HN-T010 from 0.2; the `@theme inline` bridge and cascade order are HN-T008 and HN-T009 (0.2, R-CHK-046, R-CHK-047); a Tailwind palette reference such as `var(--color-blue-300)` must sit in a foreign region or be declared external, so the example keeps `--chart-1` as a literal inside its region; see R-FILE-035(e).
- Q-EXTS-3 (resolved 2026-10-09): yes; `env` gates are checked for existence and references and never evaluated (D-16), so a prod unit that uses a dev-only ID is HN-E001 (0.2) and bundling is out of scope; see R-ENV-030.
- Q-EXTS-4 (resolved 2026-10-09): superseded: in 0.1 a version-shaped expiry is HN-S012 (error) and the `ovr` is dropped, not an info diagnostic; the 0.1 waivers here use `exp=` dates; see R-SCOPE-045, R-SYN-012.
