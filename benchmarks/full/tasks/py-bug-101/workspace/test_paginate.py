import unittest

from paginate import page_count, page_window, paginate

DATA = list(range(1, 11))  # 1..10


class PaginateTests(unittest.TestCase):
    def test_first_page(self):
        self.assertEqual(paginate(DATA, 1, 4), [1, 2, 3, 4])

    def test_middle_and_partial_last_page(self):
        self.assertEqual(paginate(DATA, 2, 4), [5, 6, 7, 8])
        self.assertEqual(paginate(DATA, 3, 4), [9, 10])

    def test_past_the_end_is_empty(self):
        self.assertEqual(paginate(DATA, 4, 4), [])

    def test_invalid_arguments(self):
        with self.assertRaises(ValueError):
            paginate(DATA, 0, 4)
        with self.assertRaises(ValueError):
            paginate(DATA, 1, 0)

    def test_does_not_return_a_view_of_the_input(self):
        out = paginate(DATA, 1, 3)
        out.append(99)
        self.assertEqual(DATA[:3], [1, 2, 3])


class PageCountTests(unittest.TestCase):
    def test_rounds_up(self):
        self.assertEqual(page_count(10, 4), 3)
        self.assertEqual(page_count(8, 4), 2)
        self.assertEqual(page_count(1, 4), 1)

    def test_zero_items(self):
        self.assertEqual(page_count(0, 4), 0)

    def test_invalid_per_page(self):
        with self.assertRaises(ValueError):
            page_count(5, 0)


class PageWindowTests(unittest.TestCase):
    def test_window_in_the_middle(self):
        self.assertEqual(page_window(5, 10), [3, 4, 5, 6, 7])

    def test_clamped_at_both_ends(self):
        self.assertEqual(page_window(1, 10), [1, 2, 3])
        self.assertEqual(page_window(10, 10), [8, 9, 10])

    def test_custom_radius_and_few_pages(self):
        self.assertEqual(page_window(2, 3, radius=1), [1, 2, 3])
        self.assertEqual(page_window(1, 1), [1])


if __name__ == "__main__":
    unittest.main()
