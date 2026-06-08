#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["rich>=13"]
# ///
"""M5 done-when: an agent in a sandbox reaches LiteLLM via a virtual key, traced in Phoenix.

This is the live integration test promoted from the spikes (planning/spike-plan.md): it drives
the real product CLIs against a running Lima+Incus VM and reports per-step pass/fail. It is *not*
a `cargo test` — it needs the VM and stands up service containers — so it lives here as `uv`
operational glue (product code is Rust).

The path it proves, end to end:
    services up        deploy svc-litellm + svc-phoenix, wire LiteLLM traces → Phoenix
    keys sync          mint the agent's virtual key (sk-llmsc-<sb>-<agent>-<random>)
    llmsc apply        create the sandbox + its agent user
    llmsc agent env    inject OPENAI_BASE_URL + the virtual key into the agent's shell
    (in sandbox)       the agent curls the proxy with its key and gets a completion
    Phoenix            the call shows up as a trace

By default the agent calls the built-in **mock** model (`mock_response`) — no provider key, no
spend, repeatable. Set `LLMSC_LIVE_PROVIDER=1` (after `keys set-provider`) to call the real
`default` model instead.

Usage:
    uv run scripts/m5_litellm_phoenix.py run      # the full done-when
    uv run scripts/m5_litellm_phoenix.py clean     # remove the sandbox (services are left up)
Options: --vm NAME (default llmsc), --sandbox NAME (default m5-agent), --agent USER
         (default agent-claude), --operator USER (default operator).
"""

from __future__ import annotations

import argparse
import os
import shlex
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

from rich.console import Console
from rich.table import Table

console = Console()
REPO = Path(__file__).resolve().parent.parent
LIVE_PROVIDER = os.environ.get("LLMSC_LIVE_PROVIDER") == "1"
MODEL = "default" if LIVE_PROVIDER else "mock"


# --------------------------------------------------------------------------- shell helpers
def sh(cmd: str, *, cwd: Path | None = None, quiet: bool = False) -> tuple[int, str]:
    if not quiet:
        console.print(f"[dim]$ {cmd}[/dim]")
    p = subprocess.run(cmd, shell=True, text=True, capture_output=True, cwd=cwd)
    out = (p.stdout or "") + (p.stderr or "")
    if not quiet and out.strip():
        console.print(f"[dim]{out.rstrip()}[/dim]")
    return p.returncode, out


def vm(
    cfg: "Cfg", cmd: str, *, sudo: bool = False, quiet: bool = False
) -> tuple[int, str]:
    inner = ("sudo " if sudo else "") + cmd
    return sh(f"limactl shell {cfg.vm} bash -lc {shlex.quote(inner)}", quiet=quiet)


def incus(cfg: "Cfg", args: str, *, quiet: bool = False) -> tuple[int, str]:
    return vm(cfg, f"incus {args}", sudo=True, quiet=quiet)


def vm_exists(cfg: "Cfg") -> bool:
    _, out = sh("limactl list --format '{{.Name}}'", quiet=True)
    return any(line.strip() == cfg.vm for line in out.splitlines())


# --------------------------------------------------------------------------- config / results
@dataclass
class Cfg:
    vm: str = "llmsc"
    sandbox: str = "m5-agent"
    agent: str = "agent-claude"
    operator: str = "operator"
    project: Path = REPO  # cwd the product CLIs run in (holds the generated llmsc.toml)


@dataclass
class Results:
    rows: list[tuple[str, bool, str]] = field(default_factory=list)

    def add(self, step: str, ok: bool, note: str = "") -> bool:
        self.rows.append((step, ok, note))
        mark = "[green]✓ PASS[/green]" if ok else "[red]✗ FAIL[/red]"
        console.print(f"  {mark}  {step}  {note and '— ' + note}")
        return ok

    def summary(self) -> int:
        t = Table(title="M5 done-when", title_style="bold")
        t.add_column("Step")
        t.add_column("Result")
        t.add_column("Note")
        for step, ok, note in self.rows:
            t.add_row(step, "[green]PASS[/green]" if ok else "[red]FAIL[/red]", note)
        console.print(t)
        return 1 if any(not ok for _, ok, _ in self.rows) else 0


