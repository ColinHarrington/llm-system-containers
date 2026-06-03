# Buildout Roadmap

## Context

Architecture is settled ([overview.md](overview.md)), the stack is chosen
([tech-stack.md](tech-stack.md)), and the riskiest assumption — rootless L3 nesting in an
unprivileged sandbox — is **proven** ([spike-plan.md](spike-plan.md)). This doc sequences the
implementation into milestones. Every milestone is **red-green TDD** with a test plan
([testing.md](testing.md)); each lists its **done-when** so progress is unambiguous.

## Repository / workspace structure (target)

```
llm-system-containers/
├── crates/
│   ├── llmsc-core/      # library: config model, Incus client, VM driver, reconcile, profiles
│   ├── llmsc/           # CLI bin — container plane (launch/ls/shell/cp/rm)
│   └── llmsctl/         # CLI bin — platform/control plane (init/up/down/status/services)
├── gui/
│   ├── src-tauri/       # Tauri (Rust) shell over llmsc-core
│   └── src/             # Svelte + TypeScript frontend (from mockups/)
├── scripts/            # uv bootstrapping (exists)
├── planning/  mockups/
```

Cargo workspace over `crates/*`; `llmsc-core` is the single source of logic the CLIs and the
Tauri shell all depend on. External systems sit behind traits (`IncusClient`, `VmDriver`) so
core logic is unit-testable with fakes.

## Milestones (dependency order)

### M0 — Workspace + test harness + config model
- **Deliverables:** Cargo workspace (`llmsc-core`, `llmsc`, `llmsctl`); CI (fmt, clippy, test);
  TOML config model (serde); the `IncusClient` / `VmDriver` boundary traits + fakes.
- **Tests (red-green):** TOML round-trip + `insta` snapshots + `proptest` invariants; CLI
  smoke (`--help`) via `assert_cmd`.
- **Done-when:** `cargo test` green in CI; a sample `llmsc.toml` round-trips losslessly.

### M1 — Platform bring-up (`llmsctl`) — codifies spike phase 0
- **Deliverables:** Lima `VmDriver` (shell out to `limactl`): create/start/stop/status. Incus
  bootstrap inside the VM (`admin init`, persist `kernel.apparmor_restrict_unprivileged_userns=0`,
  set bridge `dns.domain=llmsc`). `llmsctl init` (minimal), `up`, `down`, `status`.
- **Tests:** unit (driver logic w/ fake); **integration** against real Lima+Incus on Linux CI
  (the proven spike steps become tests).
- **Done-when:** `llmsctl up` yields a VM with Incus ready; `status` reports running.

### M2 — Sandbox lifecycle (`llmsc`) — codifies spike phase 1
- **Deliverables:** `IncusClient` over the REST API (unix socket); `llmsc launch <name>
  --image` (unprivileged L2 + `security.nesting`), `ls`, `shell user@<name>` (exec), `rm`;
  two-user model (agent + operator, with the operator/system-group fix); declarative sandbox
  specs in config + `llmsc apply` reconcile (Incus = truth, drift surfaced).
- **Tests:** unit reconcile/diff w/ fake; integration create/list/exec/delete; idempotent apply.
- **Done-when:** launch a sandbox with users from config; re-apply is a no-op.

### M3 — L3 enablement (the differentiator) — codifies spike phase 2
- **Deliverables:** bake nesting requirements into the image/bootstrap (apparmor sysctl,
  `security.nesting`, rootless deps `podman uidmap slirp4netns fuse-overlayfs`).
- **Tests:** integration — a launched sandbox runs `podman build` rootless (spike phase 2 promoted to CI).
- **Done-when:** rootless container build+run inside a product-launched sandbox, in CI.

### M4 — Networking (the deferred spike phase 3, built properly)
- **Deliverables:** host-reachable VM network in the driver (**socket_vmnet** on macOS /
  bridge on Linux); routable container IPs; split-horizon `.llmsc` DNS (host resolver
  integration); SSH; `llmsctl net setup` for the privileged host steps.
- **Tests:** integration — host reaches a sandbox by `<name>.llmsc` and SSHes in.
- **Done-when:** `ssh operator@<name>.llmsc` works from the host.

### M5 — Services (MVP set)
- **Deliverables:** service model + placement (L1 vs own L2); **LiteLLM** (virtual keys) first,
  then observability (**Phoenix**, then VictoriaMetrics/Loki/Grafana); `llmsctl services enable`.
- **Tests:** integration — an agent in a sandbox reaches LiteLLM via a virtual key and the call
  is traced in Phoenix.
- **Done-when:** the MVP success criteria in [mvp.md](mvp.md) are met (virtual-key LLM call,
  traced, system metrics visible).

### M6 — File transfer + shared storage
- **Deliverables:** `llmsc cp` (Incus file API; host↔L2, L2↔L2); SeaweedFS service + mount.
  See [file-transfer.md](file-transfer.md), [services/shared-storage.md](services/shared-storage.md).

### M7 — Security: profiles + guardrails
- **Deliverables:** agent-profile model compiling to Incus config + Tetragon policies + network
  ACLs + LiteLLM key scope; guardrail enforcement. See [agent-profiles.md](agent-profiles.md),
  [security-model.md](security-model.md).

### M8 — GUI (Tauri + Svelte)
- **Deliverables:** wire the mockups to `llmsc-core` via Tauri commands — VM status, wizard,
  sandbox management, observe/interrupt/steer. See [interfaces.md](interfaces.md).
- **Tests:** Vitest + Testing Library (components), Playwright e2e.

## MVP cut line

Per [mvp.md](mvp.md): **M0 → M1 → M2 → M5 (LiteLLM + observability) → a basic GUI
(status + wizard) from M8.**

**Deferred (decided):**
- **M3 (L3 nesting)** — the *capability* is already spike-proven; as a product feature it's
  explored/hardened **once the base system is functional**, not in the MVP.
- **M4 (networking/SSH)** — deferred (validate with the proper host-reachable mechanism later).
- M6 (storage/cp), M7 (profiles/guardrails), full GUI — post-MVP.

## Recommended first slice

**M0 then M1** — stand up the workspace + test harness, then get `llmsctl up` bringing up the
VM with Incus (the most-used control-plane verb, and it turns the manual spike into product
code with integration tests). M2 follows immediately and gives the first end-to-end "launch a
sandbox" demo.

## Open items

- Reconcile/drift model specifics ([tech-stack.md](tech-stack.md)).
- CI runners for integration (Linux now; macOS/self-hosted for networking + nested e2e).
- ✅ **License: MIT OR Apache-2.0 dual** — `LICENSE-MIT` + `LICENSE-APACHE` added. At M0:
  set Cargo `license = "MIT OR Apache-2.0"`, add a README "License" section, and adopt a
  **DCO** (Developer Certificate of Origin; inbound = outbound). Copyright line currently
  "Colin Harrington and llm-system-containers contributors" — adjust if you want an org name.
- Top-level user-facing **README** still to write (separate from `planning/`).
