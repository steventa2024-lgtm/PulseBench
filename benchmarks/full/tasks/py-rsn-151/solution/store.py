"""A tiny TTL cache."""


class TTLCache:
    """Key/value cache whose entries expire (see the original docstring for the rules)."""

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
        if self._clock() >= expires_at:
            del self._data[key]
            self.misses += 1
            return default
        self.hits += 1
        return value

    def delete(self, key):
        return self._data.pop(key, None) is not None

    def delete_prefix(self, prefix):
        doomed = [k for k in self._data if k.startswith(prefix)]
        for k in doomed:
            del self._data[k]
        return len(doomed)
