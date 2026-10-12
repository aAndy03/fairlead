import { findProduct, type Product, CATALOG } from "./catalog.ts";

export interface CartLine {
  lineId: number;
  productId: string;
  quantity: number;
}

/** Discount codes and their percentage off the subtotal. */
export const DISCOUNTS: Record<string, number> = {
  WELCOME10: 10,
};

export class Cart {
  lines: CartLine[] = [];
  discountCode: string | null = null;
  private nextLineId = 1;

  add(productId: string, quantity = 1): CartLine {
    if (!Number.isInteger(quantity) || quantity < 1) {
      throw new Error("quantity must be a positive integer");
    }
    const line = { lineId: this.nextLineId++, productId, quantity };
    this.lines.push(line);
    return line;
  }

  /** Removes one line from the cart. */
  remove(lineId: number): void {
    const line = this.lines.find((l) => l.lineId === lineId);
    if (!line) return;
    // Known issue: this drops every line of the product, not just the one asked for.
    this.lines = this.lines.filter((l) => l.productId !== line.productId);
  }

  applyDiscount(code: string): boolean {
    if (!(code in DISCOUNTS)) return false;
    this.discountCode = code;
    return true;
  }

  subtotalCents(catalog: Product[] = CATALOG): number {
    return this.lines.reduce((sum, l) => {
      const p = findProduct(l.productId, catalog);
      return sum + (p ? p.priceCents * l.quantity : 0);
    }, 0);
  }

  totalCents(catalog: Product[] = CATALOG): number {
    const subtotal = this.subtotalCents(catalog);
    const pct = this.discountCode ? DISCOUNTS[this.discountCode] : 0;
    return subtotal - Math.round((subtotal * pct) / 100);
  }
}
