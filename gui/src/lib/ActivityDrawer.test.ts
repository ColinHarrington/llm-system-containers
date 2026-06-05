import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, beforeEach } from "vitest";
import { ui, activity, showToast, clearActivity } from "./store.svelte";
import ActivityDrawer from "./ActivityDrawer.svelte";

describe("ActivityDrawer", () => {
  beforeEach(() => {
    clearActivity();
    ui.activityOpen = false;
  });

  it("logs toasts and shows them in the drawer; clear empties it", async () => {
    showToast("VM is up", "ok");
    showToast("Egress enforced", "ok");
    expect(activity.length).toBe(2);

    ui.activityOpen = true;
    render(ActivityDrawer);
    expect(await screen.findByText("Egress enforced")).toBeInTheDocument();
    expect(screen.getByText("VM is up")).toBeInTheDocument();
    // newest first
    const rows = screen.getAllByText(/VM is up|Egress enforced/);
    expect(rows[0]).toHaveTextContent("Egress enforced");

    await fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(activity.length).toBe(0);
  });
});
