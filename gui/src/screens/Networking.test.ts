import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

vi.mock("../lib/core", () => ({
  networking: vi.fn(async () => ({
    networks: [{ name: "incusbr0", kind: "bridge", ipv4: "10.71.0.1/24", nat: true, usedBy: 1 }],
    sandboxes: [
      { name: "web-agent-01", status: "running", networks: ["incusbr0"], ipv4: "10.71.0.20" },
    ],
  })),
}));

import Networking from "./Networking.svelte";

describe("Networking", () => {
  it("renders real networks and the attachment table", async () => {
    render(Networking);
    expect((await screen.findAllByText("incusbr0")).length).toBeGreaterThan(0);
    expect(screen.getByText("Attachments")).toBeInTheDocument();
    expect(screen.getByText("10.71.0.20")).toBeInTheDocument();
  });
});
