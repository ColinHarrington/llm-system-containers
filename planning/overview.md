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

## Three-layer architecture

```
Host (Linux or macOS)
└── Layer 1: Playground VM ............ native VM running Incus (nested virt enabled)
    ├── Layer 2: Service Containers ... LiteLLM, observability, storage, net inspection…
    └── Layer 3: Sandbox Containers ... agent + human Linux environments
```

1. **Playground VM** — a host-native VM (matching host architecture) that runs Incus.
   Analogous to how Docker Desktop / Colima stand up a VM. Provides nested virtualization
   so Docker/Podman work inside the sandbox containers. See
   [architecture/playground-vm.md](architecture/playground-vm.md).

2. **Service Containers** — LXC containers inside the Playground hosting shared
   infrastructure (LLM proxy, observability, storage, network inspection). Designed to
   become configurable plugins. See
   [architecture/service-containers.md](architecture/service-containers.md).

3. **Sandbox Containers** — the actual agent/human workspaces. One Linux user per agent
   plus a human operator login. Support nested Docker/Podman and GUI apps. See
   [architecture/sandbox-containers.md](architecture/sandbox-containers.md).

## Design principles

- **Infrastructure as backstop** — layered, defense-in-depth controls so a misbehaving or
  compromised agent is contained by the kernel, not just by its own rules. See
  [security-model.md](security-model.md).
- **Observable, interruptable, steerable** — humans can watch agents, interrupt them, and
  re-steer them. See [services/observability.md](services/observability.md).
- **Memory-efficient and open-source** — tooling choices favor lean, OSS components.
- **Pluggable everywhere** — VM providers and services are abstractions with swappable
  implementations.

## Interfaces

A **GUI app** to manage Playgrounds and sandboxes (including VM running/stopped status),
plus **CLI tooling**. See [interfaces.md](interfaces.md).
