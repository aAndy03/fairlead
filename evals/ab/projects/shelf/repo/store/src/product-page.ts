// fairlead: def ctr.store-pages path="src/**"
// fairlead: def cmp.product-page layer=ui.page container=ctr.store-pages
// fairlead: use root:cmp.card root:cmp.button root:cmp.badge root:cmp.format-price
import { escapeHtml, formatPrice, renderBadge, renderButton, renderCard } from "../../kit/index.ts";
import { inStock, type Product } from "./catalog.ts";

export function renderProductPage(product: Product): string {
  const stock = inStock(product)
    ? renderBadge(`${product.stock} in stock`, "success")
    : renderBadge("Out of stock", "danger");
  const body = `<p class="price">${formatPrice(product.priceCents)}</p>${stock}`;
  const footer = renderButton({
    label: "Add to cart",
    action: `add:${product.id}`,
    disabled: !inStock(product),
  });
  return renderCard({ title: escapeHtml(product.name), body, footer });
}
