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

export function cartReducer(state: CartState, action: CartAction): CartState {
  switch (action.type) {
    case "add": {
      const quantity = action.quantity ?? 1;
      if (quantity < 1) return state;
      const exists = state.lines.some((l) => l.sku === action.sku);
      const lines = exists
        ? state.lines.map((l) => (l.sku === action.sku ? { ...l, quantity: l.quantity + quantity } : l))
        : [...state.lines, { sku: action.sku, quantity }];
      return { lines };
    }
    case "remove": {
      if (!state.lines.some((l) => l.sku === action.sku)) return state;
      return { lines: state.lines.filter((l) => l.sku !== action.sku) };
    }
    case "setQuantity": {
      const line = state.lines.find((l) => l.sku === action.sku);
      if (!line) return state;
      if (action.quantity <= 0) return { lines: state.lines.filter((l) => l.sku !== action.sku) };
      if (line.quantity === action.quantity) return state;
      return { lines: state.lines.map((l) => (l.sku === action.sku ? { ...l, quantity: action.quantity } : l)) };
    }
    case "clear":
      return state.lines.length === 0 ? state : { lines: [] };
  }
}
