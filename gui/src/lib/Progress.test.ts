import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { flushSync } from "svelte";
import { describe, it, expect, vi } from "vitest";

// Capture the handler Progress registers so the test can drive step events.
const bus = vi.hoisted(() => {
  let handler: ((m: string) => void) | null = null;
  return {
    set: (h: (m: string) => void) => (handler = h),
    fire: (m: string) => handler?.(m),
  };
});
vi.mock("./core", () => ({
  onProgress: (cb: (m: string) => void) => {
    bus.set(cb);
    return () => {};
  },
}));

import Progress from "./Progress.svelte";

describe("Progress", () => {
  it("is hidden until a step arrives, then shows the latest steps", async () => {
    render(Progress);
    flushSync(); // run the subscribing $effect so onProgress registers the handler
    expect(screen.queryByRole("status")).not.toBeInTheDocument();

    bus.fire("Creating VM");
    bus.fire("Starting VM");
    expect(await screen.findByText("Creating VM")).toBeInTheDocument();
    expect(await screen.findByText("Starting VM")).toBeInTheDocument();
  });
});
