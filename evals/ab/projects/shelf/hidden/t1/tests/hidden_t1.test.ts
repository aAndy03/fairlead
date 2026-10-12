import { test } from "node:test";
import assert from "node:assert/strict";
import { renderOrdersTable, sortOrders, type Order } from "../admin/src/orders.ts";

const o = (id: string, placedAt: string): Order => ({ id, placedAt, customer: "C", totalCents: 100, status: "paid" });

test("t1: orders are newest first", () => {
  const sorted = sortOrders([o("old", "2026-01-01"), o("new", "2026-03-01"), o("mid", "2026-02-01")]);
  assert.deepEqual(sorted.map((x) => x.id), ["new", "mid", "old"]);
});

test("t1: the table shows the newest order first", () => {
  const html = renderOrdersTable([o("old", "2026-01-01"), o("new", "2026-03-01")]);
  assert.ok(html.indexOf(">new<") < html.indexOf(">old<"), html);
});
