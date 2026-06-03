# GUI Mockups

Exploratory **static HTML mockups** for the desktop GUI app, produced as three independent
design directions to compare side by side. These are visual explorations only — no backend,
realistic placeholder data.

## How to view

Open **`mockups/index.html`** in a browser. It's a launcher with a top switcher bar that
loads each variant in place:

- Click the **A / B / C** tabs, or
- Press **1 / 2 / 3**, or use **← / →** arrow keys to flip between them.
- **open standalone ↗** opens the focused variant full-window in a new tab.

Each variant is its own self-contained app shell (sidebar nav between screens).

## The three directions

| | Variant | Direction | Inspiration |
|---|---|---|---|
| **A** | [`a-devtool/`](a-devtool/index.html) | Dense, dark, keyboard-friendly developer/infra tool; shows the `llmsc`/`llmsctl` command behind actions | Linear, lazydocker/k9s, Railway |
| **B** | [`b-approachable/`](b-approachable/index.html) | Clean, light, generous whitespace; lowers the barrier; plain-language labels | Docker Desktop, OrbStack, Tailscale |
| **C** | [`c-controlroom/`](c-controlroom/index.html) | Data-forward mission-control with panels, charts, live feeds; foregrounds observe/interrupt/steer + security | Grafana, Datadog, NOC console |

## Concept screens (style-neutral, to fold into the chosen variant)

Beyond the three full directions, several **single-purpose concept screens** explore specific
ideas. They share one dark "concept" style and will be restyled to match whichever variant
(A/B/C) is chosen.

| Tab | Dir | What it explores |
|---|---|---|
| Topology (rich) | [`nested-view/`](nested-view/index.html) | Nested map VM→sandboxes→agents with per-agent activity + tool icons |
| Topology (MVP) | [`nested-simple/`](nested-simple/index.html) | Same nesting, simplified: agents **listed only**, no per-agent status |
| Networking | [`networking/`](networking/index.html) | VM networks and which sandboxes attach to which; egress + inspection |
| Security & profiles | [`security/`](security/index.html) | Guardrails (file/syscall/network ACLs) + reusable agent profiles |

## Screens each variant covers

1. **First-run setup wizard** — resources, services (+ L1-vs-L2 placement), networking, review.
2. **Dashboard / Home** — VM status + controls, host resource usage, sandbox/service health, quick actions.
3. **Sandboxes (L2)** — list + new-sandbox flow + detail (users/access, workspace mounts, nested L3 containers, `llmsc shell user@name`).
4. **Agent observability & control** — live LLM-call trace, token usage, logs, and Pause / Interrupt / Steer / Terminate.
5. *(if present)* **Services** (incl. LiteLLM virtual keys) and **Images**.

See each variant's `NOTES.md` for its specific design rationale.

> **Scope note:** these mockups surface **L3 app containers** (nested Docker/Podman) in
> dashboard counts and sandbox detail. That visualization is **deprioritized** — the L3
> *capability* stays, but managing it in the GUI is deferred to a possible future plugin.
> Treat L3 panels in these mockups as exploratory, not part of the intended core GUI.

## Vocabulary reference (so mockups stay consistent)

- **Host** — the user's computer (macOS/Linux) running the app.
- **VM (L1)** — `llmsc-vm`, the host-native VM running Incus.
- **Sandbox / LLMSC (L2)** — unprivileged system container; agent/human workspace.
- **App container (L3)** — rootless Docker/Podman nested inside a sandbox.
- **Services** — LiteLLM, Phoenix, VictoriaMetrics/Loki/Grafana, SeaweedFS, mitmproxy/Zeek.
- CLIs: **`llmsc`** (containers) · **`llmsctl`** (platform/VM).
