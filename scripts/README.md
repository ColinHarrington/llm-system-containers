# scripts/

Operational & bootstrapping scripts. These are **`uv` single-file Python scripts**
([PEP 723](https://peps.python.org/pep-0723/) inline metadata) — self-contained, cross-platform
glue that shells out to `limactl` / `incus` / `ssh`.

> Convention: **product code is Rust** ([../planning/tech-stack.md](../planning/tech-stack.md));
> **bootstrapping/automation is `uv` Python**. Each script is self-contained (deps declared
> inline, no shared local imports) and run with `uv run`.

## Prerequisites

- [`uv`](https://docs.astral.sh/uv/) installed (`curl -LsSf https://astral.sh/uv/install.sh | sh`).
- [Lima](https://lima-vm.io/) (`brew install lima`, or the Linux release).
- No manual venv / `pip install` — `uv run` resolves the inline dependencies on first run.

## Scripts

### `spike.py` — de-risking spike runner
Automates [../planning/spike-plan.md](../planning/spike-plan.md) with per-step pass/fail.

```
uv run scripts/spike.py all          # phases 0..5 in order, prints a results table
uv run scripts/spike.py phase2       # rootless L3 nesting only (the top risk)
uv run scripts/spike.py status       # show lima/incus state
uv run scripts/spike.py clean        # tear down the spike VM + containers
```
Options: `--vm`, `--container`, `--agent`, `--operator`.

Phase 3/4 touch host routes + the DNS resolver (sudo, platform-specific); the script attempts
what it safely can and **prints the manual commands** for the rest. Run it on **both** a Linux
host and a macOS (Apple Silicon) host and compare the summary tables.

## Writing a new bootstrapping script

Start every script with the uv shebang + inline metadata:

```python
#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["rich>=13"]
# ///
```
Keep it self-contained (no importing sibling files); declare deps inline.
