"""Account report processing."""

from datetime import datetime
from decimal import Decimal, InvalidOperation

KINDS = ("deposit", "withdrawal")


def parse_line(line, lineno):
    """Parse one `date,user,kind,amount` line (None for blank/comment lines)."""
    line = line.strip()
    if not line or line.startswith("#"):
        return None
    parts = [p.strip() for p in line.split(",")]
    if len(parts) != 4:
        raise ValueError(f"line {lineno}: expected 4 fields")
    date, user, kind, amount_text = parts
    try:
        amount = Decimal(amount_text)
    except InvalidOperation:
        raise ValueError(f"line {lineno}: invalid amount") from None
    return {"date": date, "user": user, "kind": kind, "amount": amount}


def validate_record(record, lineno):
    """Raise ValueError if the record breaks a business rule."""
    try:
        datetime.strptime(record["date"], "%Y-%m-%d")
    except ValueError:
        raise ValueError(f"line {lineno}: invalid date") from None
    if not record["user"]:
        raise ValueError(f"line {lineno}: empty user")
    if record["kind"] not in KINDS:
        raise ValueError(f"line {lineno}: unknown kind")
    if record["amount"] <= 0:
        raise ValueError(f"line {lineno}: amount must be positive")


def summarize(records):
    """Totals per user, record count and the largest single amount."""
    totals = {}
    largest = None
    for r in records:
        sign = 1 if r["kind"] == "deposit" else -1
        totals[r["user"]] = totals.get(r["user"], Decimal("0")) + sign * r["amount"]
        if largest is None or r["amount"] > largest[1]:
            largest = (r["user"], r["amount"])
    return {"totals": totals, "count": len(records), "largest": largest}


def process_report(lines):
    """Process `date,user,kind,amount` lines (see module helpers for the rules)."""
    records = []
    errors = []
    for lineno, raw in enumerate(lines, start=1):
        try:
            record = parse_line(raw, lineno)
            if record is None:
                continue
            validate_record(record, lineno)
        except ValueError as exc:
            errors.append(str(exc))
            continue
        records.append(record)
    result = summarize(records)
    result["errors"] = errors
    return result
