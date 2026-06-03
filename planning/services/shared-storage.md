# Service — Durable / Shared Storage (SeaweedFS)

**Priority:** Core

## Purpose

A **durable, shared, versioned** file store for the platform:

- **Durable artifact storage** — generated artifacts (build outputs, reports, datasets)
  persist **across ephemeral sandbox teardown and VM restarts**.
- **Mountable inside L2 containers** — agents read/write it like a normal directory.
- **Versioned for review** — artifact history is retained so changes can be diffed/reviewed.
- **Shared** — across containers (sandbox ↔ sandbox) and with the host.
- A cleaner alternative/complement to raw bind mounts: agents get **scoped bucket/path**
  access rather than raw filesystem access.

## Choice: SeaweedFS

[SeaweedFS](https://github.com/seaweedfs/seaweedfs) — Go, mature, **minimal footprint**,
S3-compatible object store with a filer namespace and an S3 gateway. Chosen for the low memory
footprint (fits the memory-efficiency principle). [RustFS](https://github.com/rustfs/rustfs)
remains a possible alternative (newer, Rust), but SeaweedFS is the leaning choice.

Components in play: **master** + **volume servers** (storage), **filer** (POSIX-ish
namespace), and the **S3 gateway** (S3 API). Runs as a service in its **own L2 container** (so
it has a routable interface — `seaweedfs.llmsc` — see
[../architecture/networking.md](../architecture/networking.md)), reachable by sandboxes on the
services network.

## Mounting into L2 containers

Three options, roughly cleanest-first (exact choice is a spike item):

1. **Mount on the VM, bind into containers via an Incus disk device** — most reliable; avoids
   FUSE inside unprivileged containers.
2. **`weed mount` (FUSE) inside the container** — most "native" (each sandbox mounts its scoped
   path), but **FUSE in unprivileged LXC is finicky** (device access / may be disallowed).
3. **`s3fs` / `rclone mount` against the S3 gateway** — network-based, simple, slower for heavy
   I/O.

## Versioning (for review)

SeaweedFS supports **S3 bucket versioning**; with the filer this gives artifact history that
can be listed and diffed for review. Exact mechanism (S3 versioning vs filer snapshots) and
retention policy are open items.

## Durability

- Persists across **ephemeral sandboxes** and **VM restarts** (backed by the VM's disk on the
  host).
- A **replication factor** (SeaweedFS replication) guards against volume loss.
- Optional **backup/replicate** to an external S3 or the host for real off-box durability.

## Scoped access (ties to agent profiles)

Storage access is another boundary a **profile** bundles
([../agent-profiles.md](../agent-profiles.md)): agents get **scoped buckets/paths** (e.g.
read-only shared inputs, read-write own artifacts), the operator broader. This complements the
[security model](../security-model.md) — scoped object access instead of raw FS.

## Relationship to other mechanisms

- **Workspace mounts** ([../architecture/system-containers.md](../architecture/system-containers.md))
  handle the human's host workspace directly; this store is for **shared/durable artifacts**.
- **`llmsc cp`** ([../file-transfer.md](../file-transfer.md)) is for one-shot copies; this store
  is for **ongoing shared, versioned** access.

## Open items

- Mount mechanism decision (VM-bind vs in-container FUSE vs s3fs/rclone) — spike.
- Versioning mechanism + retention (S3 versioning vs filer snapshots).
- How scoped bucket/path access maps to agent profiles / Tetragon policy.
- Replication factor + backup target defaults.
- Whether this is the default artifact-sharing mechanism or opt-in alongside bind mounts.
