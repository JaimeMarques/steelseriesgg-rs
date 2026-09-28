#!/usr/bin/env python3
"""Real isolated PipeWire-Pulse E2E. No host sockets, devices, ALSA, HID or capture.
Requires pipewire, pipewire-pulse, pactl, pacat; run after cargo build --bin ssgg-desktop.
SSGG_PACTL and SSGG_PACAT may point to unpacked distro binaries.
"""
import json
import os
import re
from pathlib import Path
import subprocess
import tempfile
import time

PULSE = '''context.properties = { support.dbus = false }
context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
context.modules = [
 { name = libpipewire-module-protocol-native }
 { name = libpipewire-module-client-node }
 { name = libpipewire-module-adapter }
 { name = libpipewire-module-metadata }
 { name = libpipewire-module-protocol-pulse }
]
pulse.properties = { server.address = [ "unix:native" ] pulse.allow-module-loading = true }
'''
CORE = '''context.properties = { core.daemon = true core.name = pipewire-0 support.dbus = false }
context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
context.modules = [
 { name = libpipewire-module-protocol-native }
 { name = libpipewire-module-metadata }
 { name = libpipewire-module-spa-node-factory }
 { name = libpipewire-module-client-node }
 { name = libpipewire-module-adapter }
 { name = libpipewire-module-link-factory }
 { name = libpipewire-module-access args = { access.legacy = false access.socket = { pipewire-0 = "unrestricted" pipewire-0-manager = "unrestricted" } } }
]
context.objects = [ { factory = metadata args = { metadata.name = default } } { factory = spa-node-factory args = { factory.name = support.node.driver node.name = Dummy-Driver node.group = pipewire.dummy node.always-process = true priority.driver = 20000 } } ]
'''


def wireplumber_policy_command(config_root: Path) -> list[str]:
    """Select audited policy components, never a default hardware-monitoring profile."""
    modern = config_root / "wireplumber.conf"
    if modern.exists():
        config = modern.read_text()
        profile = re.search(r"(?m)^\s*policy\s*=\s*\{([^{}]*)\}", config)
        if profile:
            policy = profile[1]
            assert re.search(r"inherits\s*=\s*\[\s*base\s*\]", policy)
            assert "policy.standard = required" in policy
            assert not re.search(r"hardware\.|monitor\.", policy)
            return ["wireplumber", "--profile=policy"]
    legacy = config_root / "policy.conf"
    if legacy.exists():
        # Strip SPA comment lines before validating the effective component list.
        policy = "\n".join(line.split("#", 1)[0] for line in legacy.read_text().splitlines())
        blocks = re.findall(r"wireplumber\.components\s*=\s*\[([^\]]*)\]", policy, re.S)
        assert len(blocks) == 1, "Expected exactly one WirePlumber component list"
        components = re.findall(r"\{([^{}]*)\}", blocks[0])
        assert not re.sub(r"\{[^{}]*\}", "", blocks[0]).strip(), "Unparsed WirePlumber component"
        parsed = set()
        for component in components:
            fields = dict(re.findall(r"\b(name|type)\s*=\s*([\w./-]+)", component))
            assert len(fields) == 2, "Unknown WirePlumber component field"
            assert not re.sub(r"\b(?:name|type)\s*=\s*[\w./-]+", "", component).replace(",", "").strip(), "Unexpected component arguments"
            parsed.add((fields["name"], fields["type"]))
        assert len(components) == len(parsed), "Duplicate WirePlumber components"
        assert parsed == {
            ("libwireplumber-module-lua-scripting", "module"),
            ("policy.lua", "config/lua"),
        }, "Legacy policy must load only the Lua engine and policy.lua"
        assert not re.search(r"api\.(?:alsa|bluez|v4l2|libcamera)\.|monitor\.", policy)
        return ["wireplumber", "-c", str(legacy)]
    raise AssertionError("No audited policy-only WirePlumber configuration; refusing to launch a hardware profile")


