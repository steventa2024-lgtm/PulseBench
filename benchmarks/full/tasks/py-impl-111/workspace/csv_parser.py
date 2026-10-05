"""A small RFC 4180 style CSV parser (no use of the `csv` module)."""


def parse_line(line, delimiter=","):
    """Parse ONE logical CSV record (no trailing newline) into a list of strings.

    * Fields are separated by `delimiter`.
    * A field may be wrapped in double quotes; inside quotes the delimiter is literal and a
      doubled quote ("") stands for one quote character.
    * Whitespace is preserved exactly (nothing is stripped).
    * An empty line gives [""]; a trailing delimiter gives a trailing empty field.
    * Raises ValueError("unterminated quote") if a quoted field is not closed, and
      ValueError("unexpected quote") if a quote appears in the middle of an unquoted field
      or if characters follow the closing quote of a quoted field.
    """
    raise NotImplementedError


def parse_csv(text, delimiter=","):
    """Parse a whole document into a list of records (each a list of strings).

    * Records are separated by "\\n" or "\\r\\n".
    * A quoted field may contain newlines, which are kept as-is.
    * A single trailing newline does not create an extra empty record.
    * Empty text gives [].
    * Raises ValueError for the same malformed-quote cases as `parse_line`.
    """
    raise NotImplementedError
