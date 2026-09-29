import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import Devices from "../src/Devices";
import fixture from "./fixture.json";
import type { DesktopBridge, Device } from "../src/shared/contracts";

afterEach(() => {
  cleanup();
  delete window.ssgg;
});

it("uses keyboard-specific provenance and capability copy for the exact Gen 3 model", () => {
  window.ssgg = { getArtwork: vi.fn(async () => null) } as unknown as DesktopBridge;
  const keyboard = {
    ...structuredClone(fixture.devices[0]),
    id: "1038:1642:fixture",
    name: "Apex Pro TKL Gen 3",
    productId: 0x1642,
    kind: "keyboard",
    capabilities: { rgb: { supported: true, applicable: true, reason: "Source supported; hardware test pending" } },
  } as Device;
  render(<Devices devices={[keyboard]} selected={keyboard.id} onSelect={vi.fn()} readOnly />);
  expect(screen.getByText(/keyboard layout/i)).toBeVisible();
  expect(screen.getByText(/manufacturer.*image CDN/i)).toBeVisible();
  expect(screen.queryByText(/Battery, ChatMix input, sidetone and auto-off/)).not.toBeInTheDocument();
  expect(screen.queryByText(/receiver detection is not command verification/)).not.toBeInTheDocument();
});
