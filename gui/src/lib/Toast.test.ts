import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/svelte";
import { flushSync } from "svelte";
import { describe, it, expect } from "vitest";
import { showToast } from "./store.svelte";
import Toast from "./Toast.svelte";

describe("Toast", () => {
  it("shows the most recent message", async () => {
    showToast("$ llmsctl up");
    render(Toast);
    flushSync();
    expect(await screen.findByText("$ llmsctl up")).toBeInTheDocument();
  });
});
