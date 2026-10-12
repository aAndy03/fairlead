# Agent instructions

Shelf is a UI kit (`kit/`) and two apps (`store/`, `admin/`), each owned by a different team (see
`CODEOWNERS`). Follow these rules in every change.

## Ownership
- Change only the folders of the team the task is for. The kit belongs to the kit team: an app
  team never edits `kit/`.
- If an app needs something the kit lacks (a new button look, a new badge tone), do not copy a kit
  component into the app and change the copy. Write a small wrapper in your app that uses the kit
  component, and note in a short file under your app's `docs/requests/` what you need the kit team
  to add, so the wrapper can be removed once they do.

## Design tokens
- Never write a raw colour, spacing or radius value (`#2563eb`, `8px`, `6px`) in `store/` or
  `admin/`. Use `tokens` from the kit (`tokens.color.primary`, `tokens.space[2]`, `tokens.radius`).
  The values live in `kit/tokens.css` and `kit/tokens.ts`.

## Money
- Prices are integer cents everywhere. Format them only with `formatPrice` from the kit; never
  `toFixed` or `Intl.NumberFormat` in the apps.

## The apps stay apart
- `store/` and `admin/` never import each other. Code both need goes into the kit (by the kit team).

## Before you finish
- Run `npm test` and keep it passing; add tests for what you change.
