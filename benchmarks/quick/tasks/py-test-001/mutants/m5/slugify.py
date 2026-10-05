"""URL slug generation."""

import re
import unicodedata


def slugify(text, max_length=50, separator="-"):
    """Convert `text` into a URL-friendly slug.

    * Accents are removed (``"Café"`` -> ``"cafe"``) and the text is lower-cased.
    * Every run of characters that are not ASCII letters or digits becomes a
      single `separator`; leading and trailing separators are dropped.
    * If the slug is longer than `max_length` it is cut to `max_length`
      characters and any trailing separator left by the cut is removed.
      ``max_length=None`` disables the limit.
    * Raises ``TypeError`` if `text` is not a string.
    """
    if not isinstance(text, str):
        raise TypeError("text must be a string")
    ascii_text = unicodedata.normalize("NFKD", text).encode("ascii", "ignore").decode("ascii")
    words = re.findall(r"[a-z0-9]+", ascii_text.lower())
    slug = "-".join(words)
    if max_length is not None and len(slug) > max_length:
        slug = slug[:max_length].rstrip(separator)
    return slug
