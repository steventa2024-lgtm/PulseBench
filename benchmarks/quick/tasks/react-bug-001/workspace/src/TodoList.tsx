import { useState } from "react";

interface Todo {
  id: number;
  title: string;
  done: boolean;
}

export function TodoList({ initial = [] }: { initial?: string[] }) {
  const [todos, setTodos] = useState<Todo[]>(initial.map((title, i) => ({ id: i + 1, title, done: false })));
  const [draft, setDraft] = useState("");

  function add() {
    if (draft.trim() === "") return;
    todos.push({ id: todos.length + 1, title: draft.trim(), done: false });
    setTodos(todos);
    setDraft("");
  }

  function toggle(index: number) {
    todos[index].done = !todos[index].done;
    setTodos(todos);
  }

  const remaining = todos.filter((t) => t.done).length;

  return (
    <div>
      <input aria-label="new todo" value={draft} onChange={(e) => setDraft(e.target.value)} />
      <button onClick={add}>Add</button>
      <ul>
        {todos.map((todo, index) => (
          <li key={index} onClick={() => toggle(index)} style={{ textDecoration: todo.done ? "line-through" : "none" }}>
            {todo.title}
          </li>
        ))}
      </ul>
      <p>{remaining} items left</p>
    </div>
  );
}
