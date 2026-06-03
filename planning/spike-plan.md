# De-risking Spike — Core Feasibility

## Purpose

Prove the **riskiest architectural assumptions by hand**, on real machines, before writing any
Rust. Timeboxed and throwaway (not TDD'd — see [testing.md](testing.md)); each step has an
explicit **pass/fail** criterion, and passing steps become CI integration tests later.

## What we're de-risking (in priority order)

1. **L3 differentiator** — rootless Docker/Podman inside an **unprivileged** Incus system
   container (nested *containerization*, not virtualization). If this doesn't work cleanly
   unprivileged, the headline differentiator is at risk. ([architecture/app-containers.md](architecture/app-containers.md))
2. **Host ↔ container reachability** — routable container IP from the host, split-horizon
   `.llmsc` DNS, and SSH on `:22`. The networking is the part most likely to bite, especially
   on macOS. ([architecture/networking.md](architecture/networking.md))
3. **Cross-platform** — does the above hold on **both Linux and macOS (Apple Silicon)**? The
   macOS VM networking (vmnet) is the highest-uncertainty piece.

> Run on **both** a Linux host and a macOS (Apple Silicon) host. Commands are best-known
> starting points — expect to adjust, especially Phase 3 on macOS. Note exact deviations in
> Findings.

> **Automated runner:** these phases are scripted in **[`scripts/spike.py`](../scripts/spike.py)**
> (a `uv` single-file script). Run `uv run scripts/spike.py all` (or a single `phaseN`); it
> reports per-step pass/fail and prints the manual sudo/host commands for Phase 3/4. The commands
> below document what it does.

---

## Phase 0 — VM with Incus

**Linux & macOS:**
```
brew install lima            # macOS;  Linux: install from the Lima releases/package
limactl start --name=llmsc --cpus=4 --memory=8 template://ubuntu-24.04
limactl shell llmsc
```
Inside the VM, install Incus (e.g. via the zabbly repo) and init:
```
sudo apt-get update && sudo apt-get install -y incus
sudo incus admin init --minimal
incus version
```
**Pass:** `incus version` works; `incus list` runs (empty).

## Phase 1 — L2 unprivileged system container + users
```
incus launch images:ubuntu/24.04 web-agent-01
incus config get web-agent-01 security.privileged      # expect empty/false (unprivileged)
incus exec web-agent-01 -- useradd -m -s /bin/bash operator
incus exec web-agent-01 -- useradd -m -s /bin/bash agent-claude
incus exec web-agent-01 -- bash -c 'id agent-claude'
```
**Pass:** container is **unprivileged**; both users exist; `incus exec ... -- whoami` works.

## Phase 2 — L3: rootless Podman inside the unprivileged L2  ⭐ (top risk)
```
incus config set web-agent-01 security.nesting=true
incus exec web-agent-01 -- apt-get update
incus exec web-agent-01 -- apt-get install -y podman
# ensure subuid/subgid for the agent user, then run ROOTLESS as that user:
incus exec web-agent-01 -- su - agent-claude -c 'podman run --rm hello-world'
incus exec web-agent-01 -- su - agent-claude -c 'printf "FROM alpine\nRUN echo hi > /x\n" > Dockerfile && podman build -t t .'
incus exec web-agent-01 -- su - agent-claude -c 'podman run --rm t cat /x'
```
**Pass:** `hello-world` runs, image **builds**, and runs — all **rootless, in an unprivileged
container, no `--privileged`, no Docker-in-Docker privilege**. (Docker rootless is a secondary
follow-up; Podman is the cleaner rootless test.)
**If it fails:** check `security.nesting`, `/etc/subuid`+`/etc/subgid` for the user, and
user-namespace availability; record the exact blocker.

## Phase 3 — Routable IP + split-horizon `.llmsc` DNS + SSH  ⭐ (top risk, esp. macOS)
Give the VM a host-reachable IP (shared vmnet), then route to the Incus bridge subnet.
```
# Reconfigure the Lima VM with a shared network (socket_vmnet) so the host can reach the VM IP.
# Inside VM: find bridge subnet + container IP:
incus network get incusbr0 ipv4.address          # e.g. 10.x.x.1/24
incus list web-agent-01                            # note the container IP
incus network set incusbr0 ipv4.nat false          # preserve addresses (no NAT) for routing
incus network set incusbr0 dns.domain llmsc        # <name>.llmsc resolves via Incus dnsmasq
sudo sysctl -w net.ipv4.ip_forward=1               # VM forwards to the bridge
```
On the **host**, add a route to the container subnet via the VM IP, and delegate `.llmsc` DNS:
```
# macOS:
sudo route -n add -net <container-subnet> <vm-ip>
sudo mkdir -p /etc/resolver && printf "nameserver <vm-dns-ip>\n" | sudo tee /etc/resolver/llmsc
# Linux:
sudo ip route add <container-subnet> via <vm-ip>
sudo resolvectl dns <iface> <vm-dns-ip>; sudo resolvectl domain <iface> '~llmsc'
```
Verify from the host:
```
ping <container-ip>            # routable IP reachable
ping web-agent-01.llmsc        # split-horizon DNS resolves
incus exec web-agent-01 -- bash -c 'apt-get install -y openssh-server && systemctl enable --now ssh'
ssh operator@web-agent-01.llmsc   # SSH on standard :22 by name
```
**Pass:** host pings the container by **IP** and by **`.llmsc` name**, and **SSH lands** as
`operator` on `:22`.
**If it fails:** isolate which layer broke — routing (IP ping), DNS (name resolves), or sshd —
and record per-platform differences (this is the whole point of the spike).

## Phase 4 — Service in its own L2 with routable `:22` (Forgejo-shaped)
```
incus launch images:ubuntu/24.04 forgejo
incus network set ... ; (same routing/DNS as Phase 3)
# run anything binding :22 with its own IP; verify from host:
ssh -p 22 someuser@forgejo.llmsc
```
**Pass:** a second container has its **own routable IP** and serves on **`:22`** from the host
without colliding with the host's sshd — confirming the routable-service-interface rationale.

## Phase 5 — Cross-platform delta
Repeat Phases 2–4 on the **other** platform; record what differed (esp. macOS vmnet routing and
the resolver setup). **Pass:** core results hold on both, with deviations documented.

---

## Findings (fill in as you run)

| Phase | Linux | macOS (Apple Silicon) | Notes / blockers |
|---|---|---|---|
| 0 VM + Incus | ☐ | ☐ | |
| 1 L2 + users | ☐ | ☐ | |
| 2 rootless L3 ⭐ | ☐ | ☐ | |
| 3 routable + DNS + SSH ⭐ | ☐ | ☐ | |
| 4 service `:22` | ☐ | ☐ | |
| 5 cross-platform delta | ☐ | ☐ | |

**Decision after spike:** which assumptions held, what needs a different approach (e.g.
overlay/WireGuard if host-route routing is too painful on macOS), and which steps become CI
integration tests ([testing.md](testing.md)).
