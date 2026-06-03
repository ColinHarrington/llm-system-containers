# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project status

**Planning stage — no code yet.** The repository currently contains only design/planning
documents under `planning/` (plus GUI `mockups/`). There is no build system, test suite, lint
config, or application code. Do not invent build/test commands; there are none to run.

**Tech stack is chosen** (see `planning/tech-stack.md`): **Rust** core in a shared
**`llmsc-core`** crate that the CLIs (`llmsc`, `llmsctl`) and the **Tauri** GUI all link;
**CLI-first** (the CLI is fully capable standalone); **declarative on-disk config** (TOML
leaning) shared by CLI and GUI; **Incus is the runtime source of truth** (managed in its own
Incus project, raw `incus` always usable) with config reconciled to it; **library-first, daemon
deferred**. Incus is reached via its **REST API**; Lima via **`limactl`**. A few preference
details (config TOML vs YAML, React vs Svelte) remain open in `planning/open-questions.md`.

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

## Architecture: the nesting model (the big picture)

"Layer" means a **level of virtualization nesting**, not a role. Understanding this requires
reading several docs in `planning/`:

```
Host (Linux/macOS)            your computer; llmsc/llmsctl installed here
└── L1: VM (llmsc-vm) ........ host-native VM running Incus; one shared kernel
    └── L2: system container . unprivileged LLMSC — agent/human workspace, run as a "sandbox"
        └── L3: app container  rootless Docker/Podman nested inside an L2 container
```

The L1→L2→L3 nesting is **containerization** (one shared kernel), NOT nested virtualization.
Everything is **unprivileged/rootless** — privileged containers are never used (security
posture). Don't reintroduce "nested virtualization" wording for the core stack; true nested
virt only applies to the rare VM-in-sandbox case.

- **L1 — VM** (`planning/architecture/vm.md`): a host-native VM (Docker-Desktop/Colima
  analogue) running Incus. VM backend is a **pluggable driver abstraction**
  (docker-machine-style); MVP uses **Lima** on both platforms (Linux: QEMU+KVM; macOS: Apple
  Virtualization/QEMU). Future: Parallels, libvirt, Proxmox.
- **L2 — System containers / LLMSC** (`planning/architecture/system-containers.md`):
  **unprivileged** Incus/LXC system containers (never privileged) with a **two-user model**
  (one Linux user per agent + one human operator login). The workspace units, run as
  sandboxes.
- **L3 — App containers** (`planning/architecture/app-containers.md`): **rootless**
  Docker/Podman nested inside an L2 container via `security.nesting`. **Key differentiator** —
  real container workflows without privileged DinD and without breaking the security
  backstops.
- **Services** (`planning/services/README.md`) are NOT a layer: each runs either directly in
  the L1 VM or in its own L2 container — an isolation choice, not a nesting level.

Terminology discipline: **VM** always = L1; **container/LLMSC** always = an L2 unit; **host**
= the user's computer. See `planning/naming.md`.

## Key cross-cutting concepts

- **Defense-in-depth security** (`planning/security-model.md`): layered backstops (control
  *rings*, distinct from the L1/L2/L3 nesting) — agent permissions → Linux UID isolation →
  LXC isolation → Incus network policy → **Tetragon eBPF** (kernel-level
  network/syscall/filesystem enforcement, per-container AND per-UID). Policies hang off the
  per-user model in L2.
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
