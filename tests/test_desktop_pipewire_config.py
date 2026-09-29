"""Safety and native-readback contracts; no audio daemon is started here."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from desktop_pipewire_isolated import private_environment, node_props, stream_nodes, native_effective_volume, linked_sink_name


class NativeFixtureTests(unittest.TestCase):
    def test_private_environment_discards_inherited_host_overrides(self):
        with tempfile.TemporaryDirectory() as directory:
            hostile = dict(os.environ, PIPEWIRE_REMOTE="host", PIPEWIRE_RUNTIME_DIR="/host",
                           PULSE_SERVER="unix:/host/pulse", PULSE_RUNTIME_PATH="/host/pulse",
                           PIPEWIRE_CONFIG_PREFIX="/host", PIPEWIRE_CONFIG_NAME="host.conf",
                           DBUS_SESSION_BUS_ADDRESS="unix:path=/host/bus")
            with patch.dict(os.environ, hostile, clear=True):
                env = private_environment(Path(directory))
            self.assertEqual(env["PIPEWIRE_RUNTIME_DIR"], directory)
            self.assertEqual(env["PIPEWIRE_REMOTE"], "pipewire-0")
            self.assertEqual(env["PULSE_SERVER"], f"unix:{directory}/pulse/native")
            self.assertEqual(env["DBUS_SESSION_BUS_ADDRESS"], "unix:path=/nonexistent")
            for key in ("HOME", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"):
                self.assertTrue(env[key].startswith(directory + "/"), key)
            self.assertNotIn("PIPEWIRE_CONFIG_PREFIX", env)
            self.assertNotIn("PIPEWIRE_CONFIG_NAME", env)

    def test_cubic_native_gain_and_link_readback_ignore_stale_node_target(self):
        self.assertAlmostEqual(native_effective_volume({"volume": 1.0, "channelVolumes": [0.125, 0.125]}), 0.5)
        self.assertAlmostEqual(native_effective_volume({"volume": 1.5, "channelVolumes": [0.5, 0.5]}), 0.75 ** (1/3))
        objects = [
            {"id": 23, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Stream/Output/Audio", "target.object": "test_a"}}},
            {"id": 31, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Sink", "node.name": "test_b"}}},
            {"id": 40, "type": "PipeWire:Interface:Link", "info": {"output-node-id": 23, "input-node-id": 31, "state": "active"}},
        ]
        self.assertEqual(linked_sink_name(objects, 23), "test_b")

    def test_stream_nodes_filter_media_class_and_read_native_props(self):
        dump = [
            {"id": 17, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Sink"}, "params": {"Props": [{"volume": 1.0}]}}},
            {"id": 23, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Stream/Output/Audio", "node.name": "Silence", "target.object": "test_a"}, "params": {"Props": [{"volume": 1.5, "mute": False, "channelVolumes": []}]}}},
        ]
        self.assertEqual([node["id"] for node in stream_nodes(dump)], [23])
        self.assertEqual(node_props(dump[1])["volume"], 1.5)
        self.assertEqual(node_props(dump[1])["mute"], False)
        with self.assertRaises(AssertionError):
            node_props({"info": {"params": {"Props": []}}})


if __name__ == "__main__":
    unittest.main()