# --------------------------------------------------------------------------- product CLI
def build(r: Results) -> bool:
    rc, _ = sh("cargo build -q -p llmsc -p llmsctl", cwd=REPO)
    return r.add("build CLIs", rc == 0, "cargo build llmsc + llmsctl")


def cli(cfg: "Cfg", binary: str, args: str, *, quiet: bool = False) -> tuple[int, str]:
    """Run a built product binary with the generated project config as its cwd."""
    exe = REPO / "target" / "debug" / binary
    return sh(f"{shlex.quote(str(exe))} {args}", cwd=cfg.project, quiet=quiet)


def write_project(cfg: "Cfg") -> None:
    """A minimal llmsc.toml: one sandbox with one agent + the two services we trace through."""
    toml = f"""\
[vm]
name = "{cfg.vm}"
cpus = 4
memory_gib = 8
disk_gib = 100

[[service]]
name = "litellm"

[[service]]
name = "phoenix"

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


# --------------------------------------------------------------------------- the done-when
def run(cfg: Cfg, r: Results) -> int:
    console.rule("[bold]M5 — agent → LiteLLM (virtual key) → Phoenix trace")
    console.print(
        f"[bold]model:[/bold] {MODEL}  ({'real provider' if LIVE_PROVIDER else 'hermetic mock'})"
    )

    if not vm_exists(cfg):
        r.add(
            "preflight",
            False,
            f"VM '{cfg.vm}' not found — run `llmsctl up` / spike phase0 first",
        )
        return r.summary()
    if not build(r):
        return r.summary()

    write_project(cfg)
    r.add(
        "project config",
        True,
        f"llmsc.toml with {cfg.sandbox}/{cfg.agent} + litellm,phoenix",
    )

    # 1. Stand up the services (deploys svc-litellm + svc-phoenix and wires traces). Slow.
    cli(cfg, "llmsctl", "services enable litellm")
    cli(cfg, "llmsctl", "services enable phoenix")
    rc, _ = cli(cfg, "llmsctl", "services up")
    r.add("services up", rc == 0, "deploy svc-litellm + svc-phoenix, wire traces")

    if LIVE_PROVIDER:
        prov = os.environ.get("LLMSC_PROVIDER", "openai")
        key = os.environ.get("LLMSC_PROVIDER_KEY", "")
        if not key:
            r.add(
                "set-provider",
                False,
                "LLMSC_LIVE_PROVIDER=1 but LLMSC_PROVIDER_KEY is empty",
            )
            return r.summary()
        rc, _ = cli(cfg, "llmsctl", f"keys set-provider {prov} {shlex.quote(key)}")
        r.add("set-provider", rc == 0, f"{prov} key stored in svc-litellm only")

    # 2. Mint the agent's virtual key against the running proxy.
    rc, out = cli(cfg, "llmsctl", "keys sync")
    r.add("keys sync", rc == 0 and "synced" in out, "mint sk-llmsc-… virtual key")

    # 3. Create the sandbox + its agent user.
    rc, _ = cli(cfg, "llmsc", "apply")
    online = incus(cfg, f"list {cfg.sandbox} -c ns -f csv", quiet=True)[1]
    r.add(
        "llmsc apply",
        rc == 0 and "RUNNING" in online.upper(),
        f"{cfg.sandbox} running with {cfg.agent}",
    )

    # 4. Inject the proxy URL + virtual key into the agent's shell env.
    rc, out = cli(cfg, "llmsc", f"agent env {cfg.agent}@{cfg.sandbox}")
    r.add(
        "agent env",
        rc == 0 and "injected" in out,
        "OPENAI_BASE_URL + virtual key → agent",
    )

    # 5. The agent calls the proxy with its key (login shell sources the injected env).
    body = (
        '{"model":"%s","messages":[{"role":"user","content":"ping from m5 done-when"}]}'
        % MODEL
    )
    call = (
        'curl -sS -o /tmp/resp.json -w "%{http_code}" -X POST '
        '"$OPENAI_BASE_URL/chat/completions" '
        '-H "Authorization: Bearer $OPENAI_API_KEY" -H "Content-Type: application/json" '
        f"-d {shlex.quote(body)}; echo; cat /tmp/resp.json"
    )
    rc, out = incus(
        cfg, f"exec {cfg.sandbox} -- su - {cfg.agent} -c {shlex.quote(call)}"
    )
    got_200 = "200" in out.splitlines()[0] if out.strip() else False
    completed = '"choices"' in out or "llmsc mock" in out
    r.add(
        "agent → proxy call",
        got_200 and completed,
        f"HTTP 200, completion via model={MODEL}",
    )

    # 6. The call should appear as a trace in Phoenix (best-effort: API shape varies by version).
    r.add(
        "Phoenix trace", phoenix_has_trace(cfg), "span visible in the Phoenix collector"
    )

    return r.summary()


def phoenix_has_trace(cfg: Cfg, *, attempts: int = 10, delay: float = 3.0) -> bool:
    """Poll Phoenix for at least one span. Best-effort across API versions: tries the REST span
    count, then a GraphQL fallback. Returns False (not an exception) if neither responds."""
    queries = [
        # Phoenix REST: total spans across projects.
        "curl -sS http://localhost:6006/v1/spans?limit=1",
        # GraphQL fallback: any traces recorded.
        "curl -sS -X POST http://localhost:6006/graphql "
        "-H 'Content-Type: application/json' "
        """-d '{"query":"{ projects { edges { node { traceCount } } } }"}' """,
    ]
    for _ in range(attempts):
        for q in queries:
            rc, out = incus(
                cfg, f"exec {service('phoenix')} -- sh -lc {shlex.quote(q)}", quiet=True
            )
            if rc == 0 and out.strip() and out.strip() not in ("[]", "{}"):
                # A non-empty span list or a positive traceCount both indicate a recorded call.
                if (
                    '"traceCount":0' not in out.replace(" ", "")
                    or '"id"' in out
                    or "spanId" in out.lower()
                ):
                    return True
        time.sleep(delay)
    console.print(
        "[yellow]  (Phoenix API returned no spans — verify the collector/endpoint by hand; "
        "open http://localhost:6006 in the VM)[/yellow]"
    )
    return False


def service(name: str) -> str:
    return f"svc-{name}"


def clean(cfg: Cfg, r: Results) -> int:
    incus(cfg, f"delete -f {cfg.sandbox}")
    r.add("clean", True, "removed sandbox (services left running)")
    return r.summary()


# --------------------------------------------------------------------------- main
def main() -> int:
    ap = argparse.ArgumentParser(description="M5 done-when live integration test")
    ap.add_argument("cmd", choices=["run", "clean"])
    ap.add_argument("--vm", default="llmsc")
    ap.add_argument("--sandbox", default="m5-agent")
    ap.add_argument("--agent", default="agent-claude")
    ap.add_argument("--operator", default="operator")
    a = ap.parse_args()

    # The CLIs read ./llmsc.toml; run them in a throwaway dir so we never touch the repo's config.
    with tempfile.TemporaryDirectory(prefix="llmsc-m5-") as td:
        cfg = Cfg(
            vm=a.vm,
            sandbox=a.sandbox,
            agent=a.agent,
            operator=a.operator,
            project=Path(td),
        )
        r = Results()
        rc = {"run": run, "clean": clean}[a.cmd](cfg, r)
    return rc


if __name__ == "__main__":
    sys.exit(main())
