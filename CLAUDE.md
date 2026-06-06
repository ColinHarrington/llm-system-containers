# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project status

**Pre-alpha, actively implemented.** The Cargo workspace, both CLIs, the shared core library,
and a substantial Tauri + Svelte GUI all exist and build. M0–M2 are done (workspace + config
model, platform bring-up, sandbox lifecycle); services (M5), security/enforcement (M7), and the
GUI (M8) are in progress. See `planning/buildout.md` for the milestone status. A few commands
are still stubs (notably `llmsc cp`, M6). Design docs live in `planning/`; GUI design
explorations in `mockups/`.

**Tech stack** (see `planning/tech-stack.md`): **Rust** core in a shared **`llmsc-core`** crate
that the CLIs (`llmsc`, `llmsctl`) and the **Tauri** GUI shell all link; **CLI-first** (the CLI
is fully capable standalone); **declarative on-disk config** (**TOML**, `llmsc.toml`) shared by
CLI and GUI; **Incus is the runtime source of truth** (managed in its own Incus project, raw
`incus` always usable) with config reconciled to it; **library-first, daemon deferred**. Incus
is reached via its CLI/REST surface; the VM via a `VmDriver` trait (Lima via `limactl`). GUI
frontend is **Svelte 5 (runes) + TypeScript** (no plain JS).

**Testing is red-green TDD** (`planning/testing.md`): write the failing test first, then the
minimal code to pass, then refactor — no production code without a test that drove it. **Every
feature ships a test plan.** Tooling in use: Rust `cargo test` (unit + black-box CLI tests via
`assert_cmd`, `insta` snapshots, trait-mocked Incus/VM boundaries with fakes); Svelte **Vitest**
+ Testing Library. Planned: **Playwright** e2e; real Lima+Incus integration tests promoted from
the spikes (`planning/spike-plan.md`). Current suite: **114 Rust tests + 64 GUI vitest** pass.

**Bootstrapping/operational scripts are `uv` single-file Python** (PEP 723 inline metadata, run
with `uv run`), in `scripts/` — product code is Rust, but operational glue is `uv` Python. Keep
each script self-contained (inline deps, no sibling imports). See `scripts/spike.py`.

## Build / test / lint commands

Rust workspace (run from the repo root):

```bash
cargo build                       # build core + both CLIs
cargo test --all                  # full Rust suite (unit + CLI black-box)
cargo test -p llmsc-core enforce  # a single crate / filtered by name substring
cargo fmt --all --check           # formatting gate (also a pre-commit hook)
cargo clippy --all-targets --all-features -- -D warnings   # lint gate
cargo run -p llmsctl -- status    # run a CLI (args after `--`)
cargo run -p llmsc -- ls
```

GUI (run from `gui/`; package manager is **pnpm**):

```bash
pnpm install
pnpm check          # svelte-check — must stay 0 errors / 0 warnings
pnpm test           # vitest run (one-shot)
pnpm test Security  # a single test file by name substring
pnpm build          # production build (also typechecks the Tauri-less bundle)
pnpm dev            # vite dev server (frontend only)
# Full desktop app: `cargo tauri dev` from gui/ (needs the tauri CLI).
```

The Tauri Rust shell lives in `gui/src-tauri/` (its own crate, not in the workspace members);
format it with `cargo fmt --manifest-path gui/src-tauri/Cargo.toml`.

**Pre-commit hooks** (`prek`, see `.pre-commit-config.yaml`): commit-time only — fast hygiene
plus `cargo fmt` (workspace + `gui/src-tauri`) and `ruff`. The slow gates (`cargo clippy`,
`cargo test`, `pnpm check`, `pnpm test`) are **not** run as git hooks; they're left to CI.
Install once with `prek install --install-hooks`. CI (`.github/workflows/ci.yml`) runs both a
**rust** job (fmt incl. `gui/src-tauri`, clippy, test) and a **gui** job (svelte-check, vitest,
build).

## What this project is

**llm-system-containers** is an open-source platform/tool that enables **LLM System
Containers (LLMSC)** — *Lightweight Linux Managed System Containers* — for AI agents. Built on
**Incus / unprivileged LXC**: *system* containers (full lightweight Linux machines), not
application/process containers like Docker. Each LLMSC is typically run as an ephemeral,
safer **sandbox** ("sandbox" is a mode, not the name). The goal is defense-in-depth
infrastructure backstops so an agent's own (imperfect) permission framework is never the
only thing protecting the host. Targets **Linux and macOS**.

Two Rust CLIs (see `planning/naming.md`), both implemented over `llmsc-core`:
- **`llmsc`** — daily driver for individual containers (`launch`, `ls`, `shell user@<name>`,
  `rm`, `apply`, `egress`, agent control).
- **`llmsctl`** — platform/host control plane (`init`, `up`/`down`, `status`, `services …`,
  `keys …`, `tetragon`, `doctor`).

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

Code:
- `crates/llmsc-core/` — the shared library; **all real logic lives here**. Key modules:
  `config` (the `llmsc.toml` model + helpers), `incus` (the Incus client — the largest module),
  `vm` (the `VmDriver` trait + Lima driver), `reconcile` (declared-vs-live diff/converge),
  `deploy` (per-service container deployers — LiteLLM, Phoenix, Grafana, SeaweedFS, mitmproxy,
  Zeek, plus a Tetragon-install deployer — sharing a `ServiceContainer` helper), `service` (the
  service catalog + placement), `enforce` (egress-ACL compile/plan — the per-container network
  ring), `tetragon` (per-UID kernel policy compile), `profile` (agent profiles), `bootstrap`,
  `process` (`CommandRunner` boundary + fake), `progress`/`error`.
- `crates/llmsc/` — the container-plane CLI (`launch`, `ls`, `shell`, `cp`, `rm`, `apply`,
  `egress`, `agent {pause,resume,stop,steer}`, `mount-shared`). `crates/llmsctl/` — the
  platform CLI (`init`, `up`/`down`/`destroy`, `status`, `services …`, `keys …`, `tetragon`,
  `doctor`). Both are thin shells over `llmsc-core`.
- `gui/src/` — Svelte 5 + TS frontend (screens in `src/screens/`, reusable bits + the runes
  store in `src/lib/`); `gui/src-tauri/` — the Tauri Rust shell exposing `llmsc-core` as
  commands. External systems sit behind traits (`IncusClient`, `VmDriver`, `CommandRunner`) so
  core logic is unit-testable with fakes — prefer extending a fake over shelling out in a test.

Docs:
- `planning/` — all design docs. Start with `planning/README.md` (the index), then
  `planning/overview.md`. `planning/buildout.md` is the milestone roadmap;
  `planning/mvp.md` the scoped first milestone; `planning/open-questions.md` the decisions
  parking lot.
- These are **planning docs**, intentionally separate from future **user-facing
  documentation** (which will live at the top level / a `docs/` dir if the project goes
  public). Keep that separation.

## Working conventions in the docs

- Markdown wrapped at ~100 columns; tables for catalogs/comparisons; each doc ends with an
  **Open items** section.
- Docs cross-link with relative paths — keep links valid when adding/moving files.
- When a design decision is made, update the relevant doc **and** remove/resolve the
  corresponding entry in `planning/open-questions.md`.
