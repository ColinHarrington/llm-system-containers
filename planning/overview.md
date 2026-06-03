# Overview

## The problem

AI coding agents increasingly run autonomously and need real, capable environments —
shells, package managers, browsers, the ability to build and run software. Giving them
that capability on the host is dangerous. The agents' own permission frameworks exist but
are imperfect; they should not be the only thing standing between an agent and the host.

## The idea

Provide **Linux system container environments for AI agents** with infrastructure
backstops at the kernel, container, user, and network layers. Built on **Incus /
unprivileged LXC** — *system* containers, not application/process containers. Each unit is
an **LLM System Container (LLMSC)** — a full, lightweight Linux machine with its own init,
users, services, and the ability to run nested Docker/Podman and GUI apps — typically run as
an ephemeral, safer **sandbox**. See [naming.md](naming.md).

The platform is meant to be usable to operate a **software factory** — letting technical
people build many kinds of agent orchestrations inside a controlled, observable
environment.

## Why system containers (not Docker)

| | Application container (Docker) | System container (Incus/LXC) |
|---|---|---|
| Mental model | One process / service | A whole machine |
| Init system | Usually none | Full init, services |
| Users | Typically root, single-purpose | Multiple real Linux users |
| Nested containers | Awkward | Natural (run Docker inside) |
| Fit for "a dev environment" | Poor | Strong |

System containers map cleanly onto "give this agent its own little Linux box."

## Differentiators

What sets this apart from existing agent sandboxes:

- **Real nested containers (L3).** An agent can run **rootless Docker/Podman inside** its
  sandbox — `docker build`, `docker compose`, local CI — *without* privileged
  Docker-in-Docker and *without* breaking the security backstops. Most sandboxes are
  process-level (no real machine) or app-container-level (need `--privileged` to nest). See
  [architecture/app-containers.md](architecture/app-containers.md).
- **A whole system, not a process.** Each unit is a full little Linux box (init, users,
  services), so agents work like developers, not script-runners.
- **Infrastructure backstops.** Containment, per-user isolation, network policy, and
  kernel-level (eBPF) enforcement back up the agent's own permissions. See
  [security-model.md](security-model.md).

## Layers (nesting model)

"Layer" means a *level of virtualization nesting* — not a role.

```
Host                  your computer (Linux/macOS); llmsc/llmsctl installed here
└─ L1: VM             native VM running Incus — one kernel, shared by L2/L3
   └─ L2: system container   unprivileged Incus/LXC — the LLMSC, run as a "sandbox"
        └─ L3: app container  rootless Docker/Podman nested inside an L2 container
```

The L1→L2→L3 nesting is **containerization** (one shared kernel), *not* nested
virtualization. L2 and L3 are **unprivileged/rootless** — never privileged.

1. **L1 — VM** (`llmsc-vm`) — a host-native VM (matching host architecture) running Incus.
   Analogous to Docker Desktop / Colima / Lima. Its single Linux kernel is shared by L2 and
   L3 — the nesting below is *containerization*, not nested virtualization. See
   [architecture/vm.md](architecture/vm.md).

2. **L2 — System containers (LLMSC)** — unprivileged Incus/LXC system containers: the
   agent/human **workspaces**, run as sandboxes (one Linux user per agent + a human operator
   login). See [architecture/system-containers.md](architecture/system-containers.md).

3. **L3 — App containers** — nested Docker/Podman inside an L2 container; the key
   differentiator above. See [architecture/app-containers.md](architecture/app-containers.md).

**Services** (LLM proxy, observability, storage, network inspection) are an orthogonal
concern: each may run **directly in the L1 VM** or in its **own L2 container** — an
isolation choice, not a layer. See [services/README.md](services/README.md).

## Design principles

- **Infrastructure as backstop** — layered, defense-in-depth controls so a misbehaving or
  compromised agent is contained by the kernel, not just by its own rules. See
  [security-model.md](security-model.md).
- **Observable, interruptable, steerable** — humans can watch agents, interrupt them, and
  re-steer them. See [services/observability.md](services/observability.md).
- **Memory-efficient and open-source** — tooling choices favor lean, OSS components.
- **Pluggable everywhere** — VM drivers and services are abstractions with swappable
  implementations.

## Interfaces

A **GUI app** plus two CLIs — **`llmsc`** (containers) and **`llmsctl`** (the VM/platform) —
to manage the VM and sandboxes (including VM running/stopped status). See
[interfaces.md](interfaces.md).
