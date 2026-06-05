export type VmStatus = "NotCreated" | "Stopped" | "Starting" | "Running";

export type UserKind = "agent" | "aux" | "human";

export interface UserChip {
  initials: string;
  kind: UserKind;
}

export interface Sandbox {
  name: string;
  status: "Running" | "Stopped";
  image?: string;
  // Rich, display-only fields. Present in mock/demo data; the real backend list
  // (name/status/image) leaves them undefined and the UI degrades gracefully.
  role?: string;
  tags?: string[];
  users?: UserChip[];
  nested?: number | null;
  cpuCores?: number;
  memUsed?: number;
  memTotal?: number;
}

export interface ServiceEntry {
  name: string;
  description: string;
  priority: string;
  enabled: boolean;
}

export interface ProfileInfo {
  name: string;
  summary: string;
  filesystem: string;
  network: string;
  l3: boolean;
  llmBudget: string;
  controlPlane: string;
}

export interface ProjectInfo {
  name: string;
  description: string;
  usedBy: number;
  config: Record<string, string>;
}

export interface StorageVolumeInfo {
  name: string;
  vtype: string;
  usedBy: number;
  config: Record<string, string>;
}

export interface StoragePoolInfo {
  name: string;
  driver: string;
  description: string;
  usedBy: number;
  config: Record<string, string>;
  volumes: StorageVolumeInfo[];
}

export interface IncusProfileInfo {
  name: string;
  description: string;
  usedBy: number;
  config: Record<string, string>;
  devices: Record<string, Record<string, string>>;
}

export interface AgentInfo {
  id: string;
  name: string;
  initials: string;
  kind: Exclude<UserKind, "human">;
  sandbox: string;
  uid: number;
  model: string;
  status: "working" | "paused" | "idle";
  task: string;
}

export interface ImageInfo {
  name: string;
  desc: string;
  flavor: string;
  base: string;
  arch: string;
  size: string;
  usedBy: string;
  updated: string;
}

export interface VirtualKey {
  key: string;
  assignedTo: string;
  models: string;
  budget: string;
  used: string;
  status: "active" | "idle" | "revoked";
}

export interface HostResources {
  cpuUsed: number; // cores
  cpuTotal: number; // cores
  // memory & disk are raw bytes; the UI formats them (MB / GB)
  memUsed: number;
  memTotal: number;
  diskUsed: number;
  diskTotal: number;
}

export interface SnapshotInfo {
  name: string;
  created: string;
  stateful: boolean;
}

// --- Live Incus instance surface (read back from the server) ---
export interface InstanceConfig {
  name: string;
  status: "running" | "stopped";
  description: string;
  ephemeral: boolean;
  profiles: string[];
  config: Record<string, string>;
  devices: Record<string, Record<string, string>>;
  localDevices: string[];
}

// --- Topology (nested VM -> sandboxes -> agents) ---
export type AgentState = "active" | "thinking" | "waiting" | "idle";

export interface Guardrails {
  filesystem: string;
  network: string;
  l3: boolean;
  llmBudget: string;
  controlPlane: string;
}

export interface TopoAgent {
  name: string;
  kind: "agent" | "human";
  state: AgentState;
  action: string;
  tools: string[];
  active: string | null; // currently-active tool id
  profile?: string | null; // profile the guardrails were seeded from (provenance)
  guardrails?: Guardrails | null; // the agent's own (refinable) guardrails
}

export interface TopoSandbox {
  name: string;
  image: string;
  status: "running" | "stopped";
  l3: boolean;
  cpu: string;
  mem: string;
  agents: TopoAgent[];
}

// --- Networking (real Incus topology) ---
export interface NetworkInfo {
  name: string;
  kind: string;
  ipv4: string;
  nat: boolean;
  usedBy: number;
}

export interface SandboxNet {
  name: string;
  status: "running" | "stopped";
  networks: string[];
  ipv4: string;
}

export interface NetworkingData {
  networks: NetworkInfo[];
  sandboxes: SandboxNet[];
}

export interface AclRule {
  action: string;
  source: string;
  destination: string;
  protocol: string;
  port: string;
  description: string;
}

// --- Egress policy (per-container enforcement ring) ---
export type EgressPosture = "deny-all" | "allowlist" | "open";

export interface EgressPolicy {
  posture: EgressPosture;
  allow: string[]; // named sets (llm, package-registries, web) or raw CIDR:port[/proto]
}

export interface NetworkAclInfo {
  name: string;
  description: string;
  usedBy: number;
  ingress: AclRule[];
  egress: AclRule[];
}
