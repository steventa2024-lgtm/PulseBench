"use strict";

const UNIT_MS = { ms: 1, s: 1000, m: 60_000, h: 3_600_000, d: 8_640_000 };
const TOKEN = /(\d+(?:\.\d+)?)\s*(ms|s|m|h|d)/y;

/**
 * Parse a human duration such as "1h30m", "90s", "1.5h", "2d 3h" or "500ms" into milliseconds.
 *
 *  - Units: d, h, m, s, ms (lower case). Numbers may have a decimal part.
 *  - A duration is one or more `<number><unit>` pairs; whitespace is allowed between pairs
 *    and between a number and its unit; the same unit may appear more than once (they add up).
 *  - The result is rounded to the nearest millisecond.
 *  - Throws TypeError("invalid duration") for non-strings, empty/blank strings, a bare number
 *    without a unit, unknown units, negative numbers or any other stray text.
 */
function parseDuration(text) {
  if (typeof text !== "string" || text.trim() === "") {
    throw new TypeError("invalid duration");
  }
  const input = text.trim();
  let total = 0;
  let pos = 0;
  while (pos < input.length) {
    TOKEN.lastIndex = pos;
    const m = TOKEN.exec(input);
    if (!m) throw new TypeError("invalid duration");
    total += parseFloat(m[1]) * UNIT_MS[m[2]];
    pos = TOKEN.lastIndex;
    while (input[pos] === " ") pos++;
  }
  return Math.round(total);
}

/**
 * Format milliseconds as space-separated components, largest first, omitting zero components:
 * 5405020 -> "1h 30m 5s 20ms"; 86400000 -> "1d"; 0 -> "0ms".
 * The input is rounded to the nearest millisecond.
 * Throws RangeError for negative or non-finite numbers and TypeError for non-numbers.
 */
function formatDuration(ms) {
  if (typeof ms !== "number") throw new TypeError("ms must be a number");
  if (!Number.isFinite(ms) || ms < 0) throw new RangeError("ms must be a non-negative finite number");
  let rest = Math.round(ms);
  if (rest === 0) return "0ms";
  const parts = [];
  for (const unit of ["d", "h", "m", "s", "ms"]) {
    const size = UNIT_MS[unit];
    const count = Math.floor(rest / size);
    if (count > 0) {
      parts.push(`${count}${unit}`);
      rest -= count * size;
    }
  }
  return parts.join(" ");
}

module.exports = { parseDuration, formatDuration };
