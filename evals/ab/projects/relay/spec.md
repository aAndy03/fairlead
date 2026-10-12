# Relay: product specification

Relay is a notes app split in two: a TypeScript front end and a Rust back end that talk through a
line protocol of commands. No dependencies: the front end runs in Node 22.18 or later (`node
--test`), the back end is a Cargo crate. `npm test` runs both sides' tests.

## Layout and owners

| Path | Contents | Owner |
|---|---|---|
| `frontend/src/` | the client for the commands (`api.ts`), the screens, the limits (`limits.ts`) | frontend team |
| `frontend/tests/` | front-end tests, with a fake back end | frontend team |
| `backend/` (crate `relay-backend`) | the commands (`src/commands.rs`), the store, the wire format, the limits (`src/limits.rs`), a binary reading requests on stdin | backend team |
| `contract/commands.md` | every command, its arguments, replies, errors, the limits and a contract version | both teams |

A `CODEOWNERS` file records the owners.

## Wire format

A request is one line, `<command>?<query>`, the query being `key=value` pairs joined by `&`, keys
and values percent-encoded as `encodeURIComponent` does; a command without arguments has no `?`. A
reply is one line, `ok <payload>` or `err <code>`; a payload is percent-encoded `key=value` fields
joined by `&`, and a list joins its records with `;`.

## Requirements (they hold for the whole life of the product)

1. **One contract.** Every command exists on both sides with the same name and arguments, and is
   listed in `contract/commands.md`. A change to a command changes both sides and the contract
   together, and raises the contract version.
2. **One door.** Only `frontend/src/api.ts` sends requests; the rest of the front end calls its
   functions.
3. **Limits once.** `MAX_TITLE_LEN = 80` and `MAX_BODY_LEN = 2000` are defined once per side
   (`frontend/src/limits.ts`, `backend/src/limits.rs`) and listed in the contract; no other file
   writes `80` or `2000`.
4. **Ownership.** Each change is made by one team and touches only its side, except a contract
   change, which is made on both sides together.
5. **Tests.** Every feature has tests, and `npm test` passes after every change.

## Public names (tests call these)

- Back end: `relay_backend::reply(&mut notes::Store, line) -> String` (one request line in, one
  reply line out), `notes::Store::default()`, `limits::{MAX_TITLE_LEN, MAX_BODY_LEN}`; the binary
  answers each stdin line with one stdout line.
- Front end: `createApi(invoke)` in `frontend/src/api.ts`, where `invoke(request: string):
  Promise<string>` returns the reply line; it returns an object with `listNotes()`,
  `getNote(id)`, `createNote(title, body?)` (resolves to the new id), `deleteNote(id)`; an `err`
  reply rejects with a `BackendError` whose `code` is the error code. `titleProblem(title)` and
  `renderNoteList(notes)` in `frontend/src/notes-view.ts`.
- Commands: `notes.list` (records `id`, `title`), `notes.get` (`id` -> `id`, `title`, `body`),
  `notes.create` (`title`, optional `body` -> `id`), `notes.delete` (`id`). Errors: `not-found`,
  `bad-id`, `empty-title`, `title-too-long`, `body-too-long`, `missing-arg-<name>`.

## Features

1. The wire format on both sides, and the contract file.
2. The back end: the store and the four commands, the binary.
3. The front end: the client, title checks, the notes list (titles HTML-escaped).
4. Values with `&`, `=`, spaces or non-ASCII characters round-trip through both sides.
5. Renaming a note (`notes.rename`: `id`, `title`).
6. Tags on notes (`notes.create` and `notes.rename` accept `tags`, a comma list) and
   `notes.by-tag` (`tag`).
7. Searching notes by a word in the title or body (`notes.search`: `q`).
8. Pinning notes (`notes.pin`: `id`, `pinned`), pinned notes first in `notes.list`.
9. A character counter next to the title field in the notes screen ("12/80").