def run():
    policy_command = wireplumber_policy_command(Path("/usr/share/wireplumber"))
    processes = []
    with tempfile.TemporaryDirectory(prefix="ssgg-audio-isolated-") as tmp:
        root = Path(tmp)
        env = os.environ.copy()
        for key in ("PIPEWIRE_REMOTE", "PULSE_SERVER", "PULSE_RUNTIME_PATH", "PIPEWIRE_CONFIG_PREFIX", "PIPEWIRE_CONFIG_NAME"):
            env.pop(key, None)
        env.update(XDG_RUNTIME_DIR=tmp, PIPEWIRE_RUNTIME_DIR=tmp,
                   XDG_CONFIG_HOME=str(root / "config"), XDG_STATE_HOME=str(root / "state"),
                   XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_HOME=str(root / "data"), HOME=str(root / "home"),
                   PIPEWIRE_CONFIG_DIR=tmp, DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent",
                   PULSE_SERVER=f"unix:{tmp}/pulse/native", PULSE_RUNTIME_PATH=f"{tmp}/pulse")
        (root / "pulse").mkdir(mode=0o700)
        (root / "core.conf").write_text(CORE)
        (root / "pulse.conf").write_text(PULSE)
        (root / "client.conf").write_text(Path("/usr/share/pipewire/client.conf").read_text())

        pactl = os.environ.get("SSGG_PACTL", "pactl")
        pacat = os.environ.get("SSGG_PACAT", "pacat")
        binary = os.environ.get("SSGG_DESKTOP_BIN", "target/debug/ssgg-desktop")
        env["SSGG_PACTL"] = pactl
        logs = (root / "daemons.log").open("w+")

        def spawn(args, **kwargs):
            process = subprocess.Popen(args, env=env, stderr=logs, **kwargs)
            processes.append(process)
            return process

        def pa(*args):
            return subprocess.check_output([pactl, *args], env=env, timeout=5, text=True)

        def wait(fn):
            deadline = time.monotonic() + 6
            while time.monotonic() < deadline:
                try:
                    value = fn()
                    if value:
                        return value
                except (OSError, subprocess.SubprocessError):
                    pass
                time.sleep(0.05)
            raise AssertionError("isolated daemon condition timed out")

        try:
            spawn(["pipewire", "-c", "core.conf"], stdout=logs)
            wait(lambda: (root / "pipewire-0").exists())
            spawn(["pipewire-pulse", "-c", "pulse.conf"], stdout=logs)
            wait(lambda: (root / "pulse/native").exists())
            info = pa("info")
            assert f"{tmp}/pulse/native" in info, info
            assert json.loads(pa("-f", "json", "list", "sinks")) == [], "must start with no host sinks"
            for name in ("test_a", "test_b"):
                pa("load-module", "module-null-sink", f"sink_name={name}")
            sinks = json.loads(pa("-f", "json", "list", "sinks"))
            assert {s["name"] for s in sinks} == {"test_a", "test_b"}
            # Stock 0.5 policy inherits base; 0.4 policy.conf loads policy.lua only.
            # No ALSA/Bluetooth/video monitor is loaded; all clients use our private core.
            spawn(policy_command, stdout=logs)
            wait(lambda: "WirePlumber" in pa("-f", "json", "list", "clients"))
            null_input = open("/dev/zero", "rb")
            app = spawn([pacat, "--playback", "--raw", "--device=test_a", "--client-name=SSGG-Isolated-Game", "--stream-name=Silence", "--property=media.role=game"], stdin=null_input, stdout=logs)
            wait(lambda: any(s["sink"] != 4294967295 for s in json.loads(pa("-f", "json", "list", "sink-inputs"))))
            sidecar = spawn([binary, "--stdio", "--config", str(root / "desktop/state.json")], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
            assert sidecar.stdin is not None and sidecar.stdout is not None
            counter = 0

            def rpc(method, params=None):
                nonlocal counter
                assert sidecar.stdin is not None and sidecar.stdout is not None
                counter += 1
                sidecar.stdin.write(json.dumps({"id": counter, "method": method, "params": params or {}}) + "\n")
                sidecar.stdin.flush()
                import select
                assert select.select([sidecar.stdout], [], [], 12)[0], "sidecar RPC deadline"
                response = json.loads(sidecar.stdout.readline())
                assert response["id"] == counter and "error" not in response, response
                return response["result"]

            state = rpc("state.get")
            assert len(state["streams"]) == 1 and len(state["sinks"]) == 2, state
            stream_id = state["streams"][0]["id"]
            pa("set-sink-input-volume", str(stream_id), "98304")
            rpc("stream.set", {"id": stream_id, "group": "game"})
            actual = json.loads(pa("-f", "json", "list", "sink-inputs"))[0]
            assert abs(next(iter(actual["volume"].values()))["value"] / 65536 - 1.5) < 0.001, actual
            rpc("chatmix.set", {"enabled": True, "balance": 0.5})
            actual = json.loads(pa("-f", "json", "list", "sink-inputs"))[0]
            assert abs(next(iter(actual["volume"].values()))["value"] / 65536 - 0.75) < 0.001, actual
            managed = rpc("streams.list")[0]
            assert abs(managed["volume"] - 1.5) < 0.001, managed
            assert abs(managed["effectiveVolume"] - 0.75) < 0.001, managed
            rpc("chatmix.set", {"enabled": False})
            actual = json.loads(pa("-f", "json", "list", "sink-inputs"))[0]
            assert abs(next(iter(actual["volume"].values()))["value"] / 65536 - 1.5) < 0.001, actual
            pa("set-sink-input-volume", str(stream_id), "98304")
            rpc("stream.set", {"id": stream_id, "volume": 1.0})
            actual = json.loads(pa("-f", "json", "list", "sink-inputs"))[0]
            assert abs(next(iter(actual["volume"].values()))["value"] / 65536 - 1.0) < 0.001, actual
            b_id = next(s["id"] for s in state["sinks"] if s["name"] == "test_b")
            rpc("stream.set", {"id": stream_id, "group": "game", "volume": 0.8, "sinkId": b_id})
            actual = json.loads(pa("-f", "json", "list", "sink-inputs"))[0]
            assert actual["sink"] == b_id, actual
            rpc("chatmix.set", {"enabled": True, "balance": 0.5})
            for _ in range(3):
                rpc("chatmix.set", {"balance": 0.5})
            actual = json.loads(pa("-f", "json", "list", "sink-inputs"))[0]
            assert abs(next(iter(actual["volume"].values()))["value"] / 65536 - 0.4) < 0.001, actual
            rpc("group.set", {"id": "game", "muted": True})
            assert json.loads(pa("-f", "json", "list", "sink-inputs"))[0]["mute"] is True
            rpc("group.set", {"id": "game", "muted": False})
            pa("set-sink-input-volume", str(stream_id), "19661")
            rpc("chatmix.set", {"balance": 0})
            actual = json.loads(pa("-f", "json", "list", "sink-inputs"))[0]
            assert abs(next(iter(actual["volume"].values()))["value"] / 65536 - 0.6) < 0.001
            rpc("profiles.save", {"name": "Isolated"})
            rpc("profiles.apply", {"name": "Isolated"})
            app.terminate()
            app.wait(timeout=3)
            wait(lambda: not json.loads(pa("-f", "json", "list", "sink-inputs")))
            spawn([pacat, "--playback", "--raw", "--device=test_a", "--client-name=SSGG-Isolated-Game", "--stream-name=Silence", "--property=media.role=game"], stdin=null_input, stdout=logs)
            wait(lambda: any(s["sink"] != 4294967295 for s in json.loads(pa("-f", "json", "list", "sink-inputs"))))
            def restored():
                actual = json.loads(pa("-f", "json", "list", "sink-inputs"))[0]
                return actual["sink"] == b_id and abs(next(iter(actual["volume"].values()))["value"] / 65536 - 0.6) < 0.001
            wait(restored)
            sidecar.stdin.close()
            assert sidecar.wait(timeout=4) == 0
            print(json.dumps({"result": "passed", "backend": "isolated PipeWire-Pulse", "sinks": 2, "tested": ["read-only inventory", "per-stream gain", "native 150% group-only mix 75% disable restore 150%", "native 150% to 100%", "routing", "mute", "group mute", "noncompounding chatmix", "external gain preservation", "profiles", "restarted-app restore"], "host_audio_mutated": False}))
        except BaseException:
            logs.flush()
            logs.seek(0)
            print(logs.read())
            raise
        finally:
            for process in reversed(processes):
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=3)
            logs.close()

if __name__ == "__main__":
    run()
