import { useState } from "react";
import { useInterval } from "./useInterval";

/** Shows `<label>: <seconds>s`. Start/Stop toggles counting (one tick per second); Reset sets 0s. */
export function Stopwatch({ label }: { label: string }) {
  const [running, setRunning] = useState(false);
  const [elapsed, setElapsed] = useState(0);

  useInterval(() => setElapsed(elapsed + 1), running ? 1000 : null);

  return (
    <div>
      <span data-testid="time">
        {label}: {elapsed}s
      </span>
      <button data-testid="toggle" onClick={() => setRunning(!running)}>
        {running ? "Stop" : "Start"}
      </button>
      <button data-testid="reset" onClick={() => setElapsed(0)}>
        Reset
      </button>
    </div>
  );
}
