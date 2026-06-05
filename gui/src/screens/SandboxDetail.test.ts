import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { removeAgent, instanceRemoveProfile, setAgentGuardrails, setEgressPolicy, applyEgress, applyTetragonPolicies, setWorkspaceReadonly, enforceAll, agentPause } = vi.hoisted(() => ({
  removeAgent: vi.fn(async () => {}),
  instanceRemoveProfile: vi.fn(async () => {}),
  setAgentGuardrails: vi.fn(async () => {}),
  setEgressPolicy: vi.fn(async () => {}),
  applyEgress: vi.fn(async () => 2),
  applyTetragonPolicies: vi.fn(async () => 1),
  setWorkspaceReadonly: vi.fn(async () => 1),
  enforceAll: vi.fn(async () => [
    { ring: "Egress (L3/L4)", state: "enforced", detail: "allowlist · bound" },
  ]),
  agentPause: vi.fn(async () => {}),
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
  egressPolicy: vi.fn(async () => ({ posture: "allowlist", allow: ["llm"], domains: [] })),
  setEgressPolicy,
  applyEgress,
  egressAclPreview: vi.fn(async () => ({
    name: "llmsc-egress-web-agent-01", description: "", usedBy: 0, ingress: [],
    egress: [{ action: "allow", source: "", destination: "10.21.32.0/24", protocol: "tcp", port: "4000", description: "LLM proxy" }],
  })),
  egressStatus: vi.fn(async () => ({
    managed: true, posture: "allowlist", aclName: "llmsc-egress-web-agent-01", aclExists: false, bound: false, inSync: false,
  })),
  tetragonPolicies: vi.fn(async () => [
    { name: "llmsc-web-agent-01-agent-claude", agent: "agent-claude", deniedSyscalls: ["ptrace", "mount"], egressNote: "None except LLM", fsNote: "Read-only everything", readOnly: true },
  ]),
  tetragonPolicyYaml: vi.fn(async () => "kind: TracingPolicy\n"),
  applyTetragonPolicies,
  setWorkspaceReadonly,
  enforcementOverview: vi.fn(async () => [
    { ring: "Egress (L3/L4)", state: "pending", detail: "allowlist · not bound" },
    { ring: "Kernel (Tetragon)", state: "draft", detail: "1 policy(ies) compiled" },
  ]),
  enforceAll,
  agentPause,
  agentResume: vi.fn(async () => {}),
  agentStop: vi.fn(async () => {}),
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
    expect((await screen.findAllByText("agent-claude")).length).toBeGreaterThan(0);
    expect(screen.getByText("colin")).toBeInTheDocument(); // the human operator
    expect(screen.getByText("Users")).toBeInTheDocument();
  });

  it("shows the seed profile as read-only provenance and removes an agent", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findAllByText("agent-claude");

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
    await screen.findAllByText("agent-claude");
    // The agent row has a Guardrails button (the human operator does not).
    await fireEvent.click(screen.getByTitle("Guardrails"));
    await fireEvent.click(screen.getByRole("button", { name: /Save guardrails/ }));
    expect(setAgentGuardrails).toHaveBeenCalledWith("web-agent-01", "agent-claude", expect.any(Object));
  });

  it("shows the enforcement overview and runs enforce-all", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findByText("Enforcement");
    expect(screen.getByText("Egress (L3/L4)")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: /Enforce all/ }));
    expect(enforceAll).toHaveBeenCalledWith("web-agent-01");
  });

  it("shows the egress policy with its compiled ACL and enforces it", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findByText("Network egress");
    // The compiled ACL preview shows the LLM allow rule.
    expect(await screen.findByText("10.21.32.0/24")).toBeInTheDocument();
    // Add a named set → persists via setEgressPolicy.
    await fireEvent.click(screen.getByRole("button", { name: "+ web" }));
    expect(setEgressPolicy).toHaveBeenCalledWith("web-agent-01", expect.objectContaining({ posture: "allowlist" }));
    // Apply (enforce) → applyEgress.
    await fireEvent.click(screen.getByRole("button", { name: /Apply \(enforce\)/ }));
    expect(applyEgress).toHaveBeenCalledWith("web-agent-01");
  });

  it("adds an L7 domain to the egress policy", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findByText("Network egress");
    const input = await screen.findByPlaceholderText("domain (e.g. github.com)");
    await fireEvent.input(input, { target: { value: "github.com" } });
    await fireEvent.click(screen.getByRole("button", { name: "Add domain" }));
    expect(setEgressPolicy).toHaveBeenCalledWith(
      "web-agent-01",
      expect.objectContaining({ domains: ["github.com"] }),
    );
  });

  it("shows the per-agent Tetragon policy and loads it", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findByText("Kernel enforcement");
    expect(screen.getByText("2 syscalls denied")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: /Load policies/ }));
    expect(applyTetragonPolicies).toHaveBeenCalledWith("web-agent-01");
  });

  it("pauses an agent (control-plane action)", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findAllByText("agent-claude");
    await fireEvent.click(screen.getByTitle("Pause agent"));
    expect(agentPause).toHaveBeenCalledWith("web-agent-01", "agent-claude");
  });

  it("sets the workspace mounts read-only", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    await screen.findByText("Workspace mounts");
    await fireEvent.click(screen.getByRole("button", { name: "Read-only" }));
    expect(setWorkspaceReadonly).toHaveBeenCalledWith("web-agent-01", true);
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
