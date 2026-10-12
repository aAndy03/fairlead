# Agent instructions

Relay is a TypeScript front end (`frontend/`, frontend team) and a Rust back end (`backend/`,
backend team) joined by the commands in `contract/commands.md`. Follow these rules in every change.

## The contract
- Every command exists on both sides with the same name and arguments: a function in
  `frontend/src/api.ts` and an arm in `handle` in `backend/src/commands.rs`.
- A change to a command (new command, new argument, new reply field, rename) changes both sides
  and `contract/commands.md` in the same change, and raises the contract version at the top of that
  file.

## One door to the back end
- Only `frontend/src/api.ts` sends requests (`invoke`). The rest of the front end calls the api
  functions.

## Limits
- `MAX_TITLE_LEN` (80) and `MAX_BODY_LEN` (2000) are defined once per side, in
  `frontend/src/limits.ts` and `backend/src/limits.rs`, and listed in the contract. Never write
  `80` or `2000` anywhere else; add new limits to those files and to the contract.

## Ownership
- Change only the side of the team the task is for. If your change needs the other side, change
  the contract only together with both sides; otherwise write a short note under `docs/requests/`
  saying what you need from the other team.

## Before you finish
- Run `npm test` (front-end and back-end tests) and keep it passing; add tests for what you change.
