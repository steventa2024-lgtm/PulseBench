import unittest

from inventory import low_stock, restock_plan, total_value

ITEMS = [
    {"name": "widget", "quantity": 5, "price": 2.5},
    {"name": "Bolt", "quantity": 10, "price": 0.1},
    {"name": "gadget", "quantity": 0, "price": 19.99},
    {"name": "Anchor", "quantity": 10, "price": 3.3},
]


class LowStockTests(unittest.TestCase):
    def test_includes_items_at_threshold(self):
        self.assertIn("Bolt", low_stock(ITEMS, 10))

    def test_excludes_items_above_threshold(self):
        self.assertEqual(low_stock(ITEMS, 5), ["gadget", "widget"])

    def test_sorted_case_insensitively(self):
        self.assertEqual(low_stock(ITEMS, 10), ["Anchor", "Bolt", "gadget", "widget"])

    def test_empty(self):
        self.assertEqual(low_stock([], 3), [])


class RestockTests(unittest.TestCase):
    def test_plan(self):
        self.assertEqual(restock_plan(ITEMS, 8), {"widget": 3, "gadget": 8})

    def test_nothing_to_order(self):
        self.assertEqual(restock_plan(ITEMS, 0), {})


class ValueTests(unittest.TestCase):
    def test_total_is_rounded(self):
        # 5*2.5 + 10*0.1 + 0*19.99 + 10*3.3 = 12.5 + 1.0 + 0 + 33.0
        self.assertEqual(total_value(ITEMS), 46.5)

    def test_floating_point_noise_is_removed(self):
        self.assertEqual(total_value([{"name": "x", "quantity": 3, "price": 0.1}]), 0.3)


if __name__ == "__main__":
    unittest.main()
