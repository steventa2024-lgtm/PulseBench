import unittest

from csv_parser import parse_csv, parse_line


class ParseLineTests(unittest.TestCase):
    def test_simple(self):
        self.assertEqual(parse_line("a,b,c"), ["a", "b", "c"])

    def test_empty_fields_and_empty_line(self):
        self.assertEqual(parse_line(""), [""])
        self.assertEqual(parse_line("a,,c"), ["a", "", "c"])
        self.assertEqual(parse_line("a,b,"), ["a", "b", ""])
        self.assertEqual(parse_line(","), ["", ""])

    def test_whitespace_is_preserved(self):
        self.assertEqual(parse_line(" a , b "), [" a ", " b "])

    def test_quoted_fields(self):
        self.assertEqual(parse_line('"a,b",c'), ["a,b", "c"])
        self.assertEqual(parse_line('""'), [""])
        self.assertEqual(parse_line('"",""'), ["", ""])

    def test_escaped_quotes(self):
        self.assertEqual(parse_line('"say ""hi""",x'), ['say "hi"', "x"])
        self.assertEqual(parse_line('""""'), ['"'])

    def test_custom_delimiter(self):
        self.assertEqual(parse_line("a;b,c;d", delimiter=";"), ["a", "b,c", "d"])
        self.assertEqual(parse_line('"a;b";c', delimiter=";"), ["a;b", "c"])

    def test_malformed_input(self):
        with self.assertRaisesRegex(ValueError, "unterminated quote"):
            parse_line('"abc')
        with self.assertRaisesRegex(ValueError, "unexpected quote"):
            parse_line('ab"c')
        with self.assertRaisesRegex(ValueError, "unexpected quote"):
            parse_line('"abc"d')


class ParseCsvTests(unittest.TestCase):
    def test_records(self):
        self.assertEqual(parse_csv("a,b\nc,d"), [["a", "b"], ["c", "d"]])

    def test_crlf_and_trailing_newline(self):
        self.assertEqual(parse_csv("a,b\r\nc,d\r\n"), [["a", "b"], ["c", "d"]])
        self.assertEqual(parse_csv("a\n"), [["a"]])

    def test_empty_text(self):
        self.assertEqual(parse_csv(""), [])

    def test_blank_line_inside_is_an_empty_record(self):
        self.assertEqual(parse_csv("a\n\nb"), [["a"], [""], ["b"]])

    def test_quoted_newlines(self):
        self.assertEqual(parse_csv('id,note\n1,"line1\nline2"\n2,ok'), [["id", "note"], ["1", "line1\nline2"], ["2", "ok"]])
        self.assertEqual(parse_csv('"a\r\nb",c\n'), [["a\r\nb", "c"]])

    def test_errors_propagate(self):
        with self.assertRaisesRegex(ValueError, "unterminated quote"):
            parse_csv('a,"b\nc')


if __name__ == "__main__":
    unittest.main()
