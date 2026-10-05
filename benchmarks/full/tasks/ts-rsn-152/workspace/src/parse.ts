export interface LogEvent {
  user: string;
  ts: number; // epoch milliseconds
  action: string;
}

/**
 * Parse lines of the form `<epoch-ms> <user> <action>`. Fields are separated by single spaces.
 * Blank lines and lines that do not have exactly three fields or whose timestamp is not a finite
 * non-negative integer are skipped. The order of the lines is preserved (they are NOT sorted).
 */
export function parseEvents(lines: string[]): LogEvent[] {
  const events: LogEvent[] = [];
  for (const line of lines) {
    const parts = line.trim().split(" ");
    if (parts.length !== 3) continue;
    const ts = Number(parts[0]);
    if (!Number.isInteger(ts) || ts < 0) continue;
    events.push({ ts, user: parts[1], action: parts[2] });
  }
  return events;
}
