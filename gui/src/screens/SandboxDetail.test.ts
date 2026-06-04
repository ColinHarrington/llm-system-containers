import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

vi.mock("../lib/core", () => ({
  removeSandbox: vi.fn(async () => {}),
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

import { ui } from "../lib/store.svelte";
import SandboxDetail from "./SandboxDetail.svelte";

describe("SandboxDetail", () => {
  it("shows the selected sandbox with its users and assigned profiles", async () => {
    ui.selectedSandbox = "web-agent-01";
    render(SandboxDetail);
    expect(await screen.findByText("agent-claude")).toBeInTheDocument();
    expect(screen.getByText("builder")).toBeInTheDocument(); // assigned profile
    expect(screen.getByText("colin")).toBeInTheDocument(); // the human operator
    expect(screen.getByText("Users")).toBeInTheDocument();
  });
});
