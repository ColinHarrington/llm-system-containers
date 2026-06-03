# Planning Docs

Internal design and planning documentation for the **LLM Sandbox** project.

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
- [overview.md](overview.md) — Vision, software-factory framing, the three-layer architecture
- **architecture/**
  - [playground-vm.md](architecture/playground-vm.md) — Layer 1: the VM, Incus, nested virtualization, VM provider abstraction
  - [service-containers.md](architecture/service-containers.md) — Layer 2: service catalog and plugin model
  - [sandbox-containers.md](architecture/sandbox-containers.md) — Layer 3: user model, GUI/X-forwarding, nested Docker
- [security-model.md](security-model.md) — Defense-in-depth, Tetragon, network + workspace controls
- **services/**
  - [llm-proxy.md](services/llm-proxy.md) — LiteLLM, virtual keys
  - [observability.md](services/observability.md) — VictoriaMetrics / Loki / Grafana + Phoenix
  - [shared-storage.md](services/shared-storage.md) — SeaweedFS / RustFS
  - [network-inspection.md](services/network-inspection.md) — mitmproxy + Zeek
  - [_future.md](services/_future.md) — Forgejo, NATS, openclaw
- [custom-images.md](custom-images.md) — Custom image building + internal registry
- [interfaces.md](interfaces.md) — GUI app + CLI
- [mvp.md](mvp.md) — The MVP path, scoped
- [open-questions.md](open-questions.md) — Naming, plugin interfaces, tech-stack decisions

## Status

Early design / brainstorming. No code yet. Tech-stack choices for the GUI app, CLI, and
provisioning layer are **undecided** — tracked in [open-questions.md](open-questions.md).
This document set is the umbrella; individual docs are expected to grow into their own
layered plans.
