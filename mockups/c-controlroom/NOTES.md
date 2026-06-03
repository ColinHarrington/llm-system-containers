# Control Room — design direction C

A **data-forward, dark mission-control** GUI mockup for `llm-system-containers`
(`llmsc` / `llmsctl`). The aesthetic is a network operations center / Grafana-style
console: panel/grid dashboards, inline-SVG charts, sparklines, status tiles, and
live activity feeds. The agent screen is built as a real-time **cockpit** and the UI
foregrounds the **observe / interrupt / steer** triad and the **security posture**.

## Design system
- **Dark palette** (deep navy surfaces `#0a0e17` → `#1a2334`), with a blue/violet/cyan
  accent set and green/amber/red status colors. Inter for UI, mono for identifiers,
  commands, and metrics.
- Reusable components in `styles.css`: `.panel` (+ header/body), stat `.tile`, status
  `.dot` / `.badge`, `.btn` variants (primary/ghost/danger/warn), `.tbl` tables,
  `.meter`, `.feed`, `.seg` segmented control, `.toggle`, `.tag`, `.code-pill`.
- All charts are **hand-rolled inline SVG** in `app.js` (no chart libraries):
  `sparkline`, `areaChart`, `donut`, `bars`, plus a trace waterfall built in markup.
  Several sparklines and the agent feed **tick live**.

## Vocabulary surfaced
VM (L1 `llmsc-vm`), sandbox / LLMSC (L2, unprivileged), nested app containers
(L3, rootless), services with **L1-vs-own-L2 placement**, `llmsc shell user@name`,
virtual keys (LiteLLM), Tetragon eBPF, network policy / default-deny egress,
mitmproxy, Phoenix, per-UID isolation.

## Files
- `index.html` — app shell (sidebar nav + topbar with VM status / start-stop) and all
  in-app screens. **Open this first.**
- `wizard.html` — first-run setup wizard (separate first-run flow).
- `styles.css` — design system.
- `app.js` — nav/tab/toggle wiring, SVG chart helpers, live ticks.

## Screens
1. **First-run wizard** (`wizard.html`) — 4 steps with progress: Resources (CPU/mem/disk
   sliders + allocation donuts), Services (toggles + per-service L1/own-L2 placement),
   Networking (subnet, egress allowlist, default-deny, per-UID policy), Review & Create
   (with the equivalent `llmsctl init … && llmsctl up` command).
2. **Mission Control / Dashboard** — VM status tile + topbar start/stop, host resource
   usage (live area chart), sandbox/L3/service counts & health, security-posture panel,
   quick actions, LLM activity, and a live platform event stream.
3. **Sandboxes (L2)** — table **and** card views (name, image, role, status, users,
   CPU/mem, nested L3 count; running + stopped/empty states), an inline **New sandbox**
   flow (image, resources, users, workspace mounts, nesting toggle), and a **detail**
   view (users w/ per-UID access, nested L3 containers, workspace mounts & policy,
   `llmsc shell operator@web-agent-01`).
4. **Agent Control (cockpit)** — prominent **Pause / Interrupt / Steer / Terminate**
   bar, live LLM-call trace waterfall + streaming activity feed (Phoenix-style), live
   token usage / cost / virtual-key panel, multi-tab console (stdout / last LLM msg /
   tool calls), and a guardrails/steer history. **Steer** opens an inject-message modal.
5. **Services** — status tiles for LiteLLM, Phoenix, VictoriaMetrics+Loki+Grafana,
   SeaweedFS, mitmproxy+Zeek, Tetragon (each with L1/own-L2 placement) + a **LiteLLM
   virtual keys** table.
6. **Images** — base/custom image catalog, a build form, and storage usage.
7. **Security Posture** — posture tiles, a **live Tetragon eBPF + network policy** event
   stream (allow/deny/warn), and a defense-in-depth backstops panel.

## Notes
- Pure static HTML; Tailwind CDN is loaded for utility access but the look is driven by
  `styles.css`. Files open by double-clicking — no build step.
- Realistic placeholder data throughout (sandboxes `web-agent-01`, `ci-runner`,
  `data-pipeline`; users `agent-claude`, `agent-aux`, `operator`; images
  `dev-ubuntu-24.04`, `browser-tools`; `sk-vk-…` virtual keys).
