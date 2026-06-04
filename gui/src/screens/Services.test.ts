import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { provisionService } = vi.hoisted(() => ({ provisionService: vi.fn(async () => {}) }));
vi.mock("../lib/core", () => ({
  listServices: vi.fn(async () => [
    { name: "litellm", description: "LLM proxy", priority: "MVP", enabled: true },
    { name: "phoenix", description: "Observability", priority: "MVP", enabled: false },
  ]),
  setService: vi.fn(async () => {}),
  provisionService,
  DEPLOYABLE_SERVICES: new Set(["litellm"]),
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
});
