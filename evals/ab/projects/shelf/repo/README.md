# Shelf

Shelf is a small online shop: a shared UI kit and two apps built on it. Components render to HTML
strings, so everything runs and tests in Node without a browser or a build step.

## Layout

| Folder | What it is | Team |
|---|---|---|
| `kit/` | the shared UI kit: design tokens (`tokens.css`, `tokens.ts`), components (`button`, `badge`, `card`), money formatting, HTML escaping | kit team |
| `store/` | the customer-facing shop: product pages, the cart and its discounts | store team |
| `admin/` | the back office: the orders table and refunds | admin team |
| `tests/` | tests for all three | everyone |

`CODEOWNERS` lists which team owns which folder.

## Running

```bash
npm test
```

Node 22.18 or later runs the TypeScript files directly (`node --test "tests/**/*.test.ts"`); there are no dependencies to
install.

## Features

- **Kit**: `renderButton` (primary and secondary variants, disabled state), `renderBadge` (info,
  success, danger tones), `renderCard` (title, body, optional footer), `formatPrice` (integer cents to
  a currency string), `escapeHtml`.
- **Store**: a catalogue with stock levels, a product page with an "add to cart" button and a stock
  badge, a cart with quantities, totals in cents and discount codes (`WELCOME10` takes 10% off).
- **Admin**: an orders table (newest first) with status badges and totals, and refunds (a full refund
  or a partial one, never more than was paid).

## Known issues

- Removing a product from the cart removes every line of that product, even when the customer only
  wanted to remove one of several quantities added separately.
- The admin orders table is meant to show the newest orders first, but shows the oldest first.

## Ideas the teams have mentioned

- A wishlist in the store.
- A "low stock" warning in the admin orders table for products running out.
- Secondary actions on the product page that look lighter than the main button (an outline or
  "ghost" look).
- Showing prices in the customer's currency.
- A second discount code for returning customers.
