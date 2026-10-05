"use strict";

/** In-memory todo store. Ids are positive integers assigned sequentially starting at 1. */
function createStore() {
  const todos = new Map();
  let nextId = 1;
  return {
    list() {
      return [...todos.values()];
    },
    get(id) {
      return todos.get(id) ?? null;
    },
    create({ title }) {
      const todo = { id: nextId++, title, done: false };
      todos.set(todo.id, todo);
      return todo;
    },
    update(id, patch) {
      const current = todos.get(id);
      if (!current) return null;
      const next = { ...current, ...patch, id };
      todos.set(id, next);
      return next;
    },
    remove(id) {
      return todos.delete(id);
    },
  };
}

module.exports = { createStore };
