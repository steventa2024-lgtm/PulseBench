import unittest

from keys import make_key
from service import UserService
from store import TTLCache


class FakeClock:
    def __init__(self):
        self.now = 1000.0

    def __call__(self):
        return self.now


class Backend:
    def __init__(self, users):
        self.users = dict(users)  # lower-case name -> record
        self.calls = []

    def fetch(self, name):
        self.calls.append(name)
        return self.users.get(name.strip().lower())

    def rename(self, old, new):
        record = self.users.pop(old.strip().lower())
        record = dict(record, name=new)
        self.users[new.strip().lower()] = record


class KeyTests(unittest.TestCase):
    def test_parts_are_normalized(self):
        self.assertEqual(make_key("user", " Alice "), "user:alice")
        self.assertEqual(make_key("user", "ALICE"), make_key("user", "alice"))
        self.assertEqual(make_key("page", 3, "Home"), "page:3:home")


class CacheTests(unittest.TestCase):
    def setUp(self):
        self.clock = FakeClock()
        self.cache = TTLCache(self.clock)

    def test_value_expires_exactly_at_ttl(self):
        self.cache.set("k", "v", 10)
        self.clock.now += 9.99
        self.assertEqual(self.cache.get("k"), "v")
        self.clock.now += 0.01
        self.assertIsNone(self.cache.get("k"))

    def test_expired_entries_are_removed(self):
        self.cache.set("k", "v", 1)
        self.clock.now += 5
        self.cache.get("k")
        self.assertFalse(self.cache.delete("k"), "expired entry must already be gone")

    def test_hit_and_miss_counters(self):
        self.cache.set("k", "v", 10)
        self.cache.get("k")          # hit
        self.cache.get("nope")       # miss
        self.clock.now += 20
        self.cache.get("k")          # expired -> miss
        self.assertEqual((self.cache.hits, self.cache.misses), (1, 2))

    def test_delete_prefix(self):
        for k in ("user:a", "user:b", "page:1"):
            self.cache.set(k, 1, 10)
        self.assertEqual(self.cache.delete_prefix("user:"), 2)
        self.assertEqual(self.cache.get("page:1"), 1)


class ServiceTests(unittest.TestCase):
    def setUp(self):
        self.clock = FakeClock()
        self.cache = TTLCache(self.clock)
        self.backend = Backend({"alice": {"name": "Alice", "plan": "free"}, "bob": {"name": "Bob", "plan": "pro"}})
        self.service = UserService(self.cache, self.backend.fetch, ttl=60)

    def test_spelling_variants_share_one_fetch(self):
        self.service.get_user("Alice")
        self.service.get_user(" alice ")
        self.service.get_user("ALICE")
        self.assertEqual(self.backend.calls, ["Alice"])

    def test_missing_users_are_cached_too(self):
        self.assertIsNone(self.service.get_user("zed"))
        self.assertIsNone(self.service.get_user("ZED"))
        self.assertEqual(len(self.backend.calls), 1)

    def test_get_users_deduplicates_backend_calls(self):
        result = self.service.get_users(["Alice", "alice", "BOB", " bob"])
        self.assertEqual(set(result), {"Alice", "alice", "BOB", " bob"})
        self.assertEqual(result["Alice"], result["alice"])
        self.assertEqual(len(self.backend.calls), 2)

    def test_cache_expires_and_refetches(self):
        self.service.get_user("alice")
        self.clock.now += 60
        self.service.get_user("alice")
        self.assertEqual(len(self.backend.calls), 2)

    def test_rename_invalidates_old_and_new_names(self):
        self.assertEqual(self.service.get_user("Alice")["plan"], "free")
        self.assertIsNone(self.service.get_user("Alicia"))  # cached "no such user"
        self.service.rename_user("alice", "Alicia", self.backend.rename)
        renamed = self.service.get_user("ALICIA")
        self.assertIsNotNone(renamed, "the cached 'missing' entry for the new name must be dropped")
        self.assertEqual(renamed["name"], "Alicia")
        self.assertIsNone(self.service.get_user("alice"), "the old name must not be served from the cache")

    def test_stats_reflect_real_traffic(self):
        self.service.get_user("alice")   # miss
        self.service.get_user("Alice")   # hit
        self.clock.now += 100
        self.service.get_user("alice")   # expired -> miss
        self.assertEqual((self.cache.hits, self.cache.misses), (1, 2))


if __name__ == "__main__":
    unittest.main()
