#!/usr/bin/env python3
"""Tests for small release orchestration helpers."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "extract_changelog", ROOT / "scripts/extract_changelog.py"
)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
ASSET_SPEC = importlib.util.spec_from_file_location(
    "build_release_asset_spec", ROOT / "scripts/build_release_asset_spec.py"
)
assert ASSET_SPEC and ASSET_SPEC.loader
ASSET_MODULE = importlib.util.module_from_spec(ASSET_SPEC)
ASSET_SPEC.loader.exec_module(ASSET_MODULE)


class ExtractChangelogTests(unittest.TestCase):
    def test_extracts_only_requested_section(self) -> None:
        text = "# Changes\n\n## [1.2.0] - 2026-09-29\n\n- New\n\n## [1.1.0]\n\n- Old\n"
        self.assertEqual(MODULE.extract_section(text, "1.2.0"), "- New\n")

    def test_rejects_missing_and_empty_sections(self) -> None:
        with self.assertRaises(ValueError):
            MODULE.extract_section("## [1.0.0]\n", "1.0.0")
        with self.assertRaises(ValueError):
            MODULE.extract_section("## [1.0.0]\n- ok\n", "2.0.0")


class ReleaseAssetSpecTests(unittest.TestCase):
    def test_requires_and_classifies_the_exact_first_release_set(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            version = "0.1.2-rc.1"
            names = [
                f"InstPlot-Studio-{version}-linux-x86_64.deb",
                f"InstPlot-Studio-{version}-linux-x86_64.tar.gz",
                f"InstPlot-Studio-{version}-macos-aarch64.dmg",
                f"InstPlot-Studio-{version}-windows-x86_64-setup.exe",
            ]
            for name in names:
                (root / name).write_bytes(b"package")
            payload = ASSET_MODULE.build_spec(root, version)
            platforms = payload["platforms"]
            self.assertEqual(platforms["linux-x86_64"]["preferred"], "deb")
            self.assertEqual(platforms["macos-aarch64"]["preferred"], "dmg")
            self.assertEqual(platforms["windows-x86_64"]["preferred"], "inno-setup")
            (root / "unexpected.zip").write_bytes(b"bad")
            with self.assertRaises(ValueError):
                ASSET_MODULE.build_spec(root, version)


if __name__ == "__main__":
    unittest.main()
