import unittest
from retrying import retry

class TestRetry(unittest.TestCase):
    def test_retries_selected_exception(self):
        calls = []
        def work():
            calls.append(1)
            if len(calls) < 3: raise ValueError("again")
            return "ok"
        self.assertEqual(retry(work, attempts=3, exceptions=(ValueError,)), "ok")
        self.assertEqual(len(calls), 3)
    def test_does_not_retry_other_exception(self):
        with self.assertRaises(TypeError): retry(lambda: (_ for _ in ()).throw(TypeError()), attempts=3, exceptions=(ValueError,))
