import json
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

import runner


class CodingEvalRunnerTests(unittest.TestCase):
    def test_manifest_has_required_twenty_task_distribution(self):
        tasks = runner.load_manifest(ROOT / "manifest.json")

        self.assertEqual(len(tasks), 20)
        counts = {}
        for task in tasks:
            counts[task["category"]] = counts.get(task["category"], 0) + 1
            self.assertTrue((ROOT / task["fixture"]).is_dir())
            self.assertTrue((ROOT / task["acceptance"]).is_file())

        self.assertEqual(
            counts,
            {"bugfix": 6, "feature": 6, "refactor": 4, "greenfield": 4},
        )

    def test_failure_attribution_uses_actionable_priority(self):
        self.assertEqual(
            runner.classify_failure(
                {"timed_out": False, "tool_calls": 8, "tests_run": 2, "subagent_errors": 1},
                max_tool_calls=50,
            ),
            "subagent_coordination",
        )
        self.assertEqual(
            runner.classify_failure(
                {"timed_out": False, "tool_calls": 8, "tests_run": 0, "subagent_errors": 0},
                max_tool_calls=50,
            ),
            "verification_missing",
        )
        self.assertEqual(
            runner.classify_failure(
                {"timed_out": True, "tool_calls": 8, "tests_run": 2, "subagent_errors": 0},
                max_tool_calls=50,
            ),
            "navigation",
        )
        self.assertEqual(
            runner.classify_failure(
                {"timed_out": False, "tool_calls": 8, "tests_run": 2, "subagent_errors": 0},
                max_tool_calls=50,
            ),
            "editing_failure",
        )

    def test_report_summarizes_completion_calls_tokens_and_failures(self):
        report = runner.summarize(
            [
                {
                    "task_id": "a",
                    "passed": True,
                    "tool_calls": 10,
                    "input_tokens": 100,
                    "output_tokens": 50,
                    "failure_category": None,
                },
                {
                    "task_id": "b",
                    "passed": False,
                    "tool_calls": 20,
                    "input_tokens": 200,
                    "output_tokens": 100,
                    "failure_category": "editing_failure",
                },
            ]
        )

        self.assertEqual(report["tasks_total"], 2)
        self.assertEqual(report["tasks_passed"], 1)
        self.assertEqual(report["completion_rate"], 0.5)
        self.assertEqual(report["average_tool_calls"], 15.0)
        self.assertEqual(report["average_tokens"], 225.0)
        self.assertEqual(report["failure_categories"], {"editing_failure": 1})


if __name__ == "__main__":
    unittest.main()
