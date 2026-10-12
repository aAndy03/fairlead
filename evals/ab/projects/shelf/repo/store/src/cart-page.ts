// fairlead: def cmp.cart-page layer=ui.page container=ctr.store-pages
// fairlead: use root:cmp.card root:cmp.button root:cmp.format-price
import { escapeHtml, formatPrice, renderButton, renderCard } from "../../kit/index.ts";
import { type Cart } from "./cart.ts";
import { CATALOG, findProduct } from "./catalog.ts";

export function renderCartPage(cart: Cart): string {
  const rows = cart.lines
    .map((l) => {
      const p = findProduct(l.productId, CATALOG);
      const name = p ? escapeHtml(p.name) : escapeHtml(l.productId);
      const remove = renderButton({ label: "Remove", variant: "secondary", action: `remove:${l.lineId}` });
      return `<li>${name} × ${l.quantity} ${remove}</li>`;
    })
    .join("");
  const total = `<p class="total">Total: ${formatPrice(cart.totalCents())}</p>`;
  const checkout = renderButton({ label: "Checkout", action: "checkout", disabled: cart.lines.length === 0 });
  return renderCard({ title: "Your cart", body: `<ul>${rows}</ul>${total}`, footer: checkout });
}
