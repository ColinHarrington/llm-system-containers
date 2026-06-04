import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { launchSandbox } = vi.hoisted(() => ({ launchSandbox: vi.fn(async () => {}) }));
vi.mock("../lib/core", () => ({
  listSandboxes: vi.fn(async () => [
    { name: "web-agent-01", status: "Running", image: "images:alpine/3.21" },
  ]),
  launchSandbox,
  removeSandbox: vi.fn(async () => {}),
}));

import Sandboxes from "./Sandboxes.svelte";

describe("Sandboxes", () => {
  it("lists sandboxes and launches a new one through the form", async () => {
    render(Sandboxes);
    expect(await screen.findByText("web-agent-01")).toBeInTheDocument();

    await fireEvent.input(screen.getByPlaceholderText(/name/i), {
      target: { value: "web-agent-02" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "Launch" }));

    expect(launchSandbox).toHaveBeenCalledWith("web-agent-02", expect.any(String), false);
  });
});
