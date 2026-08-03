import unittest
from config import deep_merge

class TestDeepMerge(unittest.TestCase):
    def test_nested_merge_without_mutation(self):
        left = {"db": {"host": "a", "port": 1}, "debug": False}; right = {"db": {"port": 2}}
        self.assertEqual(deep_merge(left, right), {"db": {"host": "a", "port": 2}, "debug": False})
        self.assertEqual(left["db"]["port"], 1)
