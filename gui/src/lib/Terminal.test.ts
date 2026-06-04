import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect } from "vitest";
import { ui, openTerminal } from "./store.svelte";
import Terminal from "./Terminal.svelte";

describe("Terminal", () => {
  it("opens for a target and closes", async () => {
    openTerminal("operator@web-agent-01");
    render(Terminal);
    expect(await screen.findByText("operator@web-agent-01")).toBeInTheDocument();
    expect(screen.getByText("Welcome to web-agent-01 — dev-ubuntu-24.04")).toBeInTheDocument();

    await fireEvent.click(screen.getByText("✕ close"));
    expect(ui.terminalTarget).toBeNull();
  });
});
