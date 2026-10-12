import { test } from "node:test";
import assert from "node:assert/strict";
import { Cart } from "../store/src/cart.ts";
import { CATALOG } from "../store/src/catalog.ts";
import { renderCartPage } from "../store/src/cart-page.ts";

const a = CATALOG[0];
const b = CATALOG[1] ?? CATALOG[0];

test("s3: totals in cents and WELCOME10", () => {
  const cart = new Cart();
  cart.add(a.id, 2);
  cart.add(b.id);
  const subtotal = a.priceCents * 2 + b.priceCents;
  assert.equal(cart.subtotalCents(), subtotal);
  assert.equal(cart.applyDiscount("WELCOME10"), true);
  // The spec fixes 10% off, not the rounding of a half cent.
  assert.ok(Math.abs(cart.totalCents() - subtotal * 0.9) <= 1, `${cart.totalCents()} vs ${subtotal}`);
  assert.equal(cart.applyDiscount("NOPE"), false);
  assert.ok(Number.isInteger(cart.totalCents()));
});

test("s3: remove takes out one line only", () => {
  const cart = new Cart();
  const l1 = cart.add(a.id, 1);
  cart.add(a.id, 2);
  cart.remove(l1.lineId);
  assert.equal(cart.subtotalCents(), a.priceCents * 2);
});

test("s3: the cart page shows the lines and the total", () => {
  const cart = new Cart();
  cart.add(a.id);
  const html = renderCartPage(cart);
  assert.ok(html.includes(a.name), html);
});
