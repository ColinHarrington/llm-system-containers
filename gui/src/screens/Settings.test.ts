import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { saveSettings } = vi.hoisted(() => ({ saveSettings: vi.fn(async () => {}) }));
vi.mock("../lib/core", () => ({
  getSettings: vi.fn(async () => ({ operator: "colin", vmName: "llmsc", cpus: 4, memoryGib: 8, diskGib: 60 })),
  saveSettings,
  vmStatus: vi.fn(async () => "Running"),
  vmDestroy: vi.fn(async () => {}),
}));

import Settings from "./Settings.svelte";

describe("Settings", () => {
  it("loads settings, enables Save only when dirty, and saves", async () => {
    render(Settings);
    const op = (await screen.findByLabelText("Default operator username")) as HTMLInputElement;
    expect(op.value).toBe("colin");

    // Pristine → Save disabled.
    const save = screen.getByRole("button", { name: /Save settings/ });
    expect(save).toBeDisabled();

    await fireEvent.input(op, { target: { value: "ada" } });
    await waitFor(() => expect(save).toBeEnabled());
    await fireEvent.click(save);
    expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ operator: "ada" }));
  });
});
