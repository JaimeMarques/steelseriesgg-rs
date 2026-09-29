# Experimental strict Snap

**This is not yet an install-and-use equivalent of the native Ubuntu package.**
The recipe keeps both Snap strict confinement and Electron's internal sandbox.
It does not bypass missing permissions with classic, devmode, sandbox-disable
flags, experimental hotplug, host audio policy edits, or a privileged helper.
A successful build is not a successful hardware or cross-app mixer test.

## Build from the shared distribution

Build the standalone amd64 distribution on **Ubuntu 24.04** first. Supply its
app directory (containing `ssgg-gui`, `resources/ssgg-desktop`, Electron resources
and branding). This helper does not rebuild or modify that distribution:

```sh
node desktop/scripts/package-snap.mjs \
  --app-dir desktop/release/app \
  --icon path/to/the/actual/ssgg.svg \
  --output-dir desktop/release/snap-project
```

`--icon` can be omitted when the asset is
`resources/branding/ssgg.svg` inside the distribution. `--version` defaults to
`desktop/package.json`. The output directory must not already exist and must be
outside the app directory. Use a new output directory for each build.

The helper checks amd64 ELF entry points, GLIBC symbol requirements against
core24's 2.39 ceiling, and relocatable symlinks. This is a preliminary ABI check,
not a full library-closure proof. Do not copy host Ubuntu 26.04 libraries into
core24 to silence linker errors. Rebuild on the matching base instead.

With Snapcraft and its supported isolated builder installed, the command runs
`snapcraft pack` in the prepared project and checks the resulting SquashFS
metadata for `confinement: strict` and `base: core24`. It prints the actual
artifact path. Snapcraft stages `pactl`, `libpipewire-0.3.so.0` and their dependencies from the core24
Ubuntu archive and expands GNOME's desktop/runtime/GPU integration. No package
relies on an absolute development-machine cache path. No Store login or
publication is performed.

If the build tool/provider is unavailable, add `--prepare-only`. On a suitably
provisioned build machine, run `snapcraft pack` from the printed project path.
For CI, use a supported Ubuntu 24.04 Snapcraft build environment. Do not run
`--destructive-mode` against a mismatched Ubuntu host or treat `snap pack` on
an unstaged app directory as a substitute for staging the runtime libraries.

## Permission and portability limits

### HID is the hard stock-Ubuntu blocker

