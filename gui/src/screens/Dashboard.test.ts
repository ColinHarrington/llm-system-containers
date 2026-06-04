import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

// Mock the core bridge so the test is deterministic (no Tauri, no real backend).
vi.mock("../lib/core", () => ({
  vmStatus: vi.fn(async () => "Running"),
  vmUp: vi.fn(async () => {}),
  vmDown: vi.fn(async () => {}),
  listSandboxes: vi.fn(async () => [
    { name: "web-agent-01", status: "Running" },
    { name: "scratch-01", status: "Stopped" },
  ]),
}));

import Dashboard from "./Dashboard.svelte";

describe("Dashboard", () => {
  it("shows the VM running (with a Stop control) and lists sandboxes", async () => {
    render(Dashboard);
    expect(await screen.findByText("web-agent-01")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
    expect(screen.getByText("scratch-01")).toBeInTheDocument();
  });
});
