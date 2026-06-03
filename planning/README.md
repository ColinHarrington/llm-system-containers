# Planning Docs

Internal design and planning documentation for the **llm-system-containers** project.

> These are **planning docs** — design notes and architectural intent, not user-facing
> documentation. User-focused docs will live elsewhere (e.g. top-level `README.md` and
> `docs/`) if/when this becomes a public open-source project.

## Project at a glance

**Project:** **llm-system-containers** — a platform/tool that enables **LLM System
Containers (LLMSC)**, i.e. *Little Linux Managed System Containers*. ("LLM" = *Little Linux
Managed* + *Large Language Model*.) See [naming.md](naming.md) for the full naming decisions.

An open-source platform that provisions **Linux system container environments for AI
agents**, built on **Incus / unprivileged LXC** (system containers, not application/process
containers). Each container is a full, lightweight Linux development environment, typically
run as an ephemeral, safer **sandbox**. The platform provides defense-in-depth
infrastructure backstops so that imperfect agent permission frameworks are not the only
thing protecting the host.

Two CLIs: **`llmsc`** (manage individual containers) and **`llmsctl`** (manage the
platform). Target platforms: **Linux and macOS**.

## Index

- [naming.md](naming.md) — Project/unit names, the `llmsc`/`llmsctl` CLI split, and rationale
- [overview.md](overview.md) — Vision, differentiators, software-factory framing, the Host/L1/L2/L3 nesting model
- **architecture/** (layers = nesting levels)
  - [vm.md](architecture/vm.md) — L1: the VM, Incus, nested containerization, VM driver abstraction
  - [system-containers.md](architecture/system-containers.md) — L2: the LLMSC — user model, GUI/X-forwarding, workspace mounts
  - [app-containers.md](architecture/app-containers.md) — L3: nested Docker/Podman (key differentiator)
  - [networking.md](architecture/networking.md) — Addressing, split-horizon `.llmsc` DNS, host↔container routing, SSH
- [security-model.md](security-model.md) — Defense-in-depth, Tetragon, network + workspace controls
- [agent-profiles.md](agent-profiles.md) — Reusable permission bundles (researcher/tester/builder/validation/orchestrator)
- **services/** — shared infra; may run in L1 or in their own L2 container
  - [README.md](services/README.md) — Catalog, plugin model, L1-vs-L2 placement choice
  - [llm-proxy.md](services/llm-proxy.md) — LiteLLM, virtual keys
  - [observability.md](services/observability.md) — VictoriaMetrics / Loki / Grafana + Phoenix
  - [shared-storage.md](services/shared-storage.md) — SeaweedFS / RustFS
  - [network-inspection.md](services/network-inspection.md) — mitmproxy + Zeek
  - [_future.md](services/_future.md) — Forgejo, NATS, openclaw
- [file-transfer.md](file-transfer.md) — `llmsc cp` and moving files host↔container, container↔container
- [custom-images.md](custom-images.md) — Custom image building + internal registry
- [interfaces.md](interfaces.md) — GUI app + CLI
- [mvp.md](mvp.md) — The MVP path, scoped
- [open-questions.md](open-questions.md) — Naming, plugin interfaces, tech-stack decisions

## Status

Early design / brainstorming. No code yet. Tech-stack choices for the GUI app, CLI, and
provisioning layer are **undecided** — tracked in [open-questions.md](open-questions.md).
This document set is the umbrella; individual docs are expected to grow into their own
layered plans.
