import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { adaptSnapshot, toWireCommand } from "../electron/adapter";
import { validateCommand, type Snapshot } from "../src/shared/contracts";
import Mixer from "../src/Mixer";
import fixture from "./fixture.json";

afterEach(cleanup);

it("adapts numeric Rust sink inventory and routes only an explicit bounded sink ID", () => {
  const state = adaptSnapshot({
    ...fixture,
    streams: fixture.streams.map((s) => ({ ...s, id: Number(s.id), sinkId: Number(s.sinkId) })),
    sinks: [
      { id: 51, name: "alsa_output.one", description: "Headphones", volume: 1, muted: false },
      { id: 52, name: "alsa_output.two", description: "Speakers", volume: 0.5, muted: false },
    ],
    mixer: { enabled: false, balance: 0 },
    backend: { connected: true, error: null },
  });
  expect(state.sinks.map((sink) => sink.id)).toEqual(["51", "52"]);
  expect(state.streams[0].sinkId).toBe("51");
  expect(toWireCommand("stream.set", { id: "101", sinkId: "52" })).toEqual({ id: 101, sinkId: 52 });
  expect(toWireCommand("stream.set", { id: "101", sinkId: "4294967295" })).toEqual({ id: 101, sinkId: 4294967295 });
  for (const sinkId of ["-1", "1.5", "01", "4294967296", "../52", 52, null]) {
    expect(() => validateCommand("stream.set", { id: "101", sinkId })).toThrow();
  }
  expect(() => validateCommand("stream.set", { id: "101", sinkId: "52", arbitrary: 1 })).toThrow();
  expect(() => validateCommand("stream.set", { id: "101" })).toThrow();
});

it("shows real output choices and routes one stream without changing its group", async () => {
  const snapshot = adaptSnapshot({
    ...fixture,
    sinks: [
      { id: "51", name: "alsa_output.one", description: "Headphones", volume: 1, muted: false },
      { id: "52", name: "alsa_output.two", description: "Speakers", volume: 1, muted: false },
    ],
  });
  const setStream = vi.fn(async () => {});
  render(
    <Mixer
      snapshot={snapshot}
      busy={false}
      loading={false}
      mutate={async (action) => { await action({ setStream } as never); }}
    />,
  );
  const route = screen.getByRole("combobox", { name: "Google Chrome output" });
  expect(route).toHaveValue("51");
  expect(Array.from((route as HTMLSelectElement).options).find((option) => option.textContent === "Speakers")).toHaveValue("52");
  fireEvent.change(route, { target: { value: "52" } });
  await waitFor(() => expect(setStream).toHaveBeenCalledWith({ id: "101", sinkId: "52" }));
  expect(screen.getByRole("combobox", { name: "Google Chrome group" })).toHaveValue("media");
});

it("cannot route when inventory is empty or the session is read-only", () => {
  const state = adaptSnapshot(fixture) as Snapshot;
  const mutate = vi.fn(async () => {});
  const view = render(<Mixer snapshot={state} busy={false} loading={false} mutate={mutate} />);
  expect(screen.getByRole("combobox", { name: "Google Chrome output" })).toBeDisabled();
  view.rerender(<Mixer snapshot={{ ...state, sinks: [{ id: "52", name: "speaker", description: "Speakers", volume: 1, muted: false }] }} busy={false} loading={false} mutate={mutate} readOnly />);
  expect(screen.getByRole("combobox", { name: "Google Chrome output" })).toBeDisabled();
  expect(mutate).not.toHaveBeenCalled();
});
