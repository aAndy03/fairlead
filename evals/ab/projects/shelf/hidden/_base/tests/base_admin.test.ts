import { test } from "node:test";
import assert from "node:assert/strict";
import { renderOrdersTable, type Order } from "../admin/src/orders.ts";
import { refund } from "../admin/src/refunds.ts";

const order: Order = { id: "o1", placedAt: "2026-10-01", customer: "Ada", totalCents: 5000, status: "paid" };

test("the orders table shows totals and status", () => {
  const html = renderOrdersTable([order]);
  assert.match(html, /€50.00/);
  assert.match(html, /badge-info/);
});

test("refunds never exceed the total", () => {
  assert.equal(refund(order).amountCents, 5000);
  assert.equal(refund(order, 1200).amountCents, 1200);
  assert.throws(() => refund(order, 5001));
  assert.throws(() => refund({ ...order, status: "refunded" }));
});
