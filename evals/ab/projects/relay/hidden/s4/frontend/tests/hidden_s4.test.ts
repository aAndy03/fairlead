import { test } from "node:test";
import assert from "node:assert/strict";
import { createApi } from "../src/api.ts";
import { fakeBackend } from "./hidden_fake.ts";

test("s4: tricky values round-trip through the client", async () => {
  const api = createApi(fakeBackend());
  for (const t of ["Fish & Chips = Dinner", "a+b c", "Crème brûlée ✓", "50% off?"]) {
    const id = await api.createNote(t, `${t} body`);
    const n = await api.getNote(id);
    assert.equal(n.title, t);
    assert.equal(n.body, `${t} body`);
  }
});
