import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { listAvailableImages } = vi.hoisted(() => ({
  listAvailableImages: vi.fn(async () => [
    { name: "debian/12", desc: "Debian bookworm", flavor: "Debian", base: "Debian 12", arch: "amd64", size: "92 MB", usedBy: "—", updated: "2026-06-01" },
    { name: "ubuntu/24.04", desc: "Ubuntu noble", flavor: "Ubuntu", base: "Ubuntu 24.04", arch: "amd64", size: "180 MB", usedBy: "—", updated: "2026-06-01" },
  ]),
}));
vi.mock("../lib/core", () => ({
  listImages: vi.fn(async () => [
    { name: "alpine/3.21", desc: "Alpine", flavor: "Alpine", base: "Alpine 3.21", arch: "amd64", size: "3.5 MB", usedBy: "1 sandbox", updated: "2026-05-28" },
  ]),
  listAvailableImages,
}));

import Images from "./Images.svelte";

describe("Images", () => {
  it("shows installed images, then loads + filters the available catalog", async () => {
    render(Images);
    expect(await screen.findByText("alpine/3.21")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "All available" }));
    expect(await screen.findByText("debian/12")).toBeInTheDocument();
    expect(listAvailableImages).toHaveBeenCalledOnce();
    expect(screen.getByText("ubuntu/24.04")).toBeInTheDocument();

    await fireEvent.input(screen.getByPlaceholderText("Search images…"), { target: { value: "debian" } });
    expect(screen.getByText("debian/12")).toBeInTheDocument();
    expect(screen.queryByText("ubuntu/24.04")).not.toBeInTheDocument();
  });
});
