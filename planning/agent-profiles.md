# Agent Profiles

## Context

The [security model](security-model.md) defines *primitives* — filesystem ACLs, syscall
policy, network ACLs — enforced per container and per UID via unprivileged LXC, Incus, Linux
users, and Tetragon eBPF. Those primitives are powerful but low-level: authoring them by hand
for every agent does not scale and is easy to get wrong.

**Agent profiles** are the human-legible layer on top: **reusable, named bundles of permission
boundaries** that you assign to an agent. A profile compiles down to the underlying backstops;
the operator thinks in roles ("this is a *researcher*"), not in individual ACL rules. Profiles
make the security posture **reusable, reviewable, and consistent** across many agents and
sandboxes.

## What a profile bundles

A profile is a declarative bundle spanning several axes:

| Axis | What it controls | Backed by |
|---|---|---|
| **Filesystem** | Path allow/deny, read-only vs read-write (e.g. RW `/workspace/scratch`, RO repo, deny everything else) | Linux perms + mounts + Tetragon FS policy |
| **Syscalls** | Which syscalls are denied (e.g. `ptrace`, `mount`, `kexec`, `bpf`) vs the normal dev set | Tetragon eBPF |
| **Network** | Default-deny egress + allowlist; LLM only via LiteLLM; HTTP(S) via mitmproxy | Incus network ACLs + Tetragon |
| **Containers (L3)** | Whether rootless Docker/Podman nesting is enabled | Incus `security.nesting` per sandbox |
| **LLM access** | Virtual-key scope + budget/rate limits | LiteLLM virtual keys |
| **Resources** | CPU / memory / disk limits | Incus limits |
| **Control-plane capabilities** | Whether the agent may take *platform* actions (launch/stop sandboxes, coordinate other agents) | `llmsctl` / daemon authz |

The first five map to the in-sandbox OS boundaries; **control-plane capabilities** are a
distinct axis — they govern what an agent can do to the *platform* itself (relevant to the
`orchestrator` archetype below).

## Assignment model

- A profile is assigned to an **agent**, i.e. to a **UID within a sandbox**. Different agents
  in the same L2 sandbox can carry different profiles ([architecture/system-containers.md](architecture/system-containers.md)
  — one Linux user per agent).
- The **human operator** login is separate and generally not profile-restricted the way agents
  are (the operator owns the workspace; see [security-model.md](security-model.md)).
- CLI (illustrative, matching the GUI mockup): `llmsctl security assign --profile researcher agent-claude@web-agent-01`.
- Profiles are surfaced in the GUI **Security & profiles** screen ([interfaces.md](interfaces.md));
  L3-related fields exist in the profile even though L3 isn't otherwise visualized in the GUI.

## Starter archetypes

Ship a small set of opinionated, least-privilege defaults. Each clearly summarizes what it
**allows** and **denies**:

| Profile | Filesystem | Network egress | L3 | LLM budget | Control-plane | Use |
|---|---|---|---|---|---|---|
| **researcher** | RO repo + docs, RW scratch | Web/docs allowlist via mitmproxy | off | generous | none | Read, research, gather context |
| **tester** | RW repo | Limited (package registries) | on (test infra) | medium | none | Run and write tests |
| **builder** | RW repo + artifacts | Registry/package allowlist | on (build images) | medium | none | Compile, build images |
| **validation** | Read-only everything | **None except LLM** | off | small | none | Run checks; never writes — strictest |
| **orchestrator** | Minimal (own scratch) | None raw (internal coordination only, e.g. NATS) | off | broad | **launch/stop sandboxes, coordinate agents** | Drive other agents (software-factory) |

Notes:
- **validation** is the tightest by design — read-only, no egress but the LLM proxy, no
  nesting — suitable for a gatekeeper agent whose output you trust precisely because it cannot
  mutate anything.
- **orchestrator** is the only archetype with elevated *control-plane* capability; it has weak
  in-sandbox privileges but can coordinate the fleet. This separation (weak on the box, strong
  on the platform) is intentional.

## Principles

- **Least privilege by default** — profiles grant the minimum; broaden explicitly.
- **Profiles are presets, not the enforcement** — the kernel/infra backstops in
  [security-model.md](security-model.md) are what actually hold; a profile is just a legible,
  reusable way to configure them. A bug in profile UI can't grant more than the primitives allow.
- **Reviewable** — a profile is a small declarative artifact that can be diffed, version
  controlled, and audited.

## Open items

- Profile definition format (declarative file schema) and whether profiles support
  **inheritance/extension** (e.g. `builder` extends `tester`).
- Custom/user-defined profiles vs. the shipped archetypes; org-level sharing.
- How a profile **compiles** to concrete artifacts (Tetragon TracingPolicy, Incus ACLs/limits,
  LiteLLM key config) and how conflicts/over-grants are detected.
- Whether profiles can be changed on a **running** agent, and what re-application means.
- Relationship to **custom images** ([custom-images.md](custom-images.md)) — an image bakes in
  tooling; a profile bounds permissions. Likely orthogonal but worth pairing in presets.
- Control-plane capability model for `orchestrator` (authz in the daemon).
