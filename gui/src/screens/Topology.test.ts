import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

vi.mock("../lib/core", () => ({
  TOOL_LABELS: { code: "Editor", llm: "LLM call", shell: "Terminal" },
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

import Topology from "./Topology.svelte";

describe("Topology", () => {
  it("renders the nested VM -> sandboxes -> agents view", async () => {
    render(Topology);
    expect(await screen.findByText("web-agent-01")).toBeInTheDocument();
    expect(screen.getByText("agent-claude")).toBeInTheDocument();
    expect(screen.getByText("L3 enabled")).toBeInTheDocument();
  });
});
