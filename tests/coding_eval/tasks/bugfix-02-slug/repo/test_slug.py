import unittest
from slug import slugify

class TestSlug(unittest.TestCase):
    def test_collapses_whitespace(self): self.assertEqual(slugify("  Hello   World  "), "hello-world")
    def test_removes_edge_hyphens(self): self.assertEqual(slugify("--Hello--"), "hello")
