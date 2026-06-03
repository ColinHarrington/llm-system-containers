# MVP

The smallest version that demonstrates the core value: a user can stand up the VM and
get a working, observable, credential-isolated sandbox for an agent.

## MVP path

1. **Install** the app.
2. **Wizard** configures the VM (CPU, memory, services, networking).
3. **GUI shows** whether the VM is running.
4. **Launch** the VM via **Lima**.
5. **Incus bootstraps** inside the VM.
6. **LiteLLM + observability stack** (VictoriaMetrics/Loki/Grafana + Phoenix) run as services
   (in L1 or their own L2 containers).
7. **Create/launch a basic L2 system container** with an agent user + a human user.

## In scope for MVP

- VM driver: **Lima** only (both macOS and Linux).
- Services: **LiteLLM**, **VictoriaMetrics + Loki + Grafana**, **Phoenix**.
- Sandbox container with the **two-user model** (agent user + human user).
- GUI showing VM status + the wizard; basic CLI.

## Explicitly NOT in MVP

- Other VM drivers (Parallels, libvirt/virt-manager, Proxmox).
- Forgejo, shared storage, network inspection, NATS (Core/Optional/Future).
- Custom image building UI (may rely on base images initially).
- Full Tetragon policy authoring (security backstops phased in — see
  [security-model.md](security-model.md)).
- Running real GUI apps via X-forwarding (an L2 capability, phased).

## Success criteria

- One-command/one-click from install → running VM.
- A sandbox an agent can use, where:
  - the agent reaches LLMs only via a **LiteLLM virtual key**, and
  - the agent's LLM calls are **traced in Phoenix**, and
  - system metrics/logs are visible in **Grafana**.

## Open items

- Where the line sits between MVP and the first "Core" follow-up (likely shared storage +
  network inspection + early Tetragon policy).
- Whether **nested Docker/Podman (L3)** — the headline differentiator
  ([architecture/app-containers.md](architecture/app-containers.md)) — belongs in the MVP or
  the first follow-up. Strong case for MVP given it's the core differentiator.
- Whether GUI or CLI lands first.
