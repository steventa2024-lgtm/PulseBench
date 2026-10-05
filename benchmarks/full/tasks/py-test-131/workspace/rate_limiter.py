"""Token-bucket rate limiter."""

import time


class TokenBucket:
    """A token bucket.

    * `capacity` (> 0) is the maximum number of tokens; the bucket starts full.
    * `refill_per_sec` (>= 0) tokens are added per second of elapsed clock time,
      never exceeding `capacity`.
    * `clock` is a zero-argument callable returning seconds (default: time.monotonic).
      If the clock ever goes backwards, no tokens are added or removed.

    Raises ValueError for a non-positive capacity or a negative refill rate.
    """

    def __init__(self, capacity, refill_per_sec, clock=time.monotonic):
        if capacity <= 0:
            raise ValueError("capacity must be positive")
        if refill_per_sec < 0:
            raise ValueError("refill_per_sec must be >= 0")
        self.capacity = capacity
        self.refill_per_sec = refill_per_sec
        self._clock = clock
        self._tokens = float(capacity)
        self._last = clock()

    def _refill(self):
        now = self._clock()
        elapsed = max(0.0, now - self._last)
        self._tokens = min(self.capacity, self._tokens + elapsed * self.refill_per_sec)
        self._last = now

    def allow(self, cost=1):
        """Take `cost` tokens if available and return True, otherwise return False
        (and take nothing). Raises ValueError if `cost` <= 0 or `cost` > capacity."""
        if cost <= 0 or cost > self.capacity:
            raise ValueError("invalid cost")
        self._refill()
        if self._tokens >= cost:
            self._tokens -= cost
            return True
        return False

    def tokens(self):
        """Tokens currently available (after refilling)."""
        self._refill()
        return self._tokens
