"""Local installer safety tests; never builds or modifies an installed app."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("installer", Path(__file__).with_name("install_studio_macos.py"))
INSTALLER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INSTALLER)

class BuildTests(unittest.TestCase):
    def test_preview_is_explicit_and_default_stays_closed(self):
        self.assertNotIn("--features", INSTALLER.build_command(False))
        command = INSTALLER.build_command(True)
        self.assertEqual(command[-2:], ["--features", "in-place-update-preview"])
        self.assertIn("--locked", command)

@unittest.skipUnless(hasattr(os, "getuid"), "POSIX recovery permissions")
class RecoveryTests(unittest.TestCase):
    def test_private_recovery_is_reused_without_cleaning_contents(self):
        with tempfile.TemporaryDirectory() as temporary:
            applications = Path(temporary)
            root = INSTALLER.recovery_root(applications)
            sentinel = root / "retained-backup"
            sentinel.write_bytes(b"keep")
            self.assertEqual(INSTALLER.recovery_root(applications), root)
            self.assertEqual(sentinel.read_bytes(), b"keep")
            self.assertEqual(root.stat().st_mode & 0o077, 0)
            self.assertTrue((root / ".metadata_never_index").is_file())

    def test_redirect_or_foreign_permissions_are_not_repaired(self):
        with tempfile.TemporaryDirectory() as temporary:
            applications = Path(temporary)
            root = applications / ".instplot-studio-recovery"
            foreign = applications / "foreign"
            foreign.mkdir()
            root.symlink_to(foreign)
            with self.assertRaises(SystemExit):
                INSTALLER.recovery_root(applications)
            root.unlink()  # Disposable fixture link only.
            root.mkdir(mode=0o755)
            root.chmod(0o755)
            with self.assertRaises(SystemExit):
                INSTALLER.recovery_root(applications)
            self.assertEqual(root.stat().st_mode & 0o777, 0o755)

if __name__ == "__main__":
    unittest.main()
