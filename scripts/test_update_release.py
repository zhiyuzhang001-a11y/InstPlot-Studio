#!/usr/bin/env python3
"""Contract tests for signed multi-platform OSS update metadata."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "packaging" / "update" / "prepare_oss_release.py"
SPEC = importlib.util.spec_from_file_location("prepare_oss_release", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
VERIFY_PATH = ROOT / "scripts" / "verify_public_release.py"
VERIFY_SPEC = importlib.util.spec_from_file_location("verify_public_release", VERIFY_PATH)
assert VERIFY_SPEC is not None and VERIFY_SPEC.loader is not None
VERIFY = importlib.util.module_from_spec(VERIFY_SPEC)
VERIFY_SPEC.loader.exec_module(VERIFY)


class UpdateReleaseTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.private_key = self.root / "private.pem"
        self.public_der = self.root / "public.der"
        subprocess.run(
            ["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(self.private_key)],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        subprocess.run(
            [
                "openssl",
                "pkey",
                "-in",
                str(self.private_key),
                "-pubout",
                "-outform",
                "DER",
                "-out",
                str(self.public_der),
            ],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        self.public_key_hex = self.public_der.read_bytes()[-32:].hex()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def prepare(self) -> tuple[Path, Path, Path]:
        package = self.root / "InstPlot-Studio-0.1.2-rc.1-linux-x86_64.deb"
        package.write_bytes(b"verified installer bytes")
        assets = self.root / "assets.json"
        assets.write_text(
            json.dumps(
                {
                    "platforms": {
                        "linux-x86_64": {
                            "preferred": "deb",
                            "packages": [
                                {
                                    "id": "deb",
                                    "package_type": "deb",
                                    "path": str(package),
                                    "minimum_system": "Ubuntu 22.04",
                                }
                            ],
                        }
                    }
                }
            ),
            encoding="utf-8",
        )
        return MODULE.prepare(
            asset_spec_path=assets,
            version="0.1.2-rc.1",
            release_sequence=1,
            product="instplot-studio",
            public_root="https://downloads.example.test/instplot-studio",
            private_key=self.private_key,
            public_key_hex=self.public_key_hex,
            key_id="test-key-1",
            notes_url="https://github.com/example/instplot/releases/tag/v0.1.2-rc.1",
            published_at="2026-09-29T00:00:00Z",
            expires_at="2026-12-29T00:00:00Z",
            output=self.root / "output",
        )

    def test_manifest_and_channel_pointer_are_identical_and_signed(self) -> None:
        manifest, signature, latest = self.prepare()
        self.assertEqual(manifest.read_bytes(), latest.read_bytes())
        self.assertEqual(signature.stat().st_size, 64)
        payload = json.loads(manifest.read_text(encoding="utf-8"))
        self.assertEqual(payload["channel"], "prerelease")
        self.assertEqual(payload["release_sequence"], 1)
        self.assertEqual(payload["platforms"]["linux-x86_64"]["preferred"], "deb")
        result = subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-verify",
                "-pubin",
                "-rawin",
                "-inkey",
                str(self.public_der),
                "-keyform",
                "DER",
                "-in",
                str(manifest),
                "-sigfile",
                str(signature),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())

    def test_preferred_package_must_exist(self) -> None:
        package = self.root / "InstPlot-Studio-0.1.2-linux-x86_64.deb"
        package.write_bytes(b"data")
        assets = self.root / "bad-assets.json"
        assets.write_text(
            json.dumps(
                {
                    "platforms": {
                        "linux-x86_64": {
                            "preferred": "tar",
                            "packages": [
                                {"id": "deb", "package_type": "deb", "path": str(package)}
                            ],
                        }
                    }
                }
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "preferred package does not exist"):
            MODULE.prepare(
                asset_spec_path=assets,
                version="0.1.2",
                release_sequence=2,
                product="instplot-studio",
                public_root="https://downloads.example.test/instplot-studio",
                private_key=self.private_key,
                public_key_hex=self.public_key_hex,
                key_id="test-key-1",
                notes_url="https://github.com/example/instplot/releases/tag/v0.1.2",
                published_at="2026-09-29T00:00:00Z",
                expires_at="2026-12-29T00:00:00Z",
                output=self.root / "bad-output",
            )

    def test_stable_and_prerelease_channels_follow_semver(self) -> None:
        self.assertEqual(MODULE.channel_for("0.1.2"), "stable")
        self.assertEqual(MODULE.channel_for("0.1.2-rc.1"), "prerelease")
        with self.assertRaisesRegex(ValueError, "invalid SemVer"):
            MODULE.channel_for("v0.1.2")

    def test_private_key_must_match_public_key(self) -> None:
        with self.assertRaisesRegex(ValueError, "does not match"):
            MODULE.verify_private_key(self.private_key, "00" * 32)

    def test_url_prefix_validation_uses_origin_and_path_boundaries(self) -> None:
        allowed = VERIFY.AllowedPrefix("https://downloads.example.test/instplot-studio")
        allowed.validate(
            "https://downloads.example.test/instplot-studio/releases/0.1.2/file.exe"
        )
        for invalid in (
            "http://downloads.example.test/instplot-studio/file.exe",
            "https://downloads.example.test.evil/instplot-studio/file.exe",
            "https://downloads.example.test/instplot-studio-evil/file.exe",
            "https://user@downloads.example.test/instplot-studio/file.exe",
            "https://downloads.example.test/instplot-studio/%2e%2e/private",
            "https://downloads.example.test/instplot-studio/file.exe#fragment",
        ):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                allowed.validate(invalid)

    def test_signature_key_id_mismatch_is_detectable(self) -> None:
        manifest, signature, _ = self.prepare()
        verified = VERIFY.verify_signature(
            manifest,
            signature,
            {"actual-key": self.public_key_hex, "unused-key": "11" * 32},
            self.root,
        )
        payload = VERIFY.parse_json(manifest.read_bytes())
        self.assertEqual(verified, "actual-key")
        self.assertNotEqual(payload["key_id"], verified)

    def test_duplicate_json_keys_are_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
            VERIFY.parse_json(b'{"schema":1,"schema":1}')

    def test_python_and_rust_share_one_signed_raw_byte_fixture(self) -> None:
        manifest = ROOT / "apps/instplot-studio/tests/fixtures/update-manifest.json"
        signature = self.root / "fixture.sig"
        signature.write_bytes(
            bytes.fromhex(
                (ROOT / "apps/instplot-studio/tests/fixtures/update-manifest.sig.hex")
                .read_text(encoding="ascii")
                .strip()
            )
        )
        key_id = VERIFY.verify_signature(
            manifest,
            signature,
            {
                "fixture-current": "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c"
            },
            self.root,
        )
        self.assertEqual(key_id, "fixture-current")
        self.assertEqual(VERIFY.parse_json(manifest.read_bytes())["version"], "0.1.2-rc.1")


if __name__ == "__main__":
    unittest.main()
