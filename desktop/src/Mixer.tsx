import {
  ArrowRight,
  Volume2,
  VolumeX,
  Music2,
  MessageCircle,
  Gamepad2,
  SlidersHorizontal,
  AppWindow,
} from "lucide-react";
import type { Snapshot, DesktopBridge, Stream, Group } from "./shared/contracts";
import { Range } from "./Range";
import WheelInput from "./WheelInput";
export type Mutate = (action: (bridge: DesktopBridge) => Promise<unknown>, message?: string) => Promise<void>;
const groupNames = { game: "Game", chat: "Chat", media: "Media", unmanaged: "Unmanaged" };
export function appLabel(stream: Stream) {
  if (/^(WEBRTC VoiceEngine|AudioStream|unknown)?$/i.test(stream.appName.trim())) {
    try {
      const key = JSON.parse(stream.appKey);
      if (Array.isArray(key) && typeof key[0] === "string" && key[0].trim())
        return key[0].split("/").pop() || stream.appName;
    } catch {
      /* No reliable executable identity: preserve the native label. */
    }
  }
  return stream.appName || "Unnamed application";
}
const GroupIcon = ({ id }: { id: Group["id"] }) =>
  id === "chat" ? <MessageCircle size={20} /> : id === "media" ? <Music2 size={20} /> : <Gamepad2 size={20} />;
