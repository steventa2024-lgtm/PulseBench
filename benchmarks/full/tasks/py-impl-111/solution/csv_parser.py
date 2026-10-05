"""A small RFC 4180 style CSV parser (no use of the `csv` module)."""


def _parse(text, delimiter):
    records = []
    record = []
    field = []
    i = 0
    n = len(text)
    in_quotes = False
    quoted_field = False  # current field started with a quote
    after_quote = False   # just closed a quoted field
    while i < n:
        ch = text[i]
        if in_quotes:
            if ch == '"':
                if i + 1 < n and text[i + 1] == '"':
                    field.append('"')
                    i += 2
                    continue
                in_quotes = False
                after_quote = True
            else:
                field.append(ch)
        else:
            if ch == '"':
                if field or quoted_field or after_quote:
                    raise ValueError("unexpected quote")
                in_quotes = True
                quoted_field = True
            elif ch == delimiter:
                record.append("".join(field))
                field = []
                quoted_field = False
                after_quote = False
            elif ch == "\n" or (ch == "\r" and i + 1 < n and text[i + 1] == "\n"):
                if ch == "\r":
                    i += 1
                record.append("".join(field))
                records.append(record)
                record, field = [], []
                quoted_field = False
                after_quote = False
            else:
                if after_quote:
                    raise ValueError("unexpected quote")
                field.append(ch)
        i += 1
    if in_quotes:
        raise ValueError("unterminated quote")
    if field or quoted_field or record or after_quote:
        record.append("".join(field))
        records.append(record)
    return records


def parse_line(line, delimiter=","):
    """Parse ONE logical CSV record (no trailing newline) into a list of strings."""
    if line == "":
        return [""]
    records = _parse(line, delimiter)
    if not records:
        return [""]
    return records[0]


def parse_csv(text, delimiter=","):
    """Parse a whole document into a list of records (each a list of strings)."""
    if text == "":
        return []
    return _parse(text, delimiter)
