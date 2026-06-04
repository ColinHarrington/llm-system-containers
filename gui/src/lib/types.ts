export type VmStatus = "NotCreated" | "Stopped" | "Starting" | "Running";

export interface Sandbox {
  name: string;
  status: "Running" | "Stopped";
  image?: string;
}

export interface ServiceEntry {
  name: string;
  description: string;
  priority: string;
  enabled: boolean;
}
