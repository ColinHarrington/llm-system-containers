// Bridge to llmsc-core. Inside the Tauri shell, calls the Rust commands; in a plain browser
// (Vite dev / Vitest) returns mock data so the UI is developable without the native window.
//
// Operations the backend implements (VM up/down, sandbox launch/rm, service enable/provision,
// platform init, progress, topology, host resources, images) are wired to real Tauri commands and
// fall back to mock data only in the browser. Views the backend does not expose yet (agents,
// virtual keys, networking attachments, and the rich per-sandbox metadata on the Sandboxes cards)
// return representative demo data in BOTH environments for now — clearly marked until those land.
import type {
  AgentInfo,
  HostResources,
  ImageInfo,
  InstanceConfig,
  NetworkingData,
  ProfileInfo,
  Sandbox,
  ServiceEntry,
  TopoSandbox,
  VirtualKey,
  VmStatus,
} from "./types";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invokeCmd<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

// --- progress events ---
// Long operations (vm bring-up, sandbox launch, platform init) stream step updates. In the Tauri
// shell these arrive as `progress` events from the Rust EventReporter; in the browser the mock
// paths feed the same handlers so the UI is developable without the native window.
type ProgressHandler = (msg: string) => void;
const progressHandlers = new Set<ProgressHandler>();
let tauriListening = false;

export function onProgress(cb: ProgressHandler): () => void {
  progressHandlers.add(cb);
  if (inTauri() && !tauriListening) {
    tauriListening = true;
    void (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      await listen<{ msg: string }>("progress", (e) => {
        for (const h of progressHandlers) h(e.payload.msg);
      });
    })();
  }
  return () => progressHandlers.delete(cb);
}

function emitProgress(msg: string): void {
  for (const h of progressHandlers) h(msg);
}

async function mockSteps(steps: string[], each = 220): Promise<void> {
  for (const s of steps) {
    emitProgress(s);
    await delay(each);
  }
}

// --- in-browser mock state (so the UI is interactive without the native shell) ---
let mockSandboxes: Sandbox[] = [
  {
    name: "web-agent-01", status: "Running", image: "dev-ubuntu-24.04", role: "workspace",
    tags: ["unprivileged", "nesting on"], nested: 3, cpuCores: 2, memUsed: 3.1, memTotal: 4,
    users: [{ initials: "aC", kind: "agent" }, { initials: "aX", kind: "aux" }, { initials: "op", kind: "human" }],
  },
  {
    name: "ci-runner", status: "Running", image: "dev-ubuntu-24.04", role: "workspace",
    tags: ["unprivileged", "nesting on"], nested: 4, cpuCores: 2, memUsed: 2.4, memTotal: 4,
    users: [{ initials: "aC", kind: "agent" }, { initials: "op", kind: "human" }],
  },
  {
    name: "data-pipeline", status: "Running", image: "data-tools", role: "workspace",
    tags: ["unprivileged"], nested: 0, cpuCores: 1, memUsed: 1.8, memTotal: 2,
    users: [{ initials: "aC", kind: "agent" }, { initials: "op", kind: "human" }],
  },
  {
    name: "browser-bot", status: "Stopped", image: "browser-tools", role: "workspace",
    tags: ["ephemeral"], nested: null, cpuCores: 1, memUsed: 0, memTotal: 2,
    users: [{ initials: "aC", kind: "agent" }, { initials: "op", kind: "human" }],
  },
];
let mockServices: ServiceEntry[] = [
  { name: "litellm", description: "LLM proxy — agents use virtual keys", priority: "MVP", enabled: true },
  { name: "phoenix", description: "LLM/agent observability — traces", priority: "MVP", enabled: true },
  { name: "grafana", description: "Dashboards over metrics + logs", priority: "MVP", enabled: true },
  { name: "seaweedfs", description: "Durable shared storage", priority: "Core", enabled: false },
  { name: "mitmproxy", description: "Network inspection / traffic capture", priority: "Core", enabled: false },
];

