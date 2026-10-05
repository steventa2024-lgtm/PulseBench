"""A tiny TTL cache."""


class TTLCache:
    """Key/value cache whose entries expire.

    * `set(key, value, ttl)` stores a value that is valid for `ttl` seconds.
    * An entry is valid while `now < expires_at`; at exactly `expires_at` it is expired.
    * `get(key, default=None)` returns the value if it is valid. Expired entries are removed.
      A valid entry counts as a hit; a missing or expired entry counts as a miss.
    * `delete(key)` removes an entry and returns True if it existed.
    * `delete_prefix(prefix)` removes every key starting with `prefix` and returns how many were removed.
    """

    def __init__(self, clock):
        self._clock = clock
        self._data = {}
        self.hits = 0
        self.misses = 0

    def set(self, key, value, ttl):
        self._data[key] = (value, self._clock() + ttl)

    def get(self, key, default=None):
        entry = self._data.get(key)
        if entry is None:
            self.misses += 1
            return default
        value, expires_at = entry
        self.hits += 1
        if self._clock() > expires_at:
            del self._data[key]
            return default
        return value

    def delete(self, key):
        return self._data.pop(key, None) is not None

    def delete_prefix(self, prefix):
        doomed = [k for k in self._data if k.startswith(prefix)]
        for k in doomed:
            del self._data[k]
        return len(doomed)
