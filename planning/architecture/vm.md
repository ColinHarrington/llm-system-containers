# L1 — The VM

The **VM** (long form **`llmsc-vm`**; short label **"VM"**) is a host-native virtual machine
that runs **Incus** inside it. It is L1 — the foundation everything else nests on.
Conceptually it mirrors how Docker Desktop / Colima / Lima stand up a VM, but purpose-built
to host this project's Incus-based system containers and services.

> Terminology: **Host** = your actual computer (macOS/Linux) where `llmsc`/`llmsctl` are
> installed. The **VM** is L1, created *on* the host. See [../naming.md](../naming.md).

## Responsibilities

- Run a native VM matching the **host architecture** (no cross-arch emulation in the common
  path → near-native performance): **aarch64** on Apple Silicon (e.g. an M-series Mac),
  **amd64** on x86_64 Linux. Incus image refs (e.g. `images:debian/13`) and Lima both resolve
  the host arch automatically; our binaries are cross-compiled per target.
- **VM OS:** the spike used an Ubuntu L1 VM (Lima default), which required relaxing
  `kernel.apparmor_restrict_unprivileged_userns` for rootless L3. A **Debian L1 VM** likely
  avoids that Ubuntu-specific restriction entirely — worth evaluating (open item).
- Provide a Linux kernel supporting **unprivileged container nesting** (user namespaces,
  cgroup delegation) so L2 system containers can themselves run L3 app containers
  (Docker/Podman) via `security.nesting`. This is **container-in-container (shared kernel),
  not CPU nested virtualization** — see [app-containers.md](app-containers.md). True nested
  *virtualization* (a hypervisor inside the VM) is only relevant to the rare case of running
  a full VM inside a sandbox, and is not required for the core stack.
- Host **Incus**, which manages all L2 system containers (both workspace-role and
  service-role — see [system-containers.md](system-containers.md)).
- Configure internal **networking**, services, and bootstrap items the project needs —
  including routable container addressing and split-horizon `.llmsc` DNS so containers are
  reachable (and SSH-able) from the host. See [networking.md](networking.md).

## Provisioning wizard

A small setup wizard (`llmsctl init`) collects:

- CPU count
- Memory
- Which services to enable (see [../services/README.md](../services/README.md))
- Networking configuration

After the wizard, `llmsctl up` launches the VM and bootstraps Incus and the selected
services inside it. The GUI/CLI surface VM status (running / stopped / starting).

## VM driver abstraction

How the VM is created/managed is a **pluggable driver** (same pattern as services). This
mirrors the old `docker-machine` driver model: one interface, many backends. MVP ships a
single driver on both platforms (Lima) for consistency; more follow.

| Platform | Driver | Status | Notes |
|---|---|---|---|
| macOS | **Lima** (via Colima) | MVP | Apple Virtualization framework or QEMU backend |
| macOS | Parallels | Future | Explicitly requested for later |
| Linux | **Lima** | MVP | Drives QEMU + KVM; consistent config with macOS |
| Linux | libvirt / virt-manager | Future | For users already on KVM/libvirt |
| Linux | Proxmox | Future | For homelab / server hypervisor deployments |

### Why Lima for MVP

- Colima is built on Lima, so a single Lima abstraction covers both macOS and Linux with
  one config format and one Go API.
- On Linux, Lima uses **QEMU + KVM** (hardware-accelerated; standard on modern distros).
- On macOS, Lima uses the **Apple Virtualization framework** or QEMU.

One provisioning layer, two platforms, consistent behavior.

## Open items

- Exact Incus bootstrap sequence inside the VM (networks, profiles, storage pools).
- How the driver abstraction is expressed in code (interface + driver per backend).
- Image/registry hosting location within the VM (see [../custom-images.md](../custom-images.md)).
- Whether multiple VMs / multi-host (Proxmox) implies "VMs" plural in the model later.
