export interface LineItem {
  sku: string;
  price: number;
  quantity: number;
}

export interface Cart {
  items: LineItem[];
  coupon?: string;
}

const COUPON_RATES: Record<string, number> = {
  SAVE10: 0.1,
  SAVE25: 0.25,
};

/** Sum of price * quantity for every line, in dollars. */
export function subtotal(cart: Cart): number {
  return cart.items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}

/** Apply a coupon code. Unknown or missing coupons give no discount. */
export function applyCoupon(total: number, coupon: string | undefined): number {
  const rate = coupon !== undefined && Object.prototype.hasOwnProperty.call(COUPON_RATES, coupon) ? COUPON_RATES[coupon] : 0;
  return total * (1 - rate);
}

/** Final total formatted like `$12.50`, rounded to cents. */
export function formatTotal(cart: Cart): string {
  const total = applyCoupon(subtotal(cart), cart.coupon);
  return "$" + total.toFixed(2);
}
