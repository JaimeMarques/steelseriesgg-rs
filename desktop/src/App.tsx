/// <reference types="vite/client" />
import { useEffect, useRef, useState } from "react";
import { SlidersHorizontal, Headphones, Settings2, Layers3, Keyboard, Usb, RefreshCw } from "lucide-react";
import brandIcon from "../assets/branding/ssgg.svg";
import { mutationTimeout, type Snapshot, type RuntimeInfo, type DesktopBridge } from "./shared/contracts";
import Mixer from "./Mixer";
import Devices from "./Devices";
import Profiles from "./Profiles";
import Settings from "./Settings";
export default function App() {
  const [tab, setTab] = useState("mixer");
  const [selectedDevice, setSelectedDevice] = useState("");
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [runtime, setRuntime] = useState<RuntimeInfo | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const epoch = useRef(0),
    refreshing = useRef(false),
    mutating = useRef(false),
    indeterminate = useRef(false);
  async function mutate(action: (bridge: DesktopBridge) => Promise<unknown>, message = "Change saved") {
    if (!window.ssgg || mutating.current) return;
    mutating.current = true;
    epoch.current++;
    setBusy(true);
    setNotice("");
    indeterminate.current = false;
    try {
      await action(window.ssgg);
      setSnapshot(await window.ssgg.getState());
      setError("");
      setNotice(message);
    } catch (e) {
      const message = e instanceof Error ? e.message : "Change failed. Try again.";
      if (message.includes(mutationTimeout)) {
        indeterminate.current = true;
        // This read is a snapshot, not proof that the timed-out command has finished.
        try { setSnapshot(await window.ssgg.getState()); } catch { /* Preserve the last known state and warning. */ }
      }
      setError(message);
    } finally {
      mutating.current = false;
      setBusy(false);
    }
  }

  async function refresh(explicit = false) {
    if (!window.ssgg) {
      setError("Desktop bridge unavailable");
      setLoading(false);
      return;
    }
    if (refreshing.current || mutating.current) return;
    const current = epoch.current;
    refreshing.current = true;
    try {
      const nextRuntime = await window.ssgg.getRuntime();
      if (current === epoch.current) setRuntime(nextRuntime);
      const next = await window.ssgg.getState();
      if (current === epoch.current) {
        setRuntime(nextRuntime);
        setSnapshot(next);
        if (!indeterminate.current || explicit) {
          setError("");
          if (explicit && indeterminate.current)
            setNotice("Current state refreshed. A timed-out change may still complete; verify before retrying.");
          indeterminate.current = false;
        }
      }
    } catch (e) {
      if (current === epoch.current) {
        setSnapshot(null);
        setError(e instanceof Error ? e.message : "Audio service unavailable");
      }
    } finally {
      refreshing.current = false;
      setLoading(false);
    }
  }
  useEffect(() => {
    void refresh();
    const visibleRefresh = () => {
      if (document.visibilityState === "visible") void refresh();
    };
    const timer = setInterval(visibleRefresh, 2000);
    document.addEventListener("visibilitychange", visibleRefresh);
    return () => {
      epoch.current++;
      clearInterval(timer);
      document.removeEventListener("visibilitychange", visibleRefresh);
    };
  }, []);
  return (
    <div className="app-shell">
      <aside className="rail">
        <button className="brand" onClick={() => setTab("mixer")} aria-label="SSGG home">
          <span className="brand-mark">
            <img src={brandIcon} width={25} height={25} alt="" />
          </span>
          ssgg<span className="brand-dot">.</span>
        </button>
        <nav aria-label="Main navigation">
          {[
            { id: "mixer", name: "Mixer", Icon: SlidersHorizontal },
            { id: "devices", name: "Devices", Icon: Headphones },
            { id: "profiles", name: "Profiles", Icon: Layers3 },
            { id: "settings", name: "Settings", Icon: Settings2 },
          ].map(({ id, name, Icon }) => (
            <button
              key={id}
              className={"nav-item " + (tab === id ? "selected" : "")}
              aria-current={tab === id ? "page" : undefined}
              onClick={() => setTab(id)}
            >
              <Icon />
              {name}
            </button>
          ))}
        </nav>
        <div className="rail-devices">
          <h2>Your devices</h2>
          {snapshot?.devices.length ? (
            snapshot.devices.map((device) => (
              <button
                className={
                  "rail-device " +
                  (tab === "devices" &&
                  (selectedDevice === device.id || (!selectedDevice && snapshot.devices[0].id === device.id))
                    ? "active"
                    : "")
                }
                key={device.id}
                onClick={() => {
                  setSelectedDevice(device.id);
                  setTab("devices");
                }}
              >
                {device.kind === "keyboard" ? (
                  <Keyboard size={24} />
                ) : device.kind === "headset" ? (
                  <Headphones size={24} />
                ) : (
                  <Usb size={24} />
                )}
                <strong>{device.name}</strong>
                <small>{device.connected ? "USB connected" : "Disconnected"}</small>
              </button>
            ))
          ) : (
            <>
              <p>No devices connected</p>
              <span>Hardware appears here when the local service detects it.</span>
            </>
          )}
        </div>
        <div className="rail-footer">
          <span className={"status-dot " + (snapshot ? "online" : "")} />
          {snapshot
            ? runtime?.transport === "socket"
              ? "Background service"
              : "Window-owned sidecar"
            : "Service not connected"}
          <small>Local by design. Yours to control.</small>
        </div>
      </aside>
      <main id="main">
        <header className="page-header">
          <div>
            <h1>
              {tab === "mixer"
                ? "Your audio, your balance."
                : tab === "devices"
                  ? "Made for your hardware."
                  : tab === "profiles"
                    ? "Keep your mix close."
                    : "Make yourself at home."}
            </h1>
            <p>
              {tab === "mixer"
                ? "Bring music, calls and everything else into the right mix."
                : tab === "devices"
                  ? "The right device. The real capabilities. Nothing invented."
                  : tab === "profiles"
                    ? "Your familiar sound, ready when you need it."
                    : "A few preferences. No distractions."}
            </p>
          </div>
          <button className="icon-button" onClick={() => void refresh(true)} aria-label="Refresh audio and devices">
            <RefreshCw size={18} />
          </button>
        </header>
        {runtime?.readOnly && (
          <div className="test-banner">
            READ-ONLY SESSION — real device and audio inventory; audio and HID changes are blocked
          </div>
        )}
        {runtime?.testMode && !runtime.readOnly && (
          <div className="test-banner">TEST SESSION — transport is explicitly isolated for testing</div>
        )}
        {error && (
          <div role="alert" className="error-banner">
            <div>
              <strong>{error.includes("Audio service unavailable") ? "Audio service unavailable" : error}</strong>
              <p>
                {snapshot
                  ? indeterminate.current
                    ? "The service did not respond in time. The displayed state is a snapshot, not confirmation that the change finished. Refresh and verify before retrying."
                    : "The change was not confirmed. Refresh to read the current audio state, then try again."
                  : "Check the matching SSGG service, then refresh to reconnect. Controls stay unavailable until a fresh snapshot is received."}
              </p>
            </div>
          </div>
        )}
        <div className="scene" key={tab}>
          {tab === "settings" ? (
            <Settings runtime={runtime} onRuntime={setRuntime} />
          ) : tab === "profiles" ? (
            <Profiles snapshot={snapshot} busy={busy} mutate={mutate} />
          ) : tab === "devices" ? (
            <Devices
              devices={snapshot?.devices ?? []}
              selected={selectedDevice}
              onSelect={setSelectedDevice}
              physical={snapshot?.physical}
              readOnly={snapshot?.readOnly || runtime?.readOnly}
              saving={busy}
              mutate={mutate}
            />
          ) : (
            <Mixer
              snapshot={snapshot}
              busy={busy}
              loading={loading}
              mutate={mutate}
              selectedDevice={selectedDevice}
              onChooseDevice={() => setTab("devices")}
              readOnly={runtime?.readOnly}
            />
          )}
        </div>
        <div className="save-status" role="status">
          {busy ? "Saving…" : notice}
        </div>
      </main>
    </div>
  );
}
