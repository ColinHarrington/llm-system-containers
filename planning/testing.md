# Testing Strategy

## Principles

- **Red-green-refactor (TDD).** For all product code: write a **failing test first (red)**,
  implement the minimum to pass **(green)**, then **refactor** under green. No production code
  without a test that drove it.
- **A test plan accompanies everything we build.** Every feature/component ships a short test
  plan: what is covered at each level (unit/integration/e2e), what is deliberately *not*, and
  how to run it. (For spikes, see the note at the bottom.)
- **Test behavior, not implementation.** Favor tests that survive refactors.
- **Fakes at the boundaries.** External systems (Incus, Lima, the filesystem, the network) sit
  behind traits so the core logic is unit-testable without them; real integrations are covered
  separately.

## What gets tested, and how

### Rust core (`llmsc-core`) and CLIs
- **Unit** — `cargo test`; pure logic (config model, reconcile/diff, profile compilation,
  policy decisions). The Incus/Lima boundaries are **traits** mocked via `mockall` or
  hand-written fakes.
- **Config** — TOML **round-trip** + schema-validation tests; snapshot tests with `insta`;
  property tests with `proptest` for round-trip invariants.
- **CLI** — black-box tests invoking the built binary (`assert_cmd` + `predicates`, or
  `trycmd`/`snapbox` for golden output). Because the CLI is the full surface
  ([interfaces.md](interfaces.md)), this is a primary tier.
- **Integration** — against an **ephemeral real Incus** (CI Linux runner): create/list/delete a
  container, apply a profile, reconcile drift. Gated/tagged so unit runs stay fast.
- **Coverage** — `cargo llvm-cov`; meaningful coverage, not a vanity number.

### Frontend (Svelte + TypeScript)
- **Type/lint** — `tsc` / `svelte-check`, ESLint, Prettier in CI.
- **Component/unit** — **Vitest** + `@testing-library/svelte`.
- **E2E** — **Playwright** against the app (mocked backend for fast runs; real backend in a
  nightly lane).

### Tauri (GUI shell)
- Tauri command handlers are thin wrappers over `llmsc-core` and are tested as core lib
  functions.
- Full-app e2e via **`tauri-driver`** (WebDriver) where it adds value over component + core
  tests.

### Infrastructure (VM / Incus / nesting / networking)
- The risky infra assumptions are first proven by **spikes** (see [spike-plan.md](spike-plan.md)),
  then locked in as **integration smoke tests** in CI where feasible:
  - Linux CI can run Incus, unprivileged containers, and **rootless L3 nesting**.
  - **macOS** nested/networking checks are limited on hosted CI → validated via spikes + manual
    until a self-hosted runner exists; tracked explicitly so coverage gaps aren't silent.

## CI

- Runs on Linux (full: unit + CLI + Incus integration + L3 smoke) and macOS (build + unit +
  frontend). Nightly lane runs the slower real-backend e2e and networking checks.
- CI host TBD (GitHub Actions now; Forgejo Actions once self-hosted) — see
  [open-questions.md](open-questions.md).

## Note on spikes

Spikes ([spike-plan.md](spike-plan.md)) are **timeboxed, throwaway exploration**, not TDD'd
product code. They still have explicit **pass/fail criteria** per step (which read like tests),
and their findings **feed the real test plans** — a proven spike step becomes a CI integration
test.

## Open items

- CI provider/runners (incl. a self-hosted macOS/Linux runner for nested + networking e2e).
- Coverage thresholds per crate/package.
- Test-data/fixtures strategy for Incus integration (image caching to keep CI fast).
