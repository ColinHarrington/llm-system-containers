# Layer 1 — Playground VM

The **Playground VM** is a host-native virtual machine that runs **Incus** inside it. It is
the foundation everything else sits on. Conceptually it mirrors how Docker Desktop / Colima
spin up a VM on macOS — but here the VM is purpose-built to host this project's Incus-based
sandboxes and services.

> "Playground" / "Sandbox Playground" is the working name for this VM. Naming to be
> revisited (see [../open-questions.md](../open-questions.md)).

## Responsibilities

- Run a native VM matching the **host architecture** (no cross-arch emulation in the common
  path → near-native performance).
- Provide **nested virtualization** so Docker/Podman can run inside the sandbox containers.
- Host **Incus**, which manages both the service containers (Layer 2) and the sandbox
  containers (Layer 3).
- Configure internal **networking**, services, and bootstrap items the project needs.

## Provisioning wizard

A small setup wizard collects:

- CPU count
- Memory
- Which services to enable (see [service-containers.md](service-containers.md))
- Networking configuration

After the wizard, the app launches the VM and bootstraps Incus and the selected services
inside it. The GUI surfaces VM status (running / stopped / starting).

## VM provider abstraction

The way the VM is created/managed is a **pluggable provider** — same pattern used for
services. MVP ships a single provider on both platforms (Lima) for consistency; more
providers follow.

| Platform | Provider | Status | Notes |
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
- How the provider abstraction is expressed in code (interface + driver per backend).
- Image/registry hosting location within the VM (see [../custom-images.md](../custom-images.md)).
