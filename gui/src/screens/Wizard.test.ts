import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { createPlatform } = vi.hoisted(() => ({ createPlatform: vi.fn(async () => {}) }));
vi.mock("../lib/core", () => ({
  listServices: vi.fn(async () => [
    { name: "litellm", description: "LLM proxy", priority: "MVP", enabled: true },
  ]),
  createPlatform,
  SERVICE_META: { litellm: { initials: "Li", color: "#000", placement: "own L2 container" } },
}));

import Wizard from "./Wizard.svelte";

describe("Wizard", () => {
  it("walks through the steps and creates the environment", async () => {
    render(Wizard);
    expect(await screen.findByText("How much power should the VM get?")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: /Continue/ })); // -> Services
    await fireEvent.click(screen.getByRole("button", { name: /Continue/ })); // -> Networking
    await fireEvent.click(screen.getByRole("button", { name: /Continue/ })); // -> Review
    await fireEvent.click(screen.getByRole("button", { name: /Create environment/ }));

    expect(createPlatform).toHaveBeenCalledOnce();
    expect(await screen.findByText("Environment ready")).toBeInTheDocument();
  });
});
