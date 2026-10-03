#!/usr/bin/env python3
"""Contract tests for signed multi-platform OSS update metadata."""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path
from unittest.mock import patch


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
    @staticmethod
    def windows_contract() -> dict:
        return {
            "schema": 1, "helper_protocol": 1, "transaction_schema": 1,
            "candidate_health_protocol": 1, "recovery_health_protocol": 1,
            "executable_sha256": "a" * 64, "license_sha256": "b" * 64,
        }

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.private_key = self.root / "private.pem"
        subprocess.run(
            ["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(self.private_key)],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        self.public_key_hex = subprocess.check_output(
            MODULE.signature_tool("public-key-hex", str(self.private_key)), text=True
        ).strip()

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
            MODULE.signature_tool(
                "verify", self.public_key_hex, str(manifest), str(signature)
            ),
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

    def test_unsupported_install_contract_rejected_before_keys_or_staging(self) -> None:
        assets = self.root / "unsupported-assets.json"
        staged = self.root / "output" / "instplot-studio"
        staged.mkdir(parents=True)
        sentinel = staged / "keep.txt"
        sentinel.write_bytes(b"existing staging must survive")
        for platform in ("windows-x86_64", "linux-x86_64"):
            for declaration in (None, {}, {"schema": 1}):
                with self.subTest(platform=platform, declaration=declaration):
                    assets.write_text(json.dumps({"platforms": {platform: {
                        "windows_in_place": declaration,
                    }}}), encoding="utf-8")
                    with patch.object(MODULE, "verify_private_key") as verify:
                        with self.assertRaisesRegex(ValueError, "publication is not enabled"):
                            MODULE.prepare(
                                asset_spec_path=assets,
                                version="0.1.2-rc.1",
                                release_sequence=1,
                                product="instplot-studio",
                                public_root="https://downloads.example.test/instplot-studio",
                                private_key=self.root / "must-not-access.pem",
                                public_key_hex=self.public_key_hex,
                                key_id="test-key-1",
                                notes_url="https://example.test/notes",
                                published_at="2026-09-29T00:00:00Z",
                                expires_at="2026-12-29T00:00:00Z",
                                output=self.root / "output",
                            )
                        verify.assert_not_called()
                    self.assertEqual(sentinel.read_bytes(), b"existing staging must survive")

    def test_stable_and_prerelease_channels_follow_semver(self) -> None:
        self.assertEqual(MODULE.channel_for("0.1.2"), "stable")
        self.assertEqual(MODULE.channel_for("0.1.2-rc.1"), "prerelease")
        with self.assertRaisesRegex(ValueError, "invalid SemVer"):
            MODULE.channel_for("v0.1.2")

    def test_windows_contract_validation_rejects_unknown_protocols_and_bad_hashes(self) -> None:
        valid = self.windows_contract()
        self.assertEqual(MODULE.validate_windows_contract(valid), valid)
        mutations = [None, {}, {**valid, "extra": 1}]
        for field in ("schema", "helper_protocol", "transaction_schema", "candidate_health_protocol", "recovery_health_protocol"):
            mutations.extend({**valid, field: value} for value in (True, 0, 2, "1", None))
        for field in ("executable_sha256", "license_sha256"):
            mutations.extend({**valid, field: value} for value in ("A" * 64, "x" * 64, "a" * 63, None))
        for value in mutations:
            with self.subTest(value=value), self.assertRaises(ValueError):
                MODULE.validate_windows_contract(value)

    @unittest.skipUnless(os.name == "nt" and os.environ.get("INSTPLOT_WINDOWS_PREVIEW_KIT"), "Requires disposable Windows CI preview installers")
    def test_real_preview_installers_have_verified_signed_fixture_metadata(self) -> None:
        # Only the CI script's ephemeral snapshot key, never production keys.
        # Both snapshot binaries embed this isolated current/next trust pair.
        kit = Path(os.environ["INSTPLOT_WINDOWS_PREVIEW_KIT"]).resolve()
        self.assertEqual(os.environ.get("GITHUB_ACTIONS"), "true")
        fixture_key = Path(os.environ["INSTPLOT_WINDOWS_PREVIEW_FIXTURE_KEY"]).resolve()
        self.assertEqual(fixture_key.name, "preview-fixture-key.pem")
        self.assertTrue(fixture_key.is_relative_to(Path(os.environ["RUNNER_TEMP"]).resolve()))
        trust = json.loads((kit / "fixture-trust.json").read_bytes())
        self.assertTrue(trust["binaries_use_fixture_trust"])
        self.assertEqual(len(trust["keys"]), 2)
        public_key = next(key["public_key_hex"] for key in trust["keys"] if key["id"] == "windows-preview-fixture")
        MODULE.verify_private_key(fixture_key, public_key)
        proofs = sorted(kit.glob("*-windows-in-place.json"))
        self.assertEqual(len(proofs), 2)
        now = datetime.now(timezone.utc).replace(microsecond=0)
        for proof in proofs:
            evidence = json.loads(proof.read_bytes())
            self.assertEqual(evidence["scope"], "preview-components-not-accepted-updater")
            version = evidence["version"]
            self.assertRegex(version, r"^\d+\.\d+\.\d+-rc\.\d+$")
            installer = kit / f"InstPlot-Studio-{version}-windows-x86_64-setup.exe"
            self.assertEqual(MODULE.sha256(installer), evidence["installer_sha256"])
            contract = MODULE.validate_windows_contract(evidence["contract"])
            assets = self.root / f"{version}-assets.json"
            assets.write_text(json.dumps({"platforms": {"windows-x86_64": {
                "preferred": "inno-setup", "windows_in_place": contract,
                "packages": [{"id": "inno-setup", "package_type": "exe-installer", "path": str(installer)}],
            }}}))
            output = kit / "signed-fixtures" / version
            self.assertFalse(output.exists(), "Do not overwrite previous fixture evidence")
            manifest, signature, latest = MODULE.prepare(
                asset_spec_path=assets, version=version, release_sequence=1,
                product="instplot-studio", public_root="https://windows-update.example.test/instplot-studio",
                private_key=fixture_key, public_key_hex=public_key, key_id="windows-preview-fixture",
                notes_url="https://example.test/preview-fixture", published_at=now.isoformat().replace("+00:00", "Z"),
                expires_at=(now + timedelta(days=90)).isoformat().replace("+00:00", "Z"), output=output, allow_windows_in_place=True,
            )
            self.assertEqual(latest.read_bytes(), manifest.read_bytes())
            self.assertEqual(json.loads(manifest.read_bytes())["platforms"]["windows-x86_64"]["windows_in_place"], contract)
            subprocess.run(MODULE.signature_tool("verify", public_key, str(manifest), str(signature)), check=True)

    def test_opt_in_windows_contract_is_preserved_in_the_signed_manifest(self) -> None:
        package = self.root / "InstPlot-Studio-0.1.2-rc.1-windows-x86_64-setup.exe"
        package.write_bytes(b"isolated installer fixture, not a runnable GUI package")
        platform = {
            "preferred": "inno-setup",
            "packages": [{"id": "inno-setup", "package_type": "exe-installer", "path": str(package)}],
            "windows_in_place": self.windows_contract(),
        }
        assets = self.root / "windows-assets.json"
        for wrong_platform in ("linux-x86_64", "windows-aarch64"):
            assets.write_text(json.dumps({"platforms": {wrong_platform: platform}}))
            with self.assertRaisesRegex(ValueError, "requires windows-x86_64"):
                MODULE.load_asset_spec(assets, allow_windows_in_place=True)
        assets.write_text(json.dumps({"platforms": {"windows-x86_64": platform}}))
        manifest, signature, latest = MODULE.prepare(
            asset_spec_path=assets, version="0.1.2-rc.1", release_sequence=1,
            product="instplot-studio", public_root="https://downloads.example.test/instplot-studio",
            private_key=self.private_key, public_key_hex=self.public_key_hex, key_id="test-key-1",
            notes_url="https://example.test/notes", published_at="2026-09-29T00:00:00Z",
            expires_at="2026-12-29T00:00:00Z", output=self.root / "windows-output",
            allow_windows_in_place=True,
        )
        self.assertEqual(json.loads(manifest.read_bytes())["platforms"]["windows-x86_64"]["windows_in_place"], self.windows_contract())
        self.assertEqual(latest.read_bytes(), manifest.read_bytes())
        subprocess.run(MODULE.signature_tool("verify", self.public_key_hex, str(manifest), str(signature)), check=True)
        changed = self.root / "tampered.json"
        payload = json.loads(manifest.read_bytes())
        payload["platforms"]["windows-x86_64"]["windows_in_place"]["executable_sha256"] = "c" * 64
        changed.write_bytes(MODULE.deterministic_json(payload))
        result = subprocess.run(MODULE.signature_tool("verify", self.public_key_hex, str(changed), str(signature)), capture_output=True)
        self.assertNotEqual(result.returncode, 0)

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
