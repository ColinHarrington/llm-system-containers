import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, beforeEach } from "vitest";
import { ui, activity, showToast, logActivity, clearActivity } from "./store.svelte";
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

  it("filters by kind (Alerts / Steps)", async () => {
    showToast("All good", "ok");
    showToast("Something failed", "danger");
    logActivity("Installing python", "progress");

    ui.activityOpen = true;
    render(ActivityDrawer);
    expect(await screen.findByText("Something failed")).toBeInTheDocument();

    // Alerts → only the danger one.
    await fireEvent.click(screen.getByRole("button", { name: /Alerts/ }));
    expect(screen.getByText("Something failed")).toBeInTheDocument();
    expect(screen.queryByText("All good")).not.toBeInTheDocument();
    expect(screen.queryByText("Installing python")).not.toBeInTheDocument();

    // Steps → only the progress one.
    await fireEvent.click(screen.getByRole("button", { name: /Steps/ }));
    expect(screen.getByText("Installing python")).toBeInTheDocument();
    expect(screen.queryByText("Something failed")).not.toBeInTheDocument();
  });
});
