"use strict";
const fs = require("node:fs");
const path = require("node:path");

/**
 * Resolve a user-supplied upload name to an absolute path inside `baseDir`.
 *
 * Rules:
 *  - `name` must be a non-empty string; otherwise throw TypeError.
 *  - The name is relative: reject (with an error whose `code` is "EINVALID_PATH") any name that
 *    is absolute (starts with "/" or a Windows drive such as "C:"), contains a backslash,
 *    a NUL byte, or has a ".." path segment. Empty segments ("a//b") and "." segments are fine.
 *  - The result must lie strictly inside `baseDir` (never `baseDir` itself).
 *  - Symlinks must not allow escaping: if the target (or its nearest existing ancestor)
 *    resolves through a symlink to a location outside `baseDir`, reject it with EINVALID_PATH.
 *  - The file does not have to exist yet (the path may be used for writing).
 */
function resolveUpload(baseDir, name) {
  const target = path.join(baseDir, name);
  return target;
}

/** Read an upload as UTF-8 text. Missing files reject with the usual ENOENT error. */
function readUpload(baseDir, name) {
  return fs.readFileSync(resolveUpload(baseDir, name), "utf8");
}

module.exports = { resolveUpload, readUpload };
