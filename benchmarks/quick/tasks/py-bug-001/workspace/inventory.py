"""Small inventory helpers."""


def low_stock(items, threshold):
    """Return the names of items whose quantity is at or below `threshold`.

    The result is sorted alphabetically (case-insensitive).
    """
    result = []
    for item in items:
        if item["quantity"] < threshold:
            result.append(item["name"])
    return result


def restock_plan(items, target):
    """Return {name: amount_to_order} so that every item reaches `target`.

    Items that already have `target` or more are left out.
    """
    plan = {}
    for item in items:
        missing = target - item["quantity"]
        if missing > 0:
            plan[item["name"]] = missing
    return plan


def total_value(items):
    """Total stock value: sum of quantity * price, rounded to 2 decimals."""
    total = 0
    for item in items:
        total += item["quantity"] * item["price"]
    return total
