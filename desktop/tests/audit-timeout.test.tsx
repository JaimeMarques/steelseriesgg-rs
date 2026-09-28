import { afterEach, expect, it, vi } from "vitest";
import { PassThrough } from "node:stream";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../src/App";
import { JsonLineClient } from "../electron/rpc";
import fixture from "./fixture.json";
import type { DesktopBridge, Snapshot } from "../src/shared/contracts";

afterEach(() => { cleanup(); delete window.ssgg; vi.useRealTimers(); });

it("labels a timed-out mutation indeterminate and accepts a late service completion", async () => {
  vi.useFakeTimers();
  const input = new PassThrough(), output = new PassThrough();
  const lines: { id: number; method: string }[] = [];
  input.on("data", (chunk) => lines.push(JSON.parse(String(chunk))));
  const client = new JsonLineClient(input, output, 20);
  const action = client.request("stream.set", { id: 101, muted: true });
  const id = lines[0].id;
  const rejection = expect(action).rejects.toThrow(/may still complete/i);
  await vi.advanceTimersByTimeAsync(21);
  await rejection;
  output.write(JSON.stringify({ id, result: { ok: true } }) + "\n");
  const read = client.request("state.get", {});
  output.write(JSON.stringify({ id: lines[1].id, result: { muted: true } }) + "\n");
  expect(await read).toEqual({ muted: true });
  client.close();
});

it("reconciles snapshot after mutation timeout without claiming failure or success", async () => {
  const state = structuredClone(fixture) as Snapshot;
  const bridge = {
    getRuntime: vi.fn(async () => ({ connected: true, message: "Ready", trayAvailable: false, closeToTray: false, transport: "socket", testMode: true })),
    getState: vi.fn(async () => structuredClone(state)),
    setStream: vi.fn(async () => {
      state.streams[0].muted = true; // Service may complete while its response is lost.
      throw new Error("Audio service response timed out; change may still complete. Refresh to reconcile current state before retrying.");
    }),
  } as unknown as DesktopBridge;
  window.ssgg = bridge;
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Mute Google Chrome" }));
  expect(await screen.findByRole("button", { name: "Unmute Google Chrome" })).toBeVisible();
  expect(screen.getByRole("alert")).toHaveTextContent(/may still complete/i);
  expect(screen.getByRole("alert")).not.toHaveTextContent(/change failed|was not confirmed/i);
  expect(document.querySelector(".save-status")).not.toHaveTextContent(/saved|updated/i);
  expect(bridge.getState).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole("button", { name: "Refresh audio and devices" }));
  await waitFor(() => expect(bridge.getState).toHaveBeenCalledTimes(3));
});
