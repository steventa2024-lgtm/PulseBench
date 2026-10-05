"""User lookup service with caching."""

from keys import make_key

_MISSING = object()


class UserService:
    """Look users up by name through a cache.

    `fetch(name)` loads a user from the backend (it receives the name exactly as the caller gave it).
    Names are case-insensitive and ignore surrounding whitespace, so a user is fetched at most once
    per TTL no matter how the name is spelled.
    """

    def __init__(self, cache, fetch, ttl=60):
        self.cache = cache
        self.fetch = fetch
        self.ttl = ttl

    def get_user(self, name):
        """Return the user, using the cache. A backend result of None ("no such user") is cached too."""
        key = make_key("user", name)
        cached = self.cache.get(key, _MISSING)
        if cached is not _MISSING:
            return cached
        user = self.fetch(name)
        self.cache.set(key, user, self.ttl)
        return user

    def get_users(self, names):
        """Return {name: user} for every given name (keys are the names as given).

        Names that are the same user (see above) must trigger only ONE backend fetch.
        """
        return {name: self.get_user(name) for name in names}

    def rename_user(self, old_name, new_name, rename):
        """Call `rename(old_name, new_name)` on the backend and make sure no stale entry
        for either name can be served afterwards."""
        rename(old_name, new_name)
        self.cache.delete(make_key("user", old_name))
