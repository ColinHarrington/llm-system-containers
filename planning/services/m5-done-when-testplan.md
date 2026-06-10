# M5 done-when — live integration test plan

The M5 success criterion (`planning/buildout.md`, `planning/mvp.md`): **an agent in a sandbox
reaches the LiteLLM proxy via a virtual key, and the call is traced in Phoenix.** This is a
*live* test — it needs a running Lima+Incus VM and stands up service containers — so it is not a
`cargo test`. It is promoted from the spikes (`planning/spike-plan.md`) and runs as `uv`
operational glue: **`scripts/m5_litellm_phoenix.py`**.

## What it proves

| Step | Command (real product surface) | Asserts |
|------|--------------------------------|---------|
| Services up | `llmsctl services enable litellm/phoenix` → `services up` | both deploy; traces wired (`enable_phoenix`) |
| Mint key | `llmsctl keys sync` | a `sk-llmsc-<sandbox>-<agent>-<random>` token minted + persisted |
| Sandbox | `llmsc apply` | the sandbox + its agent Linux user exist and run |
| Inject | `llmsc agent env <agent@sandbox>` | `OPENAI_BASE_URL` + the virtual key land in the agent's shell env |
| Call | in-sandbox `curl $OPENAI_BASE_URL/chat/completions` as the agent | HTTP 200 + a completion |
| Trace | poll Phoenix (`svc-phoenix:6006`) | the call appears as a span |

## Hermetic by default, real provider opt-in

By default the agent calls the built-in **`mock`** model (`mock_response` in the generated
LiteLLM config) — **no provider key, no spend, repeatable**. This proves the whole path
(virtual-key auth → proxy reachability → trace) without external credentials.

Set **`LLMSC_LIVE_PROVIDER=1`** (plus `LLMSC_PROVIDER`/`LLMSC_PROVIDER_KEY`) to first run
`keys set-provider` and call the real `default` model instead — a true end-to-end check that
costs a few cents and is not repeatable. The provider key is stored only inside `svc-litellm`
(credential isolation); it never touches `llmsc.toml` or the host key store.

## Running it

```bash
# CI path — Incus on the host, no VM (this is what the m5-done-when workflow runs):
uv run scripts/m5_litellm_phoenix.py run --mode local
# Against the Lima VM (prereq: the VM exists — llmsctl up, or spike phase0):
uv run scripts/m5_litellm_phoenix.py run --mode vm
# Real-provider end-to-end (a few cents, not repeatable):
LLMSC_LIVE_PROVIDER=1 LLMSC_PROVIDER=openai \
  LLMSC_PROVIDER_KEY=sk-… uv run scripts/m5_litellm_phoenix.py run --mode vm
uv run scripts/m5_litellm_phoenix.py clean --mode local   # remove the test sandbox
```

**In CI:** `.github/workflows/m5.yml` runs the `--mode local` mock path nightly + on demand
(`workflow_dispatch`). It's **not** a per-PR gate — it pip-installs LiteLLM + Phoenix into L2
containers (~10 min) and the Phoenix span poll is best-effort, so a red nightly is informational,
not blocking. The fast per-PR integration gate is the e2e lifecycle (`planning/testing-e2e.md`).

It writes its `llmsc.toml` to a throwaway dir and runs the built CLIs there, so the repo's own
config is never touched. Each step prints pass/fail and a summary table; exit code is non-zero if
any step fails.

## What it depends on (the glue this milestone added)

- LiteLLM config registers the `arize_phoenix` callback **and** a hermetic `mock` model.
- Virtual keys are `sk-llmsc-<sandbox>-<agent>-<random>` (identifiable prefix + rotatable random
  suffix), minted with a caller-supplied `key` and persisted 0600 in the host key store.
- `llmsc agent env` injects the proxy URL + the agent's virtual key per-user.
- `llmsctl services up` auto-wires LiteLLM traces → Phoenix when both are enabled.

## Open items

- **Phoenix span assertion is best-effort.** The poll tries the REST span list then a GraphQL
  `traceCount`; the exact endpoint shifts between Phoenix versions. If it can't read the API it
  prints a note rather than a false pass — verify by hand at `http://localhost:6006` in the VM.
- **Not in CI.** Needs a Linux runner with nested Incus; tracked with the other promoted live
  tests in `planning/spike-plan.md`. Until then it's a manual gate run before tagging M5 done.
- **Anthropic SDK base URL** is not injected yet (only the OpenAI-compatible pair) — add once the
  exact LiteLLM Anthropic route is validated live.
