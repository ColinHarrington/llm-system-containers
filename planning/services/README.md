# Services

Shared infrastructure consumed by workspace containers (the LLMSCs). Services turn the
[VM](../architecture/vm.md) into a small private network for agents.

## Placement is an isolation choice, not a layer

A service is **not inherently an L2 system container.** Where it runs is an architecture /
isolation decision, made per service:

- **In the L1 [VM](../architecture/vm.md) directly** — simpler, lower overhead, less
  isolation. Reasonable for lightweight or trusted services.
- **In its own L2 [system container](../architecture/system-containers.md)** — stronger
  isolation (its own namespaces, users, network policy, Tetragon scope); same tech as a
  workspace container. Reasonable for anything handling secrets or untrusted traffic.

Default leanings (to be confirmed): isolate anything that holds credentials or inspects
traffic (LiteLLM, mitmproxy) in its own L2 container; co-locating lighter observability
pieces in L1 is acceptable.

## Plugin model

Services are intended to become **configurable plugins**. The wizard (`llmsctl init`) lets
the user pick which to enable; over time the catalog should be extensible via a defined
service interface (packaging, lifecycle, config schema, health/status) rather than a fixed
hard-coded set, regardless of whether a given service lands in L1 or its own L2 container.

## Catalog

| Service | Purpose | Priority | Doc |
|---|---|---|---|
| **LiteLLM** | LLM proxy — agents use virtual keys; real API/token creds never exposed | MVP | [llm-proxy.md](llm-proxy.md) |
| **VictoriaMetrics + Loki + Grafana** | System metrics + log aggregation (memory-efficient) | MVP | [observability.md](observability.md) |
| **Phoenix (Arize)** | LLM/agent observability — traces, evals, prompt inspection | MVP | [observability.md](observability.md) |
| **SeaweedFS or RustFS** | Shared storage across host ↔ containers and container ↔ container | Core | [shared-storage.md](shared-storage.md) |
| **mitmproxy + Zeek** | Network inspection / proxy / traffic capture | Core | [network-inspection.md](network-inspection.md) |
| **Forgejo** | Internal git platform | Optional | [_future.md](_future.md) |
| **NATS** | Agent-to-agent communication / message bus | Future | [_future.md](_future.md) |

Priorities: **MVP** (gates first usable version) · **Core** (high value, not gating) ·
**Optional** (user-enabled) · **Future** (expansion point, not yet designed).

LiteLLM logging hooks feed Phoenix directly, so every agent LLM call is automatically traced.

## Networking between services and workspaces

Incus manages bridge networks so workspace containers reach the services they're permitted to
use (e.g. an agent reaches LiteLLM only via its virtual key). Network permissions are
enforced per container and per user — see [../security-model.md](../security-model.md).

## Open items

- Service plugin interface (packaging, lifecycle, config schema, health/status).
- Per-service default placement (L1 vs dedicated L2 container) and how the wizard exposes it.
- Service discovery / internal DNS within the VM.
