import unittest

from rate_limiter import TokenBucket


class FakeClock:
    def __init__(self, now=100.0):
        self.now = now

    def __call__(self):
        return self.now

    def advance(self, seconds):
        self.now += seconds


class TokenBucketTests(unittest.TestCase):
    def setUp(self):
        self.clock = FakeClock()

    def bucket(self, capacity=5, rate=1):
        return TokenBucket(capacity, rate, clock=self.clock)

    def test_starts_full(self):
        self.assertEqual(self.bucket(5).tokens(), 5)

    def test_allow_consumes_the_requested_cost(self):
        b = self.bucket(10, 0)
        self.assertTrue(b.allow(3))
        self.assertEqual(b.tokens(), 7)
        self.assertTrue(b.allow(4))
        self.assertEqual(b.tokens(), 3)

    def test_exact_cost_is_allowed_and_drains_to_zero(self):
        b = self.bucket(2, 0)
        self.assertTrue(b.allow(2))
        self.assertEqual(b.tokens(), 0)
        self.assertFalse(b.allow(1))

    def test_denied_request_takes_nothing(self):
        b = self.bucket(3, 0)
        self.assertTrue(b.allow(2))
        self.assertFalse(b.allow(2))
        self.assertEqual(b.tokens(), 1)

    def test_refill_is_proportional_to_elapsed_time(self):
        b = self.bucket(10, 2)
        b.allow(10)
        self.clock.advance(1.5)
        self.assertAlmostEqual(b.tokens(), 3.0)
        self.clock.advance(0.5)
        self.assertAlmostEqual(b.tokens(), 4.0)

    def test_no_time_passing_means_no_refill(self):
        b = self.bucket(10, 5)
        b.allow(10)
        self.assertEqual(b.tokens(), 0)
        self.assertEqual(b.tokens(), 0)

    def test_refill_never_exceeds_capacity(self):
        b = self.bucket(5, 3)
        b.allow(1)
        self.clock.advance(100)
        self.assertEqual(b.tokens(), 5)

    def test_zero_refill_rate_never_refills(self):
        b = self.bucket(2, 0)
        b.allow(2)
        self.clock.advance(1000)
        self.assertEqual(b.tokens(), 0)

    def test_clock_going_backwards_does_not_change_tokens(self):
        b = self.bucket(5, 1)
        b.allow(2)
        self.clock.advance(-50)
        self.assertEqual(b.tokens(), 3)
        self.clock.advance(2)
        self.assertEqual(b.tokens(), 5)

    def test_invalid_costs_raise(self):
        b = self.bucket(5, 1)
        for cost in (0, -1, 6):
            with self.assertRaises(ValueError):
                b.allow(cost)

    def test_cost_equal_to_capacity_is_valid(self):
        self.assertTrue(self.bucket(5, 1).allow(5))

    def test_invalid_constructor_arguments(self):
        with self.assertRaises(ValueError):
            TokenBucket(0, 1, clock=self.clock)
        with self.assertRaises(ValueError):
            TokenBucket(-3, 1, clock=self.clock)
        with self.assertRaises(ValueError):
            TokenBucket(5, -1, clock=self.clock)

    def test_default_clock_is_usable(self):
        b = TokenBucket(2, 1)
        self.assertTrue(b.allow())


if __name__ == "__main__":
    unittest.main()
