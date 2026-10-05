"use strict";
const fs = require("node:fs");
const path = require("node:path");

function invalid(message) {
  const err = new Error(message);
  err.code = "EINVALID_PATH";
  return err;
}

function isInside(parent, child) {
  const rel = path.relative(parent, child);
  return rel !== "" && !rel.startsWith("..") && !path.isAbsolute(rel);
}

/** realpath of the nearest existing ancestor, joined with the not-yet-existing remainder. */
function realpathLoose(target) {
  const rest = [];
  let current = target;
  for (;;) {
    try {
      return path.join(fs.realpathSync(current), ...rest.reverse());
    } catch (e) {
      if (e.code !== "ENOENT" && e.code !== "ENOTDIR") throw e;
      const parent = path.dirname(current);
      if (parent === current) throw e;
      rest.push(path.basename(current));
      current = parent;
    }
  }
}

function resolveUpload(baseDir, name) {
  if (typeof name !== "string" || name === "") {
    throw new TypeError("name must be a non-empty string");
  }
  if (name.includes("\0")) throw invalid("NUL byte in name");
  if (name.includes("\\")) throw invalid("backslashes are not allowed");
  if (name.startsWith("/") || /^[A-Za-z]:/.test(name)) throw invalid("absolute paths are not allowed");
  if (name.split("/").some((segment) => segment === "..")) throw invalid("'..' segments are not allowed");

  const base = path.resolve(baseDir);
  const target = path.resolve(base, name);
  if (!isInside(base, target)) throw invalid("path escapes the upload directory");

  const realBase = fs.realpathSync(base);
  const realTarget = realpathLoose(target);
  if (!isInside(realBase, realTarget)) throw invalid("path escapes the upload directory through a symlink");
  return target;
}

function readUpload(baseDir, name) {
  return fs.readFileSync(resolveUpload(baseDir, name), "utf8");
}

module.exports = { resolveUpload, readUpload };
