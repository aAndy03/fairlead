import { test } from "node:test";
import assert from "node:assert/strict";
import { Cart } from "../store/src/cart.ts";
import { CATALOG } from "../store/src/catalog.ts";

test("t7: removing one line keeps the other lines of the same product", () => {
  const p = CATALOG[0];
  const cart = new Cart();
  const first = cart.add(p.id, 1);
  cart.add(p.id, 2);
  cart.remove(first.lineId);
  assert.equal(cart.lines.length, 1);
  assert.equal(cart.subtotalCents(), p.priceCents * 2);
});
