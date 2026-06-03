# Open Questions & Future Sessions

Loose ends to resolve in dedicated sessions. This is the umbrella project's "parking lot."

## Naming
- ✅ **Resolved** — project **llm-system-containers**; unit **LLMSC** (*Little Linux Managed
  System Container*, L2); the **VM** is **`llmsc-vm`** (short "VM", "Playground" retired);
  CLIs **`llmsc`** (containers) + **`llmsctl`** (platform). "sandbox" is a mode, not the
  name. Layer model: **Host → L1 VM → L2 system container → L3 app container**. See
  [naming.md](naming.md).

## Tech stack (mostly resolved — see [tech-stack.md](tech-stack.md))
- ✅ **Language: Rust**; shared **`llmsc-core`** crate; CLI-first (CLI fully capable).
- ✅ **GUI: Tauri** (Rust + webview, reuses mockups).
- ✅ **Config: declarative on-disk** (TOML leaning), shared by CLI & GUI.
- ✅ **Incus = runtime truth** in its own Incus project; reconcile config → Incus; raw `incus`
  always usable.
- ✅ **Library-first; daemon deferred.**
- ⬜ Remaining: config format (TOML vs YAML); frontend (React vs Svelte); reconcile/drift model;
  Rust↔Incus client (crate vs hand-rolled); when the optional daemon is warranted.

## Architecture / design
- **Service plugin interface** — packaging, lifecycle, config schema, health/status
  ([services/README.md](services/README.md)).
- **Per-service placement** — L1 VM vs. own L2 container, and how the wizard exposes it
  ([services/README.md](services/README.md)).
- **VM driver abstraction** — interface + driver per backend
  ([architecture/vm.md](architecture/vm.md)).
- **Interrupt/steer control plane** mechanism for agents
  ([services/observability.md](services/observability.md)).
- **Display forwarding** tech for GUI apps — X11 vs Wayland vs VNC/RDP
  ([architecture/system-containers.md](architecture/system-containers.md)).
- **Unprivileged nesting** (L3) — Incus profile specifics for reliable rootless
  Docker/Podman ([architecture/app-containers.md](architecture/app-containers.md)).
- **Routable addressing + split-horizon DNS** — making the container subnet routable from the
  host per VM driver, `.llmsc` resolver setup, and SSH auth model
  ([architecture/networking.md](architecture/networking.md)). Needs spike validation.
- **Tetragon policy authoring** model and mapping to per-agent grants
  ([security-model.md](security-model.md)).
- **Agent profile** format, inheritance, and how profiles compile to concrete
  Tetragon/Incus/LiteLLM artifacts ([agent-profiles.md](agent-profiles.md)).
- **Shared storage** — SeaweedFS chosen; open: mount mechanism (VM-bind vs in-container FUSE
  vs s3fs/rclone), versioning mechanism + retention, scoped-access mapping to profiles
  ([services/shared-storage.md](services/shared-storage.md)).
- **Custom image** build format + registry implementation
  ([custom-images.md](custom-images.md)).

## Future features
- **Agent-to-agent communication** (NATS) design ([services/_future.md](services/_future.md)).
- **openclaw** integration ([services/_future.md](services/_future.md)).
- Additional VM drivers: **Parallels** (macOS), **libvirt/virt-manager**, **Proxmox**.

## Project meta
- This planning set is the **umbrella**; expect **multiple layered sub-plans** to grow from
  individual docs.
- User-facing documentation (separate from these planning docs) to come if the project goes
  public.
