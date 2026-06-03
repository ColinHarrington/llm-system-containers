# File Transfer

## Context / requirement

Users need to copy files **Host ↔ L2 container** and **L2 ↔ L2 container** — both ad-hoc
one-shot copies and larger syncs.

Several mechanisms already fall out of earlier decisions; the goal here is a **first-class
utility** so users don't have to know which transport to reach for.

## Mechanisms (already available)

| Mechanism | Best for | Notes |
|---|---|---|
| **SSH + `.llmsc` DNS** ([architecture/networking.md](architecture/networking.md)) | Power users, large/incremental | `scp` / `rsync` host↔container and container↔container, once routable IPs + SSH exist |
| **Shared storage** ([services/shared-storage.md](services/shared-storage.md)) | *Ongoing* shared access | SeaweedFS/RustFS scoped paths/buckets, not one-shot copies |
| **Workspace bind mounts** ([architecture/system-containers.md](architecture/system-containers.md)) | Host workspace already inside a sandbox | Not a copy — a mount |

## What to add: `llmsc cp`

A first-class copy utility (à la `docker cp` / `kubectl cp`) that abstracts the transport:

```
llmsc cp <src> <dst>
#   ./local/path            → a host path
#   [user@]sandbox:/path    → a container path; the optional user scopes
#                             ownership + the permission/profile context
```

Examples:
```
llmsc cp ./build.tar web-agent-01:/workspace/scratch/        # host → L2
llmsc cp web-agent-01:/workspace/out ./out                   # L2 → host
llmsc cp ci-runner:/artifacts/app.tar web-agent-01:/in/      # L2 → L2
llmsc cp agent-claude@web-agent-01:/home/agent-claude/x ./   # scoped to a UID
```

### Under the hood
- **Host ↔ L2:** the **Incus file API** (`incus file push/pull` via the Go client) — works
  **without SSH** and handles the unprivileged **UID/idmap translation** so files land with the
  correct in-container ownership.
- **L2 ↔ L2:** daemon-mediated — pull from source then push to dest (or stream through). Avoids
  requiring SSH between sandboxes and keeps the copy under policy control.
- **Advanced:** `rsync`/`scp` over the SSH + `.llmsc` path stays available for large or
  incremental transfers.

## Security — copies respect the guardrails

A copy must not become a way around the [security model](security-model.md) /
[agent profiles](agent-profiles.md):

- Writing as `user@sandbox` honors **that agent's profile filesystem ACLs** and per-UID
  ownership — `cp` cannot write outside an agent's granted paths.
- **L2 ↔ L2 copy is a data-movement flow, so it is policy-gated** (the same "addressable, then
  policy-gated" rule as networking): whether two sandboxes may exchange data is a policy
  decision, not automatic. The **operator** can copy freely; **agents** are bounded by their
  profile.

## GUI

CLI (`llmsc cp`) is the core. A GUI affordance (drag-and-drop into a sandbox, or a simple file
browser per sandbox) can follow later; not an MVP priority.

## Open items

- Recursive/directory semantics, symlink handling, and large-file streaming/progress.
- Exact UID/idmap ownership rules on push (land as which UID; preserve vs remap).
- Policy model for **agent-initiated** cross-sandbox copies (what a profile must grant).
- Whether `llmsc cp` should auto-pick SSH/rsync for big transfers vs always using the Incus
  file API.
