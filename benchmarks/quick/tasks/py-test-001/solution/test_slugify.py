import unittest

from slugify import slugify


class SlugifyTests(unittest.TestCase):
    def test_basic(self):
        self.assertEqual(slugify("Hello World"), "hello-world")

    def test_lowercases(self):
        self.assertEqual(slugify("ABC Def"), "abc-def")

    def test_strips_accents(self):
        self.assertEqual(slugify("Café Crème"), "cafe-creme")

    def test_collapses_and_trims_separators(self):
        self.assertEqual(slugify("  --Hello,   World!!  "), "hello-world")

    def test_custom_separator(self):
        self.assertEqual(slugify("Hello World", separator="_"), "hello_world")

    def test_max_length_truncates(self):
        self.assertEqual(slugify("abcdefghij", max_length=5), "abcde")

    def test_truncation_does_not_leave_trailing_separator(self):
        self.assertEqual(slugify("abcd efgh", max_length=5), "abcd")

    def test_max_length_none_means_unlimited(self):
        self.assertEqual(len(slugify("a" * 200, max_length=None)), 200)

    def test_non_string_raises(self):
        with self.assertRaises(TypeError):
            slugify(None)
        with self.assertRaises(TypeError):
            slugify(123)

    def test_empty_and_symbols_only(self):
        self.assertEqual(slugify(""), "")
        self.assertEqual(slugify("!!!"), "")


if __name__ == "__main__":
    unittest.main()
