# Open Questions & Future Sessions

Loose ends to resolve in dedicated sessions. This is the umbrella project's "parking lot."

## Naming
- ✅ **Resolved** — project **llm-system-containers**; unit **LLMSC** (*Little Linux Managed
  System Container*); CLIs **`llmsc`** (containers) + **`llmsctl`** (platform). "sandbox" is
  a mode, not the name. See [naming.md](naming.md).
- ⬜ **Still open:** name for the **VM** (Layer 1) — working name **Playground** /
  **Sandbox Playground**.

## Tech stack (undecided)
- **GUI app** framework/language (Tauri, Electron, native, …).
- **CLI** language — Go pairs naturally with Lima/Incus tooling; not yet decided.
- **Provisioning layer** language and how it drives Lima/Incus.
- **Control-plane API** shape — local daemon vs. library; REST/gRPC.

## Architecture / design
- **Service plugin interface** — packaging, lifecycle, config schema, health/status
  ([architecture/service-containers.md](architecture/service-containers.md)).
- **VM provider abstraction** — interface + driver per backend
  ([architecture/playground-vm.md](architecture/playground-vm.md)).
- **Interrupt/steer control plane** mechanism for agents
  ([services/observability.md](services/observability.md)).
- **Display forwarding** tech for GUI apps — X11 vs Wayland vs VNC/RDP
  ([architecture/sandbox-containers.md](architecture/sandbox-containers.md)).
- **Tetragon policy authoring** model and mapping to per-agent grants
  ([security-model.md](security-model.md)).
- **Shared storage** choice — SeaweedFS vs RustFS
  ([services/shared-storage.md](services/shared-storage.md)).
- **Custom image** build format + registry implementation
  ([custom-images.md](custom-images.md)).

## Future features
- **Agent-to-agent communication** (NATS) design ([services/_future.md](services/_future.md)).
- **openclaw** integration ([services/_future.md](services/_future.md)).
- Additional VM providers: **Parallels** (macOS), **libvirt/virt-manager**, **Proxmox**.

## Project meta
- This planning set is the **umbrella**; expect **multiple layered sub-plans** to grow from
  individual docs.
- User-facing documentation (separate from these planning docs) to come if the project goes
  public.
