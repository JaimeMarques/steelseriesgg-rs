import { afterEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { readFileSync } from "node:fs";
import App from "../src/App";
import fixture from "./fixture.json";
import type { DesktopBridge, Snapshot } from "../src/shared/contracts";

const runtime = { connected: true, message: "Ready", trayAvailable: false, closeToTray: false, transport: "socket" as const, testMode: true };
function setup(hardware = true) {
  const state = structuredClone(fixture) as Snapshot;
  if (hardware) {
    state.chatmix.enabled = true;
    state.chatmix.inputMode = "hardware";
    state.chatmix.wheelAvailable = true;
    state.physical = {
      deviceId: state.devices[0].id, hardwareEnabled: true, hardwareAcquired: true,
      connected: true, battery: null, statusAtMs: Date.now(), stale: false,
      pending: false, sidetone: null, autoOffMinutesSent: null,
      lastCommand: null, error: null,
      sample: { gamePercent: 100, chatPercent: 100, balance: 0, receivedAtMs: Date.now() },
    };
  }
  const bridge = {
    getRuntime: vi.fn(async () => runtime),
    getState: vi.fn(async () => structuredClone(state)),
    getArtwork: vi.fn(async () => null),
    setStream: vi.fn(async ({ id, muted }: { id: string; muted: boolean }) => {
      state.streams.find((stream) => stream.id === id)!.muted = muted;
    }),
  } as unknown as DesktopBridge;
  window.ssgg = bridge;
  return { state, bridge };
}

afterEach(() => {
  cleanup();
  delete window.ssgg;
  vi.useRealTimers();
  vi.restoreAllMocks();
});

it("reflects a physical wheel sample within 250ms while Mixer is visible without re-fetching runtime", async () => {
  const { state, bridge } = setup();
  render(<App />);
  expect(await screen.findByText("A 100% · B 100%")).toBeVisible();
  const runtimeReads = vi.mocked(bridge.getRuntime).mock.calls.length;
  vi.useFakeTimers();
  fireEvent.click(screen.getByRole("button", { name: "Devices" }));
  fireEvent.click(screen.getByRole("button", { name: "Mixer" }));
  state.physical!.sample!.chatPercent = 42;
  await act(async () => { await vi.advanceTimersByTimeAsync(250); });
  expect(screen.getByText("A 100% · B 42%")).toBeVisible();
  expect(bridge.getRuntime).toHaveBeenCalledTimes(runtimeReads);
});

it("keeps wheel snapshots flowing when the slower runtime poll would stall", async () => {
  const { state, bridge } = setup();
  vi.useFakeTimers();
  render(<App />);
  await act(async () => { await Promise.resolve(); });
  expect(screen.getByText("A 100% · B 100%")).toBeVisible();
  vi.mocked(bridge.getRuntime).mockImplementation(() => new Promise(() => {}));
  await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
  state.physical!.sample!.chatPercent = 37;
  await act(async () => { await vi.advanceTimersByTimeAsync(250); });
  expect(screen.getByText("A 100% · B 37%")).toBeVisible();
});

it("bounds fast reads to a visible active physical Mixer and resumes after visibility returns", async () => {
  const { bridge } = setup();
  render(<App />);
  await screen.findByText("A 100% · B 100%");
  vi.useFakeTimers();
  fireEvent.click(screen.getByRole("button", { name: "Devices" }));
  const reads = vi.mocked(bridge.getState).mock.calls.length;
  await act(async () => { await vi.advanceTimersByTimeAsync(800); });
  expect(bridge.getState).toHaveBeenCalledTimes(reads);
  fireEvent.click(screen.getByRole("button", { name: "Mixer" }));
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
  fireEvent(document, new Event("visibilitychange"));
  await act(async () => { await vi.advanceTimersByTimeAsync(800); });
  expect(bridge.getState).toHaveBeenCalledTimes(reads);
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
  fireEvent(document, new Event("visibilitychange"));
  await act(async () => { await vi.advanceTimersByTimeAsync(250); });
  expect(vi.mocked(bridge.getState).mock.calls.length).toBeGreaterThan(reads);
});

it("does not overlap a slow read or let stale wheel snapshots overwrite a completed mutation", async () => {
  const { state, bridge } = setup();
  render(<App />);
  await screen.findByText("A 100% · B 100%");
  let release!: (value: Snapshot) => void;
  const slow = new Promise<Snapshot>((resolve) => { release = resolve; });
  vi.mocked(bridge.getState).mockImplementationOnce(() => slow);
  vi.useFakeTimers();
  fireEvent.click(screen.getByRole("button", { name: "Devices" }));
  fireEvent.click(screen.getByRole("button", { name: "Mixer" }));
  await act(async () => { await vi.advanceTimersByTimeAsync(200); });
  const reads = vi.mocked(bridge.getState).mock.calls.length;
  await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
  expect(bridge.getState).toHaveBeenCalledTimes(reads);
  fireEvent.click(screen.getByRole("button", { name: "Mute Google Chrome" }));
  await act(async () => { await Promise.resolve(); });
  expect(bridge.setStream).toHaveBeenCalledWith({ id: "101", muted: true });
  expect(screen.getByRole("button", { name: "Unmute Google Chrome" })).toBeVisible();
  await act(async () => { release(structuredClone(fixture) as Snapshot); });
  expect(screen.getByRole("button", { name: "Unmute Google Chrome" })).toBeVisible();
  expect(state.streams[0].muted).toBe(true);
});

it("restarts the device scene only when the exact selected device changes, not on snapshots", async () => {
  const { state, bridge } = setup(false);
  state.devices.push({ ...structuredClone(state.devices[0]), id: "second-receiver", name: "Second headset" });
  render(<App />);
  await screen.findByRole("combobox", { name: "Google Chrome group" });
  fireEvent.click(screen.getByRole("button", { name: "Devices" }));
  const first = document.querySelector(".scene")!;
  expect(screen.getByRole("heading", { name: "Arctis Nova 7 Gen 2" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Refresh audio and devices" }));
  await waitFor(() => expect(bridge.getState).toHaveBeenCalledTimes(2));
  expect(document.querySelector(".scene")).toBe(first);
  fireEvent.change(screen.getByLabelText("Selected device"), { target: { value: "second-receiver" } });
  expect(document.querySelector(".scene")).not.toBe(first);
  expect(screen.getByRole("heading", { name: "Second headset" })).toBeVisible();
  expect(bridge.getState).toHaveBeenCalledTimes(2);
});

it("disables scene motion for reduced motion and suspends animation while hidden", () => {
  const css = readFileSync("src/console.css", "utf8");
  expect(css).toMatch(/@media\s*\(prefers-reduced-motion:\s*reduce\)[\s\S]*?\.scene,[\s\S]*?animation:\s*none/);
  expect(css).toMatch(/\[data-page-visible="false"\][\s\S]*?animation-play-state:\s*paused/);
});
