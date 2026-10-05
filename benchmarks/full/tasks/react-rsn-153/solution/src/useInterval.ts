import { useEffect, useRef } from "react";

export function useInterval(callback: () => void, delayMs: number | null): void {
  const latest = useRef(callback);
  useEffect(() => {
    latest.current = callback;
  });

  useEffect(() => {
    if (delayMs === null) return;
    const id = setInterval(() => latest.current(), delayMs);
    return () => clearInterval(id);
  }, [delayMs]);
}
