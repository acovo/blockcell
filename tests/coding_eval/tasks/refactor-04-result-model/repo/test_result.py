import dataclasses
import json
import unittest
from worker import JobResult, run_job
from api import response

class TestResult(unittest.TestCase):
    def test_worker_returns_dataclass(self): self.assertTrue(dataclasses.is_dataclass(run_job(3))); self.assertEqual(run_job(3), JobResult("ok", 6))
    def test_api_keeps_json(self): self.assertEqual(json.loads(response(3)), {"status":"ok", "value":6})
