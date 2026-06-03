# Layer 2 — Service Containers

Inside the Playground VM, **service containers** are LXC containers that host shared
infrastructure consumed by the sandbox containers (Layer 3). They turn the Playground into
a small private cloud / internal network.

## Plugin model

Services are intended to become **configurable plugins** for the Playground. The wizard
([playground-vm.md](playground-vm.md)) lets the user pick which services to enable; over
time the catalog should be extensible via a defined service interface rather than a fixed
hard-coded set.

## Service catalog

| Service | Purpose | Priority | Doc |
|---|---|---|---|
| **LiteLLM** | LLM proxy — agents use virtual keys; real API/token creds never exposed | MVP | [../services/llm-proxy.md](../services/llm-proxy.md) |
| **VictoriaMetrics + Loki + Grafana** | System metrics + log aggregation (memory-efficient) | MVP | [../services/observability.md](../services/observability.md) |
| **Phoenix (Arize)** | LLM/agent observability — traces, evals, prompt inspection | MVP | [../services/observability.md](../services/observability.md) |
| **SeaweedFS or RustFS** | Shared storage across host ↔ containers and container ↔ container | Core | [../services/shared-storage.md](../services/shared-storage.md) |
| **mitmproxy + Zeek** | Network inspection / proxy / traffic capture | Core | [../services/network-inspection.md](../services/network-inspection.md) |
| **Forgejo** | Internal git platform | Optional | [../services/_future.md](../services/_future.md) |
| **NATS** | Agent-to-agent communication / message bus | Future | [../services/_future.md](../services/_future.md) |

Priorities:
- **MVP** — required for the first usable version.
- **Core** — important to the project's value but not gating the MVP.
- **Optional** — nice to have, user-enabled.
- **Future** — expansion points, not yet designed.

## Networking between layers

Incus manages bridge networks so sandbox containers can reach the service containers they
are permitted to use (e.g. an agent reaches LiteLLM but only via its virtual key). Network
permissions are enforced per container and per user — see [../security-model.md](../security-model.md).

## Open items

- Service plugin interface (packaging, lifecycle, config schema, health/status).
- Service discovery / internal DNS within the Playground.
- Which services are co-located vs. one-container-per-service.
