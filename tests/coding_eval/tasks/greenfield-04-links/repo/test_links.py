import json
import pathlib
import tempfile
import unittest
from links import extract_links, main

class TestLinks(unittest.TestCase):
    def test_extract_ignores_images(self): self.assertEqual(extract_links("[A](https://a) ![x](img.png)"), [{"text":"A", "url":"https://a"}])
    def test_cli_json(self):
        with tempfile.TemporaryDirectory() as root:
            path = pathlib.Path(root) / "a.md"; path.write_text("[A](https://a)")
            self.assertEqual(json.loads(main([str(path)]))[0]["url"], "https://a")
    def test_readme(self): self.assertTrue(pathlib.Path("README.md").is_file())
