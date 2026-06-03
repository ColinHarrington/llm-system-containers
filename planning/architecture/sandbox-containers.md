# Layer 3 — Sandbox Containers

The **sandbox containers** are the point of the whole system: unprivileged LXC system
containers, each a full little Linux environment where agents (and humans) work.

## Characteristics

- **Unprivileged LXC system containers** — full init, services, real users; not single
  process/application containers.
- A lightweight Linux development environment.
- **Lifecycle spectrum:**
  - Simple **throwaway** sandboxes (spin up, use, discard).
  - Long-lived environments hosting **many agents** at once.
- Created from **images** — base distro images or user-built custom images (pre-packaged
  tooling, IDEs, browsers, runtimes). See [../custom-images.md](../custom-images.md).

## User model

Each container has multiple **real Linux users**, which is the first isolation boundary:

- **One Linux user per agent** — isolated home directory, scoped permissions. Different
  agents in the same container cannot freely read/write each other's files.
- **One human operator login** — a normal account the operator uses to inspect, debug, and
  interact with the basic system directly.

This per-user separation is what the kernel-level controls (Tetragon, file permissions,
network rules) hang off of — policies are expressed per container **and** per UID. See
[../security-model.md](../security-model.md).

## Nested containers

Sandbox containers can run **Docker / Podman** inside them. This depends on:

- Nested virtualization provided by the Playground VM (Layer 1).
- Appropriate Incus profile configuration for nested containers in unprivileged LXC.

## GUI applications

Agents and humans need to run **real GUI apps** — browsers, IDEs, etc. — from inside the
containers. Mechanism: **X-forwarding or another display technology** (e.g. Wayland
forwarding, VNC, or a remote-desktop approach). Exact tech TBD.

This is what lets an agent drive a real browser, or a human pop open an IDE, inside the
sandbox.

## Workspace mounts

Host workspace directories are mounted into containers, with tiered access:

- **Human user** — full read/write to the mounted workspace.
- **Agent users** — scoped/delegated access (e.g. read-only, or write only to specific
  subdirectories), backed by kernel-level enforcement.

Details in [../security-model.md](../security-model.md). An alternative/complement to raw
bind mounts is shared storage (SeaweedFS/RustFS) with scoped paths — see
[../services/shared-storage.md](../services/shared-storage.md).

## Open items

- Display-forwarding technology choice (X11 vs Wayland vs VNC/RDP).
- Incus profiles for nested Docker/Podman in unprivileged containers.
- Conventions for naming / labeling agent users and mapping them to orchestrations.
- How throwaway vs. persistent containers are modeled and managed.
