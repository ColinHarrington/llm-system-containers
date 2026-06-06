# LLMSC GUI

Desktop GUI for **llm-system-containers** — a [Tauri](https://tauri.app/) v2 shell over the
shared `llmsc-core` Rust library, with a **Svelte 5 (runes) + TypeScript** frontend. It's a
view onto the same config and Incus state the `llmsc` / `llmsctl` CLIs drive; nothing here is
GUI-only state.

## Layout

```
gui/
├── src/
│   ├── screens/        # one component per screen (Dashboard, Sandboxes, SandboxDetail,
│   │                   #   Services, Security, Topology, Agent, Settings, Wizard, Incus*, …)
│   ├── lib/            # reusable components + the runes store
│   │   ├── store.svelte.ts   # global UI state, toasts, confirm dialogs, live polling
│   │   ├── core.ts           # bridge: calls Tauri commands in-app, mock data otherwise
│   │   ├── types.ts          # shared DTO types (mirror the Tauri command layer)
│   │   └── *.svelte          # Modal, Toast, Copy, SortHeader, Skeleton, FetchError, …
│   ├── App.svelte      # shell: sidebar + screen router
│   └── app.css         # global styles / design tokens
└── src-tauri/          # Tauri Rust shell — exposes llmsc-core as #[tauri::command]s
```

`core.ts` is the seam: when running inside Tauri it invokes the Rust commands in
`src-tauri/src/lib.rs`; otherwise it returns mock data, so `pnpm dev` and the tests run with no
VM/Incus present.

## Commands

```bash
pnpm install
pnpm dev            # vite dev server (frontend only, mock data)
pnpm check          # svelte-check — keep at 0 errors / 0 warnings
pnpm test           # vitest run (one-shot)
pnpm test Security  # a single test file by name substring
pnpm test:watch     # vitest watch mode
pnpm build          # production build
cargo tauri dev     # full desktop app against a real backend (needs the tauri CLI)
```

## Conventions

- **Svelte 5 runes** throughout (`$state`, `$derived`, `$effect`, `$props`, snippets) — no
  legacy stores, no plain JS.
- Tests are **Vitest + @testing-library/svelte**; mock `../lib/core` with `vi.hoisted()` spies.
  Every screen with data/actions ships tests. Suite is green (`pnpm test`).
- New Tauri command? Add the Rust `#[tauri::command]` + DTO (`camelCase` serde) in
  `src-tauri/src/lib.rs`, register it in `generate_handler!`, then bridge it in `core.ts` with
  a mock fallback and type it in `types.ts`.
- `svelte-check` must stay **0/0**; it (and vitest) are not git hooks — they run in CI
  (`.github/workflows/ci.yml`, the **gui** job). Run them locally before pushing.
