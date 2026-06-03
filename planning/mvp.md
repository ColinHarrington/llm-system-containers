# MVP

The smallest version that demonstrates the core value: a user can stand up a Playground and
get a working, observable, credential-isolated sandbox for an agent.

## MVP path

1. **Install** the app.
2. **Wizard** configures the Playground (CPU, memory, services, networking).
3. **GUI shows** whether the Playground VM is running.
4. **Launch** the Playground VM via **Lima**.
5. **Incus bootstraps** inside the VM.
6. **LiteLLM + observability stack** (VictoriaMetrics/Loki/Grafana + Phoenix) run as service
   containers.
7. **Create/launch a basic sandbox container** with an agent user + a human user.

## In scope for MVP

- VM provider: **Lima** only (both macOS and Linux).
- Services: **LiteLLM**, **VictoriaMetrics + Loki + Grafana**, **Phoenix**.
- Sandbox container with the **two-user model** (agent user + human user).
- GUI showing VM status + the wizard; basic CLI.

## Explicitly NOT in MVP

- Other VM providers (Parallels, libvirt/virt-manager, Proxmox).
- Forgejo, shared storage, network inspection, NATS (Core/Optional/Future).
- Custom image building UI (may rely on base images initially).
- Full Tetragon policy authoring (security layers phased in — see
  [security-model.md](security-model.md)).
- GUI app for running real GUI apps via X-forwarding (Layer 3 capability, phased).

## Success criteria

- One-command/one-click from install → running Playground.
- A sandbox container an agent can use, where:
  - the agent reaches LLMs only via a **LiteLLM virtual key**, and
  - the agent's LLM calls are **traced in Phoenix**, and
  - system metrics/logs are visible in **Grafana**.

## Open items

- Where the line sits between MVP and the first "Core" follow-up (likely shared storage +
  network inspection + early Tetragon policy).
- Whether GUI or CLI lands first.
