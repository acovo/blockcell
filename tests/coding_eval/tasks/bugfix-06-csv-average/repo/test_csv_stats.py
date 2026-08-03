import unittest
from csv_stats import average_csv

class TestCsvAverage(unittest.TestCase):
    def test_header_blank_and_invalid_rows_are_ignored(self):
        self.assertEqual(average_csv("value\n10\n\nbad\n20\n"), 15)
    def test_no_numbers_returns_none(self): self.assertIsNone(average_csv("value\n"))
