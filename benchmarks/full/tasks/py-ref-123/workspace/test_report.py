import unittest
from decimal import Decimal

from report import parse_line, process_report, summarize, validate_record


class ParseLineTests(unittest.TestCase):
    def test_parses_and_strips_fields(self):
        self.assertEqual(
            parse_line(" 2024-01-05 , alice , deposit , 100.50 ", 1),
            {"date": "2024-01-05", "user": "alice", "kind": "deposit", "amount": Decimal("100.50")},
        )

    def test_blank_and_comment_lines_give_none(self):
        self.assertIsNone(parse_line("", 1))
        self.assertIsNone(parse_line("   ", 2))
        self.assertIsNone(parse_line("# a comment", 3))
        self.assertIsNone(parse_line("   # indented comment", 4))

    def test_wrong_field_count(self):
        with self.assertRaisesRegex(ValueError, r"^line 7: expected 4 fields$"):
            parse_line("a,b,c", 7)
        with self.assertRaisesRegex(ValueError, r"^line 2: expected 4 fields$"):
            parse_line("a,b,c,d,e", 2)

    def test_invalid_amount(self):
        with self.assertRaisesRegex(ValueError, r"^line 3: invalid amount$"):
            parse_line("2024-01-05,alice,deposit,abc", 3)


class ValidateRecordTests(unittest.TestCase):
    good = {"date": "2024-02-29", "user": "bob", "kind": "withdrawal", "amount": Decimal("5")}

    def test_valid_record_returns_none(self):
        self.assertIsNone(validate_record(self.good, 1))

    def check(self, **changes):
        record = dict(self.good, **changes)
        return validate_record(record, 9)

    def test_each_rule(self):
        with self.assertRaisesRegex(ValueError, r"^line 9: invalid date$"):
            self.check(date="2023-02-29")
        with self.assertRaisesRegex(ValueError, r"^line 9: invalid date$"):
            self.check(date="05/01/2024")
        with self.assertRaisesRegex(ValueError, r"^line 9: empty user$"):
            self.check(user="")
        with self.assertRaisesRegex(ValueError, r"^line 9: unknown kind$"):
            self.check(kind="transfer")
        with self.assertRaisesRegex(ValueError, r"^line 9: amount must be positive$"):
            self.check(amount=Decimal("0"))
        with self.assertRaisesRegex(ValueError, r"^line 9: amount must be positive$"):
            self.check(amount=Decimal("-1"))

    def test_rules_are_checked_in_order(self):
        with self.assertRaisesRegex(ValueError, "invalid date"):
            self.check(date="nope", user="", kind="x", amount=Decimal("-1"))
        with self.assertRaisesRegex(ValueError, "empty user"):
            self.check(user="", kind="x", amount=Decimal("-1"))
        with self.assertRaisesRegex(ValueError, "unknown kind"):
            self.check(kind="x", amount=Decimal("-1"))


class SummarizeTests(unittest.TestCase):
    def test_totals_count_and_largest(self):
        records = [
            {"date": "2024-01-01", "user": "a", "kind": "deposit", "amount": Decimal("100")},
            {"date": "2024-01-02", "user": "a", "kind": "withdrawal", "amount": Decimal("30.5")},
            {"date": "2024-01-03", "user": "b", "kind": "deposit", "amount": Decimal("100")},
        ]
        s = summarize(records)
        self.assertEqual(s["totals"], {"a": Decimal("69.5"), "b": Decimal("100")})
        self.assertEqual(s["count"], 3)
        self.assertEqual(s["largest"], ("a", Decimal("100")), "first record wins ties")

    def test_empty(self):
        self.assertEqual(summarize([]), {"totals": {}, "count": 0, "largest": None})


class ProcessReportTests(unittest.TestCase):
    def test_end_to_end_with_errors(self):
        lines = [
            "# header",
            "2024-01-05,alice,deposit,100.50",
            "",
            "2024-01-06,alice,withdrawal,20",
            "2024-13-01,bob,deposit,5",
            "2024-01-07,bob,deposit,abc",
            "2024-01-08,,deposit,5",
            "2024-01-09,bob,gift,5",
            "2024-01-10,bob,deposit,-5",
            "only,three,fields",
            "2024-01-11,bob,deposit,250",
        ]
        r = process_report(lines)
        self.assertEqual(r["totals"], {"alice": Decimal("80.50"), "bob": Decimal("250")})
        self.assertEqual(r["count"], 3)
        self.assertEqual(r["largest"], ("bob", Decimal("250")))
        self.assertEqual(
            r["errors"],
            [
                "line 5: invalid date",
                "line 6: invalid amount",
                "line 7: empty user",
                "line 8: unknown kind",
                "line 9: amount must be positive",
                "line 10: expected 4 fields",
            ],
        )

    def test_amount_is_parsed_before_other_checks(self):
        r = process_report(["bad-date,alice,deposit,abc"])
        self.assertEqual(r["errors"], ["line 1: invalid amount"])

    def test_empty_input(self):
        self.assertEqual(process_report([]), {"totals": {}, "errors": [], "count": 0, "largest": None})


if __name__ == "__main__":
    unittest.main()
