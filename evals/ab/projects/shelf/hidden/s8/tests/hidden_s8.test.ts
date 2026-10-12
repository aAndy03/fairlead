import { test } from "node:test";
import assert from "node:assert/strict";
import { formatPrice } from "../kit/index.ts";

test("s8: prices in EUR, USD and GBP", () => {
  assert.equal(formatPrice(1234), "€12.34");
  const usd = formatPrice(1234, "USD");
  const gbp = formatPrice(1234, "GBP");
  assert.ok(usd.includes("12.34") && usd.includes("$"), usd);
  assert.ok(gbp.includes("12.34") && gbp.includes("£"), gbp);
});
