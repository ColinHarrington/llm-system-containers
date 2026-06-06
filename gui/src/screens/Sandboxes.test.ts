import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { removeSandbox } = vi.hoisted(() => ({ removeSandbox: vi.fn(async () => {}) }));
vi.mock("../lib/core", () => ({
  listSandboxes: vi.fn(async () => [
    { name: "web-agent-01", status: "Running", image: "dev-ubuntu-24.04", tags: ["unprivileged"], users: [], nested: 3, cpuCores: 2, memUsed: 3, memTotal: 4 },
    { name: "scratch-01", status: "Stopped", image: "images:alpine/3.21", tags: ["ephemeral"], users: [], nested: null, cpuCores: 1, memUsed: 0, memTotal: 2 },
  ]),
  removeSandbox,
}));

import { ui, resolveConfirm } from "../lib/store.svelte";
import { listSandboxes } from "../lib/core";
import Sandboxes from "./Sandboxes.svelte";

describe("Sandboxes", () => {
  it("lists sandboxes as cards and filters by status", async () => {
    render(Sandboxes);
    expect(await screen.findByText("web-agent-01")).toBeInTheDocument();
    expect(screen.getByText("scratch-01")).toBeInTheDocument();

    // Filter to Running -> the stopped one drops out.
    await fireEvent.click(screen.getByRole("button", { name: "Running" }));
    expect(screen.getByText("web-agent-01")).toBeInTheDocument();
    expect(screen.queryByText("scratch-01")).not.toBeInTheDocument();
  });

  it("shows skeletons while the first fetch is in flight", async () => {
    vi.mocked(listSandboxes).mockReturnValueOnce(new Promise(() => {})); // never resolves
    render(Sandboxes);
    expect((await screen.findAllByTestId("skeleton")).length).toBeGreaterThan(0);
    expect(screen.queryByText("web-agent-01")).not.toBeInTheDocument();
  });

  it("removes a sandbox after confirming", async () => {
    render(Sandboxes);
    const removeButtons = await screen.findAllByRole("button", { name: "Remove" });
    await fireEvent.click(removeButtons[0]);
    // Gated by a confirm dialog.
    expect(ui.confirm?.title).toBe("Remove sandbox");
    resolveConfirm(true);
    await waitFor(() => expect(removeSandbox).toHaveBeenCalledWith("web-agent-01"));
  });

  it("does not remove when the confirm is cancelled", async () => {
    removeSandbox.mockClear();
    render(Sandboxes);
    const removeButtons = await screen.findAllByRole("button", { name: "Remove" });
    await fireEvent.click(removeButtons[0]);
    resolveConfirm(false);
    await Promise.resolve();
    expect(removeSandbox).not.toHaveBeenCalled();
  });
});
