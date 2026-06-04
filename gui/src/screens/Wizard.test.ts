import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { createPlatform } = vi.hoisted(() => ({ createPlatform: vi.fn(async () => {}) }));
vi.mock("../lib/core", () => ({
  listServices: vi.fn(async () => [
    { name: "litellm", description: "LLM proxy", priority: "MVP", enabled: true },
  ]),
  createPlatform,
}));

import Wizard from "./Wizard.svelte";

describe("Wizard", () => {
  it("walks through the steps and creates the platform", async () => {
    render(Wizard);
    expect(await screen.findByText("Resources")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Next" })); // -> Services
    await fireEvent.click(screen.getByRole("button", { name: "Next" })); // -> Networking
    await fireEvent.click(screen.getByRole("button", { name: "Next" })); // -> Review
    await fireEvent.click(screen.getByRole("button", { name: "Create VM" }));

    expect(createPlatform).toHaveBeenCalledOnce();
    expect(await screen.findByText("VM created")).toBeInTheDocument();
  });
});
