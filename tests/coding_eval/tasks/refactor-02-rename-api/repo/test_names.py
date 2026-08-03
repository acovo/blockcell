import pathlib
import unittest
from names import canonical_name, normalize_name
from users import user_key
from groups import group_key

class TestRename(unittest.TestCase):
    def test_new_and_compat_api(self): self.assertEqual(canonical_name(" A  B "), "a b"); self.assertEqual(normalize_name(" A  B "), "a b")
    def test_callers(self): self.assertEqual(user_key(" Ada "), "user:ada"); self.assertEqual(group_key(" Ops "), "group:ops")
    def test_production_callers_use_new_name(self):
        self.assertNotIn("normalize_name", pathlib.Path("users.py").read_text() + pathlib.Path("groups.py").read_text())
