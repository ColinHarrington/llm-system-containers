# Tech Stack

## Context

How the platform is built. Driven by three owner preferences: **Rust** over Go; **CLI-first**
(some users will only use the CLIs); and configuration that is **durable on disk and seamless
between CLI and GUI**, with **Incus usable directly** a layer beneath our tooling.

## Language: Rust

- Preference is **Rust over Go**.
- **Incus** integration via its **REST API** (stable, over a unix socket / HTTPS) — the Go
  client is just a wrapper over the same API, so Rust loses little. (Use a community crate or a
  small hand-rolled client over the socket.)
- **Lima** driven via **`limactl`** (CLI + YAML templates) — language-agnostic, shell out.
- **Tetragon** configured via **policy YAML** applied into the VM.
- Benefits: strong correctness, great CLI ergonomics (`clap`), single static binaries, and
  **Tauri** for the GUI.

## CLI-first, shared core crate

- Some users will use **only the CLIs**, so the CLI must be **fully capable standalone**.
- A shared **`llmsc-core`** Rust crate holds *all* logic; `llmsc`, `llmsctl`, and the GUI all
  link it → **CLI/GUI parity by construction** (the GUI is never more capable than the CLI).

## Config: declarative, on disk, shared by CLI & GUI

- Configuration is **durable on-disk files** that both CLI and GUI read/write — the single
  source of **intent**.
- Format: **TOML** (decided) — idiomatic Rust, supports comments, fewer footguns than YAML.
- Layout: global/user config under XDG (`~/.config/llmsc/`), plus an optional **per-project
  `llmsc.toml`** declaring a repo's sandboxes — checks into a repo, ideal for the
  software-factory model.

## Incus stays the substrate, directly usable

- **Incus is the runtime source of truth.** Our config is *desired state* layered on top.
- llmsc-managed resources live in a dedicated **Incus project** (namespacing) so power users can
  run `incus` directly without colliding with managed objects.
- Model: `llmsc apply` **reconciles** config → Incus; live state is read **back** from Incus so
  direct `incus` changes show up; **drift is surfaced, not silently clobbered**.
- Principle: **never trap the user above Incus** — dropping to raw `incus` is always supported.

## Architecture: library-first, daemon deferred

- Core is the **`llmsc-core` crate**; the CLIs and the Tauri GUI link it directly. **No
  mandatory daemon.** (This revises an earlier daemon-first lean — library-first fits CLI-first
  + config-on-disk + Incus-as-truth better.)
- Live features (stream logs/traces, lifecycle) come from the **Incus events API + Phoenix/Loki**
  directly.
- A small **optional daemon** may appear later *only if* live coordination needs a persistent
  owner — e.g. interrupt/steer of running agents, or maintaining host DNS/routes
  ([architecture/networking.md](architecture/networking.md)). **Deferred** until then.

## GUI: Tauri

- **Tauri** — Rust backend (the `llmsc-core` crate) + system webview + web frontend. Reuses the
  HTML mockups; lean footprint (no bundled Chromium).
- Frontend framework: **React or Svelte** *(open — preference)*.

## Distribution

- Static binaries for `llmsc` / `llmsctl`, cross-compiled (Linux/macOS, x86_64/aarch64).
- Tauri bundles for the GUI per platform.

## Open items

- **Frontend framework**: React vs Svelte.
- **Reconcile model** details: drift handling, and the balance between declarative `apply` and
  imperative commands (`llmsc launch …`).
- **Rust ↔ Incus**: pick a crate vs hand-roll a REST client over the unix socket.
- **When (if)** the optional daemon becomes warranted (first live interrupt/steer feature).
