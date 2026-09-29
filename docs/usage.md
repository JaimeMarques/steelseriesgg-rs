# Using SSGG

## Music and Discord with ChatMix

Start playback in Chrome and your Discord call first; applications only appear when they create playback streams visible to the selected backend.

1. In **Mixer**, set Chrome's group to **Media** and Discord's to **Chat**.
2. Set Media's **ChatMix side** to **A** and Chat's to **B**. Leave other groups on **None** if the wheel should not affect them.
3. Click **Use headset wheel**. SSGG connects to the supported receiver and waits for a fresh reading. You no longer need to find a separate hardware switch in Devices first.
4. Confirm **Headset wheel active**, then click **Enable ChatMix** to let it change audio.

If ChatMix was already enabled, selecting the wheel immediately uses its current position. Connecting the headset and enabling audio mixing are deliberately separate actions.

The Nova 7 Gen 2 reports independent gains: the center is A 100% / B 100%, not half volume on both. The displayed balance bar follows hardware when the wheel is selected; it is not draggable in that mode. Use **Use on-screen balance** for manual mixing.

**Disable ChatMix** removes its attenuation. Group gain/mute and application base volume remain independent controls. **Unmanaged** applications are not assigned to a wheel group. The group names are conveniences: Media can contain a game, Game can contain a music player, and several applications can share either side.

### Applications and browser tabs

On Linux, the service selects native libpipewire when the native socket exists (`$PIPEWIRE_RUNTIME_DIR/pipewire-0`, or `$XDG_RUNTIME_DIR/pipewire-0` when unset; `PIPEWIRE_REMOTE` names a different socket). With no native socket, it uses PulseAudio/PipeWire-Pulse compatibility, which requires `pactl`. To force a backend, start the packaged sidecar **as your desktop user** before opening the GUI:

```sh
/opt/ssgg/resources/ssgg-desktop --audio-backend pulse     # force legacy protocol
# or --audio-backend pipewire (requires a working native socket)
# Then open SSGG from the launcher; it attaches to this independent service.
```

Stop any SSGG desktop service you own before switching backends; never run two mixers against the same applications. Once a native socket is detected, failed connection or readback is an error, not a silent Pulse fallback. The native service uses PipeWire nodes/metadata. Private-core acceptance does not prove installed strict-Snap mixer authority. Neither backend can split browser tabs Chrome has already combined into one stream. Start playback and use Refresh if a program is missing. Application names come from stream metadata; unknown identities are not guessed.

Output selection moves the selected application stream. SSGG does not create Sonar-style virtual devices or DSP buses.

### Disconnects and restarting

Loss of the headset or a stale reading disarms physical mixing. SSGG does not automatically switch to speakers, restore gains, reacquire hardware or re-enable the wheel after reconnection. Reconnect explicitly and review your mix before enabling it again.

The default window-owned service stops when you quit SSGG. If Settings reports an independent background service, quitting the GUI does not stop that external owner. Closing to a supported tray keeps the app running; it is not an autostart setting. See [advanced service ownership](../desktop/README.md#independent-service-optional).

## Headset controls

Devices shows the detected model, connection state and supported operations. The Nova 7 Gen 2 implementation exposes status/battery, sidetone and auto-off controls. Availability means an implemented source-derived protocol, not a claim that every setting was validated on your physical unit.

Status queries, setting commands, confirmed sidetone readback and sent-only auto-off are shown separately. Do not interpret a sent command as a verified device setting. Unsupported EQ or microphone controls are not simulated.

## RGB lighting

Desktop lighting is currently implemented for **Apex Pro TKL Gen 3 wired, USB `1038:1642`**, using a source-derived protocol. Physical RGB acceptance is still pending.

1. Select the keyboard in **Devices**.
2. Pick a preset or custom color and adjust brightness.
3. Close other RGB controllers, then explicitly apply your changes.

Color, brightness and Off are drafts until Apply. Brightness scales the selected RGB color; 0% sends black. Changing presets, opening the app, reconnecting hardware and applying audio profiles do not automatically write lighting.

The interface distinguishes a pending command, an error and the **last sent** color/brightness. There is no device readback, firmware-effect editor or on-board profile saving in this implementation. The service does not claim exclusive control over unrelated RGB tools that ignore its advisory lock.

The Nova 7 Gen 2 has no RGB controls. A detected keyboard with unsupported lighting shows its limitation instead of an apparently working control. The older Apex TKL 2023 experimental path remains separate and unavailable in the default desktop workflow.

## Profiles

Profiles save application assignments, group settings and mixer intent. They are not hardware firmware profiles. Hardware acquisition is not replayed automatically, and an enabled physical-mix profile needs an explicitly acquired fresh receiver. Review the saved mix before applying it.

## Device photos

Model-specific photographs are optional. Choose a local image or explicitly download the catalogued manufacturer image; SSGG does not fetch it on startup. The selected generation must match. USB identifiers do not establish casing color. Unknown models do not get a misleading substitute photograph.

## Read-only inspection

```sh
ssgg-gui --read-only
```

This opens a separate safe process/configuration rather than attaching to a mutable mixer. Inventory is real, but audio, profile and hardware mutation IPC is blocked. It is useful for reporting a problem without changing your sound.
