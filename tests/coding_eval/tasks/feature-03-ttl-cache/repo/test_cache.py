import unittest
from cache import TTLCache

class Clock:
    def __init__(self): self.value = 0
    def __call__(self): return self.value

class TestTTLCache(unittest.TestCase):
    def test_expiry_and_cleanup(self):
        clock = Clock(); cache = TTLCache(ttl=5, clock=clock)
        cache.set("a", 1); self.assertEqual(cache.get("a"), 1)
        clock.value = 6; self.assertIsNone(cache.get("a")); self.assertEqual(len(cache), 0)
