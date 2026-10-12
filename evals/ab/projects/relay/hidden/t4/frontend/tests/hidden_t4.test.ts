import { test } from "node:test";
import assert from "node:assert/strict";
import { createApi } from "../src/api.ts";
import { renderNoteList } from "../src/notes-view.ts";
import { fakeBackend } from "./hidden_fake.ts";

test("t4: titles show as plain text", async () => {
  const api = createApi(fakeBackend());
  await api.createNote("<b>x</b>");
  const html = renderNoteList(await api.listNotes());
  assert.equal(html.includes("<b>x</b>"), false, html);
  assert.ok(html.includes("&lt;b&gt;x&lt;/b&gt;"), html);
});
