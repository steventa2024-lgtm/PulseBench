import { useState } from "react";
import { useInterval } from "./useInterval";

export function Stopwatch({ label }: { label: string }) {
  const [running, setRunning] = useState(false);
  const [elapsed, setElapsed] = useState(0);

  useInterval(() => setElapsed((e) => e + 1), running ? 1000 : null);

  return (
    <div>
      <span data-testid="time">
        {label}: {elapsed}s
      </span>
      <button data-testid="toggle" onClick={() => setRunning((r) => !r)}>
        {running ? "Stop" : "Start"}
      </button>
      <button data-testid="reset" onClick={() => setElapsed(0)}>
        Reset
      </button>
    </div>
  );
}
