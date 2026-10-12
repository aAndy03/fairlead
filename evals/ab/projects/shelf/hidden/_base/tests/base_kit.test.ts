import { test } from "node:test";
import assert from "node:assert/strict";
import { escapeHtml, formatPrice, renderBadge, renderButton, renderCard } from "../kit/index.ts";

test("button escapes its label and marks the variant", () => {
  const html = renderButton({ label: "<b>Buy</b>", variant: "secondary" });
  assert.match(html, /class="btn btn-secondary"/);
  assert.match(html, /&lt;b&gt;Buy&lt;\/b&gt;/);
});

test("a disabled button says so", () => {
  assert.match(renderButton({ label: "x", disabled: true }), / disabled>/);
});

test("badge tones", () => {
  assert.match(renderBadge("ok", "success"), /badge-success/);
  assert.match(renderBadge("info"), /badge-info/);
});

test("card renders title, body and footer", () => {
  const html = renderCard({ title: "T", body: "<p>b</p>", footer: "<i>f</i>" });
  assert.match(html, /<h2>T<\/h2><p>b<\/p><footer><i>f<\/i><\/footer>/);
});

test("formatPrice takes integer cents", () => {
  assert.equal(formatPrice(1234, "EUR"), "€12.34");
  assert.throws(() => formatPrice(12.5));
});

test("escapeHtml", () => {
  assert.equal(escapeHtml(`<a href="x">&</a>`), "&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;");
});
