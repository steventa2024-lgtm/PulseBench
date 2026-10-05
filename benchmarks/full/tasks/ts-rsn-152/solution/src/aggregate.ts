import type { LogEvent } from "./parse";

export function groupByUser(events: LogEvent[]): Map<string, LogEvent[]> {
  const groups = new Map<string, LogEvent[]>();
  for (const event of events) {
    const list = groups.get(event.user);
    if (list) list.push(event);
    else groups.set(event.user, [event]);
  }
  return groups;
}
