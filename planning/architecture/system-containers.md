# L2 — System Containers (LLMSC)

L2 is the heart of the system: **unprivileged Incus/LXC system containers**, each a full
little Linux environment. A unit is an **LLMSC** (*Little Linux Managed System Container* —
see [../naming.md](../naming.md)). They run *inside* the L1 [VM](vm.md) and host the L3
[app containers](app-containers.md) (nested Docker/Podman).

## Workspace containers vs. services

L2 system containers are first and foremost the **workspace / sandbox** units — where agents
and humans work (the product). Each is usually run as a **sandbox** (ephemeral, safer).

**Services** (LiteLLM, Phoenix, Grafana, SeaweedFS, …) are a separate concern, and *where* a
service runs is an **architecture / isolation choice**, not a fixed layer:

- directly in the **L1 [VM](vm.md)** (simpler, lower overhead, less isolation), or
- in its **own L2 system container** (more isolation; same tech as a workspace container).

So a service *container*, when that choice is made, is also an L2 system container — but
services are **not inherently L2**. The service catalog and this placement tradeoff live in
[../services/README.md](../services/README.md). The rest of this doc covers **workspace**
containers.

## Characteristics

- **Unprivileged LXC system containers** — full init, services, real users; not single
  process/application containers. **Always unprivileged — privileged containers are never
  used; this is a hard security-posture rule**, and L3 nesting works without relaxing it.
- A lightweight Linux development environment.
- **"Sandbox" = the mode**, not the name: workspace LLMSCs are typically run ephemeral and
  safer (disposable). Lifecycle spectrum: throwaway sandboxes → long-lived environments
  hosting many agents.
- Created from **images** — base distro or user-built custom images (pre-packaged tooling,
  IDEs, browsers, runtimes). See [../custom-images.md](../custom-images.md).

## User model

Each container has multiple **real Linux users**, the first isolation boundary:

- **One Linux user per agent** — isolated home directory, scoped permissions. Agents in the
  same container cannot freely read/write each other's files.
- **One human operator login** — a normal account to inspect, debug, and interact directly.

This per-user separation is what the kernel-level controls (Tetragon, file permissions,
network rules) hang off — policies are expressed per container **and** per UID. The CLI
surfaces it as `llmsc shell user@<container>`. See [../security-model.md](../security-model.md).

## Nested app containers (L3)

Workspace containers can run **Docker/Podman** inside them — this is a key differentiator,
covered in [app-containers.md](app-containers.md). It relies on **nested containerization** —
unprivileged LXC with `security.nesting` (container-in-container, sharing the [VM](vm.md)'s
kernel) — **not** nested virtualization.

## GUI applications

Agents and humans need to run **real GUI apps** (browsers, IDEs) from inside the containers.
Mechanism: **X-forwarding or another display technology** (Wayland forwarding, VNC, or
remote desktop). Exact tech TBD. This lets an agent drive a real browser, or a human open an
IDE, inside the sandbox.

## Workspace mounts

Host workspace directories mount into containers with tiered access:

- **Human user** — full read/write to the mounted workspace.
- **Agent users** — scoped/delegated (read-only, or write only to specific subdirs), backed
  by kernel-level enforcement.

Details in [../security-model.md](../security-model.md). An alternative/complement to raw
bind mounts is shared storage (SeaweedFS) with scoped paths — see
[../services/shared-storage.md](../services/shared-storage.md).

## Open items

- Display-forwarding technology choice (X11 vs Wayland vs VNC/RDP).
- Incus profiles for nested Docker/Podman in unprivileged containers.
- Conventions for naming / labeling agent users and mapping them to orchestrations.
- How throwaway vs. persistent containers are modeled and managed.
