<img src="desktop/assets/branding/ssgg.svg" width="88" alt="SSGG logo" />

# SSGG — devices and audio, without the extras

A Linux desktop app for SteelSeries hardware and per-application audio. Assign music and calls to opposite sides of your headset wheel, adjust application volumes, and control lighting on supported devices.

This is the [MrTheSoulz fork](https://github.com/MrTheSoulz/steelseriesgg-rs) of [Ven0m0/steelseriesgg-rs](https://github.com/Ven0m0/steelseriesgg-rs), with a Rust service and an Electron interface. **It is a beta, not an official SteelSeries product or a feature-complete replacement for Windows GG/Sonar.**

## Install on Ubuntu

**Ubuntu 24.04 amd64 is the verified package baseline.** The service defaults to native libpipewire when a native socket exists, otherwise PulseAudio/PipeWire-Pulse (`pactl`); either can be selected explicitly (see [usage](docs/usage.md#applications-and-browser-tabs)). A broken detected native socket is reported, not silently retried over Pulse. Newer Ubuntu versions need their own runtime validation.

Use the `.deb` supplied with the beta build. CI packages are available in the `ssgg-ubuntu-24.04-amd64` artifact of a successful [Ubuntu desktop build](https://github.com/MrTheSoulz/steelseriesgg-rs/actions/workflows/build-linux.yml). Extract that download to get the `.deb`; do not use an artifact from a failed run as a tested release.

Open the `.deb` with your package installer, or run this from the directory containing it:

```sh
sudo apt install ./ssgg-gui_0.1.1_amd64.deb
```

Then open **SSGG** from Ubuntu's application launcher.

The package includes Electron and the matching Rust service. Apt installs its system dependencies. **You do not need Node, npm, Rust or Google Chrome, and the installer does not ask you to reboot or log out.** It refreshes access rules for already-connected SteelSeries HID devices without resetting USB or changing audio. No background service or autostart is enabled by installation.

[Installation, upgrades and removal](docs/installation.md) · [Troubleshooting](docs/troubleshooting.md)

### Snap status

The [strict Snap recipe](snap/README.md) is **experimental, not an install-and-use alternative to the `.deb`**. Stock desktop HID slots are missing on the tested Ubuntu configuration; Electron sandbox permission needs publisher approval, and cross-application audio authority still needs confined testing. Rebooting or adding `raw-usb` does not fix these limitations. SSGG does not silently switch to classic/devmode or disable Chromium's sandbox.

## Music and Discord with the headset wheel

1. Start music in Chrome and audio in Discord so their playback streams appear in **Mixer**.
2. Assign Chrome to **Media** and Discord to **Chat**.
3. Set Media's **ChatMix side** to **A**, and Chat's to **B**.
4. Click **Use headset wheel** in Mixer. SSGG connects to the supported headset and waits for an actual wheel reading.
5. Click **Enable ChatMix** when you want the wheel to change audio.

There is no hidden requirement to visit Devices first. Connecting the headset does not automatically enable mixing. If mixing is already enabled, selecting the wheel immediately uses its current position; the app warns you before that action.

You can assign any available playback application to either side, or leave it **Unmanaged**. Chrome tabs cannot be separated when the browser combines them into one native audio stream.

[Full usage guide](docs/usage.md)

## What works in this branch

| Area | Implemented behavior | Limits |
| --- | --- | --- |
| Audio | Real playback inventory, per-application volume/mute/output, Game/Chat/Media groups and profiles | Logical playback groups, not virtual DSP buses or Windows Sonar |
| Nova 7 Gen 2 (`1038:227e`) | Dedicated receiver/control-interface support, battery/status, physical ChatMix, sidetone and auto-off commands | Wheel center/extremes were read on hardware; settings and live application mixing have separate acceptance gates |
| RGB | Solid color, presets, brightness and off for **Apex Pro TKL Gen 3 wired (`1038:1642`)** | Source-derived protocol with injected-transport tests; no physical RGB acceptance yet, no firmware effects or on-board saving |
| Desktop | Original app/tray/launcher icons, single-instance window recovery, optional tray behavior, exact-model photo selection/download | GNOME needs an AppIndicator extension for a tray; the normal launcher works without one |

The Nova 7 Gen 2 has **no supported RGB lighting controls**. Other devices in the inherited registry are not automatically certified for desktop controls. Unsupported operations remain unavailable rather than sending guessed commands. The Apex TKL 2023 experimental path is not silently enabled.

## Deliberately not included

- Accounts, promotional feeds, game capture, automatic game integrations or telemetry.
- Claims of full GG/Sonar parity, universal SteelSeries support, virtual surround or unimplemented microphone processing.
- Automatic hardware writes, lighting replay or mixer activation on launch/reconnect.
- Manufacturer product photos bundled without permission. Optional photos retain their source and attribution.

EQ/DSP, expanded microphone controls and broader verified device coverage remain future work.

## Development and protocol documentation

End users should use the installer, not compile the app.

- [Desktop development, architecture and tests](desktop/README.md)
- [Device registry and discovery](docs/development/devices.md)
- [Keyboard protocol notes](docs/development/protocol-keyboard.md)
- [Legacy GameSense API](docs/development/gamesense-api.md) — optional CLI functionality, not started by the desktop
- [Original logo source and provenance](desktop/assets/branding/PROVENANCE.md)
- [Snap build and confinement analysis](snap/README.md)

## License and attribution

SSGG code and its original application branding are MIT-licensed; see [LICENSE](LICENSE) and the [branding license](desktop/assets/branding/LICENSE). Upstream authors retain attribution. Electron and bundled third-party components retain their own licenses. SteelSeries names and product photography belong to their respective owners; this project is not affiliated with SteelSeries.
