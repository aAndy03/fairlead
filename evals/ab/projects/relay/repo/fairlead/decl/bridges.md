+++
directives = [
  'def brg.notes-list proto=line dir="ts->rs" wire=notes.list ver=1 params=none ts="frontend/src/api.ts#listNotes" rs="backend/src/commands.rs#handle" contract="contract/commands.md"',
  'def brg.notes-get proto=line dir="ts->rs" wire=notes.get ver=1 params=id ts="frontend/src/api.ts#getNote" rs="backend/src/commands.rs#handle" contract="contract/commands.md"',
  'def brg.notes-create proto=line dir="ts->rs" wire=notes.create ver=1 params=title,body ts="frontend/src/api.ts#createNote" rs="backend/src/commands.rs#handle" contract="contract/commands.md"',
  'def brg.notes-delete proto=line dir="ts->rs" wire=notes.delete ver=1 params=id ts="frontend/src/api.ts#deleteNote" rs="backend/src/commands.rs#handle" contract="contract/commands.md"',
]
+++
# Bridges

Each command of `contract/commands.md` is a bridge: the front-end function in `api.ts` and the
back-end arm in `commands.rs#handle`. A change to a command changes both ends, the contract and the
bridge's `ver` together.
