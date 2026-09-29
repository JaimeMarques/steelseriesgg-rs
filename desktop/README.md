# SSGG desktop

Developer guide for the Electron + React + TypeScript console. For ordinary use, follow the [Ubuntu installation guide](../docs/installation.md) and [usage guide](../docs/usage.md). No renderer shell, arbitrary filesystem paths, network client or production fixture fallback.

## Build and launch locally

Prerequisites: Linux, Node/npm, the repository's Rust toolchain and native build dependencies. The native default requires a running PipeWire core, `libpipewire-0.3-dev`, `libspa-0.2-dev`, libclang during compilation and `libpipewire-0.3-0t64` at runtime on Ubuntu 24.04. Pulse-only sessions use **pactl** (`pulseaudio-utils` on Debian/Ubuntu; `libpulse` on Arch). Electron needs the usual GTK/NSS/GBM libraries and a working Chromium sandbox. `xvfb` is only needed for headless smoke tests.

```sh
cd desktop
npm ci --include=dev
npm run build:local
npm run launch -- --read-only   # real inventory; no audio/HID changes
npm run launch                 # normal controls, explicit opt-in hardware
```

To reuse a trusted, already compiled Rust binary instead of running Cargo:

```sh
npm run build:local -- /absolute/path/to/ssgg-desktop
```

The build stages a mode-755 binary under the private, ignored `local-bin/` directory, builds the UI/preload/main outputs, and renders `local-bin/ssgg-desktop-local.service`. No installed binaries, user configuration or services are changed. A source checkout's group-writable Cargo output is copied, not chmodded in place. These are runnable **local artifacts**, not a portable installer. The generated unit contains this checkout's absolute executable path; the portable template is `../assets/ssgg-desktop.service`.

`npm start` remains a development shortcut using the default `../target/debug/ssgg-desktop` or an absolute `SSGG_SIDECAR` override. The bridge rejects group/world-writable executables. Packaged builds ignore the override and require a trusted binary in `process.resourcesPath`.

## Build Ubuntu packages

Build on Ubuntu 24.04 amd64 for the compatibility baseline. Install build tools (`build-essential`, `pkg-config`, `libpipewire-0.3-dev`, `libspa-0.2-dev`, `libclang-dev`, `clang`, `unzip`, `binutils`, `apparmor`, `desktop-file-utils`) and the pinned Rust toolchain. Optional PulseAudio-feature tests also require `libpulse-dev`. Headless GUI tests need `xvfb`, `xauth`, `x11-utils` and `dbus-x11`.

```sh
cd desktop
npm ci --include=dev
npm run package:app       # Electron + renderer + matching Rust service
npm run package:deb       # also builds and checks release/ssgg-gui_0.1.1_amd64.deb
npm run test:packaging
```

Packaging compiles Rust for generic `x86-64`, overriding inherited developer CPU tuning, and always extracts the checksum-verified official Electron archive, never a development `node_modules/electron/dist` tree. A custom `--sidecar` must already be built for the intended CPU baseline; ELF architecture/GLIBC checks alone cannot certify every instruction in an externally supplied binary. It checks ELF architecture/GLIBC requirements, rejects shipped symlinks, and validates the desktop entry and AppArmor syntax. The standalone output is `release/app`; unprivileged extraction cannot establish the root ownership of the DEB's sandbox helper.

The Ubuntu CI job runs both legacy and native backends against separate private PipeWire cores, checks the native ELF dependency and DEB runtime dependency, installs the DEB on a disposable runner, runs the installed native sidecar against a private core, launches the actual packaged binary in a private session without a helper override, and purges it while checking that user data survives. Archive inspection and local extraction tests alone do not certify fresh installation, Wayland/GNOME Shell behavior or hardware access.

Snap builds consume the same standalone tree; see the [Snap guide](../snap/README.md). Strict confinement's HID, browser-sandbox and cross-app audio limitations are release gates, not errors to bypass.

## Independent service (optional)

To keep mixing when the GUI quits, start the standalone Rust service **before** opening SSGG:

