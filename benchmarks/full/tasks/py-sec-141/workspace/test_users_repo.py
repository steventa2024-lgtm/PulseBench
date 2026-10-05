import sqlite3
import unittest

from users_repo import find_user_by_name, list_users, search_users, update_email

EVIL = "x' OR '1'='1"


class RepoTestCase(unittest.TestCase):
    def setUp(self):
        self.conn = sqlite3.connect(":memory:")
        self.conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)")
        self.conn.executemany(
            "INSERT INTO users (id, name, email) VALUES (?, ?, ?)",
            [
                (1, "alice", "alice@example.com"),
                (2, "Bob", "bob@example.com"),
                (3, "O'Brien", "obrien@example.com"),
                (4, "100%_sure", "sure@example.com"),
                (5, "carol", "carol@example.com"),
            ],
        )
        self.conn.commit()

    def tearDown(self):
        self.conn.close()

    def table_exists(self):
        row = self.conn.execute("SELECT name FROM sqlite_master WHERE name = 'users'").fetchone()
        return row is not None


class FindUserTests(RepoTestCase):
    def test_finds_exact_name(self):
        self.assertEqual(find_user_by_name(self.conn, "alice"), (1, "alice", "alice@example.com"))

    def test_missing_user(self):
        self.assertIsNone(find_user_by_name(self.conn, "nobody"))

    def test_names_with_quotes_work(self):
        self.assertEqual(find_user_by_name(self.conn, "O'Brien")[0], 3)

    def test_injection_returns_nothing(self):
        self.assertIsNone(find_user_by_name(self.conn, EVIL))
        self.assertIsNone(find_user_by_name(self.conn, "' OR 1=1 --"))

    def test_stacked_statement_is_not_executed(self):
        try:
            find_user_by_name(self.conn, "x'; DROP TABLE users; --")
        except Exception:
            pass
        self.assertTrue(self.table_exists())


class SearchTests(RepoTestCase):
    def test_case_insensitive_contains_sorted(self):
        self.assertEqual([r[1] for r in search_users(self.conn, "AL")], ["alice"])
        self.assertEqual([r[1] for r in search_users(self.conn, "o")], ["Bob", "O'Brien", "carol"])

    def test_wildcards_are_literal(self):
        self.assertEqual(search_users(self.conn, "%"), [(4, "100%_sure", "sure@example.com")])
        self.assertEqual([r[0] for r in search_users(self.conn, "_")], [4])
        self.assertEqual(search_users(self.conn, "0%_s")[0][0], 4)

    def test_quote_in_term(self):
        self.assertEqual([r[0] for r in search_users(self.conn, "'")], [3])

    def test_injection_in_term(self):
        self.assertEqual(search_users(self.conn, "%' OR '1'='1"), [])

    def test_limit_is_validated_and_applied(self):
        self.assertEqual(len(search_users(self.conn, "", limit=2)), 2)
        for bad in (0, 101, -1, "5", 2.5, True, None, "1; DROP TABLE users"):
            with self.assertRaises(ValueError, msg=repr(bad)):
                search_users(self.conn, "a", limit=bad)
        self.assertTrue(self.table_exists())


class ListUsersTests(RepoTestCase):
    def test_sort_columns(self):
        self.assertEqual([r[0] for r in list_users(self.conn, "id")], [1, 2, 3, 4, 5])
        self.assertEqual([r[0] for r in list_users(self.conn, "id", descending=True)], [5, 4, 3, 2, 1])
        self.assertEqual([r[1] for r in list_users(self.conn)][0], "100%_sure")
        self.assertEqual([r[2] for r in list_users(self.conn, "email")][0], "alice@example.com")

    def test_only_allow_listed_columns(self):
        for bad in ("password", "name; DROP TABLE users", "1", "", "NAME", "name DESC", "(SELECT 1)"):
            with self.assertRaises(ValueError, msg=repr(bad)):
                list_users(self.conn, bad)
        self.assertTrue(self.table_exists())


class UpdateEmailTests(RepoTestCase):
    def test_updates_and_returns_rowcount(self):
        self.assertEqual(update_email(self.conn, 2, "new@example.com"), 1)
        self.assertEqual(find_user_by_name(self.conn, "Bob")[2], "new@example.com")
        self.assertEqual(update_email(self.conn, 999, "x@y.z"), 0)

    def test_hostile_email_is_stored_literally(self):
        payload = "a@b.c'; DROP TABLE users; --"
        self.assertEqual(update_email(self.conn, 1, payload), 1)
        self.assertEqual(self.conn.execute("SELECT email FROM users WHERE id = 1").fetchone()[0], payload)
        self.assertTrue(self.table_exists())
        self.assertEqual(self.conn.execute("SELECT COUNT(*) FROM users").fetchone()[0], 5)

    def test_hostile_id_is_rejected(self):
        for bad in ("1 OR 1=1", "1; DROP TABLE users", 1.0, True, None):
            with self.assertRaises(ValueError, msg=repr(bad)):
                update_email(self.conn, bad, "a@b.c")
        emails = [r[0] for r in self.conn.execute("SELECT email FROM users ORDER BY id")]
        self.assertEqual(emails[0], "alice@example.com")

    def test_invalid_email_is_rejected(self):
        for bad in ("no-at-sign", "", None, 5):
            with self.assertRaises(ValueError, msg=repr(bad)):
                update_email(self.conn, 1, bad)


if __name__ == "__main__":
    unittest.main()
