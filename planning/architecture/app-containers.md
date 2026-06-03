# L3 — App Containers (nested Docker/Podman)

L3 is **app containers — Docker/Podman running *inside* an L2 [system container](system-containers.md)**.
This is one of the project's **key differentiators**: an agent in a sandbox can run real
container workflows, not just execute snippets.

```
Host                  your computer
└─ L1: VM             one Linux kernel, shared down the stack
   └─ L2: system container   unprivileged LXC, security.nesting
        └─ L3: app container  Docker / Podman, rootless
```

## Why this is a differentiator

Most agent sandboxes fall into two buckets, and **neither lets an agent run real
containers**:

- **Process / syscall sandboxes** (bubblewrap, gVisor, seatbelt, microVM-per-process): they
  isolate *a process*. There's no real machine underneath, so `docker build` /
  `docker compose` isn't available or doesn't work.
- **App-container sandboxes** (give the agent a Docker container): running Docker *inside*
  Docker needs **`--privileged`** (classic DinD), which punches a hole through the isolation
  — the very thing a sandbox exists to prevent. Tools like **Sysbox** exist solely to work
  around this, which shows how much demand there is.

This project sidesteps both. Because L2 is a **full unprivileged LXC system container** with
`security.nesting`, an agent can run **rootless Podman/Docker as a normal user** — *without*
privileged mode, and *without* breaking the kernel-level backstops (Tetragon, network policy)
one layer up. This is **nested containerization** (container-in-container, sharing the VM's
single kernel), **not** nested virtualization — which is exactly why it's reliable across
hosts, including Apple Silicon. Real container workflows **and** intact security guarantees.

## What it unlocks (the "software factory" payoff)

This is what turns the environment from a code-runner into a real dev environment:

- `docker build` real images; run `docker compose` stacks (DB + cache + app) for integration
  work.
- Reproduce **CI locally**; run test infrastructure.
- Multi-service development, exactly as a human developer would.

An agent in here can do what a developer does, not just "execute a snippet."

## Security note

Nested L3 containers are **unprivileged + rootless**, which is categorically different from
privileged Docker-in-Docker. **Nothing in the stack runs privileged** — L2 is unprivileged
LXC, L3 is rootless; privileged containers are never used, and that is a hard part of the
security posture. The isolation and kernel-level enforcement from
[../security-model.md](../security-model.md) still apply at L2 and below — nesting does not
require relaxing them. This is the crux: capability without the usual security trade-off.

## Validated (spike)

Rootless **Podman builds and runs inside an unprivileged Incus container** — no `--privileged`,
no privileged DinD, **overlay** storage (fuse-overlayfs). Confirmed on Linux via
[../spike-plan.md](../spike-plan.md). **VM-bootstrap requirements** that fall out of this:

- **`kernel.apparmor_restrict_unprivileged_userns=0`** on the VM — **L1-VM-OS-specific**:
  Ubuntu 23.10+ defaults this to `1`, which blocks the nested user namespace (`cannot clone:
  Permission denied`); the VM image/bootstrap must set it (persisted in `/etc/sysctl.d`) or
  ship an AppArmor profile. A **Debian L1 VM** has no such restriction and likely needs no
  workaround — to validate.
- **`security.nesting=true`** on each L2 sandbox that runs L3.
- subuid/subgid ranges for the agent user (Ubuntu `useradd` adds these automatically).
- rootless deps in the image: `podman uidmap slirp4netns fuse-overlayfs`.

## Open items

- Default runtime: Podman (rootless-first) vs. Docker — and whether both are offered.
- Whether to relax apparmor userns broadly on the VM vs. a scoped AppArmor profile (security
  tradeoff — the VM is already the isolation boundary).
- Confirm the same on **macOS (Apple Silicon)** host (spike phase 5).
- Whether L3 networking is subject to the same inspection/policy as L2 (mitmproxy/Zeek,
  Tetragon) and how egress from nested containers is handled.
