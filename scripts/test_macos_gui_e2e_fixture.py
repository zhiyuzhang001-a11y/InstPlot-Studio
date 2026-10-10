import unittest

from scripts.macos_gui_e2e_fixture import replace_exact


class SnapshotGuardTests(unittest.TestCase):
    def test_two_agents_and_utf8_are_preserved(self):
        source = "中文\nagent\nagent\n"
        self.assertEqual(replace_exact(source, "agent", "strict CA", 2),
                         "中文\nstrict CA\nstrict CA\n")

    def test_missing_or_duplicate_anchor_is_rejected(self):
        for source in ("", "agent agent"):
            with self.assertRaises(ValueError):
                replace_exact(source, "agent", "strict CA")


if __name__ == "__main__":
    unittest.main()
