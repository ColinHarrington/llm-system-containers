import { describe, it, expect, beforeEach } from "vitest";
import { ui, toggleSidebar } from "./store.svelte";

describe("sidebar collapse", () => {
  beforeEach(() => {
    ui.sidebarCollapsed = false;
    localStorage.clear();
  });

  it("toggles and persists to localStorage", () => {
    toggleSidebar();
    expect(ui.sidebarCollapsed).toBe(true);
    expect(localStorage.getItem("llmsc-sidebar")).toBe("collapsed");
    toggleSidebar();
    expect(ui.sidebarCollapsed).toBe(false);
    expect(localStorage.getItem("llmsc-sidebar")).toBe("expanded");
  });
});
