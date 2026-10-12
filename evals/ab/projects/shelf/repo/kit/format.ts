// fairlead: def cmp.format-price layer=ui.atom container=ctr.kit vis=public sum="integer cents to a price string"
export type Currency = "EUR" | "USD" | "GBP";

/** Formats integer cents as a price, for example 1234 EUR -> "€12.34". */
export function formatPrice(cents: number, currency: Currency = "EUR"): string {
  if (!Number.isInteger(cents)) {
    throw new Error(`formatPrice takes integer cents, got ${cents}`);
  }
  return new Intl.NumberFormat("en-IE", { style: "currency", currency }).format(cents / 100);
}
