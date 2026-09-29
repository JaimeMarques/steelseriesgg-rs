import { useEffect, useState, useRef } from "react";
import {
  Headphones,
  Keyboard,
  Usb,
  Download,
  ImagePlus,
  SlidersHorizontal,
  Mic,
  Battery,
  ShieldCheck,
} from "lucide-react";
import type { Device, Snapshot } from "./shared/contracts";
import type { Mutate } from "./Mixer";
import HardwareControls from "./HardwareControls";
import LightingControls from "./LightingControls";
import { artworkFor } from "./shared/artwork-catalog";
export default function Devices({
  devices,
  selected,
  onSelect,
  physical,
  readOnly = false,
  saving = false,
  mutate,
}: {
  devices: Device[];
  selected: string;
  onSelect: (id: string) => void;
  physical?: Snapshot["physical"];
  readOnly?: boolean;
  saving?: boolean;
  mutate?: Mutate;
}) {
  const device = devices.find((d) => d.id === selected) || devices[0];
  const photoGeneration = useRef(0);
  const [photo, setPhoto] = useState<string | null>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  useEffect(() => {
    const generation = ++photoGeneration.current;
    setPhoto(null);
    setError("");
    setBusy(false);
    if (device)
      void window.ssgg
        ?.getArtwork(device.id)
        .then((v) => {
          if (generation === photoGeneration.current) setPhoto(v);
        })
        .catch(() => {
          if (generation === photoGeneration.current) setError("Cached photo unavailable. Choose another image.");
        });
    return () => {
      photoGeneration.current++;
    };
  }, [device?.id]);
  async function photoAction(download: boolean) {
    if (!device || !window.ssgg) return;
    const generation = ++photoGeneration.current;
    setBusy(true);
    setError("");
    try {
      const result = await (download ? window.ssgg.downloadArtwork(device.id) : window.ssgg.chooseArtwork(device.id));
      if (generation === photoGeneration.current) setPhoto(result);
    } catch (e) {
      if (generation === photoGeneration.current) setError(e instanceof Error ? e.message : "Could not load photo.");
    } finally {
      if (generation === photoGeneration.current) setBusy(false);
    }
  }
  if (!device)
    return (
      <section className="empty-state device-empty">
        <Usb size={36} />
        <h2>No devices detected</h2>
        <p>
          Connect a SteelSeries device or its USB receiver, then refresh. If it is still missing, check the local
          service and USB permissions.
        </p>
      </section>
    );
  const artwork = artworkFor(device.vendorId, device.productId);
  const kindIcon =
    device.kind === "keyboard" ? (
      <Keyboard size={42} />
    ) : device.kind === "headset" ? (
      <Headphones size={42} />
    ) : (
      <Usb size={42} />
    );
  const unavailable = (key: string, fallback: string) => device.capabilities[key]?.reason || fallback;
  return (
    <>
      {devices.length > 1 && (
        <label className="device-select">
          Selected device
          <select value={device.id} onChange={(e) => onSelect(e.target.value)}>
            {devices.map((d) => (
              <option value={d.id} key={d.id}>
                {d.name}
              </option>
            ))}
          </select>
        </label>
      )}
      <section className="device-scene">
        <div className="device-stage">
          {photo ? (
            <img className="product-photo" src={photo} alt={`${device.name} product photograph`} />
          ) : (
            <div className="photo-empty">
              {kindIcon}
              <p>No exact-model photo cached</p>
              <span>Keep it offline, or choose a photo below.</span>
            </div>
          )}
          <div className="photo-actions">
            {artwork && !photo && (
              <button disabled={busy} onClick={() => void photoAction(true)}>
                <Download size={16} />
                {busy ? "Loading photo…" : "Download official photo"}
              </button>
            )}
            <button className="subtle" disabled={busy} onClick={() => void photoAction(false)}>
              <ImagePlus size={16} />
              Choose local photo
            </button>
          </div>
        </div>
        <div className="device-story">
          <div className={"connection-pill " + (device.connected ? "connected" : "")}>
            <span className="status-dot" />
            {device.connected ? (device.kind === "headset" ? "Receiver connected" : "Device connected") : "Disconnected"}
          </div>
          <h2>{device.name}</h2>
          <p className="device-kind">
            {device.kind === "headset"
              ? "Wireless headset"
              : device.kind === "keyboard"
                ? "Keyboard"
                : "USB peripheral"}
          </p>
          <dl className="device-facts">
            <div>
              <dt>
                <Usb size={15} />
                USB identity
              </dt>
              <dd>
                {device.vendorId.toString(16).padStart(4, "0")}:{device.productId.toString(16).padStart(4, "0")}
              </dd>
            </div>
            <div>
              <dt>
                <Battery size={15} />
                Battery
              </dt>
              <dd>{device.battery == null ? "Battery unavailable" : `${Math.round(device.battery)}%`}</dd>
            </div>
          </dl>
          {artwork && (
            <div className="validation-note">
              <ShieldCheck size={18} />
              <div>
                <strong>Source implemented</strong>
                <p>
                  {device.kind === "headset"
                    ? "Battery, ChatMix input, sidetone and auto-off have source-derived support. Hardware validation is pending; receiver detection is not command verification."
                    : "This exact keyboard has source-derived RGB support. Physical lighting validation is pending; a matching photograph does not verify device commands."}
                </p>
              </div>
            </div>
          )}
        </div>
      </section>
      {error && (
        <p className="inline-warning" role="alert">
          {error}
        </p>
      )}
      <p className="artwork-credit">
        {artwork
          ? `${artwork.attribution} ${artwork.variant}. Download contacts the manufacturer's image CDN only when requested.`
          : "No verified manufacturer photo is catalogued for this exact model. Choose your own image; SSGG will not substitute a different model."}
      </p>
      <section className="hardware-controls">
        {device.kind === "headset" && (
          <HardwareControls
            key={device.id}
            device={device}
            physical={
              device.physical ?? (physical?.deviceId === device.id || !physical?.deviceId ? physical : undefined)
            }
            bridgeAvailable={!!physical}
            readOnly={readOnly}
            busy={saving}
            mutate={mutate}
          />
        )}
        <div className="section-heading">
          <div>
            <h2>Device controls</h2>
            <p>Capability status, not promises.</p>
          </div>
        </div>
        <LightingControls
          key={`${device.id}:${device.connected}`}
          device={device}
          readOnly={readOnly}
          busy={saving}
          mutate={mutate}
        />
        {!device.capabilities.rgb?.applicable && (
          <p className="capability-note">{unavailable("rgb", "RGB is unavailable for this model.")}</p>
        )}
        <div className="capability-row">
          <SlidersHorizontal size={21} />
          <div>
            <h3>Hardware equalizer</h3>
            <p>{unavailable("equalizer", "Hardware EQ is not supported by the current device bridge.")}</p>
          </div>
          <button disabled>Hardware equalizer</button>
        </div>
        <div className="capability-row">
          <Mic size={21} />
          <div>
            <h3>Hardware microphone</h3>
            <p>
              {unavailable(
                "microphone",
                "Hardware microphone gain and mute are not supported. This is separate from native audio-server microphone controls.",
              )}
            </p>
          </div>
          <button disabled>Hardware microphone</button>
        </div>
      </section>
    </>
  );
}
