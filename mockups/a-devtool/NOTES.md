# Mockup A — Developer / infra tool

Static HTML design mockups for the **llm-system-containers** desktop GUI app
(CLIs: `llmsc`, `llmsctl`). Open `index.html` directly in a browser — no build step.

## Design direction

Dense, elegant, **dark theme**, keyboard-friendly — native to engineers who live in
the terminal. Inspiration: Linear, lazydocker / k9s, Vercel / Railway dashboards.

- **Design system:** custom dark `ink-*` palette (near-black surfaces, layered greys)
  with a single indigo `accent`. Subtle status colors (emerald / amber / rose / sky).
  Inter for UI text, JetBrains Mono for all identifiers, commands, IPs, metrics.
- **Information density:** tight tables, compact cards, thin progress bars, small
  status dots — lots of real signal per screen without feeling like a wireframe.
- **Command-equivalent motif (recurring):** action buttons carry a `data-cmd` and
  flash the underlying `llmsc` / `llmsctl` command as a toast (e.g. clicking *Stop VM*
  surfaces `$ llmsctl down`). Many panels show the equivalent command inline in mono.
- **Keyboard-friendly:** `1`–`5` jump between screens, `Esc` closes drawers/modals,
  `⌘K` / `/` affordances in the chrome.
- **Faux terminal drawer:** "open shell" slides up a terminal showing
  `llmsc shell user@sandbox` with realistic prompt/`id`/`podman ps` output.

## Vocabulary surfaced throughout

VM (L1) · sandbox / LLMSC (L2, unprivileged) · nested app containers (L3, rootless,
"no DinD") · services (L1-vs-own-L2 placement) · virtual keys · observable /
interruptable / steerable · Tetragon eBPF · network policy / default-deny egress ·
per-UID isolation · "privileged: never".

## Screens (single-file app shell with sidebar nav)

1. **First-run setup wizard** (`llmsctl init`) — 4 steps with a progress rail:
   (a) Resources (CPU / memory / disk sliders), (b) Services (toggle list, each with a
   one-line description and an **L1-VM vs own-L2** placement switch), (c) Networking
   (bridge subnet, egress policy, mitmproxy/Tetragon toggles), (d) Review & Create
   (summary + generated `llmsctl init … && llmsctl up` command).
2. **Dashboard / Home** — VM status banner with start/stop/restart, host CPU/mem/disk/net,
   summary cards (sandboxes / L3 containers / services / active agents), sandbox mini-table
   with running + starting + stopped states, quick actions, security-backstop panel.
3. **Sandboxes (L2)** — card grid (name, image, status, users, CPU/mem, L3 count) with
   status filters; **New sandbox** modal (name, image picker, resources, agent + operator
   users, workspace mount, enable-L3 toggle); **sandbox detail** (users with per-UID access,
   workspace mounts with tiered rw/ro, nested L3 container table, open-shell affordances).
4. **Agent observability & control** — the observe / interrupt / steer / terminate triad:
   live Phoenix-style trace (LLM + tool spans, a blocked egress, an injected steer),
   token/cost strip, virtual-key chip, Loki log tail, Tetragon eBPF event feed, and a
   **Steer** modal that injects a message into the running agent.
5. **Services** — status cards (LiteLLM, Phoenix, Grafana+VictoriaMetrics+Loki, SeaweedFS,
   mitmproxy+Zeek, Forgejo) with placement + priority, plus a **LiteLLM virtual keys** table
   (per-agent assignment, model scope, budget, spend, status).
6. **Images** — base + custom image table (base, tooling, size, used-by, built) with build CTA.

## Files

- `index.html` — entire app shell + all screens + interactions (Tailwind CDN + vanilla JS).
- `NOTES.md` — this file.

## Tech

Self-contained static HTML, Tailwind via CDN, minimal vanilla JS for nav / tabs / drawers /
modals / toasts. Empty + populated states shown (e.g. stopped `scratch-01`, disabled Forgejo,
revoked virtual key, starting sandbox with 0 L3 containers).
