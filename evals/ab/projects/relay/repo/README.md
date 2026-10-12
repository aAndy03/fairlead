# Relay

Relay is a small notes app split in two: a TypeScript front end and a Rust back end. They talk
through a line protocol of commands, described in `contract/commands.md`.

## Layout

| Folder | What it is | Team |
|---|---|---|
| `frontend/src/api.ts` | the front end's client for the back-end commands | frontend |
| `frontend/src/notes-view.ts` | the notes screen (renders HTML strings) and title checks | frontend |
| `frontend/src/limits.ts` | limits the front end checks before sending | frontend |
| `frontend/tests/` | front-end tests, with a fake back end | frontend |
| `backend/src/commands.rs` | the command handlers | backend |
| `backend/src/notes.rs`, `wire.rs`, `limits.rs` | the notes store, the wire format, the limits | backend |
| `backend/src/main.rs` | the back-end process: one request per line on stdin, one reply per line on stdout | backend |
| `contract/commands.md` | the commands both sides implement | both teams |

`CODEOWNERS` lists the owners.

## Running

```bash
npm test
```

Runs the front-end tests (Node 22.18 or later, no dependencies) and the back-end tests (`cargo
test`). To try the back end by hand:

```bash
echo "notes.create?title=Milk" | cargo run -q --manifest-path backend/Cargo.toml
```

## Features

- Notes with a title (at most 80 characters) and a body (at most 2000).
- Commands: `notes.list`, `notes.get`, `notes.create`, `notes.delete`.
- The front end checks a title before sending it and shows the notes as a list.

## Known issues

- A note whose title contains `&` or `=` is saved wrongly, because the front end does not encode
  the values it sends.
- The notes list shows titles as raw HTML, so a title like `<b>x</b>` is rendered bold.

## Ideas the teams have mentioned

- Renaming a note.
- Tags on notes, and listing the notes of one tag.
- Searching notes by a word in the title or body.
- Pinning notes so they stay at the top of the list.
- A character counter next to the title field.