// --- VM ---
export async function vmStatus(): Promise<VmStatus> {
  if (inTauri()) return invokeCmd<VmStatus>("vm_status");
  await delay(120);
  return "Running";
}
export async function vmUp(): Promise<void> {
  if (inTauri()) return invokeCmd<void>("vm_up");
  await mockSteps(["Creating VM", "Starting VM", "Installing Incus", "Initializing Incus", "VM ready"]);
}
export async function vmDown(): Promise<void> {
  if (inTauri()) return invokeCmd<void>("vm_down");
  await mockSteps(["Stopping VM", "VM stopped"], 180);
}

// --- sandboxes ---
export async function listSandboxes(): Promise<Sandbox[]> {
  if (inTauri()) return invokeCmd<Sandbox[]>("sandbox_list");
  await delay(120);
  return [...mockSandboxes];
}
export interface SandboxMount {
  source: string;
  path: string;
  readonly: boolean;
}
export interface NewSandboxInput {
  name: string;
  image: string;
  operator: string;
  description: string;
  ephemeral: boolean;
  nesting: boolean;
  profiles: string[];
  mounts: SandboxMount[];
  cloudInit: string;
  network: string;
}

export async function launchSandbox(input: NewSandboxInput): Promise<void> {
  if (inTauri()) return invokeCmd<void>("sandbox_launch", { spec: input });
  await mockSteps([`Launching ${input.name}`, `Pulling ${input.image}`, `Creating human user '${input.operator}'`, "Sandbox ready"]);
  mockSandboxes = [
    ...mockSandboxes,
    {
      name: input.name, status: "Running", image: input.image, role: "workspace",
      tags: input.nesting ? ["unprivileged", "nesting on"] : ["unprivileged"],
      nested: input.nesting ? 0 : null, cpuCores: 2, memUsed: 0.4, memTotal: 4,
      users: [{ initials: input.operator.slice(0, 2), kind: "human" }],
    },
  ];
}

// The default operator (human) username — config value, falling back to the host username.
export async function operatorDefault(): Promise<string> {
  if (inTauri()) return invokeCmd<string>("operator_default");
  return "operator";
}

// Add an agent (one Linux user) to a running sandbox, with an optional profile.
export async function addAgent(sandbox: string, name: string, profile: string): Promise<void> {
  if (inTauri()) return invokeCmd<void>("add_agent", { sandbox, name, profile });
  const suffix = profile ? ` (${profile})` : "";
  await mockSteps([`Adding agent '${name}' to ${sandbox}${suffix}`, `Agent '${name}' added${suffix}`], 200);
}

// Read a sandbox's live Incus surface (config/devices/profiles) back from the server.
export async function instanceConfig(name: string): Promise<InstanceConfig> {
  if (inTauri()) return invokeCmd<InstanceConfig>("instance_config", { name });
  await delay(100);
  return {
    name, status: "running", description: "dev box", ephemeral: false,
    profiles: ["default", "sandbox"],
    config: { "security.privileged": "false", "security.nesting": "true", "image.description": "Alpine 3.21" },
    devices: {
      eth0: { type: "nic", network: "incusbr0" },
      root: { type: "disk", path: "/", pool: "default" },
      work: { type: "disk", source: "~/projects/app", path: "/work", shift: "true" },
    },
  };
}

// Remove an agent (its Linux user) from a sandbox.
export async function removeAgent(sandbox: string, name: string): Promise<void> {
  if (inTauri()) return invokeCmd<void>("remove_agent", { sandbox, name });
  await mockSteps([`Removing agent '${name}' from ${sandbox}`, `Agent '${name}' removed`], 200);
}

