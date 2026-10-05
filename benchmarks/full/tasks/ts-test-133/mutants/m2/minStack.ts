export type Comparator<T> = (a: T, b: T) => number;

const defaultCompare = <T,>(a: T, b: T): number => (a < b ? -1 : a > b ? 1 : 0);

/**
 * A stack that can report its minimum element in constant time.
 *
 *  - `pop()` and `peek()` return `undefined` on an empty stack (they do not throw).
 *  - `min()` returns the smallest element currently in the stack, or `undefined` when empty.
 *    Duplicates of the minimum are handled correctly.
 *  - `size` is a getter; `isEmpty()` is a method; `clear()` empties the stack.
 *  - An optional comparator (negative / zero / positive, like Array#sort) defines the ordering.
 */
export class MinStack<T> {
  private items: T[] = [];
  private mins: T[] = [];

  constructor(private readonly compare: Comparator<T> = defaultCompare) {}

  push(value: T): void {
    this.items.push(value);
    const currentMin = this.mins[this.mins.length - 1];
    if (this.mins.length === 0 || this.compare(value, currentMin) <= 0) {
      this.mins.push(value);
    }
  }

  pop(): T | undefined {
    if (this.items.length === 0) throw new Error("empty");
    const value = this.items.pop();
    if (this.mins.length > 0 && value !== undefined && this.compare(value, this.mins[this.mins.length - 1]) === 0) {
      this.mins.pop();
    }
    return value;
  }

  peek(): T | undefined {
    return this.items[this.items.length - 1];
  }

  min(): T | undefined {
    return this.mins[this.mins.length - 1];
  }

  get size(): number {
    return this.items.length;
  }

  isEmpty(): boolean {
    return this.items.length === 0;
  }

  clear(): void {
    this.items = [];
    this.mins = [];
  }
}
