# Mockup B — Approachable native app

Static HTML mockups for the **llm-system-containers** desktop GUI app
(`llmsc` / `llmsctl`). These are visual design mockups, not a working product.

## Design direction

**B — Approachable.** Clean, light, friendly, calm. The goal is to lower the
barrier for less infra-savvy users while staying truthful to the real
architecture. Inspiration: Docker Desktop, OrbStack, Tailscale, Linear's polish.

- **Light-first with a dark-mode toggle** (top-right sun/moon; persisted to
  `localStorage`). All colors come from CSS custom properties so both themes
  share one design system.
- **Calm accent** (indigo/blue `#4f6ef7`) used sparingly; soft shadows, rounded
  cards (14–20px radii), generous whitespace.
- **Plain-language labels with the technical term as a secondary hint** —
  e.g. "Sandboxes" with "L2 system containers" underneath, "the VM" with
  `llmsc-vm` as the long form, placement shown as "In VM (L1)" vs
  "Own sandbox (L2)".
- Reusable component kit: cards, stat tiles, pills/badges, status dots,
  meters, segmented controls, toggle switches, tables, avatars, modals,
  console/log block, trace timeline, sliders.

## How to open

Double-click **`index.html`** — no build step, no server. The sidebar switches
between screens; tabs, modals, the wizard, the VM start/stop control, the theme
toggle, and the agent pause/steer controls are all driven by small vanilla JS.

## Files

- `index.html` — the whole app shell + every screen + modals (entry point)
- `app.css` — design system / all styling (light + dark themes)
- `app.js` — navigation, theme, tabs, modals, wizard, VM state, agent controls
- `icons.js` — inline SVG icon set used by JS-generated UI
- `NOTES.md` — this file

## Screens

1. **Home / Dashboard** — VM (L1) status hero with start/stop control, host
   resource meters (CPU/mem/disk reserved for `llmsc-vm`), stat tiles
   (sandboxes, active agents, nested L3 containers, services), quick actions,
   and a sandboxes table. Surfaces Tetragon eBPF + network policy.
2. **Sandboxes (L2)** — card grid of LLMSCs (`web-agent-01`, `ci-runner`,
   `data-pipeline`, `browser-bot`) with running/stopped states, role, image,
   users, nested-L3 count, CPU/mem; plus a "New sandbox" tile and the
   **New sandbox modal** (name, image, resources, users, mount, L3 toggle,
   live `llmsc launch …` preview).
3. **Sandbox detail** — tabbed: Overview (resources + security/network),
   Users & access (agent-claude / agent-aux / operator, per-UID workspace
   access, copyable `llmsc shell user@name`), Workspace mounts (tiered
   per-user access, SeaweedFS path), Nested containers (L3) (rootless
   Podman/Docker list emphasizing no privileged DinD).
4. **Agent control** — the observe / interrupt / steer / terminate triad:
   live LLM call trace (Phoenix-style), token usage + cost, Loki log console,
   prominent **Pause / Interrupt / Steer / Terminate** controls, and a working
   **Steer modal** that injects a message into the log. Shows the virtual-key
   credential isolation banner.
5. **Services** — status cards for LiteLLM, Phoenix, VictoriaMetrics+Loki+
   Grafana, SeaweedFS, mitmproxy+Zeek (each tagged L1 vs own-L2 placement),
   plus a **LiteLLM virtual keys** table (per-agent, scoped, revocable).
6. **Images** — base + custom image list (`dev-ubuntu-24.04`, `browser-tools`,
   `data-tools`, `base-debian-12`) with tooling and usage.
7. **Setup wizard** (`llmsctl init`) — 4 steps with a visible progress rail:
   (a) Resources sliders, (b) Services toggle list with per-service
   **L1-vs-own-L2 placement** segmented control, (c) Networking (egress policy,
   LiteLLM-only routing, mitmproxy/Zeek capture, Tetragon enforcement, subnet),
   (d) Review & create.

## Vocabulary surfaced

VM (L1) / sandbox · LLMSC (L2) / nested app containers (L3) / services ·
unprivileged · rootless · virtual keys · observable / interruptable / steerable
· Tetragon eBPF · network policy · `llmsc` / `llmsctl` / `llmsc shell user@name`.
