// Shared UI state (Svelte 5 runes module). Routing, theme, global modals, and a
// `dataVersion` counter that screens read in their refresh effect so an action in one
// place (e.g. launching a sandbox from the topbar) refreshes the others.
import type { AgentInfo } from "./types";

export type Screen =
  | "dashboard" | "sandboxes" | "topology" | "agent"
  | "networking" | "services" | "images" | "wizard";

function initialTheme(): "light" | "dark" {
  if (typeof localStorage !== "undefined") {
    const saved = localStorage.getItem("llmsc-theme");
    if (saved === "light" || saved === "dark") return saved;
  }
  return "dark"; // direction A is dark-first
}

export type ToastColor = "accent" | "ok" | "warn" | "danger";

export const ui = $state({
  screen: "dashboard" as Screen,
  theme: initialTheme(),
  newSandboxOpen: false,
  steerAgent: null as AgentInfo | null,
  terminalTarget: null as string | null,
  toast: null as { msg: string; color: ToastColor; id: number } | null,
  dataVersion: 0,
});

let toastId = 0;
export function showToast(msg: string, color: ToastColor = "accent"): void {
  toastId += 1;
  ui.toast = { msg, color, id: toastId };
}

export function openTerminal(target: string): void {
  ui.terminalTarget = target;
}

export function navigate(screen: Screen): void {
  ui.screen = screen;
}

export function bump(): void {
  ui.dataVersion++;
}

export function toggleTheme(): void {
  ui.theme = ui.theme === "dark" ? "light" : "dark";
  if (typeof localStorage !== "undefined") localStorage.setItem("llmsc-theme", ui.theme);
}

export const SCREEN_TITLES: Record<Screen, [string, string]> = {
  dashboard: ["Home", "Overview of your VM, sandboxes and services"],
  sandboxes: ["Sandboxes", "Your LLMSC workspaces (L2 system containers)"],
  topology: ["Topology", "Nested view · VM → sandboxes → agents"],
  agent: ["Agent control", "Observe, interrupt and steer running agents"],
  networking: ["Networking", "VM networks · attachments · egress & inspection"],
  services: ["Services", "Shared infrastructure for your sandboxes"],
  images: ["Images", "Base and custom sandbox images"],
  wizard: ["Set up your environment", "First-run configuration"],
};
