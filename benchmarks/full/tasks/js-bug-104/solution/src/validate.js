"use strict";

const ALLOWED = new Set(["name", "email", "age", "role"]);

function validateUser(payload) {
  const errors = [];
  if (payload === null || typeof payload !== "object" || Array.isArray(payload)) {
    return { valid: false, errors: [{ field: "$", message: "payload must be an object" }] };
  }
  const has = (k) => Object.prototype.hasOwnProperty.call(payload, k);

  const name = has("name") ? payload.name : undefined;
  if (typeof name !== "string" || name.trim().length < 1 || name.trim().length > 50) {
    errors.push({ field: "name", message: "name is required (1-50 characters)" });
  }

  const email = has("email") ? payload.email : undefined;
  if (typeof email !== "string" || !/^[^\s@]+@[^\s@.]+(\.[^\s@.]+)+$/.test(email)) {
    errors.push({ field: "email", message: "email is invalid" });
  }

  if (has("age")) {
    const age = payload.age;
    if (typeof age !== "number" || !Number.isInteger(age) || age < 13 || age > 120) {
      errors.push({ field: "age", message: "age must be an integer between 13 and 120" });
    }
  }

  if (has("role")) {
    if (payload.role !== "user" && payload.role !== "admin") {
      errors.push({ field: "role", message: 'role must be "user" or "admin"' });
    }
  }

  for (const key of Object.keys(payload)) {
    if (!ALLOWED.has(key)) {
      errors.push({ field: key, message: "unknown field" });
    }
  }
  return { valid: errors.length === 0, errors };
}

module.exports = { validateUser };
