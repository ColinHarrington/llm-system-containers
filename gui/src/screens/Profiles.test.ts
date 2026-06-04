import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

vi.mock("../lib/core", () => ({
  listProfiles: vi.fn(async () => [
    { name: "researcher", summary: "Read & research", filesystem: "RO repo", network: "allowlist", l3: false, llmBudget: "generous", controlPlane: "none" },
    { name: "orchestrator", summary: "Drive other agents", filesystem: "Minimal", network: "None raw", l3: false, llmBudget: "broad", controlPlane: "launch/stop sandboxes" },
  ]),
}));

import Profiles from "./Profiles.svelte";

describe("Profiles", () => {
  it("lists archetypes and flags control-plane ones", async () => {
    render(Profiles);
    expect(await screen.findByText("researcher")).toBeInTheDocument();
    expect(screen.getByText("orchestrator")).toBeInTheDocument();
    // orchestrator carries control-plane capability; researcher does not.
    expect(screen.getByText("control-plane")).toBeInTheDocument();
  });
});
