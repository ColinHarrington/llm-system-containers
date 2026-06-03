# Networking — addressing, DNS, routing, SSH

> This doc covers **how things are addressed and reached** (L1/L2 routing, DNS, SSH).
> *Egress policy / ACLs* live in [../security-model.md](../security-model.md); *traffic
> inspection* (mitmproxy/Zeek) lives in [../services/network-inspection.md](../services/network-inspection.md).

## Context / requirements

- Some **services must be usable from the host** — e.g. **LiteLLM** (call the proxy from host
  tooling) and **Forgejo** (if installed). This is a core reason services may live in their own
  L2 container: each gets a **routable service interface (its own IP)**.
- **Forgejo needs port 22** for git-over-SSH. That only works if it has its **own routable IP**
  — it can't share the host's single `:22` (the host's sshd). Port-forwarding doesn't solve
  this cleanly.
- **Split-horizon DNS**: resolve a private TLD (e.g. `forgejo.llmsc`) on the host by delegating
  `.llmsc` resolution to the VM.
- **Sandboxes must route between themselves** (east-west).
- **SSH into containers** must work.

## Design

### 1. Routable container addressing
Containers get stable IPs on an Incus bridge, and that subnet is made **routable from the
host** (not just NAT'd inside the VM):

- Host adds a **static route** for the container subnet via the VM's host-facing IP.
- The VM **forwards** (`net.ipv4.ip_forward`) and does **not NAT** that subnet, so real
  addresses are preserved end to end.
- Result: the host reaches any container by IP, and **container ↔ container** routing is free
  (same bridge). Each service container (Forgejo on `:22`, LiteLLM on its port) has a real IP.

### 2. Split-horizon DNS for `.llmsc`
- Inside the VM, the Incus bridge's built-in DNS (dnsmasq) is configured with
  **`dns.domain = llmsc`**, so `<name>.llmsc` resolves to the container's IP. (A small CoreDNS
  in the VM is the alternative if we need more control — e.g. service aliases.)
- The VM exposes that resolver on a host-reachable IP.
- On the host, **only `.llmsc`** is delegated to the VM resolver:
  - **macOS** — `/etc/resolver/llmsc` pointing at the VM DNS IP (native per-domain resolver).
  - **Linux** — systemd-resolved per-domain routing (`~llmsc` → VM DNS), or dnsmasq
    `server=/llmsc/<vm-dns-ip>`.
- DNS + routing together: the name resolves **and** the IP is reachable. Examples:
  `litellm.llmsc`, `forgejo.llmsc`, `web-agent-01.llmsc`.

### 3. SSH
Each container runs `sshd`. With routable IP + DNS, `ssh operator@web-agent-01.llmsc` works on
standard **:22**, and Forgejo serves git-over-SSH on its own `:22`. The `incus exec`-based
`llmsc shell user@<name>` path remains for no-SSH-needed access; SSH is for git remotes, IDE
remote-dev, etc.

### 4. East-west + policy
Containers on the bridge can address each other by default; **DNS gives them names**. This is
the *transport* layer only — **addressable ≠ allowed**. The default-deny egress and per-UID
network ACLs ([../security-model.md](../security-model.md)) still decide which flows are
permitted (e.g. the `isolated` network reaches nothing but LiteLLM). Reconcile as:
**addressable, then policy-gated.**

### 5. Host integration (needs privilege)
The host-side changes — adding a route, writing `/etc/resolver/llmsc`, touching
systemd-resolved — require **elevated permissions**. These belong in `llmsctl init` (or a
dedicated `llmsctl net setup`) with an explicit privilege prompt, and a clean teardown.

## Why this over alternatives

- **Port-forwarding only** — simplest, but one host `:22` and per-port NAT can't give Forgejo
  its own `:22` or give each service a real IP. Rejected for these requirements.
- **Overlay (WireGuard / Tailscale)** — heavier, but the likely **multi-host / remote** story
  (Proxmox, remote VMs) where host static routes don't reach. Good **future** option, not MVP.

## Precedent

**OrbStack** ships this exact UX for its machines/containers: routable IPs, `<name>.orb.local`
DNS, and SSH-in. Replicating it for Incus system containers is well-trodden ground.

## TLD choice

`.llmsc` is on-brand and fine for a controlled split-horizon resolver. RFC-safe alternatives if
future real-TLD collision is a concern: `.llmsc.test` (`.test` is reserved by RFC 6761) or a
`.internal` suffix. Leaning **`.llmsc`** with this caveat noted.

## Spike finding (Linux)

The **default Lima network is user-mode NAT** — the host has no route to the VM or the container
subnet, so routable addressing needs a **host-reachable VM network first**: `socket_vmnet`
shared net (macOS, turnkey), a bridged Lima net (Linux), or a **WireGuard overlay**
(cross-platform; host listens, VM dials out via `host.lima.internal` to beat the NAT — also the
multi-host/remote story). See [../spike-plan.md](../spike-plan.md).

## Open items

- Pick the default host-reachable transport per VM driver (socket_vmnet / bridge / WireGuard) —
  partially explored in the spike; macOS `socket_vmnet` still to validate.
- Subnet selection that avoids clashing with common host LAN ranges.
- IPv4 vs dual-stack; whether services get stable IPs vs DNS-only.
- DNS source of truth: Incus dnsmasq `dns.domain` vs a dedicated CoreDNS (aliases, service
  names vs container names).
- SSH key/auth model for container access (per-agent keys, operator key, host-agent forwarding).
- Teardown/repair of host resolver + routes on stop/uninstall.
