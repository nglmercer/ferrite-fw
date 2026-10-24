"""Regression checks for false-success and interrupted acceptance reports."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("acceptance", Path(__file__).with_name("acceptance.py"))
acceptance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acceptance)


class AcceptanceEvidenceTests(unittest.TestCase):
    def invoke(self, source, *, exact=True, timeout=3):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        report = Path(directory.name) / "report.json"
        command = [sys.executable, "-c", source] + (["--exact"] if exact else [])
        args = types.SimpleNamespace(profile="runtime", list=False, timeout=timeout, report=report)
        with patch.object(acceptance, "plan", return_value=[command]), patch.object(acceptance, "environment_evidence", return_value={"os": "test", "architecture": "test"}), contextlib.redirect_stdout(io.StringIO()):
            try:
                status = acceptance.run(args)
            except (RuntimeError, subprocess.TimeoutExpired) as error:
                status = error
        return status, json.loads(report.read_text())

    def test_successful_cargo_with_zero_exact_tests_is_a_failed_gate(self):
        result, report = self.invoke("print('test result: ok. 0 passed; 0 failed; 0 ignored;')")
        self.assertIsInstance(result, RuntimeError)
        self.assertEqual(report["status"], "failed")
        self.assertEqual(report["gates"][0]["exit_code"], 0)
        self.assertEqual(report["gates"][0]["status"], "failed")

    def test_executed_exact_test_records_a_pass(self):
        result, report = self.invoke("print('test result: ok. 1 passed; 0 failed; 0 ignored;')")
        self.assertEqual(result, 0)
        self.assertEqual(report["status"], "passed")
        self.assertIn("revision", report)
        self.assertIn("platform", report)
        self.assertEqual(report["environment"]["os"], "test")

    def test_nonzero_command_records_a_failure(self):
        result, report = self.invoke("raise SystemExit(7)", exact=False)
        self.assertIsInstance(result, RuntimeError)
        self.assertEqual(report["gates"][0]["exit_code"], 7)
        self.assertEqual(report["status"], "failed")

    def test_timed_out_command_records_interruption(self):
        result, report = self.invoke("import time; time.sleep(30)", exact=False, timeout=1)
        self.assertIsInstance(result, subprocess.TimeoutExpired)
        self.assertEqual(report["status"], "failed")
        self.assertEqual(report["gates"][0]["status"], "interrupted")


if __name__ == "__main__":
    unittest.main()
