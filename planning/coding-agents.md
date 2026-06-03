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

## Credential rule (firm)

**Real provider/subscription credentials live ONLY in service containers** (e.g. `svc-litellm`) —
holding credentials is a service container's purpose, and a service container is **not a sandbox**.
**L2 *sandboxes* hold ONLY LiteLLM virtual keys** — never a real API key or subscription token.

Consequence: we use the **proxy-held** credential mode, and we explicitly do **not** use LiteLLM's
*forward-client-credential* modes (its "BYOK" mode, and the documented `max_subscription` flow) —
those keep the real credential on the **client**, i.e. in the sandbox, which this rule forbids.

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

**Decision (project owner): no real auth token in an L2 sandbox.** That rules out B1. The sandbox
holds only a LiteLLM **virtual key**; the real credential stays in `svc-litellm`; and the agent's
egress is **forced through** the proxy. Full detail below.

## How a sandboxed agent reaches models — no token in the sandbox, egress forced through LiteLLM

The enforced gateway pattern (config verified against the LiteLLM gateway + BYOK docs):

### 1. Credential placement
- **The real upstream credential lives only in `svc-litellm`.** Standard (non-BYOK) LiteLLM holds
  it server-side:
  ```yaml
  model_list:
    - model_name: claude
      litellm_params:
        model: anthropic/claude-sonnet-4-5
        api_key: os.environ/ANTHROPIC_API_KEY   # real key — only in the proxy container
  ```
  (source: https://docs.litellm.ai/docs/tutorials/claude_code_byok. Note: LiteLLM's *BYOK* mode
  is the opposite — it **forwards the client's** key — so we deliberately do **not** use BYOK.)
- **The sandbox holds only a LiteLLM virtual key** (`POST /key/generate` → `sk-litellm-…`). It is
  *not* an Anthropic credential: it authenticates to **LiteLLM only**, is scoped + budgeted +
  revocable, and is useless against `api.anthropic.com` directly.

### 2. Claude Code config in the sandbox (pure gateway mode — no `/login`)
```bash
export ANTHROPIC_BASE_URL=http://svc-litellm.llmsc:4000
export ANTHROPIC_AUTH_TOKEN=sk-litellm-<agent-virtual-key>   # sent as Authorization: Bearer to LiteLLM
```
Claude Code does **not** run subscription `/login` in the sandbox (that would write a real token
to `~/.claude/.credentials.json`). It only ever holds the virtual key.

### 3. Per-request flow
```
Claude Code ──POST /v1/messages, Authorization: Bearer <virtual-key>──▶ svc-litellm:4000
svc-litellm: authenticate virtual key → enforce budget/rate-limit → log/trace (Phoenix)
svc-litellm ──real credential (x-api-key)──▶ api.anthropic.com ──▶ response ──▶ Claude Code
```
The sandbox sees only its virtual key and the model response — never the real key.

### 4. Forced egress (the "tighter" part — enforcement, not just config)
- Sandbox network policy = **default-deny egress, allow only `svc-litellm:4000`** (plus required
  internal services), enforced at the kernel via **Incus network ACLs + Tetragon**, per-UID
  ([security-model.md](security-model.md), [architecture/networking.md](architecture/networking.md)).
- So even a rogue process or a leaked credential **cannot reach `api.anthropic.com` directly** —
  there's no route/permission to anything but the proxy. Model traffic is structurally funneled
  through LiteLLM.
- `svc-litellm` itself gets a tight egress policy: it may reach **only** the chosen backends
  (e.g. `api.anthropic.com`). Optionally route that through **mitmproxy/Zeek** for inspection.
- Net effect: compromising a sandbox yields a **revocable virtual key**, not the real credential;
  and the agent physically cannot reach a model except through the audited proxy.

### 5. The subscription caveat (honest)
- **API key upstream → fully supported, today.** Proxy-held `api_key` + per-agent virtual keys, no
  token in the sandbox. This is the clean path.
- **Subscription upstream + no token in sandbox → not a documented LiteLLM path.** Both documented
  subscription/BYOK flows *forward the client's* credential (token lands in the sandbox). LiteLLM
  holding a **subscription OAuth token** as a static upstream isn't documented — and it would need
  to be sent as `Authorization: Bearer`, whereas LiteLLM's anthropic provider sends API keys as
  `x-api-key`. Possible-but-unverified workarounds: a static `extra_headers: { Authorization:
  "Bearer <oauth-token>" }` on the proxy model, or a thin forwarding shim in `svc-litellm`. Plus
  the ToS gray area for proxying one subscription across many agents.
- **Bottom line:** the no-token-in-sandbox architecture is clean and works **today with an API
  key**. For a **subscription**, the missing "proxy-held" piece appears to be solvable with a
  dedicated component — see below.

### Subscription with token-out-of-sandbox: CLIProxyAPI (candidate)

[CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI) is a standalone Go daemon that
**does the Claude subscription OAuth itself and stores the token server-side**
(`--claude-login` / `--no-browser`; token in `~/.cli-proxy-api/`), then **exposes an
OpenAI-compatible API** (`:8317/v1`) that clients call with a **separate local key** — never the
subscription token (source:
https://rogs.me/2026/02/use-your-claude-max-subscription-as-an-api-with-cliproxyapi/ ; repo:
https://github.com/router-for-me/CLIProxyAPI).

Run it in its **own service container** (`svc-cliproxy`, not a sandbox) and front it with LiteLLM:

```
Claude Code (sandbox, virtual key) → svc-litellm (budgets/tracing) → svc-cliproxy (subscription token) → Anthropic
```

LiteLLM points its upstream at `http://svc-cliproxy:8317/v1` (OpenAI-compatible). The subscription
token lives **only in `svc-cliproxy`**; sandboxes hold only virtual keys → satisfies the firm
rule, while keeping LiteLLM's per-agent virtual keys / budgets / Phoenix tracing.

**Caveats (must address before relying on it):**
- Third-party tool holding a high-value subscription token — same trust class as LiteLLM (cf. the
  1.82.7/1.82.8 malware). **Vet + pin** it; isolate `svc-cliproxy` with egress only to Anthropic.
- **ToS gray area** — proxying a Max subscription as a general API is less clearly sanctioned than
  the official `claude setup-token`. Deliberate decision required.
- Sourced from a **blog post (via summarizer)**; the repo is the primary source and is **not yet
  verified** here — confirm mechanics, maintenance, and trust before adoption.

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

- **Subscription with token-out-of-sandbox:** strongest candidate is **CLIProxyAPI** in its own
  service container (holds the subscription token server-side; OpenAI-compatible API; clients use
  a separate key) fronted by LiteLLM — see the section above. **To do:** verify the repo
  (router-for-me/CLIProxyAPI) mechanics/trust, decide on the ToS gray area, and add a deployer +
  locked-down egress for `svc-cliproxy`. (The LiteLLM-only `extra_headers`/shim idea is a fallback;
  BYOK + max_subscription both forward the client token, which the owner ruled out.)
- ✅ LiteLLM version **pinned** (1.87.0) in the deployer.
- Implement the **forced-egress** network policy (default-deny except `svc-litellm`) + the
  proxy-held `api_key` config + virtual-key minting in the deployer (currently placeholder config).
- Per-agent install recipes (Claude Code, Pi, Codex, Aider, …) — image vs on-launch.
- Clarify Pi's exact gateway config (`models.json` custom provider pointing at LiteLLM).
- Decide the default credentialing mode per agent (gateway vs native/subscription).
