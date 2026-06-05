import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { applyIncusProfile } = vi.hoisted(() => ({ applyIncusProfile: vi.fn(async () => {}) }));
vi.mock("../lib/core", () => ({
  listIncusProfiles: vi.fn(async () => [
    { name: "default", description: "Default", usedBy: 2, config: {}, devices: { eth0: { type: "nic", network: "incusbr0" } } },
    { name: "nesting", description: "L3", usedBy: 1, config: { "security.nesting": "true" }, devices: {} },
  ]),
  starterIncusProfiles: vi.fn(async () => [
    { name: "sandbox", description: "base", usedBy: 0, config: { "security.privileged": "false" }, devices: {} },
    { name: "nesting", description: "L3", usedBy: 0, config: {}, devices: {} },
  ]),
  applyIncusProfile,
}));

import IncusProfiles from "./IncusProfiles.svelte";

describe("IncusProfiles", () => {
  it("lists project profiles and recommends + applies missing starters", async () => {
    render(IncusProfiles);
    expect(await screen.findByText("default")).toBeInTheDocument();
    expect(screen.getByText("security.nesting")).toBeInTheDocument();
    expect(screen.getByText("eth0")).toBeInTheDocument();

    // "sandbox" starter isn't in the project yet → it shows in Recommended with an Apply button.
    expect(await screen.findByText("sandbox")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: /Apply/ }));
    expect(applyIncusProfile).toHaveBeenCalledWith("sandbox");
  });
});
