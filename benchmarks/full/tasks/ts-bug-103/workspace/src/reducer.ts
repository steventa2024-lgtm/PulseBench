export interface CartLine {
  sku: string;
  quantity: number;
}

export interface CartState {
  lines: CartLine[];
}

export type CartAction =
  | { type: "add"; sku: string; quantity?: number }
  | { type: "remove"; sku: string }
  | { type: "setQuantity"; sku: string; quantity: number }
  | { type: "clear" };

export const initialState: CartState = { lines: [] };

/**
 * - add: increase the quantity of an existing line, or append a new line (quantity defaults to 1;
 *   quantities below 1 are ignored).
 * - remove: delete the line (no-op if it is absent).
 * - setQuantity: set an exact quantity; a quantity of 0 or less removes the line; unknown skus are ignored.
 * - clear: empty the cart (no-op if already empty).
 */
export function cartReducer(state: CartState, action: CartAction): CartState {
  switch (action.type) {
    case "add": {
      const existing = state.lines.find((l) => l.sku === action.sku);
      if (existing) {
        existing.quantity += action.quantity ?? 1;
      } else {
        state.lines.push({ sku: action.sku, quantity: action.quantity ?? 1 });
      }
      return { lines: state.lines };
    }
    case "remove":
      return { lines: state.lines.filter((l) => l.sku !== action.sku) };
    case "setQuantity": {
      const line = state.lines.find((l) => l.sku === action.sku);
      if (line) line.quantity = action.quantity;
      return { lines: state.lines };
    }
    case "clear":
      return { lines: [] };
  }
}
