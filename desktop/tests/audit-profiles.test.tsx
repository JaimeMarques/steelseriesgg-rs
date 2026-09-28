import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { validateCommand } from "../src/shared/contracts";
import Profiles from "../src/Profiles";
import fixture from "./fixture.json";
import type { Snapshot } from "../src/shared/contracts";

afterEach(cleanup);

it("rejects C0, DEL and Unicode C1 control characters at the command boundary", () => {
  for (const name of ["valid\ninvalid", "bad\u0000name", "bad\u007fname", "bad\u0085name"])
    for (const method of ["profiles.save", "profiles.apply"])
      expect(() => validateCommand(method, { name })).toThrow();
  expect(validateCommand("profiles.save", { name: "Mix: Café / 音楽" })).toEqual({ name: "Mix: Café / 音楽" });
});

it("disables invalid profile names before they reach the bridge", () => {
  const mutate = vi.fn(async () => {});
  render(<Profiles snapshot={structuredClone(fixture) as Snapshot} busy={false} mutate={mutate} />);
  const input = screen.getByRole("textbox", { name: "Profile name" });
  const save = screen.getByRole("button", { name: "Save current mix" });
  fireEvent.change(input, { target: { value: "ok\u0085bad" } });
  expect(save).toBeDisabled();
  fireEvent.submit(input.closest("form")!);
  expect(mutate).not.toHaveBeenCalled();
  fireEvent.change(input, { target: { value: "Music & calls" } });
  expect(save).toBeEnabled();
});
