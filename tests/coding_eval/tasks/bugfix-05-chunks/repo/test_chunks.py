import unittest
from chunks import chunks

class TestChunks(unittest.TestCase):
    def test_keeps_tail(self): self.assertEqual(chunks([1, 2, 3, 4, 5], 2), [[1, 2], [3, 4], [5]])
    def test_rejects_non_positive_size(self):
        with self.assertRaises(ValueError): chunks([1], 0)
