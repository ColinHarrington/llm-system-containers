#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["rich>=13"]
# ///
"""De-risking spike runner for llm-system-containers.

Automates planning/spike-plan.md: proves the riskiest assumptions by hand on a real
Linux or macOS (Apple Silicon) host, with per-step pass/fail.

This is throwaway bootstrapping glue (not product code; product is Rust). It shells out to
limactl / incus / ssh and reports results. Some Phase 3/4 steps touch host routes + the DNS
resolver and need sudo / platform-specific tweaks — those are attempted best-effort and clearly
flagged so you can finish them by hand if needed.

Usage:
    uv run scripts/spike.py all          # run phases 0..5 in order
    uv run scripts/spike.py phase2       # run a single phase
    uv run scripts/spike.py status       # show lima/incus state
    uv run scripts/spike.py clean        # tear down the spike VM/containers

Options: --vm NAME (default: llmsc), --container NAME (default: web-agent-01),
         --agent USER (default: agent-claude), --operator USER (default: operator).
"""
from __future__ import annotations

import argparse
import json
import platform
import shlex
import subprocess
import sys
from dataclasses import dataclass, field

from rich.console import Console
from rich.table import Table

console = Console()
OS = platform.system()  # "Linux" or "Darwin"


# --------------------------------------------------------------------------- shell helpers
def sh(cmd: str, *, check: bool = False, quiet: bool = False) -> tuple[int, str]:
    """Run a host command; return (returncode, combined output)."""
    if not quiet:
        console.print(f"[dim]$ {cmd}[/dim]")
    p = subprocess.run(cmd, shell=True, text=True, capture_output=True)
    out = (p.stdout or "") + (p.stderr or "")
    if not quiet and out.strip():
        console.print(f"[dim]{out.rstrip()}[/dim]")
    if check and p.returncode != 0:
        raise RuntimeError(f"command failed ({p.returncode}): {cmd}\n{out}")
    return p.returncode, out


def vm(cfg: "Cfg", cmd: str, *, sudo: bool = False, quiet: bool = False) -> tuple[int, str]:
    """Run a command inside the Lima VM (login shell)."""
    inner = ("sudo " if sudo else "") + cmd
    return sh(f"limactl shell {cfg.vm} bash -lc {shlex.quote(inner)}", quiet=quiet)


def incus(cfg: "Cfg", args: str, *, quiet: bool = False) -> tuple[int, str]:
    """Run `incus ...` inside the VM (with sudo for simplicity during the spike)."""
    return vm(cfg, f"incus {args}", sudo=True, quiet=quiet)


def vm_exists(cfg: "Cfg") -> bool:
    _, out = sh("limactl list --format '{{.Name}}'", quiet=True)
    return any(line.strip() == cfg.vm for line in out.splitlines())


def require_vm(cfg: "Cfg", r: "Results", phase: str) -> bool:
    """Guard: phases 1+ need the VM from phase 0."""
    if not vm_exists(cfg):
        r.add(phase, False, f"VM '{cfg.vm}' not found — run `uv run scripts/spike.py phase0` first")
        return False
    return True


# --------------------------------------------------------------------------- result model
@dataclass
class Cfg:
    vm: str = "llmsc"
    container: str = "web-agent-01"
    agent: str = "agent-claude"
    operator: str = "operator"
    service: str = "forgejo"


@dataclass
class Results:
    rows: list[tuple[str, bool, str]] = field(default_factory=list)

    def add(self, phase: str, ok: bool, note: str = "") -> bool:
        self.rows.append((phase, ok, note))
        mark = "[green]✓ PASS[/green]" if ok else "[red]✗ FAIL[/red]"
        console.print(f"  {mark}  {phase}  {note and '— ' + note}")
        return ok

    def summary(self) -> int:
        t = Table(title="Spike results", title_style="bold")
        t.add_column("Phase"); t.add_column("Result"); t.add_column("Note")
        for phase, ok, note in self.rows:
            t.add_row(phase, "[green]PASS[/green]" if ok else "[red]FAIL[/red]", note)
        console.print(t)
        failed = sum(1 for _, ok, _ in self.rows if not ok)
        return 1 if failed else 0


