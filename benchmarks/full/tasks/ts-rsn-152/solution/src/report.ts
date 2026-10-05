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

export function topUsers(events: LogEvent[], n: number): UserSummary[] {
  const groups = groupByUser(events);
  const summaries: UserSummary[] = [];
  for (const [user, list] of groups) {
    const newest = [...list].sort((a, b) => b.ts - a.ts);
    summaries.push({ user, count: newest.length, lastSeen: newest[0].ts });
  }
  summaries.sort((a, b) => b.count - a.count || (a.user < b.user ? -1 : 1));
  return summaries.slice(0, n);
}

export function sessionize(events: LogEvent[], gapMs: number): Session[] {
  const sessions: Session[] = [];
  const groups = groupByUser(events);
  const users = [...groups.keys()].sort();
  for (const user of users) {
    const ordered = [...groups.get(user)!].sort((a, b) => a.ts - b.ts);
    let current: Session | undefined;
    for (const event of ordered) {
      if (current && event.ts - current.end <= gapMs) {
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
