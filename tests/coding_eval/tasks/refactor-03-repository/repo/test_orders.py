import unittest
from service import OrderService

class FakeRepository:
    def __init__(self): self.values = {}
    def save(self, key, value): self.values[key] = value
    def get(self, key): return self.values[key]

class TestOrders(unittest.TestCase):
    def test_repository_is_injected(self):
        repo = FakeRepository(); service = OrderService(repo)
        service.create("a", 12); self.assertEqual(service.total("a"), 12); self.assertEqual(repo.values, {"a": 12})
