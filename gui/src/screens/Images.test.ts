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
  it("shows installed images, then a distro picker that drills into a distro's images", async () => {
    render(Images);
    expect(await screen.findByText("alpine/3.21")).toBeInTheDocument();

    // Switch to the catalog -> distro picker (flavors, not image rows yet).
    await fireEvent.click(screen.getByRole("button", { name: "All available" }));
    expect(await screen.findByText("Debian")).toBeInTheDocument();
    expect(listAvailableImages).toHaveBeenCalledOnce();
    expect(screen.getByText("Ubuntu")).toBeInTheDocument();
    expect(screen.queryByText("debian/12")).not.toBeInTheDocument(); // not until drilled in

    // Drill into Debian -> its images appear, Ubuntu's do not.
    await fireEvent.click(screen.getByText("Debian"));
    expect(await screen.findByText("debian/12")).toBeInTheDocument();
    expect(screen.queryByText("ubuntu/24.04")).not.toBeInTheDocument();

    // Back to the picker.
    await fireEvent.click(screen.getByRole("button", { name: "‹ All distros" }));
    expect(await screen.findByText("Ubuntu")).toBeInTheDocument();
  });

  it("search jumps straight to flat results across distros", async () => {
    render(Images);
    await fireEvent.click(screen.getByRole("button", { name: "All available" }));
    await screen.findByText("Debian");

    await fireEvent.input(screen.getByPlaceholderText("Search all distros…"), { target: { value: "ubuntu" } });
    expect(await screen.findByText("ubuntu/24.04")).toBeInTheDocument();
    expect(screen.queryByText("debian/12")).not.toBeInTheDocument();
  });
});
