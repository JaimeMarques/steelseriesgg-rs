#!/usr/bin/env python3
"""Native PipeWire default E2E, exclusively against owned, policy-only daemons.

Requires a staged target/debug/ssgg-desktop with native PipeWire support.
The Pulse protocol is used ONLY by pacat/pactl to create and identify the fixture.
All service mutations and assertions use native PipeWire nodes/metadata.
"""
import json
import math
import os
from pathlib import Path
import select
import subprocess
import tempfile
import time

from desktop_isolated import PULSE, private_core_config, wireplumber_policy_command


def private_environment(root: Path) -> dict[str, str]:
    env = {k: v for k, v in os.environ.items() if not (
        k.startswith(("PIPEWIRE_", "PULSE_", "WP_", "WIREPLUMBER_", "DBUS_"))
        or k in ("HOME", "XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME")
    )}
    runtime = str(root)
    env.update(HOME=str(root / "home"), XDG_RUNTIME_DIR=runtime,
               XDG_CONFIG_HOME=str(root / "config"), XDG_STATE_HOME=str(root / "state"),
               XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_HOME=str(root / "data"),
               PIPEWIRE_RUNTIME_DIR=runtime, PIPEWIRE_REMOTE="pipewire-0",
               PIPEWIRE_CONFIG_DIR=runtime, PULSE_RUNTIME_PATH=str(root / "pulse"),
               PULSE_SERVER=f"unix:{root}/pulse/native",
               DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent",
               SSGG_PACTL="/nonexistent/ssgg-native-must-not-call-pactl")
    return env


def stream_nodes(objects: list[dict]) -> list[dict]:
    return [obj for obj in objects if obj.get("type") == "PipeWire:Interface:Node"
            and obj.get("info", {}).get("props", {}).get("media.class") == "Stream/Output/Audio"]


def node_props(node: dict) -> dict:
    params = node.get("info", {}).get("params", {}).get("Props", [])
    assert len(params) == 1 and isinstance(params[0], dict), f"Native Props missing: {node}"
    assert "volume" in params[0] and "mute" in params[0], f"Native gain/mute missing: {node}"
    return params[0]

def native_effective_volume(props: dict) -> float:
    channels = props.get("channelVolumes")
    scalar = props.get("volume")
    assert isinstance(channels, list) and channels and isinstance(scalar, (int, float)), props
    assert math.isfinite(float(scalar)) and scalar >= 0 and all(
        isinstance(v, (int, float)) and math.isfinite(float(v)) and v >= 0 for v in channels
    ), props
    return (float(scalar) * sum(float(v) for v in channels) / len(channels)) ** (1 / 3)

def linked_sink_name(objects: list[dict], stream_id: int) -> str:
    linked_ids = {obj.get("info", {}).get("input-node-id") for obj in objects
                  if obj.get("type") == "PipeWire:Interface:Link"
                  and obj.get("info", {}).get("output-node-id") == stream_id
                  and obj.get("info", {}).get("state", "").lower() in ("active", "paused")}
    assert len(linked_ids) == 1, f"Stream {stream_id} has no unique active sink link: {linked_ids}"
    target = next(iter(linked_ids))
    sinks = [obj.get("info", {}).get("props", {}) for obj in objects
             if obj.get("type") == "PipeWire:Interface:Node" and obj.get("id") == target
             and obj.get("info", {}).get("props", {}).get("media.class") == "Audio/Sink"]
    assert len(sinks) == 1 and sinks[0].get("node.name"), f"Stream {stream_id} linked to missing sink"
    return sinks[0]["node.name"]


