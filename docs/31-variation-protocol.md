# Variation Protocol
> Status: draft · Milestone: 0.3 · Decisions: D-02, D-04, D-12, D-14, D-16, D-18, D-20, D-21, D-34, D-37, D-40, D-42 · Related: 10-scopes-and-inheritance.md, 11-files-and-classes.md, 13-checks-and-diagnostics.md, 30-hierarchy-and-engines.md, 32-portability-and-restructure.md, 40-agent-integration-and-trust.md

## Purpose

Reusable units (a button, a validator, a token) attract two failure modes: every consumer forks its own copy, or the primitive grows a flag for every consumer's wish. The variation protocol gives a consumer exactly four legitimate moves, makes the primitive's owner the single place where a primitive changes, and detects the illegitimate moves: copies, and hardcoded values that should be tokens.

## Concepts

| Term | Definition |
|---|---|
| Home scope | The scope whose registry declares a unit. Variations of the unit are decided there. |
| Visibility | `vis=subtree` (default: the home scope's subtree) or `vis=public`. Gives "home" teeth. |
| Axis | A declared dimension of a unit with a closed value set (`density = sm, md, lg`). Using a declared value needs no protocol. |
| Variant | A new unit declared in the home scope with `variant-of=<base>`; or a new value on an axis. |
| Request | A consumer-scope document asking the home scope for a variation. |
| Interim wrapper | A consumer-side composition of the primitive's public surface, flagged `awaiting` a request, with an expiry. |
| Adapter | A permanent wrapper that is the home of a primitive that the owner will not change (rejected request) or cannot change (external library). |
| Fork | A copy-and-edit of a unit. Declared (an exception) or undeclared (a violation). |
| Token | An ID'd design value (`tok.*`) with a contract; its value lives in authored CSS (D-04). |

## The four outcomes (D-21)

| # | Need | Outcome | Cost to consumer |
|---|---|---|---|
| 1 | Value already on a declared axis | Use the existing axis. No protocol. | none |
| 2 | New value, new axis, or new behaviour | Request a variant; it becomes a doc-first change in the home scope. | a request doc |
| 3 | Cannot wait for the home scope | Wrap locally: an interim wrapper with `flag awaiting` and an expiry. | wrapper plus expiry |
| 4 | Home scope rejects, or the unit is external | Permanent adapter, declared and linked to the rejection. | adapter unit |

Not a variation: the consumer's own new concern (a sidebar surface colour) is a new unit or token in the consumer scope with `extends=`. If a second consumer appears, `hier.promote` fires ([30-hierarchy-and-engines.md](30-hierarchy-and-engines.md), R-HIER-071). Copy-and-edit is forbidden outside a declared fork.

## Rules

### Home scope and visibility
- **R-VAR-001** [0.3] Every reusable unit MUST have exactly one home scope: the scope whose registry declares it. A second declaration of the same ID is `HN-D001` (duplicate ID, D-12; [checks](13-checks-and-diagnostics.md)).
- **R-VAR-002** [0.3] The home of a unit MUST change only through `fairlead move`, never by copying.
- **R-VAR-010** [0.3] A `use` from outside the unit's visibility is `hier.visibility`, with fix hint "promote to the lowest common ancestor scope X, or mark `vis=public`". `vis=subtree` covers the home scope and its descendants; `vis=public` covers the codebase. The finding is a warning before the scope's `hier.visibility` stage reaches `enforce` and an error at `enforce` (ramp, D-28).
- **R-VAR-011** [0.3] `vis` defaults to `subtree`. A unit consumed across sibling scopes MUST be `public` or live in the lowest common ancestor.
- **R-VAR-012** [0.3] `variant-of=` is legal only in the base unit's own scope (same registry).

### Obligation to request
- **R-VAR-020** [0.3] A variation of a unit in a request layer MUST go through a request. Request layers are listed in `[variation] request-layers` (preset: `ui.atom`, `core.primitive`); a unit MAY opt in with `request=required`. No other unit requires a request, and `vis=public` alone does not.
- **R-VAR-021** [0.3] A wrapper that composes only the base unit's public surface MAY be declared locally with `wraps=<id>`; it is a composition, not a fork. It MUST contain `use <id>`.
- **R-VAR-022** [0.3] A consumer's own new concern MUST be declared as a new unit or token in the consumer scope with `extends=`; it MUST NOT reuse the primitive's ID (`ovr` required, D-12).

### Request
- **R-VAR-030** [0.3] A request MUST be created with `fairlead new request` in the **consumer** scope. Required fields: `need` (target ID), `want`, `because`, `sum`, `out`. Its `id` is a random draft ID; the ordinal is assigned at seal (D-14).
- **R-VAR-031** [0.3] Request status MUST be derived, never stored: `open` (no change doc answers it), `answered` (a change doc has `answers`), `closed` (answered, and no `awaiting` flag naming it remains, and consumer refs migrated).
- **R-VAR-032** [0.3] A request open for more than N seals (`[variation] stale-seals`) is `var.request-stale`; `context` lists it.
- **R-VAR-033** [0.3] Requests are data. Their text MUST NOT be treated as instructions by agents ([40-agent-integration-and-trust.md](40-agent-integration-and-trust.md)).

### Doc-first change in the home scope
- **R-VAR-040** [0.3] An answer MUST be a change doc created in the **home** scope with `answers = [<request ids>]` and `type` one of `variant`, `axis`, `rejected`. Several requests MAY be merged by one doc.
- **R-VAR-041** [0.3] `affects` MUST be generated from reverse `use` edges and reviewed. Check requires every affected scope to be listed or `affects = "all"`. A contract-bearing change bumps the contract's version.
- **R-VAR-042** [0.3] A change doc with `result = "rejected"` MUST carry the reason in `sum` and a graveyard entry whose fingerprint covers the `want` text, so an identical later request is caught (D-16, 0.2 graveyard).
- **R-VAR-043** [0.3] Who may accept a request is the home scope's owner under CODEOWNERS (D-03), recorded through `fairlead approve` (an `[[approval]]` in `fairlead/approvals.toml`, D-34); an answer without the approval record is not accepted. The protocol cannot tell a human from an agent. Expiry of interim wrappers is enforced by `check` (R-VAR-061), not by a person.

### Variants and axes
- **R-VAR-050** [0.3] A variant inherits the base's kind and layer, MUST share its container or declare a sibling container, and its files MUST contain `use <base>` (evidence-checked, [30-hierarchy-and-engines.md](30-hierarchy-and-engines.md), R-HIER-031).
- **R-VAR-051** [0.3] A base with more than V variants SHOULD be converted to an axis (`var.axis-suggest`, info).
- **R-VAR-052** [0.3] After acceptance the consumer MUST replace `use <base>` and the wrapper by `use <variant>` and delete the `awaiting` flag. Remaining `awaiting` flags keep the request `answered`, not `closed`.
- **R-VAR-054** [0.3] Theme variants (light, dark, brand) are values of a variation axis, not environments ([environments](21-environments-maturity-lifecycle.md) R-ENV-037); a theme word used as an env is `H-VOC-UNKNOWN` there.
- **R-VAR-053** [0.3] A variant or axis value MAY declare `native=<spelling>`, an alias to the native name (a `cva` variant key, a Tailwind `@custom-variant` name, a CSS class). The fairlead ID is the identity; native spellings are aliases only, are never resolved as IDs, and MUST be unique among one unit's variants.

### Interim wrappers
- **R-VAR-060** [0.3] An interim wrapper MUST carry `flag awaiting to=<request id> target=<version>` on its region (D-49: `flag` takes `target=`, never `until=`). `awaiting` is a flag word reserved by this protocol.
- **R-VAR-061** [0.3] Expiry is version-based (D-42): info when one version from `target`, warn when due next, error once the open version is `>= target`. Date expiry (`exp=YYYY-MM-DD`) is opt-in with `--as-of` (D-37).
- **R-VAR-062** [0.3] An `awaiting` flag whose request does not exist, is closed, or was rejected is an error. Rejection converts the wrapper to an adapter (R-VAR-070) or removes it.
- **R-VAR-063** [0.3] Interim wrappers are rolled up into the exception ledger at seal; sprawl is reported (D-28).
- **R-VAR-064** [0.3] `awaiting` is a reserved flag word, not an `ovr`: the wrapper needs no `why=`/`owner=` of its own (the request carries them) and its ledger entry is derived at seal (R-VAR-063).

### Adapters and external libraries
- **R-VAR-070** [0.3] A permanent adapter MUST declare `adapter=true` and either `rejected-by=<change doc id>` (a doc with `result = "rejected"` in the home scope) or `wraps=<ext id>`.
- **R-VAR-071** [0.3] An external primitive MUST be declared as `def ext.<name>` at the boundary scope with the import specifier and path. Its files are vendor-owned (D-04): hash-checked, never written by fairlead, exempt from change-doc coverage, and carry no directives.
- **R-VAR-072** [0.3] The adapter is the home of the external primitive's variants. All consumers MUST `use` the adapter. The `ext` import specifier appearing in files outside the adapter is `var.adapter-bypass` (error).
- **R-VAR-073** [0.3] A changed vendor-owned hash (CLI update, `--overwrite`) is `var.vendor-changed`, the alias of `HN-G004` with severity `warn` ([checks](13-checks-and-diagnostics.md) R-CHK-050), and requires a change doc touching the adapter.
- **R-VAR-074** [0.3] Naming and import reconciliation: the adapter owns the unprefixed unit name (`cmp.button`); the vendor file is `ext.<vendor>-<name>` (`ext.shadcn-button`). The adapter declares `import-as=<specifier>`; consumers MUST import only that specifier, and an import of the `ext` specifier outside the adapter is `var.adapter-bypass` (R-VAR-072). The check is an AST-free grep over import specifiers.

### Forks
- **R-VAR-080** [0.3] A declared fork MUST be `ovr rule.var-fork of=<id> why="..." owner=... until=<version>`; it is an exception ledger entry and expires like a waiver.
- **R-VAR-081** [0.3] Undeclared-fork detection is declared-only plus name heuristics, all AST-free: (a) the same `sym` or basename defined in two scopes; (b) a file defining a symbol that shadows a declared `sym`. Any hit is `var.fork-suspect` (warn). Similarity measures (token shingles, minhash) are not part of 0.3; the `[variation] similarity` key is reserved, default `off`, and unspecified until thresholds are measured.
- **R-VAR-082** [0.3] A suspect pair MAY be dismissed with `ovr rule.var-fork of=<id> dismiss=true pair=<hash> why="..."`. The pair hash covers the two declaring locations (scope ids plus `sym` or basename) and is stored; the pair is re-flagged only if a third location appears.
- **R-VAR-083** [0.3] Generated and vendor-owned files MUST be excluded from fork, rank and token scans.
- **R-VAR-084** [0.3] Fork detection ships off: `[variation] fork-detect` defaults to `"off"`, and when enabled starts at stage `observe` (D-28).

### Hardcoded token detection
- **R-VAR-090** [0.3] A token contract MUST declare `css=` (the custom property that carries its value in authored CSS) and `applies=` (the CSS properties it may stand for). The value is read from authored CSS into the derived index and MUST NOT be copied into a `def` (D-04), except under a per-kind generate-instead option.
- **R-VAR-091** [0.1] (the literal-leak check ships in 0.1 per D-43; the rest of this chapter is 0.3) `var.token-hardcoded` fires only when a literal equals a token value, in any theme, **and** appears in a property listed in that token's `applies`. A value match without property context MUST be info at most.
- **R-VAR-092** [0.3] Property context is extracted without an AST from: CSS declarations (`prop: value`); JS/TS style objects (`backgroundColor` is read as `background-color`); presentation attributes (`fill=`, `stroke=`); Tailwind arbitrary values (`bg-[#3B82F6]`) via a prefix-to-property table in the control file. Unsupported contexts are skipped, not guessed.
- **R-VAR-093** [0.3] Before comparing, colours MUST be canonicalised: hex (short, long, alpha), `rgb()` and `hsl()` map to one sRGB form; `oklch()` and `color()` are compared exactly after number normalisation, without gamut conversion. Lengths are unit-aware (`px`, `rem` with `[tokens] root-px`, default 16; unitless numbers on length properties in JS style objects are px). Unparseable formats are skipped.
- **R-VAR-094** [0.3] Detection runs only in `style-governed` globs. Token definition sites, generated, minified and vendor-owned files are excluded.
- **R-VAR-095** [0.3] The message MUST carry the replacement from the token's `use-css` / `use-tw` key for the file's language. A near miss (per-channel distance at most `[tokens] near`, default 2) is `var.token-near` (info).
- **R-VAR-096** [0.3] Constants with distinctive literals (URLs, long numbers, strings) are scanned only when the token declares `match=`. Ambiguous values (`5`, `0`) MUST never be scanned.
- **R-VAR-097** [0.3] A local intentional override uses `ovr <token id> why=... owner=... until=<version>` and rolls up to the ancestor's ledger. A child token with the same name as an ancestor token without `ovr` is `HN-R005` (shadowing without `ovr`, D-12); a child token with `extends=` is a new item.
- **R-VAR-098** [0.3] The derived index resolves `var()` chains one level (`--sidebar: var(--background)` takes the value of `--background`). A deeper chain is recorded `unresolved` with the chain text, is never compared by `HN-T001`, and is reported as `var.token-unresolved` (HN-T007, info). Several sources for one token (Tailwind v4 `@theme inline`, themes) are listed per theme, not merged.
- **R-VAR-099** [0.3] The property-context rules of [checks](13-checks-and-diagnostics.md) R-CHK-017 to R-CHK-022 are 0.1 (D-43; R-CHK-049). R-VAR-090 to R-VAR-098 add to them (near misses, `rem` equivalence, Tailwind arbitrary values, `var()` chains) and never weaken them.

## Examples

### Request, answer, variant (consumer `app`, home `ui`)

```toml
+++
kind = "request"
id = "d-7k3m9qxa"
scope = "app"
need = "ui:cmp.button"
want = "density axis value compact (24px height, 8px padding)"
because = "dense data tables"
sum = "Compact button for dense tables"
out = ""
created = "0.4.0"
+++
```

Interim code in the consumer while the request is open:

```tsx
// fairlead: def cmp.table-button layer=ui.molecule container=ctr.tables wraps=ui:cmp.button
// fairlead: use ui:cmp.button
// fairlead: flag awaiting to=d-7k3m9qxa target=0.4.0
```

Answer, written in the home scope:

```toml
+++
kind = "change"
id = "d-91cc04ab"
scope = "ui"
type = "variant"
answers = ["d-7k3m9qxa"]
touches = ["cmp.button"]
result = "cmp.button-compact"
affects = "auto"
status = "ready"
created = "0.4.0"
sum = "Add compact variant on the density axis"
out = "adopted"
+++
```

```fairlead
// fairlead: def axis.button-density of=cmp.button values=sm,md,lg default=md
// fairlead: def cmp.button-compact layer=ui.atom container=ctr.kit variant-of=cmp.button axis=button-density:compact native=compact vis=public
```

The consumer then writes `// fairlead: use ui:cmp.button-compact`, removes the wrapper and the `awaiting` flag, and the request becomes `closed`.

### Rejection and permanent adapter

If `d-91cc04ab` had `type = "rejected"` and `result = "rejected"`, the consumer keeps its wrapper permanently:

```fairlead
// fairlead: def cmp.table-button layer=ui.molecule container=ctr.tables wraps=ui:cmp.button adapter=true rejected-by=d-91cc04ab
```

### Hardcoded tokens

```fairlead
// fairlead: def tok.color-brand css=--color-brand applies=color,background-color,border-color,fill,stroke use-css="var(--color-brand)" use-tw=bg-brand
// fairlead: def tok.space-4 css=--space-4 applies=padding,margin,gap use-css="var(--space-4)" use-tw=p-4
```

Derived index (generated, terse; values come from authored CSS):

```text
tok.color-brand css=--color-brand light=#3B82F6 dark=#60A5FA
tok.space-4 css=--space-4 value=16px
```

Check output over `Panel.tsx`:

```text
var.token-hardcoded Panel.tsx:44 error #3B82F6 on background-color == tok.color-brand | fix: bg-brand | var(--color-brand)
var.token-hardcoded Panel.tsx:52 error rgb(59 130 246) on border-color == tok.color-brand | fix: var(--color-brand)
var.token-near      Panel.tsx:58 info  #3B83F6 on color ~ tok.color-brand (distance 1)
(no finding)        Panel.tsx:61 fontSize: 16 -> font-size is not in tok.space-4 applies
```

### External library (shadcn)

```fairlead
// fairlead: def ext.shadcn-button vendor=shadcn imports="@/components/ui/button" path="packages/ui/src/components/ui/button.tsx"
// fairlead: def cmp.button layer=ui.atom container=ctr.kit vis=public wraps=ext.shadcn-button adapter=true import-as="@/atoms/button"
```

`shadcn add --overwrite` rewrites only the vendor-owned file; variants live in `src/atoms/button.tsx`. A consumer importing `@/components/ui/button` directly is `var.adapter-bypass`; the only allowed specifier is the adapter's `import-as` (R-VAR-074).

## Edge cases

| # | Case | Behaviour | Rule |
|---|---|---|---|
| 1 | `16px` equals a spacing token but is a font size | Not flagged: `font-size` is not in `applies` | R-VAR-091 |
| 2 | Colour written as `rgb()`, `hsl()`, or short hex | Canonicalised, then compared | R-VAR-093 |
| 3 | Colour in `oklch()` (Tailwind v4 palettes) | Compared exactly; no gamut conversion; near-miss not computed | R-VAR-093 |
| 4 | Dark-theme value hardcoded | Matches the token's dark value; same finding | R-VAR-090 |
| 5 | Computed class names (`bg-${tone}`) | Not scanned; declare a safelist or accept the blind spot | R-VAR-092 |
| 6 | Token value inside the token's own CSS file | Definition site, excluded | R-VAR-094 |
| 7 | Child scope redefines a root token name | `HN-R005` unless `ovr`; `extends=` makes a new item | R-VAR-097 |
| 8 | Home-scope owner is slow, consumer blocked | Interim wrapper with `awaiting` and `target`; `context` lists expiring ones | R-VAR-060 |
| 9 | Interim wrapper outlives `target` | Warn at due-next, error at `>= until`; fix: migrate, or retarget in a change doc | R-VAR-061 |
| 10 | Request rejected while a wrapper is `awaiting` | `awaiting` becomes an error until converted to an adapter or removed | R-VAR-062 |
| 11 | Three consumers request different variants | One home change doc `answers` all three; prefers an axis; each request closes as its refs migrate | R-VAR-040 |
| 12 | Variant changes behaviour for existing consumers | `affects` from reverse `use`; every affected scope must be listed or `all` | R-VAR-041 |
| 13 | Primitive from a sealed ancestor or external library | `ext` declaration plus adapter at the boundary; adapter is the home | R-VAR-071, R-VAR-072 |
| 14 | `shadcn add --overwrite` replaces the vendor file | Only the vendor-owned file is touched; hash change is a warn (`HN-G004`) and needs a change doc on the adapter | R-VAR-073 |
| 15 | Same `sym` or basename in two scopes by coincidence | `dismiss=true` with pair hash; re-flagged only if a third location appears | R-VAR-082 |
| 16 | Renamed copy (new `sym`, new basename) | Out of reach; fork detection is declared-only plus name heuristics, a lower bound, and ships off | R-VAR-081, R-VAR-084 |
| 17 | Rejected request filed again with new words | Graveyard fingerprint match warns at proposal and check time | R-VAR-042 |
| 18 | Consumer in another repo | Invisible to local `affects`; covered only by `requires`/`provides` digests ([32-portability-and-restructure.md](32-portability-and-restructure.md)) | R-VAR-041 |
| 19 | Home moves up the tree after a second consumer appears | `fairlead move` relocates the registry lines; consumers' IDs are scope-qualified and unchanged | R-VAR-002 |
| 20 | Request addressed to a unit with no declared home | Error: `need` must resolve to a declared unit | R-VAR-030 |
| 21 | Token declared as `--sidebar: var(--background)` and `--background: var(--base)` | One level resolved; the second is `unresolved` with the chain, info only, never compared | R-VAR-098 |
| 22 | `cva` variant `size="sm"` and fairlead `cmp.button-compact` | `native=` aliases the spelling; the fairlead ID stays the identity | R-VAR-053 |
| 23 | Consumer outside the home subtree uses a `vis=subtree` unit | Warn before the enforce stage, error at enforce | R-VAR-010 |
| 24 | Wrapper marked `awaiting` has no `owner=` | Valid: the request carries the owner; ledger entry derived at seal | R-VAR-064 |

## Resolved questions

All chapter-local and register questions were accepted by their stated leaning on 2026-10-09 (D-46).

- Q31-1 (resolved 2026-10-09): requests are required by layer list plus per-unit `request=required` opt-in; see R-VAR-020.
- Q31-2 (resolved 2026-10-09): `awaiting` is a reserved flag word, and its ledger entry is derived at seal; see R-VAR-060, R-VAR-064.
- Q31-3 (resolved 2026-10-09): fork detection ships off and starts at `observe` (D-28); see R-VAR-084, R-VAR-081.
- Q31-4 (resolved 2026-10-09): the derived index resolves `var()` chains one level and flags deeper chains as unresolved; see R-VAR-098.
- Q31-5 (resolved 2026-10-09): adapter and vendor names are reconciled by a naming convention and an import-specifier grep; see R-VAR-074.
- Q-VAR-1 (resolved 2026-10-09): fork detection is declared-only plus name heuristics, with dismissal by a stored pair hash; similarity is deferred; see R-VAR-081, R-VAR-082.
- Q-VAR-2 (resolved 2026-10-09): a lint rule plus a naming convention for wrappers and vendor files; see R-VAR-074, R-VAR-072.
- Q-VAR-3 (resolved 2026-10-09): fairlead IDs alias native variant names through `native=`; see R-VAR-053.
- Q-VAR-4 (resolved 2026-10-09): the home owner under CODEOWNERS accepts requests (D-03) and `check` enforces wrapper expiry; see R-VAR-043, R-VAR-061.
- Q-VAR-5 (resolved 2026-10-09): visibility violations warn before enforce and error at enforce; see R-VAR-010.
