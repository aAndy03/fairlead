import { test } from "node:test";
import assert from "node:assert/strict";
import { Cart } from "../store/src/cart.ts";
import { CATALOG } from "../store/src/catalog.ts";

test("s9: RETURN5 takes 5% off and does not combine with WELCOME10", () => {
  const p = CATALOG[0];
  const cart = new Cart();
  cart.add(p.id, 4);
  const subtotal = cart.subtotalCents();
  assert.equal(cart.applyDiscount("RETURN5"), true);
  assert.ok(Math.abs(cart.totalCents() - subtotal * 0.95) <= 1, `${cart.totalCents()} vs ${subtotal}`);
  // Whichever code wins, the two never stack.
  cart.applyDiscount("WELCOME10");
  const total = cart.totalCents();
  assert.ok(Math.abs(total - subtotal * 0.95) <= 1 || Math.abs(total - subtotal * 0.9) <= 1, `stacked: ${total} vs ${subtotal}`);
});
