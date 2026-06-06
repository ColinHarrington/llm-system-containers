// Shared UI state (Svelte 5 runes module). Routing, theme, global modals, and a
// `dataVersion` counter that screens read in their refresh effect so an action in one
// place (e.g. launching a sandbox from the topbar) refreshes the others.
import type { AgentInfo } from "./types";

export type Screen =
  | "dashboard" | "sandboxes" | "sandbox-detail" | "topology" | "agent"
  | "incus" | "services" | "profiles" | "security" | "settings" | "wizard";

export type IncusTab = "profiles" | "networks" | "storage" | "images" | "project";

function initialTheme(): "light" | "dark" {
  if (typeof localStorage !== "undefined") {
    const saved = localStorage.getItem("llmsc-theme");
    if (saved === "light" || saved === "dark") return saved;
  }
  return "dark"; // direction A is dark-first
}

function initialCollapsed(): boolean {
  return typeof localStorage !== "undefined" && localStorage.getItem("llmsc-sidebar") === "collapsed";
}

export type ToastColor = "accent" | "ok" | "warn" | "danger";
export interface ToastAction { label: string; run: () => void }
export interface ToastItem { id: number; msg: string; color: ToastColor; action?: ToastAction }

export const ui = $state({
  screen: "dashboard" as Screen,
  incusTab: "profiles" as IncusTab,
  theme: initialTheme(),
  sidebarCollapsed: initialCollapsed(),
  newSandboxOpen: false,
  buildImageOpen: false,
  addAgentSandbox: null as string | null,
  selectedSandbox: null as string | null,
  paletteOpen: false,
  activityOpen: false,
  shortcutsOpen: false,
  confirm: null as ConfirmState | null,
  steerAgent: null as AgentInfo | null,
  terminalTarget: null as string | null,
  toasts: [] as ToastItem[],
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

// --- live polling ---
// A `tick` counter incremented on an interval (paused when the tab is hidden or the user toggles
// it off). Live screens read `live.tick` in their refresh effect to stay current automatically.
export const live = $state({ tick: 0, paused: false, lastRefresh: 0 });
export function toggleLive(): void {
  live.paused = !live.paused;
}
/** Force every live screen to refetch now (and stamp the last-refresh time). */
export function refreshNow(): void {
  live.lastRefresh = Date.now();
  ui.dataVersion++;
}
let liveTimer: ReturnType<typeof setInterval> | null = null;
export function initLivePolling(ms = 6000): () => void {
  if (liveTimer) clearInterval(liveTimer);
  live.lastRefresh = Date.now();
  liveTimer = setInterval(() => {
    if (live.paused) return;
    if (typeof document !== "undefined" && document.hidden) return;
    live.tick++;
    live.lastRefresh = Date.now();
  }, ms);
  return () => {
    if (liveTimer) clearInterval(liveTimer);
    liveTimer = null;
  };
}

// --- confirmation dialogs ---
export interface ConfirmState {
  title: string;
  message: string;
  confirmLabel: string;
  danger: boolean;
  resolve: (ok: boolean) => void;
}
/** Ask the user to confirm a hard-to-reverse action. Resolves true if confirmed. */
export function confirmAction(opts: { title: string; message: string; confirmLabel?: string; danger?: boolean }): Promise<boolean> {
  return new Promise((resolve) => {
    ui.confirm = {
      title: opts.title,
      message: opts.message,
      confirmLabel: opts.confirmLabel ?? "Confirm",
      danger: opts.danger ?? false,
      resolve,
    };
  });
}
export function resolveConfirm(ok: boolean): void {
  const c = ui.confirm;
  ui.confirm = null;
  c?.resolve(ok);
}

let toastId = 0;
/** Show a transient toast. Returns its id. Pass an `action` (e.g. Undo) to add an inline button. */
export function showToast(msg: string, color: ToastColor = "accent", action?: ToastAction): number {
  toastId += 1;
  const id = toastId;
  ui.toasts.push({ id, msg, color, action });
  if (ui.toasts.length > 4) ui.toasts.shift(); // keep the stack shallow
  logActivity(msg, color);
  return id;
}
export function dismissToast(id: number): void {
  const i = ui.toasts.findIndex((t) => t.id === id);
  if (i >= 0) ui.toasts.splice(i, 1);
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
  live.lastRefresh = Date.now();
  ui.dataVersion++;
}

/** "updated Xs ago" — pass `live.tick` so it recomputes on each poll. */
export function refreshedAgo(ts: number, _tick: number): string {
  if (!ts) return "—";
  const s = Math.max(0, Math.round((Date.now() - ts) / 1000));
  if (s < 5) return "just now";
  if (s < 60) return `${s}s ago`;
  return `${Math.round(s / 60)}m ago`;
}

export function toggleTheme(): void {
  ui.theme = ui.theme === "dark" ? "light" : "dark";
  if (typeof localStorage !== "undefined") localStorage.setItem("llmsc-theme", ui.theme);
}

export function toggleSidebar(): void {
  ui.sidebarCollapsed = !ui.sidebarCollapsed;
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("llmsc-sidebar", ui.sidebarCollapsed ? "collapsed" : "expanded");
  }
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
  security: ["Security posture", "Defense-in-depth enforcement across every sandbox"],
  settings: ["Settings", "Operator, appearance, and the Playground VM"],
  wizard: ["Set up your environment", "First-run configuration"],
};