[`hidraw`](https://snapcraft.io/docs/hidraw-interface) is **not auto-connected**.
SSGG can consume a slot but an ordinary application snap cannot supply one:
snapd's base declaration restricts slot providers to `core` and `gadget`.
The current [implementation](https://github.com/canonical/snapd/blob/master/interfaces/builtin/hidraw.go)
has no implicit classic-desktop slot and no `HotplugDeviceDetected` method.
[Hotplug documentation](https://snapcraft.io/docs/explanation/how-snaps-work/hotplug-support/)
currently describes experimental USB serial-port support, not a universal HID
permission mechanism. Enabling it is not an SSGG installation step.

On the target host, inspect (no changes):

```sh
snap interface hidraw
snap connections ssgg
```

If there is **no matching slot**, strict SSGG cannot access that device on that
system. Stop here and use the native Ubuntu package if full hardware support is
needed. Rebooting, reinstalling, connecting `raw-usb`, or installing ordinary
udev rules does not create a missing Snap slot. Raw USB access does not grant
`/dev/hidraw*` access, so this recipe deliberately omits `raw-usb`.

Only when a real matching system/gadget slot exists, the administrator can run:

```text
sudo snap connect ssgg:hidraw PROVIDER:SLOT
```

Replace `PROVIDER:SLOT` with the exact slot shown by `snap interface hidraw`;
this is a placeholder, not a literal command to copy unchanged. Check the slot's
path and device identity before connecting. A path-only slot binds to a device
number, which is not a stable hardware identity across reconnects. A USB slot
requires integer `usb-vendor`, integer `usb-product`, and a
`/dev/hidraw-[a-z0-9]+` symlink path. That form matches VID/PID, not serial number
or HID interface number; identical receivers/composite HID interfaces are not
individually distinguished by those attributes. The application must still
honor its own device selection and explicit hardware-enable gate. Device ACLs
and the actual hidapi enumeration/open behavior still need installed testing.

### Native PipeWire access is not an audio-manager entitlement

The GUI app declares the `pipewire` plug for native socket access; diagnostics
keeps only its existing read-only Pulse checks and does not receive this plug.
Check the installed `ssgg:pipewire` connection separately from the build.
Even when connected, the sound server's session policy can restrict visibility
and write authority over unrelated clients. The plug is not a blanket mixer grant.

[`audio-playback`](https://snapcraft.io/docs/audio-playback-interface) normally
auto-connects. The [snapd policy](https://github.com/canonical/snapd/blob/master/interfaces/builtin/audio_playback.go)
allows communication with the Pulse socket and explicitly delegates audio
permission enforcement to the sound service. It does **not** promise that
SSGG can enumerate, move or change volumes of arbitrary applications.

Ubuntu's Snap-aware PipeWire/WirePlumber policy can distinguish Snap clients:
[WirePlumber's access-snap.lua](https://github.com/PipeWire/wireplumber/blob/0.5.6/src/scripts/client/access-snap.lua)
hides other clients and gates sinks/sources on playback/record permission.
[PipeWire Pulse volume operations](https://github.com/PipeWire/pipewire/blob/1.0.5/src/modules/module-protocol-pulse/pulse-server.c)
check object write/execute permissions. These sources establish that access is
policy-dependent, **not** that every host permits or blocks every mixer action.
See also the [Ubuntu mediation fix](https://bugs.launchpad.net/ubuntu/+source/pipewire/+bug/1995707).

The recipe does not request recording permission for a mixer, does not use the
deprecated `pulseaudio` interface, and does not rewrite the host's audio policy.
An installed strict build must prove enumeration and native control against a
private PipeWire server and unrelated test clients before cross-app mixing can be
claimed. Unconfined `pactl` success is not that proof. No such installed test has
been performed by this packaging work. The supported Ubuntu `.deb` is independent
of Snap's installed HID slot availability.

### Keeping the Electron sandbox needs additional approval

The named `electron-sandbox` plug uses `browser-support` with
`allow-sandbox: true`. [Canonical documents](https://snapcraft.io/docs/browser-support-interface)
that this is restricted to trusted publishers and is not auto-connected.
The [base declaration](https://github.com/canonical/snapd/blob/master/interfaces/builtin/browser_support.go)
also denies connection absent an applicable override/declaration. A manual
connection command is therefore **not guaranteed to work**. Publisher review
and a granted policy declaration are a release prerequisite; sideloading alone
must not be advertised as solving it. The package does not fall back to a
weaker Electron sandbox if permission is denied.

## Install, diagnostics and recovery

Only once a real artifact exists and the above constraints are acceptable,
an administrator can sideload it with `sudo snap install --dangerous FILE.snap`.
Here `--dangerous` means an unsigned local artifact, **not** devmode; strict
confinement remains in effect. No reboot is required by these package scripts.
This does not promise that the GUI or hardware will work without the required
publisher policy and device slots.

```sh
snap connections ssgg
snap run ssgg.diagnostics
snap run ssgg.diagnostics --audio
```

The optional audio diagnostic only requests server info, sinks and sink inputs;
it never changes volume, routing, modules, hardware state or persistent config.
Its result describes the diagnostics app's permissions; the GUI's distinct
security label must also be tested. `sudo snap connect ssgg:audio-playback`
can reconnect ordinary playback if it was disconnected, but is not a fix for
missing cross-application authority. Stop on policy errors rather than editing
host policy. Do not restart PipeWire/PulseAudio or reboot as a permissions fix.

GNOME initializes display, fonts and runtime libraries. The launcher preserves
Snap's private `XDG_RUNTIME_DIR`, sets config to `$SNAP_USER_DATA/.config`, and
uses bundled `pactl` for legacy mode; the native PipeWire library is staged
separately under `$SNAP/usr/lib/x86_64-linux-gnu`. It preserves an explicit
`PULSE_SERVER` for private tests.
It does not import native SSGG state or connect to the native private service.
Leave hardware and active mixing disabled during initial inspection. To revoke
hardware access, disconnect the specific `ssgg:hidraw` connection after disabling
mixing in the app. Native package configuration is separate and untouched.

## Validation tiers

```sh
node --test snap/tests/package-snap.test.mjs
python3 snap/tests/validate-skeleton.py  # requires PyYAML and snap
uv run --with craft-parts --with pyyaml python3 snap/tests/test-stage-layout.py
```

The Node tests use explicitly labelled ELF/shell **fixtures**, never release
artifacts. They exercise preparation, overwrite refusal, launcher environment,
argument forwarding, localized ABI rejection and read-only diagnostics. The
layout regression uses real craft-parts `organize_files` with labelled temporary
text path fixtures: it checks that archive `usr/bin/pactl` and library paths stay
at the Snap root while Electron stays under `app/`. It does not stage packages
or produce runtime libraries. The skeleton check invokes real
`snap pack --check-skeleton` for authored runtime metadata and executable bits;
it does not pretend to expand the GNOME extension or verify staged libraries.
CI checks the real staged native library path in the SquashFS; this remains a
build-only gate, not installed strict mixer or HID acceptance.

Earlier packaging inspection had no shared app available and no Snapcraft
installed, so it produced or installed no `.snap`. The observed target
reported snap/snapd 2.76.3 on Ubuntu 26.04; `snap interface hidraw` returned the
interface description **without any slots**. Reading the experimental-hotplug
system setting was access-denied, so its value was not inferred. The installed
WirePlumber policy contains `access-snap.lua`. Host audio, HID, configuration and
Snap connections were not changed.
