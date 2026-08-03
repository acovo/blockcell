import unittest
from collections_utils import dedupe

class TestDedupe(unittest.TestCase):
    def test_preserves_first_seen_order(self): self.assertEqual(dedupe([3, 1, 3, 2, 1]), [3, 1, 2])
    def test_empty(self): self.assertEqual(dedupe([]), [])
