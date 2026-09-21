#!/usr/bin/env python3
"""Focused unit tests for the Part A audit's decision helpers."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("audit_part_a.py")
SPEC = importlib.util.spec_from_file_location("audit_part_a", SCRIPT)
assert SPEC and SPEC.loader
audit = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = audit
SPEC.loader.exec_module(audit)


class AuditHelpersTest(unittest.TestCase):
    def test_svg_is_not_a_required_manual_gate(self) -> None:
        ids = {
            item["id"]
            for profile in ("current", "windows")
            for item in audit.manual_items(profile)
        }
        self.assertFalse(any("svg" in check_id for check_id in ids))

    def test_windows_scaling_includes_fractional_dpi(self) -> None:
        scaling = next(
            item
            for item in audit.manual_items("windows")
            if item["id"] == "windows-scaling"
        )
        self.assertIn("125%", scaling["requirement"])
        self.assertIn("live scale change", scaling["requirement"])

    def test_automated_scope_is_explicit_in_report(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            code = audit.write_reports(
                output,
                [audit.Check("a", "test", "a", "pass", "ok")],
                {"scope": "automated"},
            )
            self.assertEqual(code, 0)
            summary = (output / "summary.json").read_text()
            report = (output / "problem-report.txt").read_text()
            self.assertIn('"scope": "automated"', summary)
            self.assertIn("Part A automated audit: PASS", report)

    def test_version_comparison_is_numeric(self) -> None:
        self.assertEqual(audit.version_tuple("1.98"), (1, 98, 0))
        self.assertGreater(audit.version_tuple("1.100.0"), (1, 98, 0))

    def test_diagnostic_excerpt_prefers_actionable_lines(self) -> None:
        output = "building\nerror: missing font\nnoise\nFAILED test_name\n"
        self.assertEqual(
            audit.diagnostic_excerpt(output),
            ["error: missing font", "FAILED test_name"],
        )

    def test_license_expression_accepts_permissive_or_path(self) -> None:
        allowed, unknown = audit.license_has_allowed_path(
            "MIT OR Apache-2.0 OR LGPL-2.1-or-later"
        )
        self.assertTrue(allowed)
        self.assertEqual(unknown, set())

    def test_license_expression_rejects_required_copyleft(self) -> None:
        allowed, unknown = audit.license_has_allowed_path("MIT AND GPL-2.0-only")
        self.assertFalse(allowed)
        self.assertEqual(unknown, set())

    def test_result_is_fail_before_incomplete(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            checks = [
                audit.Check("a", "test", "a", "manual_required", "pending"),
                audit.Check("b", "test", "b", "fail", "broken"),
            ]
            code = audit.write_reports(output, checks, {})
            self.assertEqual(code, 1)

    def test_result_is_incomplete_for_missing_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            checks = [audit.Check("a", "test", "a", "blocked", "missing tool")]
            code = audit.write_reports(output, checks, {})
            self.assertEqual(code, 2)

    def test_manual_evidence_preserves_blocked_result(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            evidence_path = Path(directory) / "manual.json"
            evidence_path.write_text(
                '{"reviewer":"Codex","observed_at_utc":"2026-09-21T00:00:00Z",'
                '"checks":[{"id":"macos-retina-preview","result":"blocked",'
                '"notes":"No Retina display attached.","artifacts":[]}]}'
            )
            checks = audit.check_manual_evidence(
                Path(directory), "current", evidence_path
            )
            retina = next(
                check for check in checks if check.check_id == "macos-retina-preview"
            )
            self.assertEqual(retina.status, "blocked")


if __name__ == "__main__":
    unittest.main()
