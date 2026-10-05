"use strict";

/**
 * Split `array` into consecutive chunks of `size` elements. The last chunk may be shorter.
 * Throws a RangeError if `size` is not a positive integer. Does not modify `array`.
 */
function chunk(array, size) {
  throw new Error("not implemented");
}

/**
 * Group the elements of `array` by the string returned from `keyFn(element)`.
 * Returns a plain object mapping each key to an array of its elements in original order.
 */
function groupBy(array, keyFn) {
  throw new Error("not implemented");
}

/**
 * Return the elements of `array` whose `keyFn(element)` has not been seen before,
 * keeping the first occurrence and the original order.
 */
function uniqueBy(array, keyFn) {
  throw new Error("not implemented");
}

/**
 * Return a new object with only the listed `keys` that are *own* properties of `object`.
 * Missing keys are skipped (they do not appear with value undefined).
 */
function pick(object, keys) {
  throw new Error("not implemented");
}

module.exports = { chunk, groupBy, uniqueBy, pick };
