"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { resolveUpload, readUpload } = require("../src/files");

function setup() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "pb-files-"));
  const base = path.join(root, "uploads");
  const evil = path.join(root, "uploads-evil");
  const outside = path.join(root, "outside");
  for (const d of [base, evil, outside, path.join(base, "docs")]) fs.mkdirSync(d, { recursive: true });
  fs.writeFileSync(path.join(base, "hello.txt"), "hello");
  fs.writeFileSync(path.join(base, "docs", "a.txt"), "nested");
  fs.writeFileSync(path.join(root, "secret.txt"), "TOP SECRET");
  fs.writeFileSync(path.join(evil, "secret.txt"), "EVIL SECRET");
  fs.writeFileSync(path.join(outside, "secret.txt"), "OUTSIDE SECRET");
  return { root, base, evil, outside, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
}

const invalid = (fn) => assert.throws(fn, (e) => e.code === "EINVALID_PATH");

test("legitimate names resolve inside the base directory", () => {
  const { base, cleanup } = setup();
  try {
    assert.equal(resolveUpload(base, "hello.txt"), path.join(base, "hello.txt"));
    assert.equal(resolveUpload(base, "docs/a.txt"), path.join(base, "docs", "a.txt"));
    assert.equal(resolveUpload(base, "./hello.txt"), path.join(base, "hello.txt"));
    assert.equal(resolveUpload(base, "docs//a.txt"), path.join(base, "docs", "a.txt"));
    assert.equal(resolveUpload(base, "not-yet/created.txt"), path.join(base, "not-yet", "created.txt"));
    assert.equal(readUpload(base, "hello.txt"), "hello");
    assert.equal(readUpload(base, "docs/a.txt"), "nested");
  } finally {
    cleanup();
  }
});

test("parent-directory traversal is rejected", () => {
  const { base, cleanup } = setup();
  try {
    for (const name of ["../secret.txt", "docs/../../secret.txt", "..", "a/../../secret.txt", "docs/..", "../uploads/hello.txt"]) {
      invalid(() => resolveUpload(base, name));
    }
    invalid(() => readUpload(base, "../secret.txt"));
  } finally {
    cleanup();
  }
});

test("a sibling directory sharing the base name as a prefix is not 'inside'", () => {
  const { base, cleanup } = setup();
  try {
    invalid(() => resolveUpload(base, "../uploads-evil/secret.txt"));
  } finally {
    cleanup();
  }
});

test("absolute paths, drive letters, backslashes and NUL bytes are rejected", () => {
  const { base, root, cleanup } = setup();
  try {
    for (const name of [path.join(root, "secret.txt"), "/etc/passwd", "C:\\Windows\\win.ini", "C:/x", "a\\b.txt", "..\\secret.txt", "a\0b"]) {
      invalid(() => resolveUpload(base, name));
    }
  } finally {
    cleanup();
  }
});

test("the base directory itself and empty names are not valid uploads", () => {
  const { base, cleanup } = setup();
  try {
    invalid(() => resolveUpload(base, "."));
    invalid(() => resolveUpload(base, "./"));
    for (const bad of ["", undefined, null, 5, {}]) {
      assert.throws(() => resolveUpload(base, bad), TypeError);
    }
  } finally {
    cleanup();
  }
});

test("symlinks cannot be used to escape", (t) => {
  const { base, outside, cleanup } = setup();
  try {
    try {
      fs.symlinkSync(outside, path.join(base, "link"), "dir");
      fs.symlinkSync(path.join(outside, "secret.txt"), path.join(base, "file-link.txt"));
    } catch (e) {
      t.skip("cannot create symlinks here: " + e.code);
      return;
    }
    invalid(() => resolveUpload(base, "link/secret.txt"));
    invalid(() => resolveUpload(base, "link/new-file.txt"));
    invalid(() => resolveUpload(base, "file-link.txt"));
    invalid(() => readUpload(base, "link/secret.txt"));
    // a symlink that stays inside the base is fine
    fs.symlinkSync(path.join(base, "docs"), path.join(base, "docs-link"), "dir");
    assert.equal(readUpload(base, "docs-link/a.txt"), "nested");
  } finally {
    cleanup();
  }
});

test("missing files surface ENOENT, not a validation error", () => {
  const { base, cleanup } = setup();
  try {
    assert.throws(() => readUpload(base, "nope.txt"), (e) => e.code === "ENOENT");
  } finally {
    cleanup();
  }
});
