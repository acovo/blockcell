import json
import pathlib
import tempfile
import unittest
from kv import main

class TestKv(unittest.TestCase):
    def test_lifecycle(self):
        with tempfile.TemporaryDirectory() as root:
            path = pathlib.Path(root) / "db.json"
            self.assertEqual(main([str(path), "set", "a", "1"]), 0); self.assertEqual(main([str(path), "get", "a"]), "1")
            self.assertEqual(main([str(path), "delete", "a"]), 0); self.assertNotIn("a", json.loads(path.read_text()))
    def test_readme(self): self.assertTrue(pathlib.Path("README.md").is_file())
