# Service — Observability

**Priority:** MVP

## Why it matters

Observability is a first-class concern. Agents in this system must be:

- **Observable** — humans can see what agents are doing.
- **Interruptable** — humans can stop an agent mid-run.
- **Steerable / re-steerable** — humans can redirect an agent.

Constraints: **memory-efficient** and **open-source**.

## Two complementary planes

### LLM / agent observability — Phoenix

[Phoenix (Arize)](https://github.com/Arize-ai/phoenix) — open-source, lightweight,
LLM-native: traces, evals, prompt/response inspection. Chosen over Langfuse to avoid the
heavier ClickHouse dependency and keep the memory footprint low.

- Fed automatically by **LiteLLM** logging hooks, so every agent LLM call is traced. See
  [llm-proxy.md](llm-proxy.md).

### System observability — VictoriaMetrics + Loki + Grafana

A lean metrics + logs stack across the VM and containers:

- **VictoriaMetrics** — Prometheus-compatible metrics, much lower memory footprint than
  Prometheus, single binary.
- **Loki** — log aggregation, fairly lean by design.
- **Grafana** — dashboards over both.
- **OpenTelemetry Collector** — lightweight glue/instrumentation layer tying things
  together.

## Interrupt / steer control plane

Observing is off-the-shelf; **interrupting and steering** is largely a control-plane
feature of this project itself, not a third-party tool. It needs a way to:

- Signal a running agent process (pause/stop) per-user / per-container.
- Inject into an agent's context to redirect it.
- Terminate an agent cleanly without taking down the whole container.

This control plane ties into the GUI/CLI ([../interfaces.md](../interfaces.md)) and the
per-UID user model ([../architecture/system-containers.md](../architecture/system-containers.md)).

## Open items

- Exact interrupt/steer mechanism (process signals, agent-side cooperation, sidecar?).
- Retention/footprint tuning for VictoriaMetrics + Loki to stay memory-efficient.
- Whether Phoenix and the system stack share a Grafana pane or stay separate.