# --------------------------------------------------------------------------- phases
def phase0(cfg: Cfg, r: Results) -> None:
    console.rule("[bold]Phase 0 — VM with Incus")
    if not vm_exists(cfg):
        sh(f"limactl start --name={cfg.vm} --cpus=4 --memory=8 --tty=false template://default")
    # install + init incus (idempotent-ish)
    vm(cfg, "command -v incus >/dev/null || (sudo apt-get update && sudo apt-get install -y incus)")
    vm(cfg, "sudo incus admin init --minimal 2>/dev/null || true")
    rc, out = incus(cfg, "version", quiet=True)
    r.add("0 VM + Incus", rc == 0, out.strip().splitlines()[-1] if out.strip() else "incus not reachable")


def phase1(cfg: Cfg, r: Results) -> None:
    console.rule("[bold]Phase 1 — L2 unprivileged container + users")
    if not require_vm(cfg, r, "1 L2 + users"):
        return
    rc, _ = incus(cfg, f"info {cfg.container}", quiet=True)
    if rc != 0:
        lrc, _ = incus(cfg, f"launch images:debian/13 {cfg.container}")
        if lrc != 0:
            r.add("1 L2 + users", False, "incus launch failed — check Phase 0 (incus init) / image server")
            return
    prc, priv = incus(cfg, f"config get {cfg.container} security.privileged", quiet=True)
    unpriv = prc == 0 and priv.strip() in ("", "false")
    for u in (cfg.operator, cfg.agent):
        # NB: "operator" collides with Ubuntu's system group of the same name, so the default
        # per-user-group useradd fails — fall back to an explicit primary group.
        incus(cfg, f"exec {cfg.container} -- bash -lc "
                   + shlex.quote(f"id {u} 2>/dev/null || useradd -m -s /bin/bash {u} || "
                                 f"useradd -m -s /bin/bash -g users {u}"), quiet=True)
    rc, _ = incus(cfg, f"exec {cfg.container} -- id {cfg.agent}", quiet=True)
    ok = unpriv and rc == 0
    note = ("unprivileged + users present" if ok
            else "config get failed (VM/container issue)" if prc != 0
            else "container is PRIVILEGED — investigate")
    r.add("1 L2 + users", ok, note)


def phase2(cfg: Cfg, r: Results) -> None:
    console.rule("[bold]Phase 2 — rootless Podman inside unprivileged L2  ⭐")
    if not require_vm(cfg, r, "2 rootless L3 ⭐"):
        return
    incus(cfg, f"config set {cfg.container} security.nesting true")
    # L1-VM-specific: Ubuntu 23.10+ defaults kernel.apparmor_restrict_unprivileged_userns=1,
    # which blocks the nested user namespace rootless podman/docker needs ("cannot clone:
    # Permission denied"). Relax it on the VM (persisted). Tolerant: a Debian L1 VM has no such
    # sysctl (and likely doesn't need the workaround at all), so this is best-effort.
    vm(cfg, "echo 'kernel.apparmor_restrict_unprivileged_userns=0' | "
            "sudo tee /etc/sysctl.d/99-llmsc-userns.conf >/dev/null 2>&1; "
            "sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0 2>/dev/null || true",
            quiet=True)
    incus(cfg, f"restart {cfg.container}")
    sh("sleep 3", quiet=True)
    incus(cfg, f"exec {cfg.container} -- bash -lc 'command -v podman >/dev/null || "
               f"(apt-get update && apt-get install -y podman uidmap slirp4netns fuse-overlayfs)'")
    # rootless, AS THE AGENT USER, in an unprivileged container, no --privileged:
    run = (f"exec {cfg.container} -- su - {cfg.agent} -c "
           + shlex.quote("cd ~ && podman run --rm hello-world >/dev/null 2>&1 && "
                         "printf 'FROM alpine\\nRUN echo hi-from-L3 > /x\\n' > Dockerfile && "
                         "podman build -t spiketest . >/dev/null 2>&1 && "
                         "podman run --rm spiketest cat /x"))
    rc, out = incus(cfg, run)
    ok = rc == 0 and "hi-from-L3" in out
    _, drv = incus(cfg, f"exec {cfg.container} -- su - {cfg.agent} -c "
                        + shlex.quote("podman info --format {{.Store.GraphDriverName}}"), quiet=True)
    r.add("2 rootless L3 ⭐", ok,
          f"rootless build+run OK (storage: {drv.strip().splitlines()[-1] if drv.strip() else '?'}) — differentiator holds"
          if ok else "rootless nesting failed — check apparmor_restrict_unprivileged_userns + "
                     "security.nesting + /etc/subuid,subgid")


