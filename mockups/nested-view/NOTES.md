# Topology — nested view (concept)

A spatial visualization of the **nesting**, requested to make the system legible:

```
L1 VM (llmsc-vm)  ── outer block
  └─ L2 sandboxes (LLMSC)  ── cards within the VM
       └─ agents + human operator  ── mini-cards within each sandbox
```

## Intent

- **Show the nesting**: VM as the containing block, L2 sandboxes inside it, agents inside each sandbox.
- **Per-agent activity**: each agent shows a one-line "what it's doing" plus a row of **tool icons**
  (terminal, editor, git, browser, run/tests, containers, database, web search, LLM call, files).
  The currently-used tool is highlighted and gently animates.
- **Agent state** via colored status ring/dot: active / thinking / waiting / idle.
- **Human operator** is shown distinctly (violet) alongside the agents.

## Deliberately NOT shown

- **L3 app containers are not drawn.** Per scope, enabling L3 is what matters, not visualizing
  it — so a sandbox only carries a small **"L3 enabled"** badge marking where nesting is allowed.

## Notes

- Self-contained (`index.html`, Tailwind CDN + inline SVG icon sprite + vanilla JS).
- Includes light "liveness": an active agent occasionally switches its active tool / action.
- Services (LiteLLM, Phoenix, Grafana, mitmproxy, SeaweedFS) appear as a subtle band in the VM,
  reinforcing that they live in L1 / their own L2.
- This is a **concept screen** to fold into whichever variant (A/B/C) is chosen, restyled to match.
