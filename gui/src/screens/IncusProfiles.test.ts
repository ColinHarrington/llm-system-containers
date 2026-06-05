import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

vi.mock("../lib/core", () => ({
  listIncusProfiles: vi.fn(async () => [
    { name: "default", description: "Default", usedBy: 2, config: {}, devices: { eth0: { type: "nic", network: "incusbr0" } } },
    { name: "nesting", description: "L3", usedBy: 1, config: { "security.nesting": "true" }, devices: {} },
  ]),
}));

import IncusProfiles from "./IncusProfiles.svelte";

describe("IncusProfiles", () => {
  it("lists project profiles with their config/devices", async () => {
    render(IncusProfiles);
    expect(await screen.findByText("default")).toBeInTheDocument();
    expect(screen.getByText("nesting")).toBeInTheDocument();
    expect(screen.getByText("security.nesting")).toBeInTheDocument();
    expect(screen.getByText("eth0")).toBeInTheDocument();
  });
});
