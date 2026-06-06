import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { agentStop } = vi.hoisted(() => ({ agentStop: vi.fn(async () => {}) }));
vi.mock("../lib/core", () => ({
  topology: vi.fn(async () => [
    {
      name: "web-agent-01", image: "x", status: "running", l3: true, cpu: "—", mem: "—",
      agents: [
        { name: "colin", kind: "human", state: "idle", action: "", tools: [], active: null, profile: null, guardrails: null },
        { name: "agent-claude", kind: "agent", state: "idle", action: "", tools: [], active: null, profile: "builder",
          guardrails: { filesystem: "RW repo", network: "Registry allowlist", l3: true, llmBudget: "medium", controlPlane: "none" } },
      ],
    },
  ]),
  agentPause: vi.fn(async () => {}),
  agentResume: vi.fn(async () => {}),
  agentStop,
}));

import { ui } from "../lib/store.svelte";
import Agent from "./Agent.svelte";

describe("Agent control", () => {
  it("lists real agents (humans excluded), shows guardrails, and stops one", async () => {
    render(Agent);
    expect((await screen.findAllByText("agent-claude")).length).toBeGreaterThan(0);
    expect(screen.queryByText("colin")).not.toBeInTheDocument(); // human excluded
    // Real guardrails surfaced (no fabricated trace/tokens).
    expect(screen.getByText("Registry allowlist")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: /Stop/ }));
    expect(agentStop).toHaveBeenCalledWith("web-agent-01", "agent-claude");
  });

  it("opens the steer modal", async () => {
    render(Agent);
    await screen.findAllByText("agent-claude");
    await fireEvent.click(screen.getByRole("button", { name: /Steer/ }));
    expect(ui.steerAgent?.name).toBe("agent-claude");
  });
});
