import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { flushSync } from "svelte";
import { describe, it, expect, vi } from "vitest";
import { showToast, ui } from "./store.svelte";
import Toast from "./Toast.svelte";

describe("Toast", () => {
  it("stacks multiple messages", async () => {
    ui.toasts.length = 0;
    showToast("$ llmsctl up");
    showToast("$ llmsctl down");
    render(Toast);
    flushSync();
    expect(await screen.findByText("$ llmsctl up")).toBeInTheDocument();
    expect(screen.getByText("$ llmsctl down")).toBeInTheDocument();
  });

  it("runs the action and dismisses when its button is clicked", async () => {
    ui.toasts.length = 0;
    const run = vi.fn();
    showToast("Disabled svc-litellm", "ok", { label: "Undo", run });
    render(Toast);
    flushSync();
    await fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(run).toHaveBeenCalledOnce();
    expect(screen.queryByText("Disabled svc-litellm")).not.toBeInTheDocument();
  });

  it("dismisses on the close button", async () => {
    ui.toasts.length = 0;
    showToast("$ heads up");
    render(Toast);
    flushSync();
    await fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText("$ heads up")).not.toBeInTheDocument();
  });
});
