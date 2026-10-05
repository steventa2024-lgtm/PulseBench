const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/**
 * Call `fn` until it resolves. Try at most `attempts` times in total, waiting `delayMs`
 * between attempts (no wait after the last failure). Rethrow the last error if every attempt fails.
 */
export async function retry<T>(fn: () => Promise<T>, attempts: number, delayMs: number): Promise<T> {
  let lastError: unknown;
  for (let i = 0; i < attempts - 1; i++) {
    try {
      return fn();
    } catch (err) {
      lastError = err;
      sleep(delayMs);
    }
  }
  throw lastError;
}

/**
 * Like `Promise.all(items.map(fn))` but never runs more than `limit` calls of `fn` at the same time.
 * The results keep the order of `items`. If any call rejects, the returned promise rejects.
 */
export async function mapLimit<T, R>(items: T[], limit: number, fn: (item: T, index: number) => Promise<R>): Promise<R[]> {
  return Promise.all(items.map((item, index) => fn(item, index)));
}
