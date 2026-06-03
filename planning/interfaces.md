# Interfaces — GUI & CLI

Two interfaces are planned over the same underlying control plane.

## GUI app

A desktop GUI app to **manage Playgrounds and sandboxes**:

- Run the **setup wizard** (CPU, memory, services, networking) to configure a Playground.
- Show whether the Playground VM is **running / stopped / starting**.
- Manage sandbox containers (create, launch, stop, destroy).
- Surface observability and the **interrupt/steer** controls for running agents (see
  [services/observability.md](services/observability.md)).

## CLI tooling

CLI alongside the GUI for scripting and power users — same operations as the GUI, suitable
for automation and for the "software factory" use case where orchestrations are built
programmatically.

## Shared control plane

Both interfaces should sit on a common control plane / API so behavior stays consistent and
so the CLI can drive everything the GUI can. This is where the **interrupt/steer** agent
operations live.

## Open items (tech stack — undecided)

These are tracked in [open-questions.md](open-questions.md):

- GUI framework / language (e.g. Tauri, Electron, native, etc.).
- CLI language (likely aligned with the provisioning layer; Go pairs naturally with Lima/Incus).
- Control-plane API shape (local daemon? library? REST/gRPC?).
- How the GUI/CLI talk to Lima, Incus, and the services inside the Playground.
