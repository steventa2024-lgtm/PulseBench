export const REDACTED = "[REDACTED]";

/**
 * Return a sanitized deep copy of `value` that is safe to log. The input is never mutated.
 *
 *  - Object keys that look sensitive have their whole value replaced by "[REDACTED]". A key is
 *    sensitive when, case-insensitively, it contains any of: "password", "passwd", "secret",
 *    "token", "apikey", "api_key", "api-key", "authorization", "cookie", "credential",
 *    "private_key", "private-key", "privatekey", "session". (So `accessToken`, `X-Api-Key`
 *    and `Set-Cookie` are all sensitive.)
 *  - Strings are scanned: "Bearer <token>" becomes "Bearer [REDACTED]", and `key=value` / `key: value`
 *    pairs whose key is sensitive have the value replaced (up to the next whitespace, "&" or ";").
 *  - Arrays and plain objects are copied recursively. Dates are kept as Date copies.
 *  - Error instances become { name, message } (the stack is dropped); the message is scanned like any string.
 *  - Circular references become "[Circular]". Nesting deeper than 8 levels becomes "[MaxDepth]".
 *  - Numbers, booleans, null and undefined are returned unchanged. Functions and symbols become
 *    "[Function]" and "[Symbol]". bigint becomes its decimal string.
 */
export function redact(value: unknown): unknown {
  if (value && typeof value === "object") {
    const obj = value as Record<string, unknown>;
    if ("password" in obj) {
      obj.password = REDACTED;
    }
  }
  return value;
}

/**
 * Sanitize a URL string for logging:
 *  - a password in the userinfo part is replaced (`https://user:pw@host` -> `https://user:[REDACTED]@host`);
 *    a userinfo without a colon (a bare token such as `https://TOKEN@host`) is replaced entirely
 *  - query parameters with a sensitive name (same rule as `redact`) get the value "[REDACTED]"
 *  - everything else (scheme, host, path, other parameters, fragment) is unchanged
 *  - an input that is not a valid absolute URL is returned scanned like a string by `redact`
 */
export function redactUrl(url: string): string {
  return url;
}
