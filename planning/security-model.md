# Security Model — Defense in Depth

The core security philosophy: **infrastructure is the backstop.** AI agents typically have
their own permission frameworks, but those are imperfect. The sandbox, network isolation,
and Linux user permissions form hard infrastructure layers so that the agent's own rules
are never the only thing protecting the host.

## Layered backstops

```
Agent permission framework .............. first line, imperfect
  └─ Linux user permissions ............. UID isolation per agent, file ownership
      └─ LXC container isolation ........ namespaces / cgroups, unprivileged
          └─ Incus network policies ..... per-container, per-user traffic rules
              └─ Tetragon eBPF .......... kernel-level: network, syscalls, filesystem
```

Each layer is independent. A compromised or misbehaving agent that defeats or ignores its
own permission framework still hits Linux user permissions, then container isolation, then
network policy, then non-bypassable kernel enforcement.

## Tetragon (eBPF)

[Tetragon](https://github.com/cilium/tetragon) runs inside the Playground VM and enforces
controls at the kernel level — non-bypassable from userspace — across three layers:

- **Network** — permit/deny connections per container, per UID, per destination.
- **Syscalls** — restrict dangerous syscalls for agent users (e.g. `ptrace`, `mount`,
  `kexec`).
- **Filesystem** — path-level access control beyond standard Unix permissions.

Policies are expressed **per container and per user (UID)**, matching the Layer 3 user
model ([architecture/sandbox-containers.md](architecture/sandbox-containers.md)).

## Network controls

- Incus manages bridge networks between containers and to the service containers.
- Per-container and per-UID rules decide what can talk to what:
  - Which sandbox containers may reach which services (e.g. LiteLLM yes, raw internet no).
  - An agent's UID can have different egress rules than the human user's UID in the same
    container.
- Network inspection/capture is available as a service (mitmproxy + Zeek) — see
  [services/network-inspection.md](services/network-inspection.md).

## Workspace mounting

Host workspace directories mount into containers with tiered, delegated access:

- **Human user** — full read/write to the mounted workspace, works naturally as if local.
- **Agent users** — scoped/delegated: read-only to the whole workspace, or read/write only
  to specific delegated subdirectories.
- **Kernel enforcement** — Tetragon's filesystem layer backs this up so an agent that tries
  to traverse outside its allowed paths is denied regardless of what it believes it can do.

The pattern: the human **owns** the workspace; agents are **granted** portions of it; the
kernel enforces the grants.

## Credential isolation

Agents never hold real API/token credentials. They use **virtual keys** issued by the
LiteLLM proxy; real credentials live only in the proxy service container. See
[services/llm-proxy.md](services/llm-proxy.md).

## Open items

- Concrete Tetragon policy authoring model and how it maps to per-agent grants.
- How workspace delegation is expressed in the UI/CLI and translated to mounts + policy.
- Default-deny vs. default-allow posture per layer, and sane preset profiles.