export default function Mixer({
  snapshot,
  busy,
  loading,
  mutate,
  selectedDevice = "",
  onChooseDevice,
  readOnly = false,
}: {
  snapshot: Snapshot | null;
  busy: boolean;
  loading: boolean;
  mutate: Mutate;
  selectedDevice?: string;
  onChooseDevice?: () => void;
  readOnly?: boolean;
}) {
  const enabled = !!snapshot?.chatmix.enabled;
  const ready = !!snapshot?.audio.available && !snapshot.readOnly && !busy;
  const sideLabel = (side: "a" | "b") =>
    snapshot?.groups
      .filter((g) => g.wheelSide === side)
      .map((g) => g.name)
      .join(" + ") || "No group assigned";
  const balance = snapshot?.chatmix.balance ?? 0;
  const hardware = snapshot?.chatmix.inputMode === "hardware";
  const sample = snapshot?.chatmix.wheelAvailable ? snapshot.physical?.sample : null;
  const sinks = snapshot?.sinks ?? [];
  return (
    <>
      <section className={"balance-panel " + (enabled ? "mix-enabled" : "")} aria-label="ChatMix">
        <div className="section-heading">
          <div>
            <h2>ChatMix</h2>
            <p>Two sides. The mix you choose.</p>
          </div>
          <button
            className={enabled ? "" : "primary"}
            disabled={!ready}
            onClick={() =>
              void mutate((b) => b.setChatmix({ enabled: !enabled }), enabled ? "ChatMix disabled" : "ChatMix enabled")
            }
          >
            {enabled ? "Disable ChatMix" : "Enable ChatMix"}
            <ArrowRight size={16} />
          </button>
        </div>
        <WheelInput
          snapshot={snapshot}
          busy={busy}
          mutate={mutate}
          selectedDevice={selectedDevice}
          onChooseDevice={onChooseDevice}
          readOnly={readOnly}
        />
        <div className="balance-control">
          <div className="balance-labels">
            <div>
              <span className="side-token">A</span>
              <strong>{sideLabel("a")}</strong>
            </div>
            <span className="balance-readout">
              {hardware
                ? sample
                  ? `A ${sample.gamePercent}% · B ${sample.chatPercent}%`
                  : "Wheel unavailable"
                : balance === 0
                  ? "Centered"
                  : `${Math.round(Math.abs(balance) * 100)}% toward ${balance < 0 ? "A" : "B"}`}
            </span>
            <div>
              <strong>{sideLabel("b")}</strong>
              <span className="side-token">B</span>
            </div>
          </div>
          {(!hardware || sample) && (
            <div className="balance-track">
              <Range
                label="ChatMix balance"
                min={-1}
                value={hardware ? sample!.balance : balance}
                disabled={!ready || !enabled || hardware}
                onCommit={(balance) => void mutate((b) => b.setChatmix({ balance }), "Balance updated")}
              />
              <span className="center-mark" />
            </div>
          )}
          <div className="balance-ticks">
            <span>Only side A</span>
            <button
              className="text-button"
              disabled={!ready || !enabled || hardware || balance === 0}
              onClick={() => void mutate((b) => b.setChatmix({ balance: 0 }), "Balance centered")}
            >
              Equal balance
            </button>
            <span>Only side B</span>
          </div>
        </div>
        <p className="helper">
          {snapshot?.readOnly
            ? "Read-only session — mix controls are blocked."
            : enabled
              ? "Your base volumes stay separate from ChatMix attenuation. Disable ChatMix to restore the full mix."
              : "ChatMix disabled. Assign applications and choose each group’s ChatMix side, then enable. Group gain and native stream volume work independently."}
        </p>
      </section>
      {snapshot && !snapshot.audio.available && (
        <p className="inline-warning">
          {snapshot.audio.reason || "Native audio is unavailable. Check your PipeWire or PulseAudio session."}
        </p>
      )}
      {snapshot?.groups.length ? (
        <section className="channels" aria-label="Group channels">
          {snapshot.groups.map((group) => (
            <div className={"channel channel-" + group.id} key={group.id}>
              <div className="channel-heading">
                <span className="channel-icon">
                  <GroupIcon id={group.id} />
                </span>
                <h3>{group.name}</h3>
                <span className={"side-badge " + (group.wheelSide === "none" ? "unassigned" : "")}>
                  {group.wheelSide === "none" ? "No side" : `Side ${group.wheelSide.toUpperCase()}`}
                </span>
              </div>
              <div className="channel-body">
                <div className="channel-detail">
                  <label>
                    ChatMix side
                    <select
                      aria-label={`${group.name} ChatMix side`}
                      value={group.wheelSide}
                      disabled={!ready}
                      onChange={(e) =>
                        void mutate(
                          (b) => b.setGroup({ id: group.id, wheelSide: e.target.value as Group["wheelSide"] }),
                          "ChatMix side updated",
                        )
                      }
                    >
                      <option value="none">Not balanced</option>
                      <option value="a">A · first side</option>
                      <option value="b">B · second side</option>
                    </select>
                  </label>
                  <p>
                    {snapshot.streams
                      .filter((s) => s.group === group.id)
                      .map(appLabel)
                      .join(", ") || "No applications assigned"}
                  </p>
                  <button
                    className={"mute-button " + (group.muted ? "is-muted" : "")}
                    aria-label={`${group.muted ? "Unmute" : "Mute"} ${group.name} group`}
                    aria-pressed={group.muted}
                    disabled={!ready}
                    onClick={() => void mutate((b) => b.setGroup({ id: group.id, muted: !group.muted }))}
                  >
                    {group.muted ? <VolumeX size={16} /> : <Volume2 size={16} />} {group.muted ? "Muted" : "Mute"}
                  </button>
                </div>
                <div className="fader-column">
                  <output>
                    {Math.round(group.volume * 100)}
                    <small>%</small>
                  </output>
                  <Range
                    label={`${group.name} group volume`}
                    value={group.volume}
                    disabled={!ready}
                    vertical
                    onCommit={(volume) =>
                      void mutate((b) => b.setGroup({ id: group.id, volume }), "Group volume updated")
                    }
                  />
                </div>
              </div>
            </div>
          ))}
        </section>
      ) : (
        <section className="empty-state">
          <SlidersHorizontal size={32} />
          <h2>{loading ? "Connecting to your audio…" : "Your channels will appear here"}</h2>
          <p>
            SSGG reads active application streams from your local audio service. No synthetic devices or audio levels.
          </p>
        </section>
      )}
      {snapshot && (
        <section className="apps-section">
          <style>{`
            .app-table-head, .app-row { grid-template-columns: minmax(130px, 1fr) minmax(90px, 115px) minmax(110px, 150px) minmax(125px, 180px) 36px; }
            .app-row > select { min-width: 0; }
            @media (max-width: 1100px) { .app-table-head, .app-row { grid-template-columns: minmax(110px, 1fr) 100px 110px minmax(100px, 135px) 30px; gap: 10px; } }
            @media (max-width: 850px) {
              .app-table-head, .app-row { grid-template-columns: minmax(110px, 1fr) 100px 30px; }
              .app-table-head > span:nth-child(3), .app-table-head > span:nth-child(4) { display: none; }
              .app-row > select:nth-child(3) { grid-column: 1 / 3; grid-row: 2; }
              .app-row .app-volume { grid-row: 3; }
              .app-row .icon-button { grid-column: 3; grid-row: 1; }
            }
          `}</style>
          <div className="section-heading">
            <div>
              <h2>
                Applications <span className="count">{snapshot.streams.length}</span>
              </h2>
              <p>Native stream volume is separate from group gain and ChatMix attenuation.</p>
            </div>
            <span className="quiet-label">Native audio streams</span>
          </div>
          {snapshot.streams.length ? (
            <div className="app-table">
              <div className="app-table-head">
                <span>Application / stream</span>
                <span>Group</span>
                <span>Output</span>
                <span>Volume</span>
                <span className="sr-only">Mute</span>
              </div>
              {snapshot.streams.map((stream) => (
                <div className={"app-row " + (stream.muted ? "muted-row" : "")} key={stream.id}>
                  <div className="app-identity">
                    <span className="app-icon">
                      <AppWindow size={19} />
                    </span>
                    <div>
                      <strong>{appLabel(stream)}</strong>
                      <small>
                        {appLabel(stream) !== stream.appName ? `${stream.appName} · ${stream.name}` : stream.name}
                      </small>
                    </div>
                  </div>
                  <select
                    aria-label={`${appLabel(stream)} group`}
                    value={stream.group}
                    disabled={!ready}
                    onChange={(e) =>
                      void mutate(
                        (b) => b.setStream({ id: stream.id, group: e.target.value as Stream["group"] }),
                        "Application assigned",
                      )
                    }
                  >
                    {Object.entries(groupNames).map(([id, name]) => (
                      <option key={id} value={id}>
                        {name}
                      </option>
                    ))}
                  </select>
                  <select
                    aria-label={`${appLabel(stream)} output`}
                    value={sinks.some((sink) => sink.id === stream.sinkId) ? stream.sinkId ?? "" : ""}
                    disabled={!ready || readOnly || sinks.length === 0}
                    onChange={(e) =>
                      void mutate((b) => b.setStream({ id: stream.id, sinkId: e.target.value }), "Application output updated")
                    }
                  >
                    {!sinks.some((sink) => sink.id === stream.sinkId) && (
                      <option value="">{sinks.length ? "Current output unavailable" : "No outputs available"}</option>
                    )}
                    {sinks.map((sink) => (
                      <option key={sink.id} value={sink.id}>{sink.description || sink.name}</option>
                    ))}
                  </select>
                  <div className="app-volume">
                    <Range
                      label={`${appLabel(stream)} volume`}
                      value={Math.min(1, stream.volume)}
                      disabled={!ready}
                      onCommit={(volume) =>
                        void mutate((b) => b.setStream({ id: stream.id, volume }), "Application volume updated")
                      }
                    />
                    <output className="native-volume">
                      Native{"\u00a0"}
                      {Math.round((stream.effectiveVolume ?? stream.volume) * 100)}% · Base{"\u00a0"}
                      {Math.round(stream.volume * 100)}%{stream.effectiveMuted ? " · Muted" : ""}
                    </output>
                  </div>
                  <button
                    className={"icon-button " + (stream.muted ? "is-muted" : "")}
                    disabled={!ready}
                    aria-label={`${stream.muted ? "Unmute" : "Mute"} ${appLabel(stream)}`}
                    aria-pressed={stream.muted}
                    onClick={() => void mutate((b) => b.setStream({ id: stream.id, muted: !stream.muted }))}
                  >
                    {stream.muted ? <VolumeX size={17} /> : <Volume2 size={17} />}
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <div className="stream-empty">
              <Music2 size={22} />
              <p>No active audio streams. Play music or join a call and the application will appear here.</p>
            </div>
          )}
          <p className="helper app-footnote">
            A browser reports its combined audio stream. SSGG cannot separate individual tabs that the browser has
            already mixed.
          </p>
        </section>
      )}
    </>
  );
}
