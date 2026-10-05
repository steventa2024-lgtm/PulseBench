import test from "node:test";
import assert from "node:assert/strict";
import { applyCoupon, formatTotal, subtotal, type Cart } from "../src/cart";

const cart: Cart = {
  items: [
    { sku: "A", price: 10, quantity: 2 },
    { sku: "B", price: 2.5, quantity: 4 },
  ],
};

test("subtotal is a number", () => {
  assert.equal(subtotal(cart), 30);
  assert.equal(typeof subtotal(cart), "number");
});

test("known coupons discount the total", () => {
  assert.equal(applyCoupon(200, "SAVE25"), 150);
  assert.equal(applyCoupon(100, "SAVE10"), 90);
});

test("unknown or missing coupons give no discount", () => {
  assert.equal(applyCoupon(100, "BOGUS"), 100);
  assert.equal(applyCoupon(100, undefined), 100);
});

test("formatTotal rounds to cents", () => {
  assert.equal(formatTotal({ ...cart, coupon: "SAVE10" }), "$27.00");
  assert.equal(formatTotal({ items: [{ sku: "C", price: 0.1, quantity: 3 }] }), "$0.30");
  assert.equal(formatTotal({ items: [] }), "$0.00");
});
