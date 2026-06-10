#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["rich>=13"]
# ///
"""End-to-end lifecycle test: clean slate → configure → launch a sandbox → verify → teardown.

Exercises the real integration seams that unit tests (FakeRunner) can't reach: the host CLIs
driving Incus to create an L2 sandbox, its Linux users, exec, and the default-profile root disk.
It's `uv` operational glue (product code is Rust), not a `cargo test`.

Two modes — the same `llmsc-core` Incus code, different transport:

    --mode local   Incus runs directly on this host (no VM). The CI path: a Linux runner with
                   `incus admin init --minimal`. No nested virtualization needed.
    --mode vm      Incus runs inside the Lima VM (the default macOS/Linux-desktop path). Requires
                   the VM already up (`llmsctl up`).

Usage:
    uv run scripts/e2e.py run   --mode local
    uv run scripts/e2e.py clean --mode local      # just remove the test sandbox
Options: --vm NAME (default llmsc), --sandbox NAME (default e2e-sandbox),
         --agent USER (default agent-smith), --operator USER (default operator).
"""

from __future__ import annotations

import argparse
import shlex
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

from rich.console import Console
from rich.table import Table

console = Console()
REPO = Path(__file__).resolve().parent.parent


# --------------------------------------------------------------------------- shell helpers
def sh(cmd: str, *, cwd: Path | None = None, quiet: bool = False) -> tuple[int, str]:
    if not quiet:
        console.print(f"[dim]$ {cmd}[/dim]")
    p = subprocess.run(cmd, shell=True, text=True, capture_output=True, cwd=cwd)
    out = (p.stdout or "") + (p.stderr or "")
    if not quiet and out.strip():
        console.print(f"[dim]{out.rstrip()}[/dim]")
    return p.returncode, out


# --------------------------------------------------------------------------- config / results
@dataclass
class Cfg:
    mode: str = "local"  # "local" | "vm"
    vm: str = "llmsc"
    sandbox: str = "e2e-sandbox"
    agent: str = "agent-smith"
    operator: str = "operator"
    project: Path = REPO

    def is_vm(self) -> bool:
        return self.mode == "vm"


@dataclass
class Results:
    rows: list[tuple[str, bool, str]] = field(default_factory=list)

    def add(self, step: str, ok: bool, note: str = "") -> bool:
        self.rows.append((step, ok, note))
        mark = "[green]✓ PASS[/green]" if ok else "[red]✗ FAIL[/red]"
        console.print(f"  {mark}  {step}  {note and '— ' + note}")
        return ok

    def summary(self) -> int:
        t = Table(title="e2e lifecycle", title_style="bold")
        t.add_column("Step")
        t.add_column("Result")
        t.add_column("Note")
        for step, ok, note in self.rows:
            t.add_row(step, "[green]PASS[/green]" if ok else "[red]FAIL[/red]", note)
        console.print(t)
        return 1 if any(not ok for _, ok, _ in self.rows) else 0


# --------------------------------------------------------------------------- incus / cli access
def incus_q(cfg: Cfg, args: str, *, quiet: bool = False) -> tuple[int, str]:
    """Run an `incus` command directly (for verification), routed to where Incus actually runs."""
    if cfg.is_vm():
        return sh(f"limactl shell {cfg.vm} sudo incus {args}", quiet=quiet)
    return sh(f"incus {args}", quiet=quiet)


def cli(cfg: Cfg, binary: str, args: str, *, quiet: bool = False) -> tuple[int, str]:
    """Run a built product binary with the generated project config as its cwd (mode-agnostic —
    the CLI resolves the transport from llmsc.toml's `mode`)."""
    exe = REPO / "target" / "debug" / binary
    return sh(f"{shlex.quote(str(exe))} {args}", cwd=cfg.project, quiet=quiet)


def write_project(cfg: Cfg) -> None:
    """A minimal llmsc.toml: the deployment target + one sandbox with an agent and an operator."""
    mode_line = "" if cfg.is_vm() else 'mode = "local"\n'
    toml = f"""\
{mode_line}[vm]
name = "{cfg.vm}"
cpus = 4
memory_gib = 8
disk_gib = 100

[[sandbox]]
name = "{cfg.sandbox}"
image = "images:debian/12"

[[sandbox.user]]
name = "{cfg.operator}"
role = "human"

[[sandbox.user]]
name = "{cfg.agent}"
role = "agent"
"""
    (cfg.project / "llmsc.toml").write_text(toml)


# --------------------------------------------------------------------------- lifecycle
def preflight(cfg: Cfg, r: Results) -> bool:
    rc, out = incus_q(cfg, "version", quiet=True)
    if rc != 0:
        hint = (
            f"VM '{cfg.vm}' not reachable — run `llmsctl up` first"
            if cfg.is_vm()
            else "incus not reachable — install it + `incus admin init --minimal`"
        )
        return r.add("preflight", False, hint)
    return r.add(
        "preflight: incus reachable",
        True,
        out.strip().splitlines()[-1] if out.strip() else "",
    )


