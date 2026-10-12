import { test } from "node:test";
import assert from "node:assert/strict";
import { CATALOG, findProduct, inStock, type Product } from "../store/src/catalog.ts";
import { renderProductPage } from "../store/src/product-page.ts";

test("s2: the catalogue", () => {
  assert.ok(CATALOG.length > 0);
  const first = CATALOG[0];
  assert.equal(findProduct(first.id)?.id, first.id);
  assert.equal(findProduct("no-such-product"), undefined);
  for (const p of CATALOG) assert.ok(Number.isInteger(p.priceCents), "prices are integer cents");
});

test("s2: product page shows price and stock, disables add when out of stock", () => {
  const inS: Product = { id: "a", name: "Mug", priceCents: 1450, stock: 3 };
  const out: Product = { id: "b", name: "Tote", priceCents: 2200, stock: 0 };
  assert.equal(inStock(inS), true);
  assert.equal(inStock(out), false);
  const pi = renderProductPage(inS);
  assert.ok(pi.includes("€14.50") && pi.includes("Mug"), pi);
  assert.equal(/<button[^>]*\bdisabled\b/.test(pi), false, "in stock: enabled");
  assert.match(renderProductPage(out), /<button[^>]*\bdisabled\b/);
});
