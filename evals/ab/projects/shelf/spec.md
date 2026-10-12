# Shelf: product specification

Shelf is a small online shop: a shared UI kit and two apps built on it, in TypeScript. Components
render to HTML strings, so everything runs and tests in Node 22.18 or later with no dependencies
and no build step (`node --test`, TypeScript run directly). `npm test` runs every test.

## Layout and owners

| Folder | Contents | Owner |
|---|---|---|
| `kit/` | design tokens (`tokens.css`, `tokens.ts`), components, money formatting, HTML escaping; `kit/index.ts` exports the kit's public names | kit team |
| `store/src/` | the customer-facing shop | store team |
| `admin/src/` | the back office | admin team |
| `tests/` | tests (`*.test.ts`) | everyone |

A `CODEOWNERS` file records the owners.

## Requirements (they hold for the whole life of the product)

1. **Ownership.** Each change is made by one team and touches only that team's folder (and tests).
   An app team never edits `kit/`; when an app needs something the kit lacks, it builds on the kit
   from its own folder and asks the kit team for the addition.
2. **Design tokens.** Colours, spacing and radii exist once, in `kit/tokens.css` and `kit/tokens.ts`.
   The apps never contain a raw colour (`#2563eb`) or length (`8px`); they use the kit's `tokens`.
3. **Money.** Prices are integer cents everywhere and are formatted only by the kit's `formatPrice`;
   the apps never use `toFixed` or `Intl.NumberFormat`.
4. **The apps stay apart.** `store/` and `admin/` never import each other; shared code lives in the kit.
5. **No copies.** The apps never re-implement a kit element (their own `<button>`, badge or card
   markup); they render the kit's.
6. **Tests.** Every feature has tests, and `npm test` passes after every change.

## Public names (tests call these)

- `kit/index.ts` exports `renderButton({ label, variant?, disabled?, action? })` (variants
  `"primary"`, `"secondary"`), `renderBadge(text, tone?)` (tones `"info"`, `"success"`,
  `"danger"`), `renderCard({ title, body, footer? })`, `formatPrice(cents, currency?)` (default
  `"EUR"`, `1234` -> `"€12.34"`, throws on non-integer cents), `escapeHtml(text)`, `tokens`.
- `store/src/catalog.ts`: `type Product { id, name, priceCents, stock }`, `CATALOG`,
  `findProduct(id)`, `inStock(product)`.
- `store/src/cart.ts`: `class Cart` with `add(productId, quantity?)` returning a line with a
  `lineId`, `remove(lineId)` removing that line only, `applyDiscount(code)` returning a boolean,
  `subtotalCents()`, `totalCents()`; the code `WELCOME10` takes 10% off.
- `store/src/product-page.ts`: `renderProductPage(product)`; `store/src/cart-page.ts`:
  `renderCartPage(cart)`.
- `admin/src/orders.ts`: `type Order { id, placedAt, customer, totalCents, status }` (status
  `"paid"`, `"shipped"`, `"refunded"`), `sortOrders(orders)` newest first,
  `renderOrdersTable(orders)`.
- `admin/src/refunds.ts`: `refund(order, amountCents?)`: the whole order when omitted, never more
  than was paid, never twice.

## Features

1. The kit: tokens, button, badge, card, `formatPrice`, `escapeHtml`.
2. The store catalogue and the product page (price, a stock badge, an "add to cart" button that is
   disabled when out of stock).
3. The cart: lines, quantities, totals in cents, the `WELCOME10` code, and the cart page.
4. The admin orders table (newest first, status badges, totals) and refunds.
5. A wishlist in the store (add, remove, list, a page).
6. A "low stock" warning in the admin orders table for products with fewer than 5 left.
7. Lighter secondary actions on the product page (an outline or "ghost" look), which the kit does
   not offer at first.
8. Prices shown in the customer's currency (EUR, USD, GBP).
9. A second discount code, `RETURN5`, 5% off for returning customers, not combinable with
   `WELCOME10`.
