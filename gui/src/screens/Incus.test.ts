import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

// The default tab renders IncusProfiles; mock the core calls it makes.
vi.mock("../lib/core", () => ({
  listIncusProfiles: vi.fn(async () => []),
  starterIncusProfiles: vi.fn(async () => []),
  applyIncusProfile: vi.fn(async () => {}),
}));

import { ui } from "../lib/store.svelte";
import Incus from "./Incus.svelte";

describe("Incus", () => {
  it("renders sub-tabs and switches to the Storage stub", async () => {
    ui.incusTab = "profiles";
    render(Incus);
    for (const t of ["Profiles", "Networks", "Storage", "Images", "Project"]) {
      expect(screen.getByRole("button", { name: t })).toBeInTheDocument();
    }
    await fireEvent.click(screen.getByRole("button", { name: "Storage" }));
    expect(await screen.findByText(/Storage pools/)).toBeInTheDocument();
  });
});