def _container_ip(cfg: Cfg) -> str | None:
    rc, out = incus(cfg, f"list {cfg.container} --format json", quiet=True)
    if rc != 0:
        return None
    try:
        data = json.loads(out[out.index("["):])
        for net in data[0].get("state", {}).get("network", {}).values():
            for addr in net.get("addresses", []):
                if addr.get("family") == "inet" and addr.get("scope") == "global":
                    return addr["address"]
    except Exception:
        return None
    return None


def phase3(cfg: Cfg, r: Results) -> None:
    console.rule("[bold]Phase 3 — routable IP + .llmsc DNS + SSH  ⭐ (host steps need sudo)")
    if not require_vm(cfg, r, "3 routable + DNS + SSH ⭐"):
        return
    incus(cfg, "network set incusbr0 ipv4.nat false")
    incus(cfg, "network set incusbr0 dns.domain llmsc")
    vm(cfg, "sudo sysctl -w net.ipv4.ip_forward=1", quiet=True)
    incus(cfg, f"exec {cfg.container} -- bash -lc 'command -v sshd >/dev/null || "
               f"(apt-get update && apt-get install -y openssh-server)'")
    incus(cfg, f"exec {cfg.container} -- systemctl enable --now ssh 2>/dev/null || true", quiet=True)

    ip = _container_ip(cfg)
    if not ip:
        r.add("3 routable + DNS + SSH ⭐", False, "could not determine container IP")
        return

    console.print(f"[yellow]Container IP: {ip}[/yellow]")
    console.print("[yellow]MANUAL (sudo, platform-specific) — host route + resolver:[/yellow]")
    subnet = ip.rsplit(".", 1)[0] + ".0/24"
    if OS == "Darwin":
        console.print(f"  sudo route -n add -net {subnet} <VM_IP>")
        console.print("  echo 'nameserver <VM_BRIDGE_IP>' | sudo tee /etc/resolver/llmsc")
    else:
        console.print(f"  sudo ip route add {subnet} via <VM_IP>")
        console.print("  sudo resolvectl dns <iface> <VM_BRIDGE_IP>; "
                      "sudo resolvectl domain <iface> '~llmsc'")
    console.print("  (find VM_IP via the Lima shared network; VM_BRIDGE_IP = incusbr0 ipv4.address)")

    ping_ip, _ = sh(f"ping -c1 -W2 {ip}", quiet=True) if OS == "Darwin" else sh(f"ping -c1 -w2 {ip}", quiet=True)
    ping_dns, _ = sh(f"ping -c1 {cfg.container}.llmsc", quiet=True)
    ssh_rc, _ = sh(
        f"ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o ConnectTimeout=5 "
        f"{cfg.operator}@{cfg.container}.llmsc true", quiet=True)
    r.add("3a route (host→container IP)", ping_ip == 0, f"ping {ip}")
    r.add("3b split-horizon .llmsc DNS", ping_dns == 0, f"ping {cfg.container}.llmsc")
    r.add("3c SSH on :22 by name", ssh_rc == 0,
          "needs operator key in authorized_keys if it failed on auth")


