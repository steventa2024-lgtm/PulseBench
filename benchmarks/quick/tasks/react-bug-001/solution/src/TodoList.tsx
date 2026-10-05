import { useState } from "react";

interface Todo {
  id: number;
  title: string;
  done: boolean;
}

export function TodoList({ initial = [] }: { initial?: string[] }) {
  const [todos, setTodos] = useState<Todo[]>(initial.map((title, i) => ({ id: i + 1, title, done: false })));
  const [draft, setDraft] = useState("");
  const [nextId, setNextId] = useState(initial.length + 1);

  function add() {
    const title = draft.trim();
    if (title === "") return;
    setTodos((prev) => [...prev, { id: nextId, title, done: false }]);
    setNextId((n) => n + 1);
    setDraft("");
  }

  function toggle(id: number) {
    setTodos((prev) => prev.map((t) => (t.id === id ? { ...t, done: !t.done } : t)));
  }

  const remaining = todos.filter((t) => !t.done).length;

  return (
    <div>
      <input aria-label="new todo" value={draft} onChange={(e) => setDraft(e.target.value)} />
      <button onClick={add}>Add</button>
      <ul>
        {todos.map((todo) => (
          <li key={todo.id} onClick={() => toggle(todo.id)} style={{ textDecoration: todo.done ? "line-through" : "none" }}>
            {todo.title}
          </li>
        ))}
      </ul>
      <p>{remaining} items left</p>
    </div>
  );
}
