import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { buildImage } = vi.hoisted(() => ({ buildImage: vi.fn(async () => {}) }));
vi.mock("./core", () => ({ buildImage }));

import { ui } from "./store.svelte";
import BuildImage from "./BuildImage.svelte";

describe("BuildImage", () => {
  it("builds an image with the chosen base, name, and packages", async () => {
    ui.buildImageOpen = true;
    render(BuildImage);

    await fireEvent.input(screen.getByPlaceholderText("dev-debian-12"), { target: { value: "my-dev" } });
    await fireEvent.click(screen.getByRole("button", { name: "images:ubuntu/24.04" }));
    await fireEvent.input(screen.getByPlaceholderText(/git curl/), { target: { value: "git, curl python3" } });

    await fireEvent.click(screen.getByRole("button", { name: "Build image" }));

    expect(buildImage).toHaveBeenCalledOnce();
    expect(buildImage).toHaveBeenCalledWith(
      expect.objectContaining({
        name: "my-dev",
        base: "images:ubuntu/24.04",
        packages: ["git", "curl", "python3"],
      }),
    );
  });
});
