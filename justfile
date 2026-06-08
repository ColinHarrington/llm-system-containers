# llm-system-containers task runner — mirrors CLAUDE.md and .github/workflows/ci.yml.
# Install `just` with `cargo install just`. Run `just` (or `just --list`) to see recipes.
# Toolchains used: cargo (Rust), pnpm (GUI), prek (hooks), uvx (scripts) — install as needed.

# List available recipes.
default:
    @just --list

# ---------------------------------------------------------------------------- #
# Rust workspace (llmsc-core + the llmsc / llmsctl CLIs)
# ---------------------------------------------------------------------------- #

# Build the workspace (core + both CLIs).
build:
    cargo build

# Run the full Rust test suite (unit + black-box CLI).
test:
    cargo test --all

# Run a filtered subset of Rust tests, e.g. `just test-one enforce`.
test-one filter:
    cargo test --all {{ filter }}

# Format all Rust: the workspace + the out-of-workspace Tauri shell crate.
fmt:
    cargo fmt --all
    cargo fmt --manifest-path gui/src-tauri/Cargo.toml --all

# Check Rust formatting without writing (workspace + Tauri shell) — a CI gate.
fmt-check:
    cargo fmt --all --check
    cargo fmt --manifest-path gui/src-tauri/Cargo.toml --all --check

# Clippy lint gate (warnings are errors).
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Compile-check the Tauri shell crate (it is not a workspace member).
tauri-check:
    cargo check --manifest-path gui/src-tauri/Cargo.toml

# Run a CLI, e.g. `just llmsctl status` or `just llmsctl services list`.
llmsctl *args:
    cargo run -p llmsctl -- {{ args }}

# Run the container-plane CLI, e.g. `just llmsc ls` or `just llmsc display web-agent-01`.
llmsc *args:
    cargo run -p llmsc -- {{ args }}

# ---------------------------------------------------------------------------- #
# GUI (Svelte 5 frontend + Tauri shell; package manager: pnpm)
# ---------------------------------------------------------------------------- #

# Install GUI dependencies (run once / after lockfile changes).
gui-install:
    cd gui && pnpm install

# svelte-check — must stay 0 errors / 0 warnings.
gui-check:
    cd gui && pnpm check

# Run the GUI unit tests (vitest, one-shot). Optional name filter: `just gui-test Security`.
gui-test filter='':
    cd gui && pnpm test {{ filter }}

# Production build of the frontend (also typechecks the Tauri-less bundle).
gui-build:
    cd gui && pnpm build

# Vite dev server (frontend only, mock data — no VM/Incus needed).
gui-dev:
    cd gui && pnpm dev

# Full desktop app against a real backend (needs the tauri CLI installed).
gui-app:
    cd gui && cargo tauri dev

# ---------------------------------------------------------------------------- #
# Aggregate gates + hooks
# ---------------------------------------------------------------------------- #

# CI gate: Rust (fmt, clippy, test) + GUI (check, test, build). Needs `just gui-install` first.
ci: fmt-check clippy test gui-check gui-test gui-build

# Run the pre-commit hooks across all files (requires prek).
hooks:
    prek run --all-files

# Install the git hooks once (commit-time hygiene + fmt).
hooks-install:
    prek install --install-hooks
