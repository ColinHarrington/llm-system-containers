import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

vi.mock("../lib/core", () => ({
  networking: vi.fn(async () => [
    {
      name: "web-agent-01", image: "dev-ubuntu-24.04", profile: "standard",
      nets: ["svc-net", "egress-net"], inspected: true, llm: "LiteLLM",
      uids: [{ uid: "agent-claude", kind: "agent", egress: "allowlist (github)" }],
    },
  ]),
}));

import Networking from "./Networking.svelte";

describe("Networking", () => {
  it("renders the networks and the attachment table", async () => {
    render(Networking);
    // Name appears in both the diagram node and the attachment table row.
    expect((await screen.findAllByText("web-agent-01")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("egress-net").length).toBeGreaterThan(0);
    expect(screen.getByText("Attachments & policy")).toBeInTheDocument();
  });
});
