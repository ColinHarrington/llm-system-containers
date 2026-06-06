import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect } from "vitest";
import { ui, confirmAction } from "./store.svelte";
import ConfirmModal from "./ConfirmModal.svelte";

describe("ConfirmModal", () => {
  it("resolves true on confirm and false on cancel", async () => {
    render(ConfirmModal);

    const p = confirmAction({ title: "Remove sandbox", message: "Sure?", confirmLabel: "Remove sandbox", danger: true });
    expect(await screen.findByText("Sure?")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Remove sandbox" }));
    expect(await p).toBe(true);
    expect(ui.confirm).toBeNull();

    const p2 = confirmAction({ title: "X", message: "Cancel me" });
    await screen.findByText("Cancel me");
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await p2).toBe(false);
  });
});
