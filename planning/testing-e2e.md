# End-to-end testing

Unit tests cover `llmsc-core` logic behind the `IncusClient` / `VmDriver` / `CommandRunner`
traits with fakes (`planning/testing.md`). They deliberately stop at the process boundary — so
the seams they *can't* see (the CLIs actually driving Incus to create a real L2 sandbox, its
Linux users, its devices) are exactly where bugs like "No root device could be found" live. This
doc is the plan for testing those seams end to end.

## The two runnable surfaces

The stack is `host CLI → L1 Lima VM → Incus → L2 sandbox`. There are two ways to run it, and the
split is what makes e2e CI-able:

| Mode | Where Incus runs | Needs a VM / nested virt? | Runs in |
|------|------------------|---------------------------|---------|
| **`local`** (`mode = "local"`) | directly on the host | **No** — LXC system containers share the host kernel | **GitHub Actions (Linux)** ✅ + Linux desktops |
| **`vm`** (default) | inside the Lima VM | Yes (Lima = QEMU/Apple Virtualization) | your Mac / Linux desktop |

Both drive the **same** `llmsc-core` Incus code (`launch_args`, reconcile, users, egress, the
default-profile root disk) — `local` just skips the Lima hop. So a Linux `local`-mode run catches
the large majority of integration bugs, including the root-disk one.

## The lifecycle the harness exercises

`scripts/e2e.py run --mode {local,vm}` runs, with per-step pass/fail:

1. **preflight** — Incus reachable (VM up for `vm` mode).
2. **clean slate** — remove any leftover test sandbox (proves teardown / re-runnability).
3. **root-disk invariant** — the `default` profile carries a root disk (the fix from the
   no-root-device bug; `vm`-mode `llmsctl up` and the CI's `admin init` both ensure it).
4. **configure** — write an `llmsc.toml` (a sandbox + an agent + an operator).
5. **`llmsc apply`** — reconcile creates the instance and runs it.
6. **users created** — both Linux users (the two-user model) exist in the sandbox.
7. **exec as agent** — `llmsc exec agent@sandbox` works.
8. **idempotency** — a second `apply` is a clean no-op.
9. **teardown** — `llmsc rm` removes it; confirm it's gone.

It writes its `llmsc.toml` to a throwaway dir so the repo's own config is never touched.

## CI: `.github/workflows/e2e.yml`

A Linux `local`-mode job on `ubuntu-24.04`: install Incus from apt, `incus admin init --minimal`,
give the runner socket access (`usermod -aG incus-admin` + `sg`), build the CLIs, run the harness.
No nested virtualization required. Runs on push to `main`, on PRs, nightly, and on demand. It is a
genuine reproduction + regression gate for the root-disk class of bug.

## What is *not* in CI (and why)

- **`vm`-mode full stack (Lima)** — hosted runners don't reliably provide the VM/nested-virt Lima
  needs (macOS runners especially). Run it manually on a real host:
  `uv run scripts/e2e.py run --mode vm` (after `llmsctl up`). Optional nightly Linux-Lima job
  could be added later but adds little over `local` mode.
- **Full uninstall/reinstall of the VM** — on CI the runner is ephemeral (a clean install every
  run), so reinstall is implicit. The VM-level teardown is `llmsctl destroy`; exercise it on a
  real host when validating the bring-up path.
- **Services (M5)** — the LiteLLM/Phoenix done-when has its own live harness
  (`scripts/m5_litellm_phoenix.py`, `planning/services/m5-done-when-testplan.md`). Folding it into
  this e2e (deploy services → keys → call → trace) is a follow-up once it's run green on a host.

## Open items

- **L3 (nested app container)** — add a step that launches a rootless container inside the sandbox
  once M3 is productized.
- **Promote `vm`-mode to a nightly Linux-Lima job** if desktop-path regressions start slipping
  through the `local` gate.
- **Egress / enforcement** — extend the harness to apply an egress policy and assert the ACL binds.
