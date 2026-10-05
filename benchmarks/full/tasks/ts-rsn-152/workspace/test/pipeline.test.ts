import test from "node:test";
import assert from "node:assert/strict";
import { parseEvents, type LogEvent } from "../src/parse";
import { groupByUser } from "../src/aggregate";
import { sessionize, topUsers } from "../src/report";

const lines = [
  "5000 bob click",
  "1000 alice login",
  "1500 alice click",
  "9000 alice click",
  "",
  "bad line",
  "3000 bob login",
  "-5 bob nope",
  "12000 alice logout",
  "5500 bob click",
  "x 2 y",
];

function fresh(): LogEvent[] {
  return parseEvents(lines);
}

test("parseEvents skips malformed lines and keeps file order", () => {
  const events = fresh();
  assert.deepEqual(events.map((e) => e.ts), [5000, 1000, 1500, 9000, 3000, 12000, 5500]);
});

test("groupByUser keeps per-user input order and hands out independent copies", () => {
  const events = fresh();
  const groups = groupByUser(events);
  assert.deepEqual(groups.get("alice")!.map((e) => e.ts), [1000, 1500, 9000, 12000]);
  groups.get("alice")!.reverse();
  const again = groupByUser(events);
  assert.deepEqual(again.get("alice")!.map((e) => e.ts), [1000, 1500, 9000, 12000], "mutating a result must not affect later calls");
  assert.notEqual(again, groups);
});

test("topUsers ranks by count then name and reports the newest timestamp", () => {
  const top = topUsers(fresh(), 2);
  assert.deepEqual(top, [
    { user: "alice", count: 4, lastSeen: 12000 },
    { user: "bob", count: 3, lastSeen: 5500 },
  ]);
  assert.deepEqual(topUsers(fresh(), 1), [{ user: "alice", count: 4, lastSeen: 12000 }]);
});

test("topUsers breaks ties by user name", () => {
  const events = parseEvents(["1 zed a", "2 amy a"]);
  assert.deepEqual(topUsers(events, 2).map((s) => s.user), ["amy", "zed"]);
});

test("sessionize works on unsorted input", () => {
  const sessions = sessionize(fresh(), 1500);
  assert.deepEqual(sessions, [
    { user: "alice", start: 1000, end: 1500, events: 2 },
    { user: "alice", start: 9000, end: 9000, events: 1 },
    { user: "alice", start: 12000, end: 12000, events: 1 },
    { user: "bob", start: 3000, end: 3000, events: 1 },
    { user: "bob", start: 5000, end: 5500, events: 2 },
  ]);
});

test("a gap of exactly gapMs stays in the same session", () => {
  const events = parseEvents(["0 u a", "1000 u b", "2001 u c"]);
  assert.deepEqual(sessionize(events, 1000), [
    { user: "u", start: 0, end: 1000, events: 2 },
    { user: "u", start: 2001, end: 2001, events: 1 },
  ]);
});

test("calling topUsers first does not change sessionize results", () => {
  const events = fresh();
  const expected = sessionize(fresh(), 1500);
  topUsers(events, 5);
  assert.deepEqual(sessionize(events, 1500), expected);
});

test("neither function modifies the input array or its events' order", () => {
  const events = fresh();
  const snapshot = events.map((e) => ({ ...e }));
  topUsers(events, 3);
  sessionize(events, 1000);
  assert.deepEqual(events, snapshot);
});

test("results are stable across repeated calls on the same array", () => {
  const events = fresh();
  const a = sessionize(events, 1500);
  topUsers(events, 2);
  const b = sessionize(events, 1500);
  const c = topUsers(events, 2);
  assert.deepEqual(a, b);
  assert.deepEqual(c, topUsers(events, 2));
});
