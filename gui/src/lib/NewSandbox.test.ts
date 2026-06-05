import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect, vi } from "vitest";

const { launchSandbox } = vi.hoisted(() => ({ launchSandbox: vi.fn(async () => {}) }));
vi.mock("./core", () => ({
  launchSandbox,
  operatorDefault: vi.fn(async () => "colin"),
}));

import { ui } from "./store.svelte";
import NewSandbox from "./NewSandbox.svelte";

describe("NewSandbox", () => {
  it("launches with the Incus-shaped spec (name, image, operator, a mount)", async () => {
    ui.newSandboxOpen = true;
    render(NewSandbox);

    await fireEvent.input(screen.getByPlaceholderText("web-agent-02"), { target: { value: "web-02" } });
    await fireEvent.click(screen.getByRole("button", { name: /Add/ })); // add a mount row
    await fireEvent.input(screen.getByPlaceholderText(/host/), { target: { value: "~/proj" } });
    await fireEvent.input(screen.getByPlaceholderText(/container/), { target: { value: "/work" } });

    await fireEvent.click(screen.getByRole("button", { name: /Launch sandbox/ }));

    expect(launchSandbox).toHaveBeenCalledOnce();
    const arg = (launchSandbox.mock.calls[0] as unknown[])[0] as {
      name: string; operator: string; mounts: { source: string; path: string }[];
    };
    expect(arg.name).toBe("web-02");
    expect(arg.operator).toBe("colin"); // prefilled from operatorDefault
    expect(arg.mounts).toEqual([{ source: "~/proj", path: "/work", readonly: false }]);
  });
});
