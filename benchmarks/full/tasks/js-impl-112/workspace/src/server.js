"use strict";
const http = require("node:http");

/**
 * Create (but do not start) an HTTP server exposing a JSON todo API backed by `store`.
 *
 *   GET    /todos        -> 200 with an array of todos
 *   POST   /todos        -> body {"title": string}; title must be a non-blank string (trim it).
 *                           201 with the created todo, or 400 {"error": "..."}.
 *   GET    /todos/:id    -> 200 with the todo, 404 if unknown, 400 if :id is not a positive integer
 *   PATCH  /todos/:id    -> body {"done"?: boolean, "title"?: string}; only those fields may be changed,
 *                           with the same validation as POST. 200 with the updated todo, 404 if unknown.
 *   DELETE /todos/:id    -> 204 with no body, 404 if unknown
 *
 * Rules:
 *   - every response except 204 is JSON with the header `content-type: application/json`
 *   - errors look like {"error": "<message>"}
 *   - unknown paths -> 404; a known path with an unsupported method -> 405 with an `allow` header
 *   - an invalid JSON body -> 400; a body larger than 10 kB -> 413
 */
function createServer(store) {
  throw new Error("not implemented");
}

module.exports = { createServer };
