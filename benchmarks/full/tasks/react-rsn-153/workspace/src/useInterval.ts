import { useEffect } from "react";

/**
 * Call `callback` every `delayMs` milliseconds. Pass `null` to pause.
 *
 *  - Exactly one interval exists while `delayMs` is a number; none while it is null.
 *  - The interval is cleared on unmount and whenever `delayMs` changes.
 *  - The interval is NOT restarted when only the callback changes between renders, and each tick
 *    calls the most recent callback.
 */
export function useInterval(callback: () => void, delayMs: number | null): void {
  useEffect(() => {
    if (delayMs === null) return;
    setInterval(callback, delayMs);
  }, [callback, delayMs]);
}
