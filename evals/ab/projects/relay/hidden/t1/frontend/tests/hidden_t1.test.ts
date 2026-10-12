import { test } from "node:test";
import assert from "node:assert/strict";
import { createApi } from "../src/api.ts";
import { fakeBackend } from "./hidden_fake.ts";

test("t1: titles with & and = round-trip exactly", async () => {
  const api = createApi(fakeBackend());
  const title = "Fish & Chips = Dinner";
  const id = await api.createNote(title, "a=b&c");
  assert.deepEqual((await api.listNotes()).map((n) => n.title), [title]);
  assert.equal((await api.getNote(id)).body, "a=b&c");
});