def run():
    # Fail before spawning anything if session policy is not audited.
    policy = wireplumber_policy_command(Path("/usr/share/wireplumber"))
    fixture_only = os.environ.get("SSGG_NATIVE_FIXTURE_ONLY") == "1"
    binary = Path(os.environ.get("SSGG_DESKTOP_BIN", "target/debug/ssgg-desktop")).resolve()
    if not fixture_only:
        assert binary.is_file() and os.access(binary, os.X_OK), f"Build the native service first: {binary}"
    processes: list[subprocess.Popen] = []
    with tempfile.TemporaryDirectory(prefix="ssgg-native-isolated-") as tmp:
        root = Path(tmp)
        root.chmod(0o700)
        env = private_environment(root)
        for name in ("home", "config", "state", "cache", "data", "pulse"):
            (root / name).mkdir(mode=0o700)
        (root / "core.conf").write_text(private_core_config(policy))
        (root / "pulse.conf").write_text(PULSE)
        # The private client uses only its explicit native socket; no host config.
        (root / "client.conf").write_text('''context.properties = { support.dbus = false }
context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
context.modules = [
 { name = libpipewire-module-protocol-native }
 { name = libpipewire-module-client-node }
 { name = libpipewire-module-adapter }
 { name = libpipewire-module-metadata }
 { name = libpipewire-module-session-manager }
]
''')
        log = (root / "daemons.log").open("w+")
        silence = open("/dev/zero", "rb")

        def spawn(args, **kwargs):
            proc = subprocess.Popen(args, env=env, stderr=log, **kwargs)
            processes.append(proc)
            return proc

        def command(*args):
            return subprocess.check_output(args, env=env, timeout=5, text=True, stderr=log)

        def wait(check, description, timeout=8):
            deadline = time.monotonic() + timeout
            last = None
            while time.monotonic() < deadline:
                try:
                    value = check()
                    if value:
                        return value
                except (OSError, subprocess.SubprocessError, AssertionError, json.JSONDecodeError) as error:
                    # Noble's pw-dump can emit two JSON snapshots while the graph
                    # changes between a stream removal and replacement. A bounded
                    # retry must wait for one complete settled inventory.
                    last = error
                time.sleep(0.05)
            raise AssertionError(f"Timed out waiting for {description}: {last}")

        def pulse(*args):
            return json.loads(command(os.environ.get("SSGG_PACTL", "pactl"), "-f", "json", *args))

        def dump():
            return json.loads(command("pw-dump"))

        def native_stream():
            nodes = stream_nodes(dump())
            assert len(nodes) == 1, f"Expected exactly one private native stream: {nodes}"
            return nodes[0]

        def assert_native(expected_volume, expected_mute=None, expected_sink=None):
            node = native_stream()
            props = node_props(node)
            assert abs(native_effective_volume(props) - expected_volume) < 0.025, (props, expected_volume)
            if expected_mute is not None:
                assert props["mute"] is expected_mute, props
            if expected_sink is not None:
                assert linked_sink_name(dump(), node["id"]) == expected_sink, node
            return node

        sidecar = None
        try:
            spawn(["pipewire", "-c", "core.conf"], stdout=log)
            wait(lambda: (root / "pipewire-0").is_socket(), "owned PipeWire socket")
            assert not (root / "pipewire-0-manager").exists() or (root / "pipewire-0-manager").is_socket()
            assert stream_nodes(dump()) == [], "Private core already contains a stream"
            spawn(["pipewire-pulse", "-c", "pulse.conf"], stdout=log)
            wait(lambda: (root / "pulse/native").is_socket(), "owned Pulse fixture socket")
            info = command(os.environ.get("SSGG_PACTL", "pactl"), "info")
            assert f"{root}/pulse/native" in info, info
            assert pulse("list", "sinks") == [], "Host sinks leaked into private session"
            for name in ("test_a", "test_b"):
                command(os.environ.get("SSGG_PACTL", "pactl"), "load-module", "module-null-sink", f"sink_name={name}")
            assert {s["name"] for s in pulse("list", "sinks")} == {"test_a", "test_b"}
            spawn(policy, stdout=log)
            wait(lambda: "WirePlumber" in command(os.environ.get("SSGG_PACTL", "pactl"), "-f", "json", "list", "clients"), "policy-only WirePlumber")
            pacat = os.environ.get("SSGG_PACAT", "pacat")
            fixture = [pacat, "--playback", "--raw", "--device=test_a", "--client-name=SSGG-Isolated-Native-Game",
                       "--stream-name=Silence", "--property=media.role=game"]
            app = spawn(fixture, stdin=silence, stdout=log)
            wait(lambda: len(stream_nodes(dump())) == 1 and
                 len(pulse("list", "sink-inputs")) == 1 and pulse("list", "sink-inputs")[0]["sink"] != 4294967295,
                 "policy-linked pacat playback")
            if fixture_only:
                assert_native(1.0, False, "test_a")
                command("pw-cli", "set-param", str(native_stream()["id"]), "Props", "{ channelVolumes: [3.375, 3.375] }")
                wait(lambda: assert_native(1.5), "native fixture 150% readback")
                print(json.dumps({"result": "fixture-only", "backend_tested": False, "host_audio_mutated": False}))
                return
            sidecar = spawn([str(binary), "--stdio", "--config", str(root / "desktop/state.json")],
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
            assert sidecar.stdin is not None and sidecar.stdout is not None
            sequence = 0

            rpc_input = sidecar.stdin
            rpc_output = sidecar.stdout

            def rpc(method, params=None, expect_error=False):
                nonlocal sequence
                sequence += 1
                rpc_input.write(json.dumps({"id": sequence, "method": method, "params": params or {}}) + "\n")
                rpc_input.flush()
                assert select.select([rpc_output], [], [], 12)[0], f"{method} response deadline"
                line = rpc_output.readline()
                assert line, f"{method} closed instead of responding"
                response = json.loads(line)
                assert response.get("id") == sequence, response
                if expect_error:
                    assert "error" in response, f"Fail-closed expected, got: {response}"
                    return response["error"]
                if "error" in response:
                    details = {"metadata": command("pw-metadata", "-n", "default"),
                               "links": [obj.get("info") for obj in dump() if obj.get("type") == "PipeWire:Interface:Link"]}
                    raise AssertionError(f"native RPC error {response}; private graph: {details}")
                return response["result"]

            state = rpc("state.get")
            assert state["backend"]["name"] == "Native PipeWire", state
            assert {s["name"] for s in state["sinks"]} == {"test_a", "test_b"}, state
            assert len(state["streams"]) == 1, state
            stream = state["streams"][0]
            original = native_stream()
            assert stream["id"] == original["id"], (stream, original)
            app_key = stream["appKey"]
            serial = original["info"]["props"].get("object.serial")
            sink_b = next(s["id"] for s in state["sinks"] if s["name"] == "test_b")
            assert_native(1.0, False, "test_a")
            # External fixture adjustment uses native Props, never the service's Pulse shim.
            command("pw-cli", "set-param", str(stream["id"]), "Props", "{ channelVolumes: [3.375, 3.375] }")
            wait(lambda: assert_native(1.5), "native external 150%")
            rpc("stream.set", {"id": stream["id"], "group": "game"})
            rpc("chatmix.set", {"enabled": True, "balance": 0.5})
            wait(lambda: assert_native(0.75), "150% base attenuated to 75%")
            managed = rpc("streams.list")[0]
            assert abs(managed["volume"] - 1.5) < 0.025 and abs(managed["effectiveVolume"] - 0.75) < 0.025, managed
            rpc("chatmix.set", {"enabled": False})
            wait(lambda: assert_native(1.5), "150% base restored")
            rpc("stream.set", {"id": stream["id"], "volume": 1.0})
            wait(lambda: assert_native(1.0), "explicit base recovery to unity")
            rpc("stream.set", {"id": stream["id"], "sinkId": sink_b, "muted": True})
            wait(lambda: assert_native(1.0, True, "test_b"), "native gain/mute/move readback")
            assert pulse("list", "sink-inputs")[0]["sink"] == next(s["index"] for s in pulse("list", "sinks") if s["name"] == "test_b")
            metadata = command("pw-metadata", "-n", "default")
            assert "target.node" in metadata or "target.object" in metadata, metadata
            rpc("stream.set", {"id": stream["id"], "muted": False, "volume": 0.8})
            wait(lambda: assert_native(0.8, False, "test_b"), "native unmute and gain")
            app.terminate()
            app.wait(timeout=4)
            wait(lambda: not stream_nodes(dump()), "old native stream removal")
            app = spawn(fixture, stdin=silence, stdout=log)
            new = wait(lambda: native_stream() if native_stream()["info"]["props"].get("object.serial") != serial else None,
                       "new native stream serial")
            assert new["info"]["props"].get("object.serial") != serial
            def restored_identity():
                listed = rpc("streams.list")
                return rpc("state.get") if len(listed) == 1 and listed[0]["appKey"] == app_key else None
            restored = wait(restored_identity, "stable application identity")
            assert restored["streams"][0]["appKey"] == app_key, restored
            wait(lambda: assert_native(0.8, False, "test_b"), "restart routing and gain restoration")
            # Core death must not redirect to a host default socket or silently succeed.
            core = processes[0]
            core.terminate()
            core.wait(timeout=4)
            try:
                disconnected = rpc("state.get")
                assert disconnected["backend"]["connected"] is False, disconnected
                assert disconnected["streams"] == [] and disconnected["sinks"] == [], disconnected
                assert disconnected["mixer"]["enabled"] is False, disconnected
                assert disconnected["backend"]["name"] == "Native PipeWire", disconnected
            except (BrokenPipeError, AssertionError) as error:
                assert sidecar.poll() is not None, f"Unexpected sidecar response after core death: {error}"
            print(json.dumps({"result": "passed", "backend": "native PipeWire private core", "host_audio_mutated": False,
                              "native_checks": ["inventory", "gain", "mute", "move", "150%-base-recovery", "restart-identity", "core-failure"]}))
        except BaseException:
            log.flush()
            log.seek(0)
            print(log.read())
            raise
        finally:
            for process in reversed(processes):
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=4)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=4)
            silence.close()
            log.close()


if __name__ == "__main__":
    run()
