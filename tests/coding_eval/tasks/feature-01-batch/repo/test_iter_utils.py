import unittest
from iter_utils import batched

class TestBatched(unittest.TestCase):
    def test_generator_and_tail(self): self.assertEqual(list(batched(iter(range(5)), 2)), [(0, 1), (2, 3), (4,)])
    def test_invalid_size(self):
        with self.assertRaises(ValueError): list(batched([], 0))
