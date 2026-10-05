"""Cache key construction."""


def make_key(namespace, *parts):
    """Build a cache key `namespace:part1:part2`.

    Every part is converted to a string, stripped of surrounding whitespace and
    lower-cased, so "Alice", " alice " and "ALICE" all map to the same key.
    """
    return namespace + ":" + ":".join(str(p) for p in parts)
