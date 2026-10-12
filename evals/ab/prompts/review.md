# Task: judge whether a change does what its task asked

Attached: `task.md` (the task a developer was given) and `change.diff` (everything the developer
changed: `+` lines added, `-` lines removed; new files appear whole). Judge only from these two
files; do not run anything or open other files.

Decide whether the change does what the task asks, from the point of view of the person who wrote
the task:

- `done`: every part of the task is there and would work as described.
- `partial`: some of it is there, or it would only work in part.
- `not-done`: little or none of it is there, or it would not work.

Do not judge style, structure, tests or anything the task did not ask for.

Reply with two lines, nothing else:

REASON: <one sentence>
LABEL: done | partial | not-done
