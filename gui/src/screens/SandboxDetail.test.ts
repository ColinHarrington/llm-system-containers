import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { removeAgent, setAgentProfile } = vi.hoisted(() => ({
  removeAgent: vi.fn(async () => {}),
  setAgentProfile: vi.fn(async () => {}),
}));
vi.mock("../lib/core", () => ({
  removeSandbox: vi.fn(async () => {}),
  removeAgent,
  setAgentProfile,
  listProfiles: vi.fn(async () => [
    { name: "builder", summary: "", filesystem: "", network: "", l3: true, llmBudget: "", controlPlane: "none" },
    { name: "tester", summary: "", filesystem: "", network: "", l3: true, llmBudget: "", controlPlane: "none" },
  ]),
  topology: vi.fn(async () => [
    {
      name: "web-agent-01", image: "dev-ubuntu-24.04", status: "running", l3: true, cpu: "—", mem: "3.4 GB",
      agents: [
        { name: "colin", kind: "human", state: "idle", action: "", tools: [], active: null, profile: null },
        { name: "agent-claude", kind: "agent", state: "idle", action: "", tools: [], active: null, profile: "builder" },
      ],
    },
  ]),
}));

import { fireEvent } from "@testing-library/svelte";
import { ui } from "../lib/store.svelte";
import SandboxDetail from "./SandboxDetail.svelte";

describe("SandboxDetail", () => {
  it("shows the selected sandbox with its users and assigned profiles", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    expect(await screen.findByText("agent-claude")).toBeInTheDocument();
    expect(screen.getByText("colin")).toBeInTheDocument(); // the human operator
    expect(screen.getByText("Users")).toBeInTheDocument();
  });

  it("reassigns an agent's profile and removes the agent", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findByText("agent-claude");

    // Profile dropdown change -> setAgentProfile.
    const select = screen.getByRole("combobox");
    await fireEvent.change(select, { target: { value: "tester" } });
    expect(setAgentProfile).toHaveBeenCalledWith("web-agent-01", "agent-claude", "tester");

    // Remove agent (the human has no remove button, so there's exactly one).
    await fireEvent.click(screen.getByTitle("Remove agent"));
    expect(removeAgent).toHaveBeenCalledWith("web-agent-01", "agent-claude");
  });
});
