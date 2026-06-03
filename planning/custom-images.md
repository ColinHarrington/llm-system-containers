# Custom Images

Incus uses an **image** model for system containers. This project lets users build their own
custom images so sandboxes can start pre-loaded with the tooling they need.

## Goals

- Users can **build custom images** containing pre-packaged tooling, platforms, IDEs,
  browsers, runtimes, etc.
- An internal **image registry** is hosted in the Playground VM.
- Images become **templates** for spinning up sandbox containers — a Containerfile/Dockerfile
  analogue, but for **system** containers.

## Why

- Fast, reproducible sandbox creation: start from an image that already has the agent's
  toolchain, browser, IDE, etc.
- Standardize environments across many agents / orchestrations.
- Throwaway sandboxes become cheap when the heavy setup is baked into an image.

## Relationship to other layers

- Sandbox containers (Layer 3) are launched **from** these images. See
  [architecture/sandbox-containers.md](architecture/sandbox-containers.md).
- The registry lives in the Playground VM (Layer 1). See
  [architecture/playground-vm.md](architecture/playground-vm.md).

## Open items

- Image build definition format (declarative file vs. scripted build vs. both).
- Registry implementation (Incus's simplestreams, a custom registry, or reuse of shared
  storage).
- Versioning, tagging, and sharing/exporting images between users.
- Base image set shipped by default (e.g. a "dev environment" image with X-forwarding +
  browser preconfigured).
