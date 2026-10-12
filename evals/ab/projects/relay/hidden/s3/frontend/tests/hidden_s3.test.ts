import { test } from "node:test";
import assert from "node:assert/strict";
import { BackendError, createApi } from "../src/api.ts";
import { renderNoteList, titleProblem } from "../src/notes-view.ts";
import { fakeBackend } from "./hidden_fake.ts";

test("s3: the client covers the four commands", async () => {
  const api = createApi(fakeBackend());
  const id = await api.createNote("Milk", "two litres");
  assert.deepEqual(await api.listNotes(), [{ id, title: "Milk" }]);
  assert.equal((await api.getNote(id)).body, "two litres");
  await api.deleteNote(id);
  await assert.rejects(api.getNote(id), (e: unknown) => e instanceof BackendError && (e as BackendError).code === "not-found");
});

test("s3: title checks and an escaped list", () => {
  assert.equal(titleProblem("Fine"), null);
  assert.notEqual(titleProblem("   "), null);
  assert.notEqual(titleProblem("x".repeat(81)), null);
  const html = renderNoteList([{ id: 1, title: "<b>x</b>" }]);
  assert.equal(html.includes("<b>x</b>"), false, html);
});