```sh
# In its own terminal; no installer or systemd changes:
./local-bin/ssgg-desktop                              # native PipeWire if socket exists; otherwise Pulse
# Override if needed (do not run both services):
./local-bin/ssgg-desktop --audio-backend pulse
./local-bin/ssgg-desktop --audio-backend pipewire
# Then, in another terminal:
npm run launch
```

The daemon listens at `$XDG_RUNTIME_DIR/ssgg-desktop/service.sock`. Electron prefers this socket after checking owner, filesystem type, no symlinks at the runtime directory/service directory/socket, and private permissions (0700 directories, 0600 socket). Renderer IPC cannot select a socket or executable. There is no TCP listener started by production code.

Native selection checks `$PIPEWIRE_RUNTIME_DIR/${PIPEWIRE_REMOTE:-pipewire-0}` (or `$XDG_RUNTIME_DIR` if `PIPEWIRE_RUNTIME_DIR` is unset). A detected native socket whose connection/readback fails stops startup; it never falls back to Pulse. Use `--audio-backend pulse` only when you explicitly want the legacy protocol.

A connected external service is **never killed or reset on GUI quit**. A window-owned `--stdio` sidecar is used only when no socket is available, and is labelled accordingly. A failed/untrusted existing socket is an error, not permission to start a competing service. After a previously attached service disappears, refresh retries the socket without falling back to a competitor. Reopening a hidden window also resumes polling/reconnection. Config-lock conflicts show a specific recovery message.

The optional systemd user-unit template is supplied, **not installed or enabled**. It uses a restrictive umask, runtime directory, no new privileges, and no automatic hardware acquisition. Do not run the legacy `ssgg` audio loop concurrently against the same devices/audio policies. Service startup itself only inventories; hardware and ChatMix require separate explicit activation.

Assignments, group gains and profiles are persisted by Rust. Hardware ownership and mixer activation are session state and must be re-enabled after restart/disconnection. Loss/staleness disarms hardware mixing without restoring gains or rerouting audio. Closing a GUI is not the same as restarting a service. Login/logout lifetime is determined by how the standalone process is managed.

## Safe inspection

`--read-only` always launches a **separate `--stdio --safe-mode` process with an isolated temporary config**, even if a writable socket is available. Electron also denies all audio, HID and profile mutation IPC; the read-only badge is not the security boundary. Real inventory is retained. No HID acquisition/query is initiated by opening the GUI. In unpackaged development, `SSGG_READ_ONLY=1` is equivalent.

The tray preference is local desktop state. “Keep running in the tray” hides the window. If using stdio, quitting ends the owned child; if connected via socket, the external owner continues. Desktop settings display transport and persistence consequences explicitly.

## Controls and observation

- Streams use validated numeric backend IDs. Requested gain remains **0..1**; observed native amplification above 100% is retained, not clamped or rejected. Native effective volume/mute is displayed separately from base intent, group gain and ChatMix. A bounded base slider cannot request amplification.
- Generic `WEBRTC VoiceEngine` labels use the backend's JSON app-key executable when present (for example `Discord`); the original raw application/stream remains the subtitle. Unknown identity is never guessed. A browser's already combined tabs cannot be separated.
- Group **ChatMix side** assignment is independent of whether ChatMix is enabled or a physical wheel is available.
- `device.set` is a narrow named API: selected ID, explicit hardware acquisition/release, sidetone 0..3, auto-off 0..255, status refresh. Controls require backend capability support and are disabled in safe mode. Baseline `UNSUPPORTED` remains a visible failure.
- **Use headset wheel** is available directly in Mixer for one connected supported receiver. The explicit click acquires that receiver if needed, shows **Connecting to headset…**, and polls `getState` every 250 ms for up to 8 seconds for a matching, acquired, connected, non-stale sample with no pending operation/error. Only then does it select `inputMode: hardware` and read back confirmation. Already-acquired hardware needs no second opt-in. No `enabled: true`, balance, group or application changes are sent by setup.
- Acquisition/source selection does **not** enable mixing. Source state (**Headset wheel active**) and mixer state (**ChatMix is off**) are separate. If ChatMix is already on, the setup action discloses that selecting the wheel immediately uses its current position. The frontend never switches source on detection/reconnection or retries automatically after failure. On-screen balance remains a separate explicit choice.
- Read-only sessions, unavailable/older services, unsupported headsets and missing USB receivers have nearby reasons and recovery guidance. Multiple receivers require an explicit device choice. Connection failures stay visible with **Retry headset wheel**; **Cancel wheel setup**, leaving Mixer, or timeout prevents a late acquisition/read from subsequently selecting the source. Acquisition already accepted by the service may remain in place; release it in Devices if desired. Cancellation is available during connection, before the source command is sent.
- Hardware samples expose independent A/B gains; a scalar slider is only a disabled visualization, never converted back to a software request. No sample means no invented centered wheel.
- Wireless connection, battery, last status/wheel timestamps, pending/error states, verified sidetone and sent-only auto-off are distinguished. `supported` means source-supported; `locallyValidated` is separate and currently false. This frontend integration does not claim physical command validation.
- Saved profile names follow Rust's nonempty, at-most-80-UTF-8-byte policy, including punctuation. They are JSON names, not filesystem paths.

