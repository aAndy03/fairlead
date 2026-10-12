import { test } from "node:test";
import assert from "node:assert/strict";
import { renderOrdersTable, sortOrders, type Order } from "../admin/src/orders.ts";
import { refund } from "../admin/src/refunds.ts";

const o = (id: string, placedAt: string, status: Order["status"] = "paid"): Order => ({ id, placedAt, customer: "Ada", totalCents: 5000, status });

test("s4: orders newest first, with totals", () => {
  assert.deepEqual(sortOrders([o("a", "2026-01-01"), o("c", "2026-03-01"), o("b", "2026-02-01")]).map((x) => x.id), ["c", "b", "a"]);
  const html = renderOrdersTable([o("a", "2026-01-01"), o("c", "2026-03-01")]);
  assert.ok(html.indexOf(">c<") < html.indexOf(">a<") || html.indexOf("c") < html.lastIndexOf("a"), html);
  assert.ok(html.includes("€50.00"), html);
});

test("s4: refunds never exceed what was paid and never repeat", () => {
  assert.equal(refund(o("a", "2026-01-01")).amountCents, 5000);
  assert.equal(refund(o("a", "2026-01-01"), 1200).amountCents, 1200);
  assert.throws(() => refund(o("a", "2026-01-01"), 5001));
  assert.throws(() => refund(o("a", "2026-01-01", "refunded")));
});
