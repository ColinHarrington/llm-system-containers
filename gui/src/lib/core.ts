// Bridge to llmsc-core. Inside the Tauri shell, calls the Rust commands; in a plain browser
// (Vite dev / Vitest) returns mock data so the UI is developable without the native window.
import type { Sandbox, VmStatus } from "./types";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invokeCmd<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

const MOCK_SANDBOXES: Sandbox[] = [
  { name: "web-agent-01", status: "Running" },
  { name: "ci-runner", status: "Running" },
  { name: "scratch-01", status: "Stopped" },
];

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

export async function listSandboxes(): Promise<Sandbox[]> {
  if (inTauri()) return invokeCmd<Sandbox[]>("sandbox_list");
  await delay(120);
  return MOCK_SANDBOXES;
}
