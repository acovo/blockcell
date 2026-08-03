import unittest
from parsing import parse_bool

class TestParseBool(unittest.TestCase):
    def test_true_values(self):
        for value in ("true", " TRUE ", "Yes", "1"): self.assertTrue(parse_bool(value))
    def test_false_values(self):
        for value in ("false", " NO ", "0"): self.assertFalse(parse_bool(value))
    def test_invalid_raises(self):
        with self.assertRaises(ValueError): parse_bool("maybe")
