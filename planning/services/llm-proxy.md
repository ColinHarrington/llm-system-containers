# Service — LLM Proxy (LiteLLM)

**Priority:** MVP

## Purpose

Agents must be able to call LLMs **without ever holding real API or token credentials**.
An LLM proxy issues **virtual keys** to agents; the real provider credentials live only in
the proxy service (best isolated in its own L2 container — see
[README.md](README.md#placement-is-an-isolation-choice-not-a-layer)).

## Choice: LiteLLM

[LiteLLM](https://github.com/BerriAI/litellm) provides a unified proxy in front of many LLM
providers, with virtual key management, budgets/rate limits, and logging hooks.

Benefits for this project:

- **Virtual keys** — each agent/container/user gets its own scoped key; real credentials
  never leave the proxy container.
- **Per-key budgets and rate limits** — bound spend and throughput per agent.
- **Logging hooks** — feed observability directly (see below).
- **Provider-agnostic** — swap or mix providers without touching agent code.

## Integration with observability

LiteLLM's logging hooks feed **Phoenix** so that every agent LLM call is automatically
traced (prompts, responses, tokens, latency). See
[observability.md](observability.md).

## Security role

This is a key part of the credential-isolation backstop in the
[security model](../security-model.md): even an agent that fully escapes its own permission
framework still only ever sees a virtual key, scoped and revocable, never the real secret.

## Open items

- Key issuance lifecycle: per agent, per container, or per orchestration? Rotation/revocation.
- Budget/rate-limit defaults and how they surface in the GUI/CLI.
- Network policy so agents can reach **only** the proxy for LLM egress (no direct provider calls).
