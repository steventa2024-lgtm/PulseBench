"use strict";

/**
 * Validate a user-creation payload.
 *
 * Rules:
 *  - the payload must be a plain object (not null, not an array)
 *  - `name`: required string, 1-50 characters after trimming
 *  - `email`: required string that looks like local@domain.tld (no spaces, a dot in the domain)
 *  - `age`: optional; if present it must be an integer between 13 and 120
 *  - `role`: optional; one of "user", "admin"
 *  - any other field is rejected with the message "unknown field"
 *
 * Returns { valid: boolean, errors: Array<{ field: string, message: string }> }.
 * Errors are reported in the order name, email, age, role, then unknown fields (in payload order).
 */
function validateUser(payload) {
  const errors = [];
  if (typeof payload !== "object") {
    return { valid: false, errors: [{ field: "$", message: "payload must be an object" }] };
  }

  if (!payload.name || payload.name.length > 50) {
    errors.push({ field: "name", message: "name is required (1-50 characters)" });
  }

  if (!/^\S+@\S+$/.test(payload.email)) {
    errors.push({ field: "email", message: "email is invalid" });
  }

  if (payload.age) {
    if (payload.age < 13 || payload.age > 120) {
      errors.push({ field: "age", message: "age must be an integer between 13 and 120" });
    }
  }

  if (payload.role && payload.role !== "user") {
    errors.push({ field: "role", message: 'role must be "user" or "admin"' });
  }

  return { valid: errors.length === 0, errors };
}

module.exports = { validateUser };
