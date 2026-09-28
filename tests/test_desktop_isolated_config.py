"""The private audio test must never start a hardware-monitoring session manager."""
import tempfile
import unittest
from pathlib import Path

from desktop_isolated import wireplumber_policy_command


class PolicySelectionTests(unittest.TestCase):
    def test_modern_policy_inherits_base_without_hardware(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / "wireplumber.conf").write_text(
                "policy = { inherits = [ base ] policy.standard = required }\n"
            )
            self.assertEqual(wireplumber_policy_command(path), ["wireplumber", "--profile=policy"])

    def test_ubuntu_2404_legacy_policy_has_no_hardware_monitors(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / "policy.conf").write_text(
                "context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert "
                "support.* = support/libspa-support }\n"
                "wireplumber.components = [ { name = libwireplumber-module-lua-scripting, type = module } "
                "{ name = policy.lua, type = config/lua } ]\n"
            )
            self.assertEqual(
                wireplumber_policy_command(path), ["wireplumber", "-c", str(path / "policy.conf")]
            )

    def test_legacy_policy_rejects_unexpected_component(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / "policy.conf").write_text(
                "wireplumber.components = [ { name = libwireplumber-module-lua-scripting, type = module } "
                "{ name = policy.lua, type = config/lua } "
                "{ name = unexpected-device-plugin, type = module } ]\n"
            )
            with self.assertRaises(AssertionError):
                wireplumber_policy_command(path)

    def test_legacy_policy_rejects_monitor_or_main_component(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)
            (path / "policy.conf").write_text(
                "api.alsa.* = alsa/libspa-alsa\n"
                "wireplumber.components = [ { name = policy.lua, type = config/lua } "
                "{ name = main.lua, type = config/lua } ]\n"
            )
            with self.assertRaises(AssertionError):
                wireplumber_policy_command(path)


if __name__ == "__main__":
    unittest.main()
