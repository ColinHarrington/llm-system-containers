# Design Principles

The load-bearing convictions behind llm-system-containers. When a design choice is unclear,
these decide it. Distilled from the Incus-ecosystem evaluation (2026-06) — keep them short and
keep them honored.

## 1. Respect the Incus ecosystem — don't paint into a corner

Incus is the runtime source of truth, and everything we manage must stay legible to plain
`incus`. Express state as **Incus-native primitives** (profiles, devices, instance config keys,
networks, a dedicated project for namespacing) — never a parallel hidden store that `incus`
can't see. A user must always be able to drop to raw `incus` and not be surprised.

Corollary: don't foreclose larger Incus deployments (multi-user, remotes, clustering) even
though the MVP uses none of them. Avoid assumptions that would make those impossible to add
later; we just don't *build* them now.

## 2. Cattle, not pets

Sandboxes are **disposable**. The durable artifact is the **image**, not a long-lived OS drive.
Use a running sandbox to *drive image building*, then publish and relaunch — don't keep pets
alive with backups/restore/migration. This deletes a large amount of product surface
(no OS-drive backup lifecycle, no migration) and makes the **configure → publish image →
relaunch** loop the core value, not a side feature ([custom-images.md](custom-images.md)).

## 3. Localhost-only control surface

The Incus API is reached over a **local** socket/forward (Incus lives in the L1 VM), never
exposed on the network. This handles most of the network threat model by default. Harden the
data plane with Incus-native controls (NIC mac/ipv4/ipv6 anti-spoof filtering, egress ACLs) —
see [security-model.md](security-model.md).

## 4. Library-first; daemon (and any served UI) deferred

All logic lives in `llmsc-core`; the CLIs and the Tauri GUI are thin shells over it. A **served
web UI / PWA implies a daemon/API** — so that idea is the *evolution* that arrives **with** the
daemon, when remote/multi-host or browser-anywhere access actually matters. Until then: keep the
Tauri shell, and keep the GUI's backend boundary (`gui/src/lib/core.ts`) clean enough that it
could later target an HTTP API instead of Tauri `invoke` without rewriting screens.

## 5. Single-operator MVP — owns the VM, no RBAC

For now there is **one operator who owns the whole VM**; that is *why* we need no RBAC/OpenFGA.
Multi-user/RBAC is a real Incus capability we deliberately don't stomp on (principle 1) but also
don't build. Don't bake in assumptions that block a future multi-user mode, but don't pay for
one either.

## 6. "host" is a logical target, not the metal

A **target/context** = an **Incus remote**: `local` (Linux metal socket), `vm` (the L1 VM — the
macOS default), or `remote` (an existing Incus endpoint). Modeling it this way makes
local / nested / remote three instances of one abstraction and keeps multi-host *possible* later
without a rewrite. (The current code/docs conflate "host" with the metal — a rename worth doing
early; see [open-questions.md](open-questions.md).)

## 7. Focus the use cases; resist the catalog

A sprawling app/agent catalog is a backlog masquerading as a plan. Nail a thin spine —
**(a) headless single user (shell + a coding agent)**, **(b) single user + one GUI app via the
display transport**, **(c) the image-build loop** — end to end before chasing breadth. Pick 2–3
flagship demos; make the rails (launch → configure → connect → rebuild-as-image) flawless.

## Open items

- Where principle 6 (the `host`→`target` rename) lands in the model/CLI/GUI — tracked in
  [open-questions.md](open-questions.md).
- When principle 4's daemon is warranted (and thus a served web UI) — [tech-stack.md](tech-stack.md).
