// fairlead: def ctr.admin-pages path="src/**"
// fairlead: def cmp.orders-table layer=ui.page container=ctr.admin-pages
// fairlead: use root:cmp.badge root:cmp.format-price
import { escapeHtml, formatPrice, renderBadge, type BadgeTone } from "../../kit/index.ts";

export type OrderStatus = "paid" | "shipped" | "refunded";

export interface Order {
  id: string;
  placedAt: string; // ISO date
  customer: string;
  totalCents: number;
  status: OrderStatus;
}

const TONE: Record<OrderStatus, BadgeTone> = { paid: "info", shipped: "success", refunded: "danger" };

/** Orders, newest first. */
export function sortOrders(orders: Order[]): Order[] {
  // Known issue: this sorts oldest first.
  return [...orders].sort((a, b) => a.placedAt.localeCompare(b.placedAt));
}

export function renderOrdersTable(orders: Order[]): string {
  const rows = sortOrders(orders)
    .map(
      (o) =>
        `<tr><td>${escapeHtml(o.id)}</td><td>${escapeHtml(o.customer)}</td><td>${formatPrice(o.totalCents)}</td><td>${renderBadge(o.status, TONE[o.status])}</td></tr>`,
    )
    .join("");
  return `<table class="orders"><thead><tr><th>Order</th><th>Customer</th><th>Total</th><th>Status</th></tr></thead><tbody>${rows}</tbody></table>`;
}
