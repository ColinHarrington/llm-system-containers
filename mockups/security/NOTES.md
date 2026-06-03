# Security mockup — Guardrails &amp; Agent profiles

Static, self-contained HTML mockup (`index.html`) for the desktop GUI of
**llm-system-containers** (`llmsc` / `llmsctl`). No build step — open by double-click.
Tailwind via CDN, inline SVG icons, vanilla JS. Visual style matches
`../nested-view/index.html` (dark concept aesthetic).

An **app shell** at the top toggles between the two screens via a segmented nav.

## Posture reinforced throughout

- Everything is **unprivileged** (L2 LXC) and **rootless** where L3 is enabled — privileged
  containers are never used.
- The GUI surfaces the defense-in-depth backstops as **configurable ACLs** enforced at the
  kernel (**Tetragon eBPF**) plus **Incus** network policy plus **Linux UID** isolation.
- Policies are expressed **per sandbox (L2 / LLMSC) and per UID** (agent UID vs human
  operator UID), matching the per-user model.
- **default-deny** everywhere (filesystem outside grants, egress, dangerous syscalls).
- Agents never hold real credentials — only **LiteLLM virtual keys** scoped per profile.

## Screen 1 — Guardrails

A scope bar selects the **sandbox (L2)** and toggles the **UID** (agent 1001 vs operator
1000); the rules and event feed re-render per UID to make "applies per sandbox and per UID"
concrete. Three editable rule-list domains:

- **Filesystem ACLs** — RW / RO / DENY path rules (RW `/workspace/scratch`, RO repo & docs,
  DENY `/etc`, DENY everything outside grants). Operator UID owns the full workspace.
- **Syscall policy** — deny list of dangerous syscalls (`ptrace`, `mount`, `kexec_load`,
  `bpf`, module ops, `setns`, `pivot_root`…) plus an allow set of normal dev syscalls.
- **Network ACLs** — default-deny egress + allowlist: LLM only via **LiteLLM** (virtual
  key), HTTP(S) forced through **mitmproxy**, allowlisted docs; raw egress and the cloud
  metadata endpoint denied.

A live **Tetragon-style enforcement event feed** streams recent allow/deny decisions across
the three domains for the selected sandbox + UID.

## Screen 2 — Agent profiles

Profiles are **reusable named bundles** of the guardrail boundaries (filesystem + syscall +
network ACLs) plus resource limits, L3 capability, LLM budget, and virtual-key scope. A
**gallery of cards** each summarizes ALLOW/DENY across filesystem / syscalls / network /
containers, LLM budget, resource limits, and which agents currently use it. Clicking a card
opens a **detail / edit** view; an **"Assign to sandbox"** modal binds the bundle to a
sandbox UID (shows the `llmsctl security assign` command).

### Archetype profiles

- **researcher** — broad read; web/docs egress via mitmproxy; RO repo, RW only scratch; no
  L3 builds; generous LLM budget.
- **tester** — RW repo; run tests; L3 (rootless) test infra enabled; limited egress (pkg
  mirrors + LLM); medium budget.
- **builder** — RW repo + artifacts; L3 enabled; registry/package egress allowlist; builds
  images; medium budget.
- **validation** — strictest: read-only everything, checks only, NO egress except LLM, no
  writes, tightest syscall set, no L3; small budget.
- **orchestrator** — coordinates agents over NATS, can launch/stop sandboxes via the control
  plane, minimal direct filesystem, broad LLM budget, no raw egress.

## Files

- `index.html` — entry point (both screens + app shell).
- `NOTES.md` — this file.
