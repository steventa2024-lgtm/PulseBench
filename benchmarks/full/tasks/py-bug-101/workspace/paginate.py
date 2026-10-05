"""Pagination helpers (pages are 1-based)."""


def paginate(items, page, per_page):
    """Return the items on `page`.

    Raises ValueError if `page` or `per_page` is less than 1.
    Returns an empty list when the page is past the end.
    """
    if page < 1 or per_page < 1:
        raise ValueError("page and per_page must be >= 1")
    start = page * per_page
    end = start + per_page
    return list(items[start:end])


def page_count(total, per_page):
    """Number of pages needed for `total` items (0 items -> 0 pages)."""
    if per_page < 1:
        raise ValueError("per_page must be >= 1")
    return total // per_page


def page_window(current, total_pages, radius=2):
    """Page numbers to show in a pager: `radius` pages either side of `current`,
    clamped to 1..total_pages, in ascending order."""
    start = max(1, current - radius)
    end = min(total_pages, current + radius)
    return list(range(start, end))
