import io
import unittest
from json_io import read_jsonl

class TestJsonl(unittest.TestCase):
    def test_reads_non_blank_lines(self): self.assertEqual(list(read_jsonl(io.StringIO('{"a":1}\n\n{"b":2}\n'))), [{"a":1}, {"b":2}])
    def test_error_has_line_number(self):
        with self.assertRaisesRegex(ValueError, "line 2"): list(read_jsonl(io.StringIO('{"a":1}\nbad\n')))
