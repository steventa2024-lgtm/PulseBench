"use strict";
const http = require("node:http");

const MAX_BODY = 10 * 1024;

function send(res, status, body) {
  if (status === 204) {
    res.writeHead(204);
    res.end();
    return;
  }
  const payload = JSON.stringify(body);
  res.writeHead(status, { "content-type": "application/json", "content-length": Buffer.byteLength(payload) });
  res.end(payload);
}

class HttpError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

function readJson(req) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks = [];
    let failed = false;
    req.on("data", (chunk) => {
      if (failed) return;
      size += chunk.length;
      if (size > MAX_BODY) {
        failed = true;
        reject(new HttpError(413, "request body too large"));
        return;
      }
      chunks.push(chunk);
    });
    req.on("end", () => {
      if (failed) return;
      const text = Buffer.concat(chunks).toString("utf8");
      try {
        const value = JSON.parse(text);
        if (value === null || typeof value !== "object" || Array.isArray(value)) {
          reject(new HttpError(400, "body must be a JSON object"));
        } else {
          resolve(value);
        }
      } catch {
        reject(new HttpError(400, "invalid JSON"));
      }
    });
    req.on("error", (e) => reject(e));
  });
}

function validTitle(value) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new HttpError(400, "title must be a non-blank string");
  }
  return value.trim();
}

function createServer(store) {
  async function handle(req, res) {
    const url = new URL(req.url, "http://localhost");
    const parts = url.pathname.split("/").filter(Boolean);
    if (parts[0] !== "todos" || parts.length > 2) throw new HttpError(404, "not found");

    if (parts.length === 1) {
      if (req.method === "GET") return send(res, 200, store.list());
      if (req.method === "POST") {
        const body = await readJson(req);
        return send(res, 201, store.create({ title: validTitle(body.title) }));
      }
      res.setHeader("allow", "GET, POST");
      throw new HttpError(405, "method not allowed");
    }

    if (!/^[1-9]\d*$/.test(parts[1])) throw new HttpError(400, "id must be a positive integer");
    const id = Number(parts[1]);
    if (req.method === "GET") {
      const todo = store.get(id);
      if (!todo) throw new HttpError(404, "todo not found");
      return send(res, 200, todo);
    }
    if (req.method === "PATCH") {
      const body = await readJson(req);
      const patch = {};
      if ("done" in body) {
        if (typeof body.done !== "boolean") throw new HttpError(400, "done must be a boolean");
        patch.done = body.done;
      }
      if ("title" in body) patch.title = validTitle(body.title);
      if (!store.get(id)) throw new HttpError(404, "todo not found");
      return send(res, 200, store.update(id, patch));
    }
    if (req.method === "DELETE") {
      if (!store.remove(id)) throw new HttpError(404, "todo not found");
      return send(res, 204);
    }
    res.setHeader("allow", "GET, PATCH, DELETE");
    throw new HttpError(405, "method not allowed");
  }

  return http.createServer((req, res) => {
    handle(req, res).catch((err) => {
      if (err instanceof HttpError) {
        if (err.status === 413) res.setHeader("connection", "close");
        send(res, err.status, { error: err.message });
      } else {
        send(res, 500, { error: "internal error" });
      }
    });
  });
}

module.exports = { createServer };
