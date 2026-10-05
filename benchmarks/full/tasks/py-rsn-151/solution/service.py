"""User lookup service with caching."""

from keys import make_key

_MISSING = object()


class UserService:
    """Look users up by name through a cache."""

    def __init__(self, cache, fetch, ttl=60):
        self.cache = cache
        self.fetch = fetch
        self.ttl = ttl

    def get_user(self, name):
        key = make_key("user", name)
        cached = self.cache.get(key, _MISSING)
        if cached is not _MISSING:
            return cached
        user = self.fetch(name)
        self.cache.set(key, user, self.ttl)
        return user

    def get_users(self, names):
        return {name: self.get_user(name) for name in names}

    def rename_user(self, old_name, new_name, rename):
        rename(old_name, new_name)
        self.cache.delete(make_key("user", old_name))
        self.cache.delete(make_key("user", new_name))