// The shipped agent-profile archetypes (definition layer).
export async function listProfiles(): Promise<ProfileInfo[]> {
  if (inTauri()) return invokeCmd<ProfileInfo[]>("profiles");
  return [
    { name: "researcher", summary: "Read, research, gather context", filesystem: "RO repo + docs, RW scratch", network: "Web/docs allowlist via mitmproxy", l3: false, llmBudget: "generous", controlPlane: "none" },
    { name: "tester", summary: "Run and write tests", filesystem: "RW repo", network: "Limited (package registries)", l3: true, llmBudget: "medium", controlPlane: "none" },
    { name: "builder", summary: "Compile, build images", filesystem: "RW repo + artifacts", network: "Registry/package allowlist", l3: true, llmBudget: "medium", controlPlane: "none" },
    { name: "validation", summary: "Run checks; never writes — strictest", filesystem: "Read-only everything", network: "None except LLM", l3: false, llmBudget: "small", controlPlane: "none" },
    { name: "orchestrator", summary: "Drive other agents (software-factory)", filesystem: "Minimal (own scratch)", network: "None raw (internal coordination only)", l3: false, llmBudget: "broad", controlPlane: "launch/stop sandboxes, coordinate agents" },
  ];
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

// Services that have a deployer in src-tauri (can be provisioned, not just toggled in config).
export const DEPLOYABLE_SERVICES = new Set(["litellm"]);

export async function provisionService(name: string): Promise<void> {
  if (inTauri()) return invokeCmd<void>("service_up", { name });
  await mockSteps([
    "Creating LiteLLM service container",
    "Installing Python (apt)",
    "Creating virtualenv",
    "Installing LiteLLM 1.87.0 (pip, pinned)",
    "Writing config + systemd unit",
    "Starting LiteLLM",
    "LiteLLM deployed",
  ]);
}

// Display metadata for the Services screen cards (icon initials + brand color + placement).
export interface ServiceMeta {
  initials: string;
  color: string;
  placement: string;
}
export const SERVICE_META: Record<string, ServiceMeta> = {
  litellm: { initials: "Li", color: "#6b5bd2", placement: "own L2 container" },
  phoenix: { initials: "Ph", color: "#e06f3a", placement: "in L1 VM" },
  grafana: { initials: "Gr", color: "#2a9d8f", placement: "in L1 VM" },
  seaweedfs: { initials: "Sw", color: "#3a7de0", placement: "own L2 container" },
  mitmproxy: { initials: "Mi", color: "#c0455a", placement: "own L2 container" },
};

// --- read-only demo views (no backend yet) ---
export async function hostResources(): Promise<HostResources | null> {
  if (inTauri()) {
    try {
      return await invokeCmd<HostResources>("host_resources");
    } catch {
      return null; // VM not running / usage not readable
    }
  }
  await delay(80);
  return { cpuUsed: 5.2, cpuTotal: 8, memUsed: 9.4, memTotal: 16, diskUsed: 34, diskTotal: 120 };
}

export async function listAgents(): Promise<AgentInfo[]> {
  await delay(80);
  return [
    {
      id: "agent-claude", name: "agent-claude", initials: "aC", kind: "agent",
      sandbox: "web-agent-01", uid: 1001, model: "claude-opus-4-8", status: "working",
      task: "Fix failing auth tests & open PR",
    },
    {
      id: "agent-aux", name: "agent-aux", initials: "aX", kind: "aux",
      sandbox: "ci-runner", uid: 1002, model: "claude-sonnet-4-6", status: "working",
      task: "Review the open PR diff",
    },
    {
      id: "agent-claude-dp", name: "agent-claude", initials: "aC", kind: "agent",
      sandbox: "data-pipeline", uid: 1001, model: "claude-sonnet-4-6", status: "idle",
      task: "Awaiting work",
    },
  ];
}

export async function listImages(): Promise<ImageInfo[]> {
  if (inTauri()) return invokeCmd<ImageInfo[]>("images");
  await delay(80);
  return [
    { name: "dev-ubuntu-24.04", desc: "general dev workspace", flavor: "Ubuntu", base: "Ubuntu 24.04", arch: "amd64", size: "1.4 GB", usedBy: "2 sandboxes", updated: "2026-05-30" },
    { name: "alpine/3.21", desc: "Alpinelinux 3.21 amd64", flavor: "Alpine", base: "Alpine 3.21", arch: "amd64", size: "3.5 MB", usedBy: "1 sandbox", updated: "2026-05-28" },
  ];
}

export interface BuildImageOpts {
  base: string;
  name: string;
  packages: string[];
  script: string;
  description: string;
}

// Build a custom image (publish-from-container). Streams steps via the progress event.
export async function buildImage(o: BuildImageOpts): Promise<void> {
  if (inTauri()) {
    return invokeCmd<void>("build_image", {
      base: o.base, name: o.name, packages: o.packages, script: o.script, description: o.description,
    });
  }
  await mockSteps([
    `Launching builder from ${o.base}`,
    "Running setup inside builder",
    "Stopping builder",
    `Publishing image '${o.name}'`,
    "Removing builder",
    `Image '${o.name}' built`,
  ]);
}

// The full remote catalog (`images:`). Large + network-bound → fetched on demand by the screen.
export async function listAvailableImages(): Promise<ImageInfo[]> {
  if (inTauri()) return invokeCmd<ImageInfo[]>("images_available");
  await delay(200);
  return [
    { name: "alpine/3.21", desc: "Alpinelinux 3.21", flavor: "Alpine", base: "Alpine 3.21", arch: "amd64", size: "3.5 MB", usedBy: "—", updated: "2026-06-01" },
    { name: "alpine/edge", desc: "Alpinelinux edge", flavor: "Alpine", base: "Alpine edge", arch: "amd64", size: "3.6 MB", usedBy: "—", updated: "2026-06-02" },
    { name: "alpine/3.21", desc: "Alpinelinux 3.21", flavor: "Alpine", base: "Alpine 3.21", arch: "arm64", size: "3.4 MB", usedBy: "—", updated: "2026-06-01" },
    { name: "debian/12", desc: "Debian bookworm", flavor: "Debian", base: "Debian 12", arch: "amd64", size: "92 MB", usedBy: "—", updated: "2026-06-01" },
    { name: "debian/13", desc: "Debian trixie", flavor: "Debian", base: "Debian 13", arch: "amd64", size: "95 MB", usedBy: "—", updated: "2026-06-01" },
    { name: "debian/12", desc: "Debian bookworm", flavor: "Debian", base: "Debian 12", arch: "arm64", size: "90 MB", usedBy: "—", updated: "2026-06-01" },
    { name: "ubuntu/24.04", desc: "Ubuntu noble", flavor: "Ubuntu", base: "Ubuntu 24.04", arch: "amd64", size: "180 MB", usedBy: "—", updated: "2026-06-01" },
    { name: "ubuntu/22.04", desc: "Ubuntu jammy", flavor: "Ubuntu", base: "Ubuntu 22.04", arch: "amd64", size: "175 MB", usedBy: "—", updated: "2026-06-01" },
    { name: "fedora/41", desc: "Fedora 41", flavor: "Fedora", base: "Fedora 41", arch: "amd64", size: "210 MB", usedBy: "—", updated: "2026-06-01" },
    { name: "archlinux/current", desc: "Arch Linux", flavor: "Archlinux", base: "Archlinux current", arch: "amd64", size: "320 MB", usedBy: "—", updated: "2026-06-02" },
  ];
}

export async function listVirtualKeys(): Promise<VirtualKey[]> {
  await delay(80);
  return [
    { key: "sk-vk-…a91f", assignedTo: "agent-claude @ web-agent-01", models: "opus, sonnet", budget: "$50 / day", used: "$0.86", status: "active" },
    { key: "sk-vk-…77c2", assignedTo: "agent-aux @ ci-runner", models: "sonnet, haiku", budget: "$20 / day", used: "$3.40", status: "active" },
    { key: "sk-vk-…1d08", assignedTo: "agent-claude @ data-pipeline", models: "sonnet", budget: "$20 / day", used: "$0.00", status: "idle" },
    { key: "sk-vk-…be40", assignedTo: "browser-bot (stopped)", models: "haiku", budget: "$10 / day", used: "—", status: "revoked" },
  ];
}

// Tool id -> human label (topology agent activity chips).
export const TOOL_LABELS: Record<string, string> = {
  shell: "Terminal", code: "Editor", git: "Git", web: "Browser", run: "Run / tests",
  pkg: "Containers (L3)", db: "Database", search: "Web search", llm: "LLM call", files: "Files",
};

export async function topology(): Promise<TopoSandbox[]> {
  if (inTauri()) return invokeCmd<TopoSandbox[]>("topology");
  await delay(80);
  return [
    {
      name: "web-agent-01", image: "dev-ubuntu-24.04", status: "running", l3: true, cpu: "2.1", mem: "3.4 GB",
      agents: [
        { name: "agent-claude", kind: "agent", state: "active", action: "Editing src/api/router.ts", tools: ["code", "shell", "git", "llm"], active: "code", profile: "builder" },
        { name: "agent-aux", kind: "agent", state: "thinking", action: "Planning test changes", tools: ["shell", "run", "llm"], active: "llm", profile: "tester" },
        { name: "operator", kind: "human", state: "idle", action: "Attached · read-only watch", tools: ["shell"], active: null },
      ],
    },
    {
      name: "ci-runner", image: "ci-base", status: "running", l3: true, cpu: "3.6", mem: "5.1 GB",
      agents: [
        { name: "agent-ci", kind: "agent", state: "active", action: "Building image · docker build", tools: ["pkg", "shell", "git", "run"], active: "pkg" },
      ],
    },
    {
      name: "data-pipeline", image: "dev-ubuntu-24.04", status: "running", l3: false, cpu: "1.4", mem: "2.2 GB",
      agents: [
        { name: "agent-etl", kind: "agent", state: "active", action: "Querying warehouse", tools: ["db", "run", "files", "llm"], active: "db" },
        { name: "agent-report", kind: "agent", state: "waiting", action: "Awaiting upstream data", tools: ["files", "llm"], active: null },
      ],
    },
    {
      name: "research-01", image: "browser-tools", status: "running", l3: true, cpu: "1.8", mem: "2.9 GB",
      agents: [
        { name: "agent-browse", kind: "agent", state: "active", action: "Reading docs · 4 tabs", tools: ["web", "search", "files", "llm"], active: "web" },
      ],
    },
    { name: "scratch-01", image: "dev-ubuntu-24.04", status: "stopped", l3: true, cpu: "0", mem: "0", agents: [] },
  ];
}

export async function networking(): Promise<NetworkingData> {
  if (inTauri()) return invokeCmd<NetworkingData>("networking");
  await delay(80);
  return {
    networks: [
      { name: "incusbr0", kind: "bridge", ipv4: "10.71.0.1/24", nat: true, usedBy: 3 },
    ],
    sandboxes: [
      { name: "web-agent-01", status: "running", networks: ["incusbr0"], ipv4: "10.71.0.20" },
      { name: "ci-runner", status: "running", networks: ["incusbr0"], ipv4: "10.71.0.21" },
      { name: "scratch-01", status: "stopped", networks: ["incusbr0"], ipv4: "—" },
    ],
  };
}

// --- first-run setup ---
export interface SetupConfig {
  operator: string;
  cpus: number;
  memoryGib: number;
  diskGib: number;
  services: string[];
  defaultDenyEgress: boolean;
}

export async function createPlatform(cfg: SetupConfig): Promise<void> {
  if (inTauri()) return invokeCmd<void>("platform_init", { cfg });
  await mockSteps([
    "Wrote configuration",
    "Creating VM",
    "Starting VM",
    "Installing Incus",
    "Initializing Incus",
    "Platform ready",
  ]);
}
