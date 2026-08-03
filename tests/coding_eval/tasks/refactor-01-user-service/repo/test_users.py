import unittest
import formatter
from api import get_user_label

class TestUsers(unittest.TestCase):
    def test_formatter_is_reusable(self): self.assertEqual(formatter.format_user({"name":" ada ", "email":" ADA@EXAMPLE.COM "}), "Ada <ada@example.com>")
    def test_api_uses_same_format(self): self.assertEqual(get_user_label({"name":" ada ", "email":" ADA@EXAMPLE.COM "}), "Ada <ada@example.com>")