def phase4(cfg: Cfg, r: Results) -> None:
    console.rule("[bold]Phase 4 — service in its own L2 with routable :22 (Forgejo-shaped)")
    if not require_vm(cfg, r, "4 service own :22"):
        return
    rc, _ = incus(cfg, f"info {cfg.service}", quiet=True)
    if rc != 0:
        incus(cfg, f"launch images:debian/13 {cfg.service}")
    incus(cfg, f"exec {cfg.service} -- bash -lc 'command -v sshd >/dev/null || "
               f"(apt-get update && apt-get install -y openssh-server)'")
    incus(cfg, f"exec {cfg.service} -- systemctl enable --now ssh 2>/dev/null || true", quiet=True)
    ip = None
    rc, out = incus(cfg, f"list {cfg.service} --format json", quiet=True)
    try:
        data = json.loads(out[out.index("["):])
        for net in data[0]["state"]["network"].values():
            for a in net["addresses"]:
                if a["family"] == "inet" and a["scope"] == "global":
                    ip = a["address"]
    except Exception:
        pass
    ssh_rc, _ = sh(
        f"ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o ConnectTimeout=5 -p 22 "
        f"root@{cfg.service}.llmsc true", quiet=True) if ip else (1, "")
    r.add("4 service own :22", ssh_rc == 0,
          f"{cfg.service} reachable on its own IP {ip or '?'} :22" if ssh_rc == 0
          else "verify route/DNS from Phase 3 + auth")


def phase5(cfg: Cfg, r: Results) -> None:
    console.rule("[bold]Phase 5 — cross-platform delta")
    console.print(f"Current host: [bold]{OS}[/bold]. Run this same script on the OTHER platform "
                  "(Linux ↔ macOS Apple Silicon) and compare the summary tables.")
    console.print("Record deviations (esp. Phase 3 networking) in planning/spike-plan.md findings.")
    r.add("5 cross-platform delta", True, f"ran on {OS}; compare with the other platform")


def status(cfg: Cfg, _r: Results) -> None:
    sh("limactl list")
    incus(cfg, "list")


def clean(cfg: Cfg, _r: Results) -> None:
    for c in (cfg.container, cfg.service):
        incus(cfg, f"delete -f {c} 2>/dev/null || true")
    sh(f"limactl stop {cfg.vm} 2>/dev/null || true")
    sh(f"limactl delete {cfg.vm} 2>/dev/null || true")
    console.print("[green]cleaned[/green]")


PHASES = {"phase0": phase0, "phase1": phase1, "phase2": phase2,
          "phase3": phase3, "phase4": phase4, "phase5": phase5}


def main() -> int:
    ap = argparse.ArgumentParser(description="llm-system-containers de-risking spike")
    ap.add_argument("cmd", choices=[*PHASES, "all", "status", "clean"])
    ap.add_argument("--vm", default="llmsc")
    ap.add_argument("--container", default="web-agent-01")
    ap.add_argument("--agent", default="agent-claude")
    ap.add_argument("--operator", default="operator")
    a = ap.parse_args()
    cfg = Cfg(vm=a.vm, container=a.container, agent=a.agent, operator=a.operator)

    if sh("command -v limactl", quiet=True)[0] != 0:
        console.print("[red]limactl not found — install Lima first (brew install lima).[/red]")
        return 2

    r = Results()
    if a.cmd == "status":
        status(cfg, r); return 0
    if a.cmd == "clean":
        clean(cfg, r); return 0
    if a.cmd == "all":
        for fn in PHASES.values():
            fn(cfg, r)
        return r.summary()
    PHASES[a.cmd](cfg, r)
    return r.summary()


if __name__ == "__main__":
    sys.exit(main())
