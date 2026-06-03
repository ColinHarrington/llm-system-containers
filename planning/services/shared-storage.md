# Service — Shared Storage

**Priority:** Core

## Purpose

Provide shared files and mount points across the system:

- Between **host and containers**.
- Between **containers** (e.g. multiple sandboxes or agents sharing data).

This is a cleaner alternative/complement to raw bind mounts for multi-container sharing:
agents can be granted scoped bucket/path access rather than raw filesystem access.

## Candidates

| Option | Notes |
|---|---|
| [SeaweedFS](https://github.com/seaweedfs/seaweedfs) | Go, mature, S3-compatible, distributed object store with filer |
| [RustFS](https://github.com/rustfs/rustfs) | Rust, newer, S3-compatible |

Both are S3-compatible and run as a service container. Choice TBD — SeaweedFS is the more
mature/proven option; RustFS is the newer Rust alternative.

## Relationship to workspace mounts

Layer 3 workspace mounts ([../architecture/sandbox-containers.md](../architecture/sandbox-containers.md))
handle the human's host workspace directly. Shared storage is better suited to:

- Sharing artifacts **between** containers/agents.
- Giving agents **scoped** access (per-path / per-bucket) rather than raw FS access, which
  complements the [security model](../security-model.md).

## Open items

- SeaweedFS vs. RustFS decision (maturity vs. footprint).
- How scoped access maps to agent users / Tetragon policy.
- Whether this is the default sharing mechanism or opt-in alongside bind mounts.
