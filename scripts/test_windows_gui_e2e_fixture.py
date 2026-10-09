"""Fixture guard tests only; not real Windows GUI acceptance."""
import json
import os
import tempfile
import hashlib
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import windows_gui_e2e_fixture as FIXTURE
from scripts import test_windows_gui_qa as GUI_GUARDS


class SnapshotTlsGuardTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.snapshot = self.root / "snapshot"
        self.source = self.snapshot / "apps/instplot-studio/src/app_update.rs"
        self.source.parent.mkdir(parents=True)
        (self.snapshot / "packaging/update").mkdir(parents=True)
        (self.snapshot / ".studio-disposable-gui-fixture").touch()
        self.original = "// 更新夹具保持 UTF-8 和 LF\n" + "ureq::Agent::config_builder()\n        .max_redirects(0)\n" * 2
        self.source.write_bytes(self.original.encode())
        self.ca = self.root / "fixture-ca.pem"
        self.ca.write_bytes(b"-----BEGIN CERTIFICATE-----\nfixture\n-----END CERTIFICATE-----")
        self.environment = patch.dict(os.environ, {
            "GITHUB_ACTIONS": "true", "RUNNER_OS": "Windows", "RUNNER_TEMP": str(self.root),
        })
        self.environment.start()
        self.git_source = patch.object(FIXTURE.subprocess, "check_output", return_value=self.original.encode())
        self.git_source.start()

    def tearDown(self):
        self.environment.stop()
        self.git_source.stop()
        self.temporary.cleanup()

    def test_only_two_agents_in_marked_snapshot_use_strict_ca(self):
        proof = FIXTURE.patch_snapshot(self.snapshot, self.ca, "a" * 40)
        text = self.source.read_text(encoding="utf-8")
        self.assertEqual(text.count(".tls_config("), 2)
        self.assertEqual(text.count(".max_redirects(0)"), 2)
        self.assertNotIn("disable_verification", text)
        self.assertTrue(proof["strict_certificate_and_hostname_validation"])
        self.assertNotEqual(proof["before_sha256"], proof["after_sha256"])
        json.dumps(proof)

    def test_checkout_unmarked_snapshot_and_agent_drift_rejected(self):
        (self.snapshot / ".git").touch()
        with self.assertRaisesRegex(ValueError, "marked disposable"):
            FIXTURE.patch_snapshot(self.snapshot, self.ca, "a" * 40)
        (self.snapshot / ".git").unlink()
        (self.snapshot / ".studio-disposable-gui-fixture").unlink()
        with self.assertRaises(ValueError):
            FIXTURE.patch_snapshot(self.snapshot, self.ca, "a" * 40)
        (self.snapshot / ".studio-disposable-gui-fixture").touch()
        self.source.write_bytes((self.original + self.original).encode())
        with self.assertRaisesRegex(ValueError, "exact git archive"):
            FIXTURE.patch_snapshot(self.snapshot, self.ca, "a" * 40)

    def test_non_runner_and_secret_certificate_rejected_without_source_edit(self):
        with patch.dict(os.environ, {"GITHUB_ACTIONS": "false"}), self.assertRaises(ValueError):
            FIXTURE.patch_snapshot(self.snapshot, self.ca, "a" * 40)
        self.ca.write_bytes(b"-----BEGIN PRIVATE KEY-----")
        with self.assertRaisesRegex(ValueError, "public CA"):
            FIXTURE.patch_snapshot(self.snapshot, self.ca, "a" * 40)
        self.assertEqual(self.source.read_bytes(), self.original.encode("utf-8"))


class LoopbackSignedStageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        GUI_GUARDS.WindowsGuiQaTests.setUpClass()

    @classmethod
    def tearDownClass(cls):
        GUI_GUARDS.WindowsGuiQaTests.tearDownClass()

    def setUp(self):
        # The mock installer data proves signing/guards, never GUI/PE acceptance.
        self.fixture = GUI_GUARDS.WindowsGuiQaTests()
        self.fixture.setUp()
        self.sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=FIXTURE.QA.ROOT, text=True).strip()
        before = subprocess.check_output(["git", "show", f"{self.sha}:apps/instplot-studio/src/app_update.rs"], cwd=FIXTURE.QA.ROOT)
        trust = self.fixture.trust | {"public_root": FIXTURE.FIXTURE_ROOT}
        (self.fixture.kit / "fixture-trust.json").write_text(json.dumps(trust))
        self.proof = {"scope": FIXTURE.SCOPE, "source_sha": self.sha, "agent_count": 2,
                      "strict_certificate_and_hostname_validation": True,
                      "before_sha256": hashlib.sha256(before).hexdigest()}
        (self.fixture.kit / "gui-snapshot-patch.json").write_text(json.dumps(self.proof))

    def tearDown(self):
        self.fixture.tearDown()

    def test_signed_pair_bootstraps_baseline_with_exact_source(self):
        index = FIXTURE.stage(self.fixture.kit, self.fixture.output, self.fixture.key, self.sha)
        latest = json.loads((self.fixture.output / "instplot-studio/channels/prerelease/latest.json").read_bytes())
        self.assertEqual(index["source_sha"], self.sha)
        self.assertEqual(latest["version"], index["baseline"])
        self.assertEqual(latest["release_sequence"], 1)
        self.assertTrue((self.fixture.output / f"instplot-studio/releases/{index['candidate']}/metadata/2/manifest.json.sig").is_file())
        self.assertFalse(list(self.fixture.output.rglob("*.pem")))

    def test_wrong_source_evidence_and_non_runner_output_rejected(self):
        self.proof["before_sha256"] = "0" * 64
        (self.fixture.kit / "gui-snapshot-patch.json").write_text(json.dumps(self.proof))
        with self.assertRaisesRegex(ValueError, "build source"):
            FIXTURE.stage(self.fixture.kit, self.fixture.output, self.fixture.key, self.sha)
        with self.assertRaisesRegex(ValueError, "temporary directory"):
            FIXTURE.stage(self.fixture.kit, self.fixture.root.parent / "outside-gui-stage", self.fixture.key, self.sha)
        self.assertFalse(self.fixture.output.exists())


if __name__ == "__main__":
    unittest.main()
