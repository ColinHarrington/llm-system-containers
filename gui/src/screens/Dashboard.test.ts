import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

// Mock the core bridge so the test is deterministic (no Tauri, no real backend).
vi.mock("../lib/core", () => ({
  vmStatus: vi.fn(async () => "Running"),
  vmUp: vi.fn(async () => {}),
  vmDown: vi.fn(async () => {}),
  listSandboxes: vi.fn(async () => [
    { name: "web-agent-01", status: "Running", image: "dev-ubuntu-24.04", nested: 3 },
    { name: "scratch-01", status: "Stopped", image: "images:alpine/3.21", nested: null },
  ]),
  listServices: vi.fn(async () => [{ name: "litellm", description: "p", priority: "MVP", enabled: true }]),
  listAgents: vi.fn(async () => [{ id: "a", name: "agent-claude", initials: "aC", kind: "agent", sandbox: "web-agent-01", uid: 1001, model: "m", status: "working", task: "t" }]),
  hostResources: vi.fn(async () => ({ cpuUsed: 5, cpuTotal: 8, memUsed: 9, memTotal: 16, diskUsed: 34, diskTotal: 120 })),
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
