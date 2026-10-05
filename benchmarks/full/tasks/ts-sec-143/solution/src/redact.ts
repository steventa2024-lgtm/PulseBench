export const REDACTED = "[REDACTED]";

const SENSITIVE_KEY = /pass(?:word|wd)|secret|token|api[-_]?key|authorization|cookie|credential|private[-_]?key|session/i;
const MAX_DEPTH = 8;

function isSensitiveKey(key: string): boolean {
  return SENSITIVE_KEY.test(key);
}

function scanString(text: string): string {
  let out = text.replace(/\bBearer\s+[A-Za-z0-9._~+/=-]+/g, `Bearer ${REDACTED}`);
  out = out.replace(/([A-Za-z0-9_-]+)(\s*[=:]\s*)([^\s&;]+)/g, (match, key: string, sep: string) =>
    isSensitiveKey(key) ? `${key}${sep}${REDACTED}` : match,
  );
  return out;
}

function walk(value: unknown, depth: number, seen: Set<object>): unknown {
  switch (typeof value) {
    case "string":
      return scanString(value);
    case "number":
    case "boolean":
    case "undefined":
      return value;
    case "bigint":
      return value.toString();
    case "function":
      return "[Function]";
    case "symbol":
      return "[Symbol]";
  }
  if (value === null) return null;
  const obj = value as object;
  if (depth >= MAX_DEPTH) return "[MaxDepth]";
  if (seen.has(obj)) return "[Circular]";
  if (obj instanceof Date) return new Date(obj.getTime());
  seen.add(obj);
  try {
    if (obj instanceof Error) {
      return { name: obj.name, message: scanString(obj.message) };
    }
    if (Array.isArray(obj)) {
      return obj.map((item) => walk(item, depth + 1, seen));
    }
    const out: Record<string, unknown> = {};
    for (const [key, v] of Object.entries(obj)) {
      out[key] = isSensitiveKey(key) ? REDACTED : walk(v, depth + 1, seen);
    }
    return out;
  } finally {
    seen.delete(obj);
  }
}

export function redact(value: unknown): unknown {
  try {
    return walk(value, 0, new Set());
  } catch {
    return "[Unserializable]";
  }
}

export function redactUrl(url: string): string {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return String(redact(url));
  }
  const userinfo = parsed.username !== "" || parsed.password !== "";
  if (userinfo) {
    if (parsed.password !== "") {
      parsed.password = "REDACTED_PLACEHOLDER";
    } else {
      parsed.username = "REDACTED_PLACEHOLDER";
    }
  }
  for (const key of [...parsed.searchParams.keys()]) {
    if (isSensitiveKey(key)) parsed.searchParams.set(key, "REDACTED_PLACEHOLDER");
  }
  return parsed.toString().split("REDACTED_PLACEHOLDER").join(REDACTED).replace(/%5BREDACTED%5D/g, REDACTED);
}
