#!/usr/bin/env python3
"""Tests for small release orchestration helpers."""

from __future__ import annotations

import importlib.util
import hashlib
import json
import os
import subprocess
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


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


class ReleaseDispatchTests(unittest.TestCase):
    @unittest.skipIf(os.name == "nt", "Dispatch job runs on Ubuntu with Bash")
    def test_oss_dispatch_identifies_repository_without_checkout(self) -> None:
        workflow = (ROOT / ".github/workflows/release.yml").read_text()
        command = workflow.split("          gh workflow run publish-oss-update.yml", 1)[1]
        command = "gh workflow run publish-oss-update.yml" + command.split(
            '          echo "OSS publication', 1
        )[0]
        command = command.replace("${{ inputs.release_sequence }}", "2").replace(
            "${{ inputs.expires_at }}", "2026-12-29T00:00:00Z"
        )
        # Execute the actual workflow command outside a checkout without network writes.
        with tempfile.TemporaryDirectory() as temporary:
            result = subprocess.run(
                ["bash", "-eu", "-c", 'gh() { printf "%s\\n" "$@"; }; ' + command],
                cwd=temporary,
                env={
                    **os.environ,
                    "GITHUB_REPOSITORY": "example/studio",
                    "RELEASE_TAG": "v0.1.2-rc.2",
                    "SOURCE_SHA": "frozen-source",
                },
                capture_output=True,
                text=True,
                check=True,
            )
        self.assertEqual(result.stdout.splitlines(), [
            "workflow", "run", "publish-oss-update.yml", "--repo", "example/studio",
            "--ref", "main", "-f", "release_tag=v0.1.2-rc.2", "-f",
            "source_sha=frozen-source", "-f", "release_sequence=2", "-f",
            "expires_at=2026-12-29T00:00:00Z",
        ])


class ReleaseAssetSpecTests(unittest.TestCase):
    def test_windows_contract_evidence_is_bound_to_exact_installer_and_version(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = root / "assets"
            assets.mkdir()
            version = "0.1.2-rc.3"
            for suffix in ("linux-x86_64.deb", "linux-x86_64.tar.gz", "macos-aarch64.dmg", "windows-x86_64-setup.exe"):
                (assets / f"InstPlot-Studio-{version}-{suffix}").write_bytes(b"fixture")
            installer_bytes = b"fixture" * 200000  # Cross the 1 MiB streaming boundary.
            (assets / f"InstPlot-Studio-{version}-windows-x86_64-setup.exe").write_bytes(installer_bytes)
            proof = root / "evidence.json"
            evidence = {
                "scope": "preview-components-not-accepted-updater", "version": version,
                "installer_sha256": hashlib.sha256(installer_bytes).hexdigest(),
                "contract": {
                    "schema": 1, "helper_protocol": 1, "transaction_schema": 1,
                    "candidate_health_protocol": 1, "recovery_health_protocol": 1,
                    "executable_sha256": "a" * 64, "license_sha256": "b" * 64,
                },
            }
            proof.write_text(json.dumps(evidence))
            self.assertNotIn("windows_in_place", ASSET_MODULE.build_spec(assets, version)["platforms"]["windows-x86_64"])
            with patch.object(ASSET_MODULE.hashlib, "file_digest", side_effect=AssertionError("Python 3.11-only API must not be called"), create=True):
                self.assertEqual(ASSET_MODULE.build_spec(assets, version, proof)["platforms"]["windows-x86_64"]["windows_in_place"], evidence["contract"])
            for field, value in (("installer_sha256", "0" * 64), ("version", "0.1.2-rc.2"), ("scope", "accepted"), ("contract", {"schema": 1})):
                proof.write_text(json.dumps({**evidence, field: value}))
                with self.subTest(field=field), self.assertRaises(ValueError):
                    ASSET_MODULE.build_spec(assets, version, proof)

    def test_windows_icon_contains_all_required_png_resolutions(self) -> None:
        data = (ROOT / "apps/instplot-studio/assets/InstPlotStudio.ico").read_bytes()
        self.assertEqual(struct.unpack_from("<HHH", data), (0, 1, 7))
        expected_offset = 6 + 16 * 7
        for index, size in enumerate((16, 24, 32, 48, 64, 128, 256)):
            width, height, colors, reserved, planes, depth, length, offset = struct.unpack_from(
                "<BBBBHHII", data, 6 + 16 * index
            )
            self.assertEqual((width or 256, height or 256), (size, size))
            self.assertEqual((colors, reserved, planes, depth), (0, 0, 1, 32))
            self.assertEqual(offset, expected_offset)
            image = data[offset:offset + length]
            self.assertEqual(len(image), length)
            self.assertEqual(image[:8], b"\x89PNG\r\n\x1a\n")
            self.assertEqual(struct.unpack_from(">II", image, 16), (size, size))
            expected_offset += length
        self.assertEqual(expected_offset, len(data))

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
