import pathlib
import tempfile
import unittest
from todo import TodoStore

class TestTodo(unittest.TestCase):
    def test_persists_and_completes(self):
        with tempfile.TemporaryDirectory() as root:
            path = pathlib.Path(root) / "todo.json"; store = TodoStore(path); item = store.add("ship")
            store.complete(item["id"]); self.assertTrue(TodoStore(path).list()[0]["completed"])
    def test_readme(self): self.assertTrue(pathlib.Path("README.md").is_file())
