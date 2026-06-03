# Interfaces — GUI & CLI

Two interfaces are planned over the same underlying control plane.

## GUI app

A desktop GUI app to **manage the VM and sandboxes**:

- Run the **setup wizard** (CPU, memory, services, networking) to configure the VM.
- Show whether the **VM** is **running / stopped / starting**.
- Manage L2 system containers (create, launch, stop, destroy).
- Surface observability and the **interrupt/steer** controls for running agents (see
  [services/observability.md](services/observability.md)).

**Out of GUI scope for now:** visualizing/managing **L3 app containers** (nested
Docker/Podman). The L3 *capability* remains a platform differentiator
([architecture/app-containers.md](architecture/app-containers.md)) — agents still run
rootless containers inside their sandbox — but surfacing them in the GUI is deferred,
likely as a future **plugin**, not a core screen.

## CLI tooling

Two commands, split by audience and cadence (control plane vs. daily driver) — see
[naming.md](naming.md):

- **`llmsctl`** — platform control plane (occasional): `init`, `up`/`down`,
  `status`, `services enable …`. Drives the VM (`llmsc-vm`) and services.
- **`llmsc`** — container plane (daily driver): `launch`, `ls`, `shell user@<name>`, `rm`, and
  `cp` (copy files host↔container and container↔container — see
  [file-transfer.md](file-transfer.md)). Manages individual LLM System Containers.

Both cover the same operations as the GUI, suitable for automation and the "software
factory" use case where orchestrations are built programmatically. The `user@<container>`
form reflects the two-user model (agent users + human operator) in
[architecture/system-containers.md](architecture/system-containers.md).

## Shared control plane

Both interfaces should sit on a common control plane / API so behavior stays consistent and
so the CLI can drive everything the GUI can. This is where the **interrupt/steer** agent
operations live.

## Tech stack (see [tech-stack.md](tech-stack.md))

- **Rust** core: a shared **`llmsc-core`** crate that the CLIs and the **Tauri** GUI all link —
  so the CLI is fully capable standalone and GUI/CLI stay at parity.
- **Config is declarative on-disk** (TOML), the single source of intent shared by CLI and GUI.
- **Incus is the runtime source of truth** (managed in its own Incus project); config is desired
  state reconciled to it, and raw `incus` stays usable a layer beneath.
- **Library-first; daemon deferred** — live features stream from the Incus events API +
  Phoenix/Loki.

Remaining preference choices (config TOML vs YAML; React vs Svelte) are in
[tech-stack.md](tech-stack.md) / [open-questions.md](open-questions.md).
