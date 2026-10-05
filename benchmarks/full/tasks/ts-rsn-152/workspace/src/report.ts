import { groupByUser } from "./aggregate";
import type { LogEvent } from "./parse";

export interface UserSummary {
  user: string;
  count: number;
  lastSeen: number;
}

export interface Session {
  user: string;
  start: number;
  end: number;
  events: number;
}

/**
 * The `n` users with the most events (ties broken by user name ascending), each with the
 * timestamp of their most recent event.
 */
export function topUsers(events: LogEvent[], n: number): UserSummary[] {
  const groups = groupByUser(events);
  const summaries: UserSummary[] = [];
  for (const [user, list] of groups) {
    list.sort((a, b) => b.ts - a.ts); // newest first
    summaries.push({ user, count: list.length, lastSeen: list[0].ts });
  }
  summaries.sort((a, b) => b.count - a.count || (a.user < b.user ? -1 : 1));
  return summaries.slice(0, n);
}

/**
 * Split each user's activity into sessions. Events of one user belong to the same session when
 * the gap between consecutive events (in time order) is at most `gapMs`; a larger gap starts a new
 * session. The input may be in any order. Sessions are returned ordered by user name, then start time.
 */
export function sessionize(events: LogEvent[], gapMs: number): Session[] {
  const sessions: Session[] = [];
  const groups = groupByUser(events);
  for (const [user, list] of groups) {
    let current: Session | undefined;
    for (const event of list) {
      if (current && event.ts - current.end < gapMs) {
        current.end = event.ts;
        current.events++;
      } else {
        current = { user, start: event.ts, end: event.ts, events: 1 };
        sessions.push(current);
      }
    }
  }
  return sessions;
}
