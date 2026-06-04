// Bridge to llmsc-core. Inside the Tauri shell, calls the Rust commands; in a plain browser
// (Vite dev / Vitest) returns mock data so the UI is developable without the native window.
import type { Sandbox, ServiceEntry, VmStatus } from "./types";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invokeCmd<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

// --- in-browser mock state (so the UI is interactive without the native shell) ---
let mockSandboxes: Sandbox[] = [
  { name: "web-agent-01", status: "Running", image: "images:alpine/3.21" },
  { name: "ci-runner", status: "Running", image: "images:debian/12" },
  { name: "scratch-01", status: "Stopped", image: "images:alpine/3.21" },
];
let mockServices: ServiceEntry[] = [
  { name: "litellm", description: "LLM proxy — agents use virtual keys", priority: "MVP", enabled: true },
  { name: "phoenix", description: "LLM/agent observability — traces", priority: "MVP", enabled: false },
  { name: "grafana", description: "Dashboards over metrics + logs", priority: "MVP", enabled: false },
  { name: "seaweedfs", description: "Durable shared storage", priority: "Core", enabled: false },
];

// --- VM ---
export async function vmStatus(): Promise<VmStatus> {
  if (inTauri()) return invokeCmd<VmStatus>("vm_status");
  await delay(120);
  return "Running";
}
export async function vmUp(): Promise<void> {
  if (inTauri()) return invokeCmd<void>("vm_up");
  await delay(300);
}
export async function vmDown(): Promise<void> {
  if (inTauri()) return invokeCmd<void>("vm_down");
  await delay(200);
}

// --- sandboxes ---
export async function listSandboxes(): Promise<Sandbox[]> {
  if (inTauri()) return invokeCmd<Sandbox[]>("sandbox_list");
  await delay(120);
  return [...mockSandboxes];
}
export async function launchSandbox(name: string, image: string, nesting: boolean): Promise<void> {
  if (inTauri()) return invokeCmd<void>("sandbox_launch", { name, image, nesting });
  await delay(250);
  mockSandboxes = [...mockSandboxes, { name, status: "Running", image }];
}
export async function removeSandbox(name: string): Promise<void> {
  if (inTauri()) return invokeCmd<void>("sandbox_rm", { name });
  await delay(150);
  mockSandboxes = mockSandboxes.filter((s) => s.name !== name);
}

// --- services ---
export async function listServices(): Promise<ServiceEntry[]> {
  if (inTauri()) return invokeCmd<ServiceEntry[]>("service_list");
  await delay(120);
  return [...mockServices];
}
export async function setService(name: string, enabled: boolean): Promise<void> {
  if (inTauri()) return invokeCmd<void>(enabled ? "service_enable" : "service_disable", { name });
  await delay(120);
  mockServices = mockServices.map((s) => (s.name === name ? { ...s, enabled } : s));
}
