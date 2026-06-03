# Services — Optional & Future

Services not in the MVP/Core path. Kept here as known expansion points.

## Forgejo — internal git (Optional)

[Forgejo](https://forgejo.org/) as an internal git platform inside the Playground.

- Lets agents push/pull without touching external services.
- **Not required for MVP** — explicitly optional / user-enabled.

## NATS — agent-to-agent communication (Future)

A future service to facilitate **agent-to-agent communication**. This is an expansion point,
not yet designed.

- [NATS](https://github.com/nats-io/nats-server) is a natural fit: tiny memory footprint,
  open-source, supports pub/sub and request/reply patterns well-suited to agent
  coordination.
- Enables the "software factory" vision where many agents collaborate across containers.

## openclaw integration (Future)

[openclaw] — an AI agent tool for running **scheduled and autonomous** agents. This system
would be a good place to run such dev-style autonomous agents safely (sandboxed, observable,
interruptable). Integration deferred; noted so it isn't lost.

## Plugin direction

All of the above reinforce the goal of a **service plugin interface** (see
[../architecture/service-containers.md](../architecture/service-containers.md)) so new
services can be added without changing the core.
