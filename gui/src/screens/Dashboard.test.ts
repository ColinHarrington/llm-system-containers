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
  topology: vi.fn(async () => [
    { name: "web-agent-01", image: "dev-ubuntu-24.04", status: "running", l3: true, cpu: "—", mem: "—", agents: [
      { name: "colin", kind: "human", state: "idle", action: "", tools: [], active: null },
      { name: "agent-claude", kind: "agent", state: "idle", action: "", tools: [], active: null },
    ] },
  ]),
  serviceStates: vi.fn(async () => ({ litellm: "running", phoenix: "not-provisioned" })),
  fleetEnforcement: vi.fn(async () => [
    { sandbox: "web-agent-01", egressPosture: "allowlist", domains: 1, agents: 1, readOnlyAgents: 0, controlPlaneAgents: 0 },
  ]),
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

  it("surfaces live service health and security posture", async () => {
    render(Dashboard);
    expect(await screen.findByText("Service health")).toBeInTheDocument();
    expect(screen.getByText("Security posture")).toBeInTheDocument();
    expect(screen.getByText("Managed egress")).toBeInTheDocument();
    // No fabricated "Tetragon active" claim anymore.
    expect(screen.queryByText(/Tetragon eBPF enforcement active/)).not.toBeInTheDocument();
  });
});
