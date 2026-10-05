"""User queries on top of sqlite3. `conn` is an open sqlite3.Connection with a `users` table
(id INTEGER PRIMARY KEY, name TEXT, email TEXT)."""

SORTABLE = ("id", "name", "email")


def _is_int(value):
    return isinstance(value, int) and not isinstance(value, bool)


def find_user_by_name(conn, name):
    """Return (id, name, email) of the user with exactly this name, or None."""
    cur = conn.execute("SELECT id, name, email FROM users WHERE name = ?", (name,))
    return cur.fetchone()


def search_users(conn, term, limit=10):
    """Return users whose name contains `term` (case-insensitive), ordered by name."""
    if not _is_int(limit) or not 1 <= limit <= 100:
        raise ValueError("limit must be an integer between 1 and 100")
    escaped = term.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")
    cur = conn.execute(
        "SELECT id, name, email FROM users WHERE name LIKE ? ESCAPE '\\' ORDER BY name LIMIT ?",
        (f"%{escaped}%", limit),
    )
    return cur.fetchall()


def list_users(conn, sort_by="name", descending=False):
    """Return all users sorted by an allow-listed column."""
    if sort_by not in SORTABLE:
        raise ValueError(f"cannot sort by {sort_by!r}")
    direction = "DESC" if descending else "ASC"
    cur = conn.execute(f"SELECT id, name, email FROM users ORDER BY {sort_by} {direction}")
    return cur.fetchall()


def update_email(conn, user_id, email):
    """Set a user's email and return the number of rows changed."""
    if not _is_int(user_id):
        raise ValueError("user_id must be an integer")
    if not isinstance(email, str) or "@" not in email:
        raise ValueError("email must be a string containing '@'")
    cur = conn.execute("UPDATE users SET email = ? WHERE id = ?", (email, user_id))
    conn.commit()
    return cur.rowcount
