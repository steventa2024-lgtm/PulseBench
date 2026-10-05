"""Account report processing."""

from datetime import datetime
from decimal import Decimal, InvalidOperation


def process_report(lines):
    """Process `date,user,kind,amount` lines.

    Returns {"totals": {user: Decimal}, "errors": [str], "count": int, "largest": (user, Decimal) | None}.
    Malformed or invalid lines are skipped and described in "errors" ("line N: reason").
    Blank lines and lines starting with "#" are ignored.
    """
    totals = {}
    errors = []
    count = 0
    largest = None
    for lineno, raw in enumerate(lines, start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = [p.strip() for p in line.split(",")]
        if len(parts) != 4:
            errors.append(f"line {lineno}: expected 4 fields")
            continue
        date, user, kind, amount_text = parts
        try:
            amount = Decimal(amount_text)
        except InvalidOperation:
            errors.append(f"line {lineno}: invalid amount")
            continue
        try:
            datetime.strptime(date, "%Y-%m-%d")
        except ValueError:
            errors.append(f"line {lineno}: invalid date")
            continue
        if not user:
            errors.append(f"line {lineno}: empty user")
            continue
        if kind not in ("deposit", "withdrawal"):
            errors.append(f"line {lineno}: unknown kind")
            continue
        if amount <= 0:
            errors.append(f"line {lineno}: amount must be positive")
            continue
        sign = 1 if kind == "deposit" else -1
        totals[user] = totals.get(user, Decimal("0")) + sign * amount
        count += 1
        if largest is None or amount > largest[1]:
            largest = (user, amount)
    return {"totals": totals, "errors": errors, "count": count, "largest": largest}
