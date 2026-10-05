"""Cache key construction."""


def make_key(namespace, *parts):
    """Build a cache key `namespace:part1:part2` (parts stripped and lower-cased)."""
    return namespace + ":" + ":".join(str(p).strip().lower() for p in parts)