## Tests

```sh
PYTHONPATH=tests python3 -m unittest tests/test_desktop_pipewire_config.py
node --test tests/native-packaging.test.mjs
# Only after building the integrated native-capable sidecar; this
# starts owned private daemons/null sinks and NEVER writes to host audio:
python3 tests/desktop_pipewire_isolated.py
npm test
npm run typecheck
npm run smoke
# In-Mixer wheel flow, real sandboxed Electron + private fixture only:
xvfb-run -a node tests/wheel-setup-smoke.mjs
# Real inventory only; uses the staged local binary and never acquires HID:
SSGG_PACTL=/absolute/path/to/pactl xvfb-run -a node tests/launch-readonly.mjs
SSGG_SIDECAR="$PWD/local-bin/ssgg-desktop" xvfb-run -a node tests/live-readonly.mjs
```

`smoke` runs real sandboxed Electron, checks renderer Linux seccomp/context isolation, native control bridge requests/read-back, profiles, tray lifetime, private UDS transport/reconnection, external owner survival, config-lock error, unsupported hardware rejection and safe-mode gating. Fixture sidecars are installed as private 755 test copies; production executable validation is not relaxed. Unit tests cover amplified observations, hardware DTO/commands, profile names, stale photo promises, disabled slider synchronization and pre-decode artwork limits. Hardware tests use fixtures; no live audio/HID writes are performed.

The actual local-launch check exercises `scripts/launch.mjs` with real Rust safe inventory and checks 1320/900px layouts. Artifacts are ignored under `artifacts/`; read-only design evidence also lives under `.impeccable/review`.

### Chromium sandbox

Never pass `--no-sandbox` or disable host security. Playwright sets `chromiumSandbox:true` explicitly. An npm-installed Electron helper is not automatically root-owned. Development-only extraction checks can use an existing trusted, root-owned 4755 helper in a separate test tree, but must disclose that override in their evidence. **Never ship a machine-specific helper symlink.** The Debian builder extracts a checksum-verified pristine runtime and gives the genuine packaged helper its required ownership/mode. Installed-package CI must run without a test helper override. Headless tests select X11 explicitly for Xvfb; normal launch keeps platform defaults.

## Artwork and privacy

Exact-model photography is optional and downloaded only on request from the catalogued manufacturer URL, or chosen through a native local picker. Only static PNG and ordinary baseline/progressive JPEG are accepted. A bounded header/chunk scan enforces 8 MiB input, 8000px per side and 16-megapixel budgets **before native decoding**, including cached image reads; malformed/truncated metadata, SVG and animated PNG are rejected. Device/request generations prevent a late photo from being displayed as another selected model.

SteelSeries owns its product photography; it is not bundled or relicensed with this code. The catalog records model, URL and attribution; cached sidecars retain provenance. The black-variant disclaimer remains visible because USB IDs do not identify casing color. Unknown models get no substitute image. No accounts, promotions, telemetry, automatic downloads or synthetic meters.
