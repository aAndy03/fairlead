// fairlead: use cmp.orders-table
import { type Order } from "./orders.ts";

export interface Refund {
  orderId: string;
  amountCents: number;
}

/** A refund of `amountCents`, or of the whole order when omitted; never more than was paid. */
export function refund(order: Order, amountCents?: number): Refund {
  if (order.status === "refunded") {
    throw new Error(`order ${order.id} is already refunded`);
  }
  const amount = amountCents ?? order.totalCents;
  if (!Number.isInteger(amount) || amount <= 0) {
    throw new Error("refund amount must be a positive integer number of cents");
  }
  if (amount > order.totalCents) {
    throw new Error(`refund of ${amount} exceeds the order total ${order.totalCents}`);
  }
  return { orderId: order.id, amountCents: amount };
}
