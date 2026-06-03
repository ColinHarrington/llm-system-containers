# Coding Agents

Coding agents are first-class **inhabitants** of L2 sandboxes — the whole point of the platform
is to run many kinds of them safely. This doc covers which agents we support, how they get
installed, and how they're wired to models (the credentialing matrix). It builds on
[services/llm-proxy.md](services/llm-proxy.md), [agent-profiles.md](agent-profiles.md), and the
verified [research/litellm-claude-subscription.md](research/litellm-claude-subscription.md).

## Goal

Any mainstream coding agent should be runnable in a sandbox and able to reach models — ideally
through the **LiteLLM gateway** so the real provider credential stays out of the sandbox and
each agent gets a scoped **virtual key**.

## The unifying mechanism: point the agent at LiteLLM

Most agents accept a **custom base URL / OpenAI- or Anthropic-compatible endpoint** — exactly
what LiteLLM (`svc-litellm:4000`) exposes. So the general pattern is:

```
agent (in sandbox) ── virtual key ──▶ LiteLLM ── real credential ──▶ backend
```

The real credential (API key, cloud creds, or subscription token) lives in the proxy; the agent
holds only a virtual key, bounded by its [profile](agent-profiles.md) budget/scope.

## Agent catalog (initial)

| Agent | Type | How it reaches models / points at a gateway |
|---|---|---|
| **Claude Code** (Anthropic) | CLI | `ANTHROPIC_BASE_URL` → LiteLLM + `ANTHROPIC_AUTH_TOKEN` (virtual key); native subscription/API/Bedrock/Vertex auth too (see below) |
| **Pi** ([pi.dev](https://pi.dev)) | CLI | 15+ providers; **custom providers via `models.json`** → point at LiteLLM as a custom/OpenAI-compatible provider; API key or OAuth |
| **Codex / OpenAI-style** | CLI | OpenAI base URL + key → LiteLLM (OpenAI-compatible); or ChatGPT subscription login (native) |
| **Gemini CLI** (Google) | CLI | Google AI / Vertex; via LiteLLM's Gemini/Vertex support |
| **Aider** | CLI | Uses LiteLLM internally already → set provider/base URL + key |
| **OpenHands** | agent runtime | Uses LiteLLM internally → configure model/base URL |
| **Goose** (Block) | CLI | Provider-configurable → LiteLLM-compatible endpoint |

(Cursor and other IDE-bound agents are GUI/host-side and out of scope for in-sandbox CLI agents
for now.) The catalog is meant to grow; the *mechanism* (gateway + virtual key) is what matters.

## Backends are LiteLLM's concern, not each agent's

"Support Vertex/Bedrock/Azure/etc." means **LiteLLM is configured with those backends** — any
agent pointed at LiteLLM can then use them. One gateway, many backends:

- **Anthropic API**, **OpenAI**, **Google Vertex AI**, **AWS Bedrock**, **Azure**, **local /
  Ollama**, **OpenRouter**, … — all LiteLLM providers.
- This keeps agents backend-agnostic and centralizes credential handling + budgets + tracing.

## Two credentialing modes (the real design fork)

### Mode 1 — Gateway / virtual-key (preferred; fits the isolation model)
Agent → LiteLLM (virtual key) → backend. Real credential only in the proxy; agent never sees it.
Works for any agent that supports a custom base URL. This is the default the project is built
around ([services/llm-proxy.md](services/llm-proxy.md)).

### Mode 2 — Native / subscription auth
The agent authenticates directly with a **subscription** (e.g. Claude Pro/Max via OAuth, or a
ChatGPT plan). Verified specifics for **Claude Code + Max subscription via LiteLLM** (source:
https://docs.litellm.ai/docs/tutorials/claude_code_max_subscription,
https://code.claude.com/docs/en/llm-gateway — see the research doc):

- **B1 (documented/verified):** Claude Code does the subscription OAuth; LiteLLM **forwards** the
  client's `Authorization` token to Anthropic (`general_settings: forward_client_headers_to_llm_api:
  true`) while a separate `x-litellm-api-key` virtual key meters it. **The subscription token then
  lives in the sandbox** (Claude Code's `~/.claude/.credentials.json`), protected by sandbox
  boundaries — not by the proxy.
- **B2 (preferred for isolation; unconfirmed):** mint a long-lived token with `claude setup-token`
  → `CLAUDE_CODE_OAUTH_TOKEN`, and hold it **only in `svc-litellm`** as the Anthropic upstream, so
  it never enters the sandbox. Whether LiteLLM accepts the OAuth token as a static upstream
  credential (vs. only forwarding the client header) is still to be confirmed.

**Decision needed:** for subscription-backed Claude Code, do we accept the token-in-sandbox (B1,
works today) or hold out for proxy-held (B2, better isolation)? Until B2 is confirmed, B1 is the
working path, with the token treated as a sandbox-scoped secret.

## Installing agents into a sandbox

Two options (likely both):
- **Baked into a custom image** ([custom-images.md](custom-images.md)) — fast, reproducible (e.g.
  a `claude-code` or `pi` image). Preferred for agents used often.
- **Installed on launch / on demand** — a provisioning step per agent.

Runtime note: most agents are Node or Python; on the **Alpine** sandbox default this means
installing `nodejs`/`python3` via `apk` (lighter than a full toolchain image). Heavier agents may
warrant a Debian-based sandbox image instead — the base image is per-sandbox configurable.

## Ties to the rest of the system

- **agent-profiles** ([agent-profiles.md](agent-profiles.md)) already has an "LLM budget +
  virtual-key scope" axis — that's where per-agent model access and spend limits live. Critically
  relevant for **subscription** backends, which are metered (see below).
- **Observability**: LiteLLM → Phoenix tracing is agent-agnostic; the `X-Claude-Code-*`
  session/agent headers can be logged for per-agent attribution.
- **Network policy**: agents should reach **only** LiteLLM for model egress (Tetragon/Incus ACLs).

## Security / operational notes (from the research)

- **Pin LiteLLM** to a vetted version: PyPI **1.82.7 / 1.82.8 shipped credential-stealing
  malware** (source: https://code.claude.com/docs/en/llm-gateway, BerriAI/litellm#24518). A proxy
  holding real credentials / a subscription token is a high-value target. **Action item:** the
  `LiteLlmDeployer` currently `pip install`s litellm unpinned — pin + verify it.
- **Subscription is metered:** from **2026-06-15**, Agent SDK / `claude -p` usage draws on a
  separate monthly "Agent SDK credit" (source: https://code.claude.com/docs/en/authentication).
  Fan-out across many sandboxed agents can exhaust it → use LiteLLM per-key budgets.
- **ToS gray area:** backing many sandboxed agents with one subscription via a shared proxy isn't
  addressed by the docs — flag and verify before relying on it.

## Open items

- Confirm **B2** (proxy-held subscription token) against the LiteLLM `byok` tutorial.
- Pin/verify the LiteLLM version in the deployer.
- Per-agent install recipes (Claude Code, Pi, Codex, Aider, …) — image vs on-launch.
- Clarify Pi's exact gateway config (`models.json` custom provider pointing at LiteLLM).
- Decide the default credentialing mode per agent (gateway vs native/subscription).
