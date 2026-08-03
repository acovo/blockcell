import unittest
from math_utils import safe_divide

class TestSafeDivide(unittest.TestCase):
    def test_divides_numbers(self): self.assertEqual(safe_divide(8, 2), 4)
    def test_zero_returns_none(self): self.assertIsNone(safe_divide(8, 0))
