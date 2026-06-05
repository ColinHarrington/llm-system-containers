import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { removeAgent, instanceRemoveProfile, setAgentGuardrails } = vi.hoisted(() => ({
  removeAgent: vi.fn(async () => {}),
  instanceRemoveProfile: vi.fn(async () => {}),
  setAgentGuardrails: vi.fn(async () => {}),
}));
vi.mock("../lib/core", () => ({
  removeSandbox: vi.fn(async () => {}),
  removeAgent,
  instanceConfig: vi.fn(async () => ({
    name: "web-agent-01", status: "running", description: "", ephemeral: false,
    profiles: ["default"], config: { "security.nesting": "true" },
    devices: { work: { type: "disk", source: "~/proj", path: "/work" } },
    localDevices: ["work"],
  })),
  instanceSetConfig: vi.fn(async () => {}),
  instanceUnsetConfig: vi.fn(async () => {}),
  instanceAddMount: vi.fn(async () => {}),
  instanceRemoveDevice: vi.fn(async () => {}),
  instanceAddProfile: vi.fn(async () => {}),
  instanceRemoveProfile,
  applySandbox: vi.fn(async () => 0),
  instanceYaml: vi.fn(async () => "profiles:\n- default\n"),
  listSnapshots: vi.fn(async () => []),
  snapshotCreate: vi.fn(async () => {}),
  snapshotRestore: vi.fn(async () => {}),
  snapshotDelete: vi.fn(async () => {}),
  setAgentGuardrails,
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

  it("shows the seed profile as read-only provenance and removes an agent", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findByText("agent-claude");

    // Profile is shown as provenance ("from builder"), not an editable control.
    expect(screen.getByText("from builder")).toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();

    // Remove agent (the human has no remove button, so there's exactly one).
    await fireEvent.click(screen.getByTitle("Remove agent"));
    expect(removeAgent).toHaveBeenCalledWith("web-agent-01", "agent-claude");
  });

  it("refines an agent's guardrails", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findByText("agent-claude");
    // The agent row has a Guardrails button (the human operator does not).
    await fireEvent.click(screen.getByTitle("Guardrails"));
    await fireEvent.click(screen.getByRole("button", { name: /Save guardrails/ }));
    expect(setAgentGuardrails).toHaveBeenCalledWith("web-agent-01", "agent-claude", expect.any(Object));
  });

  it("edits the live Incus surface (remove a profile)", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findByText("Incus configuration");
    // The one profile chip ("default") has a remove (×) button.
    await fireEvent.click(screen.getByTitle("Remove profile"));
    expect(instanceRemoveProfile).toHaveBeenCalledWith("web-agent-01", "default");
  });
});
