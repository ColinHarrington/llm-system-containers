# Open Questions & Future Sessions

Loose ends to resolve in dedicated sessions. This is the umbrella project's "parking lot."
Guiding convictions live in [principles.md](principles.md); items below are the open decisions
those principles don't yet settle.

## Decision direction (from the Incus-ecosystem evaluation, 2026-06)
- 🚧 **`host` → `target`/`context` (in progress).** Modeled as `config.mode`
  (`DeploymentMode { vm, local, remote }`); the CLIs/GUI resolve the target from the loaded
  config (`Config::vm_target`), and `CliIncus` is transport-aware. `vm` (default) + `local`
  (host Incus, no VM) are wired in `llmsc`; `remote` is reserved. ⬜ Remaining: `llmsctl`/GUI
  `local` (their deployers are still VM-bound), the `remote` endpoint model, and a GUI target
  picker. See [principles.md](principles.md) §6 and [architecture/vm.md](architecture/vm.md).
- ✅ **Adopt as principles:** cattle-not-pets (images are the durable artifact, no OS-drive
  backup/restore), respect-the-ecosystem (Incus-native state only), localhost-only API,
  single-operator-owns-the-VM (no RBAC yet). See [principles.md](principles.md).

## Parked — revisit when X (deliberately *not* MVP)
- **Storage drivers** (zfs/btrfs/ceph/linstor/truenas), pools, **buckets** — use `dir`; keep the
  pool name in config so it's swappable. *Revisit when* a real durability/perf need appears
  (SeaweedFS already covers shared storage). Note: host↔VM file sharing is the VM driver's mount
  + an Incus `disk` device — **not** a storage-driver concern.
- **Projects as user-facing isolation** — we already use *one* project for namespacing; per-user
  /per-tenant projects are multi-tenant scope. *Revisit when* multi-user lands.
- **OpenFGA / RBAC** — *revisit when* multi-user lands (single operator owns the VM today).
- **Clustering** — distant; *revisit when* a multi-node "factory" is a real goal.
- **BPF token delegation** — not on the path (Tetragon runs in the VM, not the L2). *Revisit
  when* we want eBPF *inside* an unprivileged L2 / nested observability.
- **VM-in-sandbox** (e.g. macOS VM inside the L1 VM) — true nested-virt, VNC-only, niche; cloud
  is better. Skip unless a concrete need appears.
- **Served web UI / PWA (and the daemon it implies)** — *revisit when* remote/multi-host or
  browser-anywhere access matters; keep the `core.ts` boundary clean until then
  ([principles.md](principles.md) §4).
- **Metrics + Events API consumption** — cheap, on-principle (observability); adopt
  opportunistically rather than building bespoke monitoring ([metrics docs upstream]).
- **incus agent** — only relevant to VM-type instances; N/A for the LXC system-container path.

## Naming
- ✅ **Resolved** — project **llm-system-containers**; unit **LLMSC** (*Little Linux Managed
  System Container*, L2); the **VM** is **`llmsc-vm`** (short "VM", "Playground" retired);
  CLIs **`llmsc`** (containers) + **`llmsctl`** (platform). "sandbox" is a mode, not the
  name. Layer model: **Host → L1 VM → L2 system container → L3 app container**. See
  [naming.md](naming.md).

## Tech stack (mostly resolved — see [tech-stack.md](tech-stack.md))
- ✅ **Language: Rust**; shared **`llmsc-core`** crate; CLI-first (CLI fully capable).
- ✅ **GUI: Tauri** (Rust + webview, reuses mockups); frontend **Svelte + TypeScript**.
- ✅ **Testing: red-green TDD**, test plan per feature ([testing.md](testing.md)).
- ✅ **Config: declarative on-disk, TOML**, shared by CLI & GUI.
- ✅ **Incus = runtime truth** in its own Incus project; reconcile config → Incus; raw `incus`
  always usable.
- ✅ **Library-first; daemon deferred.**
- ⬜ Remaining: reconcile/drift model; Rust↔Incus client (crate vs hand-rolled); when the
  optional daemon is warranted; CI provider/runners ([testing.md](testing.md)).

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
- **Base images (verified):** **sandboxes default to Alpine** (`images:alpine/3.21`); **service
  containers use debian/12**. `debian/13`/trixie's systemd hangs at boot under the current Incus
  → avoided. ⬜ Open: whether the **L1 VM** should be Debian (vs the current Ubuntu Lima default)
  to drop the apparmor userns workaround ([architecture/vm.md](architecture/vm.md)); and making
  per-user provisioning **OS-aware** (Alpine uses `adduser`/`sh`, not `useradd`/`bash`).
- **Architecture** handled automatically (aarch64 on Apple Silicon / amd64 on x86_64 Linux);
  Incus + Lima resolve host arch; binaries cross-compiled per target.

## Future features
- **Agent-to-agent communication** (NATS) design ([services/_future.md](services/_future.md)).
- **openclaw** integration ([services/_future.md](services/_future.md)).
- Additional VM drivers: **Parallels** (macOS), **libvirt/virt-manager**, **Proxmox**.

## Project meta
- This planning set is the **umbrella**; expect **multiple layered sub-plans** to grow from
  individual docs.
- User-facing documentation (separate from these planning docs) to come if the project goes
  public.
