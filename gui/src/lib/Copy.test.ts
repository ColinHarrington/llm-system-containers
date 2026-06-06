import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach } from "vitest";
import Copy from "./Copy.svelte";

const writeText = vi.fn(async () => {});
beforeEach(() => {
  writeText.mockClear();
  Object.assign(navigator, { clipboard: { writeText } });
});

describe("Copy", () => {
  it("writes the value to the clipboard on click", async () => {
    render(Copy, { value: "web-agent-01", label: "sandbox name" });
    await fireEvent.click(screen.getByRole("button", { name: "Copy sandbox name" }));
    expect(writeText).toHaveBeenCalledWith("web-agent-01");
  });

  it("falls back to the raw value in the label when none is given", async () => {
    render(Copy, { value: "svc-litellm" });
    expect(screen.getByRole("button", { name: "Copy svc-litellm" })).toBeInTheDocument();
  });
});
