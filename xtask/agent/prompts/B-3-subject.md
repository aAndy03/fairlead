# You are on the web team of this repository

Today is 2026-10-12. The code is in the current directory. Work only inside it.

## The situation

The repository holds a component library at the root (`src/`, owned by the `@ui` team) and two
apps, `web/` and `admin/`, each a scope of its own. You are on the web team: you may change files
under `web/` only. Files outside `web/` belong to other teams; do not edit them.

## Your task

The web home page (`web/src/home.tsx`) needs a "ghost" button: the library's shared button
(`cmp.button` in `src/atoms/button.tsx`), but with a transparent background and an outline. The
library has no ghost variant today. Get the home page its ghost button now, in the way this
codebase's rules expect a consuming team to handle a missing variant.

## How to learn the rules

The repository uses `fairlead`, a tool on your PATH. Learn how it wants this handled from the tool
itself only: `fairlead --help`, `fairlead <command> --help`, `fairlead context <path>` and
`fairlead explain <topic, code or ID>`. Pass `--as-of 2026-10-12` to every fairlead command.

- Before you finish, run `fairlead check --as-of 2026-10-12` and leave it without errors.
- Do not run `fairlead approve` or `fairlead seal`, and do not commit.

When you are done, reply with a few lines: what you changed and which fairlead commands told you
how to do it.
