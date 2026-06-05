import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { provisionService, syncVirtualKeys } = vi.hoisted(() => ({
  provisionService: vi.fn(async () => {}),
  syncVirtualKeys: vi.fn(async () => 1),
}));
vi.mock("../lib/core", () => ({
  listServices: vi.fn(async () => [
    { name: "litellm", description: "LLM proxy", priority: "MVP", enabled: true },
    { name: "phoenix", description: "Observability", priority: "MVP", enabled: false },
  ]),
  setService: vi.fn(async () => {}),
  provisionService,
  syncVirtualKeys,
  listVirtualKeys: vi.fn(async () => [
    { key: "llmsc-web-agent-01-agent-claude", assignedTo: "agent-claude @ web-agent-01", models: "all", budget: "$100 / 30d", used: "—", status: "planned" },
  ]),
  DEPLOYABLE_SERVICES: new Set(["litellm"]),
  SERVICE_META: { litellm: { initials: "Li", color: "#000", placement: "own L2 container" } },
}));

import Services from "./Services.svelte";

describe("Services", () => {
  it("provisions an enabled deployable service", async () => {
    render(Services);
    // litellm is enabled + deployable -> Provision button shows; phoenix is not enabled -> none.
    const provision = await screen.findByRole("button", { name: "Provision" });
    expect(provision).toBeInTheDocument();

    await fireEvent.click(provision);
    expect(provisionService).toHaveBeenCalledWith("litellm");
  });

  it("syncs compiled virtual keys to the proxy", async () => {
    render(Services);
    expect(await screen.findByText("llmsc-web-agent-01-agent-claude")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: /Sync keys/ }));
    expect(syncVirtualKeys).toHaveBeenCalled();
  });
});
