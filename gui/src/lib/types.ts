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
  base: string;
  size: string;
  tooling: string;
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
  cpuUsed: number;
  cpuTotal: number;
  memUsed: number;
  memTotal: number;
  diskUsed: number;
  diskTotal: number;
}
