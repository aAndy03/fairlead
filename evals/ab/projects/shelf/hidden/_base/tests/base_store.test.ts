import { test } from "node:test";
import assert from "node:assert/strict";
import { Cart } from "../store/src/cart.ts";
import { CATALOG, findProduct } from "../store/src/catalog.ts";
import { renderProductPage } from "../store/src/product-page.ts";
import { renderCartPage } from "../store/src/cart-page.ts";

test("cart totals in cents with a discount", () => {
  const cart = new Cart();
  cart.add("mug", 2);
  cart.add("notebook");
  assert.equal(cart.subtotalCents(), 3800);
  assert.equal(cart.applyDiscount("WELCOME10"), true);
  assert.equal(cart.totalCents(), 3420);
  assert.equal(cart.applyDiscount("NOPE"), false);
});

test("an out-of-stock product page disables the button", () => {
  const html = renderProductPage(findProduct("tote", CATALOG)!);
  assert.match(html, /Out of stock/);
  assert.match(html, / disabled>/);
});

test("the cart page lists lines and the total", () => {
  const cart = new Cart();
  cart.add("mug");
  const html = renderCartPage(cart);
  assert.match(html, /Enamel mug × 1/);
  assert.match(html, /Total: €14.50/);
});
