// fairlead: use un.api un.notes-view
import { test } from "node:test";
import assert from "node:assert/strict";
import { BackendError, createApi } from "../src/api.ts";
import { renderNotesScreen, titleProblem } from "../src/notes-view.ts";
import { fakeBackend } from "./fake-backend.ts";

test("create, list, get and delete", async () => {
  const api = createApi(fakeBackend());
  const id = await api.createNote("Milk", "two litres");
  assert.deepEqual(await api.listNotes(), [{ id, title: "Milk" }]);
  assert.deepEqual(await api.getNote(id), { id, title: "Milk", body: "two litres" });
  await api.deleteNote(id);
  await assert.rejects(api.getNote(id), BackendError);
});

test("the notes screen lists titles", async () => {
  const api = createApi(fakeBackend());
  await api.createNote("Call Ada");
  assert.match(await renderNotesScreen(api), /<li data-id="1">Call Ada<\/li>/);
});

test("title problems", () => {
  assert.equal(titleProblem("ok"), null);
  assert.match(titleProblem("  ")!, /needs a title/);
  assert.match(titleProblem("x".repeat(81))!, /at most 80/);
});
