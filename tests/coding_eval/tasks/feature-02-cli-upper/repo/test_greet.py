import unittest
from greet import main

class TestGreet(unittest.TestCase):
    def test_default(self): self.assertEqual(main(["Ada"]), "Hello, Ada!")
    def test_upper(self): self.assertEqual(main(["Ada", "--upper"]), "HELLO, ADA!")
