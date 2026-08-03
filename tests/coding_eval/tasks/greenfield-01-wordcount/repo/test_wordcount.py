import io
import pathlib
import tempfile
import unittest
from wordcount import count_words, main

class TestWordCount(unittest.TestCase):
    def test_count(self): self.assertEqual(count_words("one  two\nthree"), 3)
    def test_file_and_stdin(self):
        with tempfile.TemporaryDirectory() as root:
            path = pathlib.Path(root) / "a.txt"; path.write_text("a b")
            self.assertEqual(main([str(path)], io.StringIO("")), 2); self.assertEqual(main(["-"], io.StringIO("x y z")), 3)
    def test_readme(self): self.assertTrue(pathlib.Path("README.md").is_file())
