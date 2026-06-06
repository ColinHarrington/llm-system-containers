import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { agentPause } = vi.hoisted(() => ({ agentPause: vi.fn(async () => {}) }));
vi.mock("../lib/core", () => ({
  TOOL_LABELS: { code: "Editor", llm: "LLM call", shell: "Terminal" },
  vmStatus: vi.fn(async () => "Running"),
  listServices: vi.fn(async () => [{ name: "litellm", description: "p", priority: "MVP", enabled: true }]),
  fleetEnforcement: vi.fn(async () => [
    { sandbox: "web-agent-01", egressPosture: "allowlist", domains: 1, agents: 1, readOnlyAgents: 0, controlPlaneAgents: 0 },
  ]),
  agentPause,
  agentResume: vi.fn(async () => {}),
  agentStop: vi.fn(async () => {}),
  topology: vi.fn(async () => [
    {
      name: "web-agent-01", image: "dev-ubuntu-24.04", status: "running", l3: true, cpu: "2.1", mem: "3.4 GB",
      agents: [
        { name: "agent-claude", kind: "agent", state: "active", action: "Editing", tools: ["code", "llm"], active: "code" },
      ],
    },
    { name: "scratch-01", image: "x", status: "stopped", l3: false, cpu: "0", mem: "0", agents: [] },
  ]),
}));

import { ui } from "../lib/store.svelte";
import Topology from "./Topology.svelte";

describe("Topology", () => {
  it("renders the nested VM -> sandboxes -> agents view with live posture", async () => {
    render(Topology);
    expect(await screen.findByText("web-agent-01")).toBeInTheDocument();
    expect(screen.getByText("agent-claude")).toBeInTheDocument();
    expect(screen.getByText("L3 enabled")).toBeInTheDocument();
    expect(screen.getByText("allowlist")).toBeInTheDocument(); // egress posture badge
  });

  it("pauses an agent inline and opens steer", async () => {
    render(Topology);
    await screen.findByText("agent-claude");
    await fireEvent.click(screen.getByTitle("Pause"));
    expect(agentPause).toHaveBeenCalledWith("web-agent-01", "agent-claude");
    await fireEvent.click(screen.getByTitle("Steer"));
    expect(ui.steerAgent?.name).toBe("agent-claude");
    expect(ui.steerAgent?.sandbox).toBe("web-agent-01");
  });
});
