"use strict";

function chunk(array, size) {
  if (!Number.isInteger(size) || size <= 0) {
    throw new RangeError("size must be a positive integer");
  }
  const out = [];
  for (let i = 0; i < array.length; i += size) {
    out.push(array.slice(i, i + size));
  }
  return out;
}

function groupBy(array, keyFn) {
  const out = {};
  for (const item of array) {
    const key = String(keyFn(item));
    if (!Object.prototype.hasOwnProperty.call(out, key)) {
      Object.defineProperty(out, key, { value: [], enumerable: true, writable: true, configurable: true });
    }
    out[key].push(item);
  }
  return out;
}

function uniqueBy(array, keyFn) {
  const seen = new Set();
  const out = [];
  for (const item of array) {
    const key = keyFn(item);
    if (!seen.has(key)) {
      seen.add(key);
      out.push(item);
    }
  }
  return out;
}

function pick(object, keys) {
  const out = {};
  for (const key of keys) {
    if (Object.prototype.hasOwnProperty.call(object, key)) {
      out[key] = object[key];
    }
  }
  return out;
}

module.exports = { chunk, groupBy, uniqueBy, pick };
