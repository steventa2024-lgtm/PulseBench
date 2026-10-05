"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { loadProfile } = require("../src/profile");

/** A callback-style fake database with configurable latency and failures. */
function makeDb({ users = {}, posts = {}, comments = {}, failOn = {}, delays = {} } = {}) {
  const stats = { inFlight: 0, peak: 0, calls: [] };
  const run = (name, key, fn, cb) => {
    stats.calls.push(`${name}:${key}`);
    stats.inFlight++;
    stats.peak = Math.max(stats.peak, stats.inFlight);
    setTimeout(() => {
      stats.inFlight--;
      if (failOn[`${name}:${key}`]) return cb(failOn[`${name}:${key}`]);
      cb(null, fn());
    }, delays[`${name}:${key}`] ?? 10);
  };
  return {
    stats,
    getUser: (id, cb) => run("user", id, () => users[id] ?? null, cb),
    getPosts: (userId, cb) => run("posts", userId, () => posts[userId] ?? [], cb),
    getComments: (postId, cb) => run("comments", postId, () => comments[postId] ?? [], cb),
  };
}

const data = {
  users: { 1: { id: 1, name: "Ada" } },
  posts: { 1: [{ id: 10, title: "First" }, { id: 11, title: "Second" }, { id: 12, title: "Third" }] },
  comments: { 10: [1, 2, 3], 11: [], 12: [1] },
};

test("returns a promise, not a callback API", async () => {
  const p = loadProfile(makeDb(data), 1);
  assert.ok(p instanceof Promise);
  await p;
});

test("resolves with the user and posts with comment counts in order", async () => {
  const profile = await loadProfile(makeDb(data), 1);
  assert.deepEqual(profile, {
    user: { id: 1, name: "Ada" },
    posts: [
      { id: 10, title: "First", commentCount: 3 },
      { id: 11, title: "Second", commentCount: 0 },
      { id: 12, title: "Third", commentCount: 1 },
    ],
  });
});

test("keeps post order even when later posts finish first", async () => {
  const db = makeDb({ ...data, delays: { "comments:10": 60, "comments:11": 30, "comments:12": 1 } });
  const profile = await loadProfile(db, 1);
  assert.deepEqual(profile.posts.map((p) => p.id), [10, 11, 12]);
});

test("a user without posts gives an empty list", async () => {
  const db = makeDb({ users: { 2: { id: 2, name: "Bob" } } });
  assert.deepEqual(await loadProfile(db, 2), { user: { id: 2, name: "Bob" }, posts: [] });
});

test("rejects with 'user not found'", async () => {
  await assert.rejects(loadProfile(makeDb(data), 99), { message: "user not found" });
});

test("database errors are passed through unchanged", async () => {
  const boom = new Error("connection reset");
  await assert.rejects(loadProfile(makeDb({ ...data, failOn: { "user:1": boom } }), 1), (e) => e === boom);
  await assert.rejects(loadProfile(makeDb({ ...data, failOn: { "posts:1": boom } }), 1), (e) => e === boom);
  await assert.rejects(loadProfile(makeDb({ ...data, failOn: { "comments:11": boom } }), 1), (e) => e === boom);
});

test("comment counts are fetched concurrently", async () => {
  const db = makeDb(data);
  await loadProfile(db, 1);
  assert.ok(db.stats.peak >= 3, `expected the 3 comment lookups to overlap, peak in-flight was ${db.stats.peak}`);
});

test("user lookup happens before posts, posts before comments", async () => {
  const db = makeDb(data);
  await loadProfile(db, 1);
  assert.deepEqual(db.stats.calls.slice(0, 2), ["user:1", "posts:1"]);
});

test("several failing comment lookups cause one rejection and no unhandled rejections", async () => {
  const unhandled = [];
  const onUnhandled = (e) => unhandled.push(e);
  process.on("unhandledRejection", onUnhandled);
  try {
    const db = makeDb({ ...data, failOn: { "comments:10": new Error("a"), "comments:12": new Error("b") } });
    await assert.rejects(loadProfile(db, 1));
    await new Promise((r) => setTimeout(r, 50));
  } finally {
    process.off("unhandledRejection", onUnhandled);
  }
  assert.deepEqual(unhandled, []);
});