def remove_sandbox(cfg: Cfg) -> None:
    incus_q(cfg, f"delete -f {cfg.sandbox}", quiet=True)


def run(cfg: Cfg, r: Results) -> int:
    console.rule(f"[bold]e2e — sandbox lifecycle ({cfg.mode})")
    if not preflight(cfg, r):
        return r.summary()

    rc, _ = sh("cargo build -q -p llmsc -p llmsctl", cwd=REPO)
    if not r.add("build CLIs", rc == 0, "cargo build llmsc + llmsctl"):
        return r.summary()

    # Clean slate (proves uninstall/idempotent re-runs): no leftover from a prior run.
    remove_sandbox(cfg)
    gone = incus_q(cfg, f"list {cfg.sandbox} -f csv -c n", quiet=True)[1].strip()
    r.add("clean slate", cfg.sandbox not in gone, "no leftover sandbox")

    # The fix invariant: the default profile must carry a root disk (else launch → "No root
    # device could be found"). vm-mode `llmsctl up` and the workflow's admin init both ensure it.
    prof = incus_q(cfg, "profile show default", quiet=True)[1]
    r.add(
        "default profile has root disk",
        "path: /" in prof or 'path: "/"' in prof,
        "root disk present",
    )

    write_project(cfg)
    r.add(
        "configure",
        True,
        f"llmsc.toml: {cfg.sandbox} + {cfg.agent}/{cfg.operator} (mode={cfg.mode})",
    )

    # Reconcile the declared sandbox into Incus (creates the instance + its Linux users).
    rc, out = cli(cfg, "llmsc", "apply")
    running = incus_q(cfg, f"list {cfg.sandbox} -c ns -f csv", quiet=True)[1]
    r.add(
        "llmsc apply",
        rc == 0 and "RUNNING" in running.upper(),
        f"{cfg.sandbox} created + running",
    )

    # The two-user model: both Linux users exist inside the sandbox. Read /etc/passwd directly so
    # the check is portable across glibc/musl images (busybox lacks `getent`).
    passwd = incus_q(cfg, f"exec {cfg.sandbox} -- cat /etc/passwd", quiet=True)[1]
    r.add(
        "users created",
        f"{cfg.agent}:" in passwd and f"{cfg.operator}:" in passwd,
        f"{cfg.agent} + {cfg.operator}",
    )

    # exec works as the agent (login shell).
    rc, out = cli(cfg, "llmsc", f"exec {cfg.agent}@{cfg.sandbox} -- whoami")
    r.add("exec as agent", rc == 0 and cfg.agent in out, "llmsc exec")

    # Idempotency: a second apply is a clean no-op (no error, instance still there).
    rc, _ = cli(cfg, "llmsc", "apply")
    still = incus_q(cfg, f"list {cfg.sandbox} -c s -f csv", quiet=True)[1]
    r.add(
        "apply is idempotent", rc == 0 and "RUNNING" in still.upper(), "re-apply clean"
    )

    # Teardown: remove the sandbox and confirm it's gone.
    rc, _ = cli(cfg, "llmsc", f"rm {cfg.sandbox}")
    after = incus_q(cfg, f"list {cfg.sandbox} -f csv -c n", quiet=True)[1].strip()
    r.add("teardown", cfg.sandbox not in after, "sandbox removed")

    return r.summary()


def clean(cfg: Cfg, r: Results) -> int:
    remove_sandbox(cfg)
    r.add("clean", True, f"removed {cfg.sandbox} (if present)")
    return r.summary()


# --------------------------------------------------------------------------- main
def main() -> int:
    ap = argparse.ArgumentParser(description="e2e sandbox lifecycle test")
    ap.add_argument("cmd", choices=["run", "clean"])
    ap.add_argument("--mode", choices=["local", "vm"], default="local")
    ap.add_argument("--vm", default="llmsc")
    ap.add_argument("--sandbox", default="e2e-sandbox")
    ap.add_argument("--agent", default="agent-smith")
    ap.add_argument("--operator", default="operator")
    a = ap.parse_args()

    # The CLIs read ./llmsc.toml; run them in a throwaway dir so the repo config is untouched.
    with tempfile.TemporaryDirectory(prefix="llmsc-e2e-") as td:
        cfg = Cfg(
            mode=a.mode,
            vm=a.vm,
            sandbox=a.sandbox,
            agent=a.agent,
            operator=a.operator,
            project=Path(td),
        )
        r = Results()
        return {"run": run, "clean": clean}[a.cmd](cfg, r)


if __name__ == "__main__":
    sys.exit(main())
