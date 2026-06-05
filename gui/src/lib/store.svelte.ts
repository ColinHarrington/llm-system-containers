// Shared UI state (Svelte 5 runes module). Routing, theme, global modals, and a
// `dataVersion` counter that screens read in their refresh effect so an action in one
// place (e.g. launching a sandbox from the topbar) refreshes the others.
import type { AgentInfo } from "./types";

export type Screen =
  | "dashboard" | "sandboxes" | "sandbox-detail" | "topology" | "agent"
  | "incus" | "services" | "profiles" | "wizard";

export type IncusTab = "profiles" | "networks" | "storage" | "images" | "project";

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
  incusTab: "profiles" as IncusTab,
  theme: initialTheme(),
  newSandboxOpen: false,
  buildImageOpen: false,
  addAgentSandbox: null as string | null,
  selectedSandbox: null as string | null,
  paletteOpen: false,
  activityOpen: false,
  steerAgent: null as AgentInfo | null,
  terminalTarget: null as string | null,
  toast: null as { msg: string; color: ToastColor; id: number } | null,
  dataVersion: 0,
});

// --- activity log (a reviewable history of toasts + progress steps) ---
export type ActivityKind = ToastColor | "progress";
export interface ActivityItem { id: number; msg: string; kind: ActivityKind; time: number }
export const activity = $state<ActivityItem[]>([]);
let activityId = 0;
export function logActivity(msg: string, kind: ActivityKind = "accent"): void {
  activityId += 1;
  activity.unshift({ id: activityId, msg, kind, time: Date.now() });
  if (activity.length > 120) activity.length = 120;
}
export function clearActivity(): void {
  activity.length = 0;
}

let toastId = 0;
export function showToast(msg: string, color: ToastColor = "accent"): void {
  toastId += 1;
  ui.toast = { msg, color, id: toastId };
  logActivity(msg, color);
}

export function openTerminal(target: string): void {
  ui.terminalTarget = target;
}

export function navigate(screen: Screen): void {
  ui.screen = screen;
}

export function openSandbox(name: string): void {
  ui.selectedSandbox = name;
  ui.screen = "sandbox-detail";
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
  sandboxes: ["Sandbox containers", "Your LLMSC workspaces (L2 system containers)"],
  "sandbox-detail": ["Sandbox", "Sandbox container detail"],
  topology: ["Topology", "Nested view · VM → sandboxes → agents"],
  agent: ["Agent control", "Observe, interrupt and steer running agents"],
  incus: ["Incus", "The Incus control surface — profiles, networks, storage, images"],
  services: ["Services", "Shared infrastructure for your sandboxes"],
  profiles: ["Agent profiles", "Reusable permission profiles assigned to agents"],
  wizard: ["Set up your environment", "First-run configuration"],
};
