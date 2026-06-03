# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project status

**Planning stage — no code yet.** The repository currently contains only design/planning
documents under `planning/`. There is no build system, test suite, lint config, or
application code. Do not invent build/test commands; there are none to run. Tech-stack
choices (GUI framework, CLI language, provisioning layer) are deliberately **undecided** and
tracked in `planning/open-questions.md` — do not assume a language or framework until those
are resolved.

## What this project is

**llm-system-containers** is an open-source platform/tool that enables **LLM System
Containers (LLMSC)** — *Little Linux Managed System Containers* — for AI agents. Built on
**Incus / unprivileged LXC**: *system* containers (full lightweight Linux machines), not
application/process containers like Docker. Each LLMSC is typically run as an ephemeral,
safer **sandbox** ("sandbox" is a mode, not the name). The goal is defense-in-depth
infrastructure backstops so an agent's own (imperfect) permission framework is never the
only thing protecting the host. Targets **Linux and macOS**.

Two CLIs are planned (names decided, implementation language not — see
`planning/naming.md`):
- **`llmsc`** — daily driver for individual containers (`launch`, `ls`, `shell user@<name>`,
  `rm`).
- **`llmsctl`** — platform/host control plane (`init`, `up`/`down`, `status`,
  `services …`).

## Three-layer architecture (the big picture)

Understanding this requires reading several docs in `planning/`; the layers are nested:

```
Host (Linux/macOS)
└── Layer 1: Playground VM ......... host-native VM running Incus, nested virt enabled
    ├── Layer 2: Service Containers . shared infra (LLM proxy, observability, storage, …)
    └── Layer 3: Sandbox Containers . agent + human Linux workspaces
```

- **Layer 1 — Playground VM** (`planning/architecture/playground-vm.md`): a host-native VM
  (Docker-Desktop/Colima analogue) running Incus. VM backend is a **pluggable provider
  abstraction**; MVP uses **Lima** on both platforms (Linux: QEMU+KVM; macOS: Apple
  Virtualization/QEMU). Future: Parallels, libvirt/virt-manager, Proxmox.
- **Layer 2 — Service Containers** (`planning/architecture/service-containers.md`): LXC
  containers hosting shared services, intended to become **configurable plugins**.
- **Layer 3 — Sandbox Containers** (`planning/architecture/sandbox-containers.md`):
  unprivileged LXC system containers with a **two-user model** (one Linux user per agent +
  one human operator login). Support nested Docker/Podman and GUI apps via display
  forwarding.

## Key cross-cutting concepts

- **Defense-in-depth security** (`planning/security-model.md`): layered backstops — agent
  permissions → Linux UID isolation → LXC isolation → Incus network policy → **Tetragon
  eBPF** (kernel-level network/syscall/filesystem enforcement, per-container AND per-UID).
  Policies hang off the per-user model in Layer 3.
- **Credential isolation**: agents never hold real API keys — they use **virtual keys** from
  the LiteLLM proxy (`planning/services/llm-proxy.md`).
- **Observable / interruptable / steerable** (`planning/services/observability.md`): a core
  principle. Observability uses a memory-efficient OSS stack (Phoenix for LLM traces;
  VictoriaMetrics+Loki+Grafana for system). Interrupt/steer is a control-plane feature of
  this project itself.
- **Priority tiers** are used consistently across service docs: **MVP / Core / Optional /
  Future**. Preserve these when editing.

## Repository layout

- `planning/` — all design docs. Start with `planning/README.md` (the index), then
  `planning/overview.md`. `planning/mvp.md` defines the scoped first milestone;
  `planning/open-questions.md` is the decisions parking lot.
- These are **planning docs**, intentionally separate from future **user-facing
  documentation** (which will live at the top level / a `docs/` dir if the project goes
  public). Keep that separation.

## Working conventions in the docs

- Markdown wrapped at ~100 columns; tables for catalogs/comparisons; each doc ends with an
  **Open items** section.
- Docs cross-link with relative paths — keep links valid when adding/moving files.
- When a design decision is made, update the relevant doc **and** remove/resolve the
  corresponding entry in `planning/open-questions.md`.
