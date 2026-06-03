# Custom Images

Incus uses an **image** model for system containers. This project lets users build their own
custom images so sandboxes can start pre-loaded with the tooling they need.

## Goals

- Users can **build custom images** containing pre-packaged tooling, platforms, IDEs,
  browsers, runtimes, etc.
- An internal **image registry** is hosted in the VM.
- Images become **templates** for spinning up L2 system containers — a Containerfile/Dockerfile
  analogue, but for **system** containers.

## Why

- Fast, reproducible sandbox creation: start from an image that already has the agent's
  toolchain, browser, IDE, etc.
- Standardize environments across many agents / orchestrations.
- Throwaway sandboxes become cheap when the heavy setup is baked into an image.

## Relationship to other layers

- L2 system containers are launched **from** these images. See
  [architecture/system-containers.md](architecture/system-containers.md).
- The registry lives in the L1 VM. See [architecture/vm.md](architecture/vm.md).

## Open items

- Image build definition format (declarative file vs. scripted build vs. both).
- Registry implementation (Incus's simplestreams, a custom registry, or reuse of shared
  storage).
- Versioning, tagging, and sharing/exporting images between users.
- Default base distro is **Debian** (smaller/quicker than Ubuntu). Base image set shipped by
  default (e.g. a "dev environment" image with X-forwarding + browser preconfigured).
- **Architecture:** images are arch-specific, but Incus resolves the host arch automatically
  (**aarch64** on Apple Silicon, **amd64** on x86_64 Linux); custom-image builds must produce
  the matching arch (no cross-arch in the common path).
