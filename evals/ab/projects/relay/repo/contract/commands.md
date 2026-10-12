# Commands (contract version 1)

The front end and the back end both implement every command below, with the same name and the
same arguments. A change to a command (a new argument, a new reply field, a new command) changes
both sides and this file in the same change, and raises the contract version.

## Wire format

- A request is one line: `<command>?<query>`, where the query is `key=value` pairs joined by `&`,
  each key and value percent-encoded (as `encodeURIComponent` does). A command without arguments
  has no `?`.
- A reply is one line: `ok <payload>` or `err <code>`. A payload is `key=value` fields joined by
  `&`, values percent-encoded; a list joins its records with `;`.

## Limits

| Limit | Value |
|---|---|
| title length | 80 characters (`MAX_TITLE_LEN`) |
| body length | 2000 characters (`MAX_BODY_LEN`) |

Both sides define these constants once, in `frontend/src/limits.ts` and `backend/src/limits.rs`.

## Commands

| Command | Arguments | Reply payload | Errors |
|---|---|---|---|
| `notes.list` | none | records `id`, `title`, oldest first | none |
| `notes.get` | `id` | `id`, `title`, `body` | `not-found`, `bad-id`, `missing-arg-id` |
| `notes.create` | `title`, `body` (optional) | `id` | `empty-title`, `title-too-long`, `body-too-long`, `missing-arg-title` |
| `notes.delete` | `id` | empty | `not-found`, `bad-id`, `missing-arg-id` |
