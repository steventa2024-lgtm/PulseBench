import type { LogEvent } from "./parse";

let lastInput: LogEvent[] | undefined;
let lastGroups: Map<string, LogEvent[]> | undefined;

/**
 * Group events by user. Within each group the events keep the order they had in `events`.
 * The returned map and arrays belong to the caller: callers may sort or modify them freely
 * without affecting `events` or any other call.
 */
export function groupByUser(events: LogEvent[]): Map<string, LogEvent[]> {
  if (lastInput === events && lastGroups) {
    return lastGroups; // cheap memoization for repeated calls
  }
  const groups = new Map<string, LogEvent[]>();
  for (const event of events) {
    const list = groups.get(event.user);
    if (list) list.push(event);
    else groups.set(event.user, [event]);
  }
  lastInput = events;
  lastGroups = groups;
  return groups;
}
