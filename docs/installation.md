# Install, update and remove SSGG

## Ubuntu desktop — recommended

The desktop package targets **Ubuntu 24.04 amd64**. The default mixer uses a PipeWire-Pulse or PulseAudio session; the opt-in native backend requires a PipeWire session and `libpipewire-0.3-0t64` (installed by apt with the package). Ubuntu 22.04 and ARM are not targets of this package. Newer Ubuntu versions require their own ABI/runtime validation.

Use the `.deb` supplied with the beta, or extract `ssgg-ubuntu-24.04-amd64` from a successful [Ubuntu desktop build](https://github.com/MrTheSoulz/steelseriesgg-rs/actions/workflows/build-linux.yml). GitHub may require signing in to download CI artifacts. The old CLI/Arch archive is not the Electron installer.

1. Open `ssgg-gui_0.1.1_amd64.deb` with a graphical Debian package installer and choose Install.
2. If Ubuntu's file handler does not offer package installation, use the equivalent command:

   ```sh
   sudo apt install ./ssgg-gui_0.1.1_amd64.deb
   ```

3. Open **SSGG** from the application launcher. Do not launch it with `sudo`.
4. Follow the [music/call setup](usage.md#music-and-discord-with-chatmix).

Apt resolves runtime dependencies. Node, npm, Rust and Chrome are **build tools or development conveniences, not end-user prerequisites**. Prefer `apt install ./file.deb` over `dpkg -i`: apt resolves missing dependencies.

### No reboot or group changes

The package installs a narrow SteelSeries `uaccess` rule, reloads rules and reevaluates existing SteelSeries hidraw nodes. It does not reset USB, restart audio, add you to a broad input group, enable an autostart/service, or require logout/reboot. HID access depends on an active local desktop seat; an SSH-only session is not equivalent.

Installing or opening SSGG does not acquire a headset or activate ChatMix. **Use headset wheel** is the explicit setup action. RGB uses an explicit Apply action.

### GNOME launcher and tray

The application is named **SSGG**. Its desktop/icon identity is `io.github.MrTheSoulz.SSGG`; launching it again brings the existing window forward rather than starting another owner.

GNOME's app launcher does not need a tray extension. A system tray does need an AppIndicator/StatusNotifier extension, often supplied by Ubuntu. Without a watcher, SSGG disables close-to-tray and cannot hide its only window. See Settings for whether your session supports the tray.

### Upgrade

Close SSGG normally, then install the newer `.deb` using the same command. If you want ChatMix attenuation removed before quitting, explicitly disable ChatMix first. An independently managed service is a separate owner: stop your own SSGG desktop service normally before replacing it; do not restart unrelated audio or legacy services.

User profiles and assignments are retained. Reopen SSGG and explicitly reconnect hardware/enable mixing. Updates do not automatically replay device settings or lighting.

### Remove

```sh
sudo apt remove ssgg-gui
```

To remove package-owned configuration too:

```sh
sudo apt purge ssgg-gui
```

Neither operation deletes your home-directory settings or changes the legacy `ssgg.service`. Backend assignments/profiles normally live in `$XDG_CONFIG_HOME/ssgg-desktop/state.json` (usually `~/.config/ssgg-desktop/state.json`); desktop preferences/artwork use Electron's SSGG user-data directory (normally `~/.config/SSGG`). Back up these directories before intentionally deleting user data.

## Snap — experimental

The [strict Snap build](../snap/README.md) remains experimental and build-only evidence: its native PipeWire plug needs connection/policy approval and does not prove cross-app control. The tested Ubuntu desktop has no usable hidraw slots for this application, and its Electron sandbox requires a publisher policy declaration. Missing Snap HID does not block the supported `.deb` path.

Do not try to solve this with rebooting, broad device permissions, a `raw-usb` connection, `--devmode` or `--no-sandbox`. A classic Snap has different security implications and is not silently substituted. Use the `.deb` for the supported desktop path.

## Build instead of install

Developers should use the [desktop build guide](../desktop/README.md). A source checkout, `local-bin` executable, standalone application directory or optional user-unit template is not a Debian/Snap installation.
