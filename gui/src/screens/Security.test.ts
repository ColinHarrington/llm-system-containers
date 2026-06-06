import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { enforceAll } = vi.hoisted(() => ({ enforceAll: vi.fn(async () => []) }));
vi.mock("../lib/core", () => ({
  fleetEnforcement: vi.fn(async () => [
    { sandbox: "web-agent-01", egressPosture: "allowlist", domains: 2, agents: 1, readOnlyAgents: 0, controlPlaneAgents: 1 },
    { sandbox: "scratch", egressPosture: "unmanaged", domains: 0, agents: 0, readOnlyAgents: 0, controlPlaneAgents: 0 },
  ]),
  enforceAll,
}));

import { ui } from "../lib/store.svelte";
import Security from "./Security.svelte";

describe("Security", () => {
  it("shows the per-sandbox posture matrix and opens a sandbox", async () => {
    render(Security);
    expect(await screen.findByText("web-agent-01")).toBeInTheDocument();
    expect(screen.getByText("allowlist")).toBeInTheDocument();
    expect(screen.getByText("unmanaged")).toBeInTheDocument();

    ui.selectedSandbox = null;
    await fireEvent.click(screen.getByText("web-agent-01"));
    expect(ui.selectedSandbox).toBe("web-agent-01");
    expect(ui.screen).toBe("sandbox-detail");
  });

  it("enforces a single managed sandbox and the whole fleet", async () => {
    enforceAll.mockClear();
    render(Security);
    await screen.findByText("web-agent-01");
    // Per-row Enforce shows only for managed sandboxes (web-agent-01, not scratch).
    const rowEnforce = screen.getAllByRole("button", { name: "Enforce" });
    expect(rowEnforce.length).toBe(1);
    await fireEvent.click(rowEnforce[0]);
    expect(enforceAll).toHaveBeenCalledWith("web-agent-01");

    // Fleet enforce hits every managed sandbox (1 here).
    enforceAll.mockClear();
    await fireEvent.click(screen.getByRole("button", { name: /Enforce all \(1\)/ }));
    expect(enforceAll).toHaveBeenCalledWith("web-agent-01");
  });

  it("sorts the matrix when a column header is clicked", async () => {
    const { container } = render(Security);
    await screen.findByText("web-agent-01");
    const rowNames = () => [...container.querySelectorAll("tbody tr")].map((r) => r.querySelector("td")?.textContent ?? "");

    // Sort by Agents ascending → scratch (0 agents) before web-agent-01 (1).
    await fireEvent.click(screen.getByRole("button", { name: /Agents/ }));
    expect(rowNames()[0]).toContain("scratch");
    // Toggle to descending → web-agent-01 first.
    await fireEvent.click(screen.getByRole("button", { name: /Agents/ }));
    expect(rowNames()[0]).toContain("web-agent-01");
  });

  it("filters the matrix by search and posture", async () => {
    render(Security);
    await screen.findByText("web-agent-01");
    // Search narrows to the matching sandbox.
    await fireEvent.input(screen.getByPlaceholderText("Search sandboxes…"), { target: { value: "scratch" } });
    expect(screen.queryByText("web-agent-01")).not.toBeInTheDocument();
    expect(screen.getByText("scratch")).toBeInTheDocument();
    // Managed filter drops the unmanaged one.
    await fireEvent.input(screen.getByPlaceholderText("Search sandboxes…"), { target: { value: "" } });
    await fireEvent.click(screen.getByRole("button", { name: "Managed" }));
    expect(screen.getByText("web-agent-01")).toBeInTheDocument();
    expect(screen.queryByText("scratch")).not.toBeInTheDocument();
  });
});
