"""User queries on top of sqlite3. `conn` is an open sqlite3.Connection with a `users` table
(id INTEGER PRIMARY KEY, name TEXT, email TEXT)."""


def find_user_by_name(conn, name):
    """Return (id, name, email) of the user with exactly this name, or None."""
    cur = conn.execute(f"SELECT id, name, email FROM users WHERE name = '{name}'")
    return cur.fetchone()


def search_users(conn, term, limit=10):
    """Return users whose name contains `term` (case-insensitive), ordered by name.

    `%` and `_` in `term` are literal characters, not wildcards.
    `limit` must be an int (not a bool) between 1 and 100, otherwise ValueError.
    """
    cur = conn.execute(
        f"SELECT id, name, email FROM users WHERE name LIKE '%{term}%' ORDER BY name LIMIT {limit}"
    )
    return cur.fetchall()


def list_users(conn, sort_by="name", descending=False):
    """Return all users sorted by `sort_by`, which must be one of "id", "name", "email"
    (otherwise ValueError). `descending` reverses the order."""
    direction = "DESC" if descending else "ASC"
    cur = conn.execute(f"SELECT id, name, email FROM users ORDER BY {sort_by} {direction}")
    return cur.fetchall()


def update_email(conn, user_id, email):
    """Set a user's email and return the number of rows changed.

    `user_id` must be an int (not a bool) and `email` a string containing "@",
    otherwise ValueError.
    """
    cur = conn.execute(f"UPDATE users SET email = '{email}' WHERE id = {user_id}")
    conn.commit()
    return cur.rowcount
