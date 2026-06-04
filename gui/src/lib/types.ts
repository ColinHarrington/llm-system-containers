export type VmStatus = "NotCreated" | "Stopped" | "Starting" | "Running";

export interface Sandbox {
  name: string;
  status: "Running" | "Stopped";
}
