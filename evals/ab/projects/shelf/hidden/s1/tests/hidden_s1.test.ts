import { test } from "node:test";
import assert from "node:assert/strict";
import * as kit from "../kit/index.ts";

test("s1: the kit exports its public names", () => {
  for (const name of ["renderButton", "renderBadge", "renderCard", "formatPrice", "escapeHtml", "tokens"]) {
    assert.ok(name in kit, `kit/index.ts exports ${name}`);
  }
});

test("s1: formatPrice", () => {
  assert.equal(kit.formatPrice(1234), "€12.34");
  assert.equal(kit.formatPrice(1234, "EUR"), "€12.34");
  assert.throws(() => kit.formatPrice(12.5));
});

test("s1: escapeHtml", () => {
  assert.equal(kit.escapeHtml(`<a href="x">&</a>`).includes("<"), false);
  assert.match(kit.escapeHtml("&"), /&amp;/);
});

test("s1: button variants, disabled state, escaped label", () => {
  const p = kit.renderButton({ label: "<b>Go</b>" });
  assert.match(p, /<button\b/);
  assert.equal(p.includes("<b>Go</b>"), false);
  const s = kit.renderButton({ label: "x", variant: "secondary" });
  assert.notEqual(p.replace("&lt;b&gt;Go&lt;/b&gt;", "x"), s, "secondary differs from primary");
  assert.match(kit.renderButton({ label: "x", disabled: true }), /\bdisabled\b/);
});

test("s1: badge tones and card", () => {
  const tones = ["info", "success", "danger"].map((t) => kit.renderBadge("x", t as "info"));
  assert.equal(new Set(tones).size, 3, "each tone renders differently");
  const card = kit.renderCard({ title: "Title", body: "<p>Body</p>", footer: "<i>Foot</i>" });
  for (const part of ["Title", "<p>Body</p>", "<i>Foot</i>"]) assert.ok(card.includes(part), part);
});
