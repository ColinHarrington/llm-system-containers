import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect } from "vitest";
import { ui } from "./store.svelte";
import CommandPalette from "./CommandPalette.svelte";

describe("CommandPalette", () => {
  it("filters commands and runs the selected one", async () => {
    ui.paletteOpen = true;
    ui.screen = "dashboard";
    render(CommandPalette);

    const input = await screen.findByPlaceholderText("Type a command…");
    await fireEvent.input(input, { target: { value: "topology" } });

    const item = screen.getByText("Go to Topology");
    expect(item).toBeInTheDocument();
    await fireEvent.click(item);

    expect(ui.screen).toBe("topology");
    expect(ui.paletteOpen).toBe(false);
  });

  it("jumps to a sandbox loaded from the backend", async () => {
    ui.paletteOpen = true;
    ui.selectedSandbox = null;
    render(CommandPalette);
    // Sandboxes load async from listSandboxes (mock data includes web-agent-01).
    const item = await screen.findByText("Open web-agent-01");
    await fireEvent.click(item);
    expect(ui.selectedSandbox).toBe("web-agent-01");
    expect(ui.screen).toBe("sandbox-detail");
  });
});
