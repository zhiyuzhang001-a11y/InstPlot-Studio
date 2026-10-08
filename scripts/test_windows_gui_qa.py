#!/usr/bin/env python3
"""QA publication guard tests; mock installers are NOT Windows GUI evidence."""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import patch

from scripts import windows_gui_qa as QA


class WindowsGuiQaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.keys_temporary = tempfile.TemporaryDirectory()
        cls.keys_root = Path(cls.keys_temporary.name)
        cls.key = cls.keys_root / "preview-fixture-key.pem"
        subprocess.run(["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(cls.key)], check=True)
        cls.public = subprocess.check_output(QA.PREPARE.signature_tool("public-key-hex", str(cls.key)), text=True).strip()
        second = cls.keys_root / "next.pem"
        subprocess.run(["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(second)], check=True)
        cls.next_public = subprocess.check_output(QA.PREPARE.signature_tool("public-key-hex", str(second)), text=True).strip()

    @classmethod
    def tearDownClass(cls):
        cls.keys_temporary.cleanup()

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.key = self.root / "preview-fixture-key.pem"
        shutil.copy2(type(self).key, self.key)
        self.kit = self.root / "kit"
        self.kit.mkdir()
        self.output = self.root / "staged"
        self.identity = "123-1"
        self.trust = {
            "public_root": QA.public_root(self.identity), "binaries_use_fixture_trust": True,
            "keys": [{"id": "windows-preview-fixture", "public_key_hex": self.public},
                     {"id": "windows-preview-fixture-next", "public_key_hex": self.next_public}],
        }
        (self.kit / "fixture-trust.json").write_text(json.dumps(self.trust))
        for version in ("0.1.2-rc.2", "0.1.2-rc.3"):
            installer = self.kit / f"InstPlot-Studio-{version}-windows-x86_64-setup.exe"
            installer.write_bytes(f"fake installer for guard tests only {version}".encode())
            proof = {"scope": "preview-components-not-accepted-updater", "version": version,
                     "windows_gui_subsystem": 2,
                     "installer_sha256": QA.PREPARE.sha256(installer),
                     "contract": {"schema": 1, "helper_protocol": 1, "transaction_schema": 1,
                                  "candidate_health_protocol": 1, "recovery_health_protocol": 1,
                                  "executable_sha256": "a" * 64, "license_sha256": "b" * 64}}
            (self.kit / f"{version}-windows-in-place.json").write_text(json.dumps(proof))
        self.environment = patch.dict(os.environ, {
            "GITHUB_ACTIONS": "true", "RUNNER_OS": "Windows", "RUNNER_TEMP": str(self.root),
            "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "1", "GITHUB_REF": "refs/heads/qa",
            "GITHUB_REPOSITORY": "zhiyuzhang001-a11y/InstPlot-Studio",
        })
        self.environment.start()

    def tearDown(self):
        self.environment.stop()
        self.temporary.cleanup()

    def stage(self):
        return QA.stage(self.kit, self.output, self.identity, "a" * 40, self.key)

    def test_production_and_malformed_roots_are_rejected(self):
        for value in ("", "1", "123/../channels/stable", "123-0", "0-1", "123-1/x", "123-1?query"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                QA.public_root(value)

    def test_stage_preserves_both_versions_but_bootstraps_old_latest(self):
        index = self.stage()
        self.assertEqual(index["baseline"], "0.1.2-rc.2")
        self.assertEqual(index["candidate"], "0.1.2-rc.3")
        tree = self.output / "public"
        self.assertEqual((tree / "channels/prerelease/latest.json").read_bytes(),
                         (tree / "releases/0.1.2-rc.2/metadata/1/manifest.json").read_bytes())
        self.assertTrue((tree / "releases/0.1.2-rc.3/metadata/2/manifest.json").is_file())
        self.assertFalse(list(tree.rglob("*.pem")))
        with self.assertRaisesRegex(ValueError, "overwrite"):
            self.stage()

    def test_snapshot_trust_mismatch_stops_before_staging(self):
        self.trust["public_root"] = "https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio"
        (self.kit / "fixture-trust.json").write_text(json.dumps(self.trust))
        with self.assertRaisesRegex(ValueError, "snapshot trust"):
            self.stage()
        self.assertFalse(self.output.exists())

    def test_real_client_rejects_correct_signature_with_commit_notes_url(self):
        self.stage()
        manifest = self.output / "public/releases/0.1.2-rc.2/metadata/1/manifest.json"
        signature = manifest.with_name("manifest.json.sig")
        payload = json.loads(manifest.read_bytes())
        payload["notes_url"] = "https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/commit/" + "a" * 40
        manifest.write_bytes(QA.PREPARE.deterministic_json(payload))
        subprocess.run(QA.PREPARE.signature_tool("sign", str(self.key), str(manifest), str(signature)), check=True)
        # Raw cryptography passes, but the exact GUI validator MUST reject it.
        subprocess.run(QA.PREPARE.signature_tool("verify", self.public, str(manifest), str(signature)), check=True)
        result = subprocess.run(QA.PREPARE.signature_tool(
            "verify-manifest", "windows-preview-fixture", self.public,
            QA.public_root(self.identity), str(manifest), str(signature), "0.1.2-rc.2",
        ), capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("outside the allowed root", result.stderr)

    def test_console_build_evidence_is_rejected(self):
        proof_path = self.kit / "0.1.2-rc.2-windows-in-place.json"
        proof = json.loads(proof_path.read_bytes())
        proof["windows_gui_subsystem"] = 3
        proof_path.write_text(json.dumps(proof))
        with self.assertRaisesRegex(ValueError, "GUI-subsystem"):
            self.stage()
        self.assertFalse(self.output.exists())

    def test_production_key_path_and_wrong_run_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "ephemeral"):
            QA.stage(self.kit, self.output, self.identity, "a" * 40, self.root / "production.pem")
        with self.assertRaisesRegex(ValueError, "differs from this build"):
            QA.stage(self.kit, self.output, "124-1", "a" * 40, self.key)

    def test_exact_installer_evidence_is_required_before_staging(self):
        (self.kit / "InstPlot-Studio-0.1.2-rc.3-windows-x86_64-setup.exe").write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "exact-file"):
            self.stage()
        self.assertFalse(self.output.exists())

    def test_extra_public_file_is_rejected_even_if_inventory_is_updated(self):
        self.stage()
        path = self.output / "public" / "private.pem"
        path.write_bytes(b"must never upload")
        inventory = json.loads((self.output / "inventory.json").read_bytes())
        inventory["private.pem"] = {"sha256": QA.PREPARE.sha256(path), "size": path.stat().st_size}
        (self.output / "inventory.json").write_text(json.dumps(inventory))
        with self.assertRaisesRegex(ValueError, "unexpected or missing"):
            QA.validate(self.output, self.identity)

    def test_unsigned_manifest_and_premature_candidate_pointer_rejected(self):
        self.stage()
        tree = self.output / "public"
        latest = tree / "channels/prerelease/latest.json"
        candidate = tree / "releases/0.1.2-rc.3/metadata/2/manifest.json"
        shutil.copyfile(candidate, latest)
        inventory = json.loads((self.output / "inventory.json").read_bytes())
        inventory["channels/prerelease/latest.json"] = {"sha256": QA.PREPARE.sha256(latest), "size": latest.stat().st_size}
        (self.output / "inventory.json").write_text(json.dumps(inventory))
        with self.assertRaisesRegex(ValueError, "bootstrap latest"):
            QA.validate(self.output, self.identity)
        candidate.write_bytes(candidate.read_bytes().replace(b'"schema":1', b'"schema":2', 1))
        with self.assertRaisesRegex(ValueError, "manifest verified with 0"):
            QA.validate(self.output, self.identity)

    def test_non_main_upload_never_invokes_ossutil(self):
        with patch.object(QA, "validate", return_value={}), patch.object(QA.subprocess, "run") as execute:
            with self.assertRaisesRegex(ValueError, "reviewed main"):
                QA.publish(self.output, self.identity, "ossutil", False)
            execute.assert_not_called()

    def test_initial_upload_is_confined_and_latest_is_last(self):
        self.stage()
        missing = urllib.error.HTTPError("https://example.test", 404, "missing", {}, None)
        with patch.dict(os.environ, {"GITHUB_REF": "refs/heads/main"}), patch.object(QA, "verify_public") as verify, patch.object(QA.VERIFY, "download", side_effect=missing), patch.object(QA.subprocess, "run") as execute, patch.object(QA, "validate", return_value=json.loads((self.output / "public/qa-index.json").read_bytes())):
            QA.publish(self.output, self.identity, "ossutil", False)
            commands = [call.args[0] for call in execute.call_args_list]
            self.assertEqual(len(commands), 8)
            for command in commands:
                key = command[command.index("--key") + 1]
                self.assertTrue(key.startswith("instplot-studio/windows-gui-qa/123-1/"))
                self.assertEqual(command[command.index("--forbid-overwrite") + 1], "true")
            self.assertEqual(commands[-1][commands[-1].index("--key") + 1], "instplot-studio/windows-gui-qa/123-1/channels/prerelease/latest.json")
            self.assertEqual(verify.call_count, 3)

    def test_partial_upload_never_overwrites_an_existing_object(self):
        self.stage()
        index = json.loads((self.output / "public/qa-index.json").read_bytes())
        path = self.output / "public/qa-index.json"

        def existing(url, destination, **kwargs):
            shutil.copyfile(path, destination)

        with patch.object(QA.VERIFY, "download", side_effect=existing), patch.object(QA.subprocess, "run") as execute:
            QA.put_if_missing("qa-index.json", path, index, self.output, execute)
            execute.assert_not_called()

        def wrong(url, destination, **kwargs):
            destination.write_bytes(b"foreign object")

        with patch.object(QA.VERIFY, "download", side_effect=wrong), patch.object(QA.subprocess, "run") as execute:
            with self.assertRaisesRegex(ValueError, "refuse overwrite"):
                QA.put_if_missing("other.json", path, index, self.output, execute)
            execute.assert_not_called()

    def test_activation_replaces_only_qa_latest_and_is_idempotent(self):
        self.stage()
        index = json.loads((self.output / "public/qa-index.json").read_bytes())

        def observed_old(url, destination, **kwargs):
            shutil.copyfile(self.output / "public/channels/prerelease/latest.json", destination)

        with patch.dict(os.environ, {"GITHUB_REF": "refs/heads/main"}), patch.object(QA, "validate", return_value=index), patch.object(QA, "verify_public"), patch.object(QA.VERIFY, "download", side_effect=observed_old), patch.object(QA.subprocess, "run") as execute:
            QA.publish(self.output, self.identity, "ossutil", True)
            command = execute.call_args.args[0]
            self.assertEqual(execute.call_count, 1)
            self.assertEqual(command[command.index("--key") + 1], "instplot-studio/windows-gui-qa/123-1/channels/prerelease/latest.json")
            self.assertEqual(command[command.index("--forbid-overwrite") + 1], "false")

        def observed_new(url, destination, **kwargs):
            shutil.copyfile(self.output / "public/releases/0.1.2-rc.3/metadata/2/manifest.json", destination)

        with tempfile.TemporaryDirectory() as fresh:
            extracted = Path(fresh) / "qa"
            shutil.copytree(self.output / "public", extracted / "public")
            with patch.dict(os.environ, {"GITHUB_REF": "refs/heads/main"}), patch.object(QA, "validate", return_value=index), patch.object(QA, "verify_public"), patch.object(QA.VERIFY, "download", side_effect=observed_new), patch.object(QA.subprocess, "run") as execute:
                QA.publish(extracted, self.identity, "ossutil", True)
                execute.assert_not_called()


if __name__ == "__main__":
    unittest.main()
