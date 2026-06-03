# Naming

Decisions from the naming session. Captures *what* we settled on and *why*, so the
rationale isn't lost.

## Decisions

| Thing | Name |
|---|---|
| Project / repo | **llm-system-containers** |
| The unit (L2) | **LLMSC** — *Little Linux Managed System Container* |
| The VM (L1) | **`llmsc-vm`** (long); **"VM"** (short UI label) |
| Container CLI (daily driver) | **`llmsc`** |
| Platform CLI (control plane) | **`llmsctl`** |
| "sandbox" | a *mode*, not a name — describes ephemeral/safer operation, used in prose only |

"Playground" is **retired**. **Host** = the user's own computer (where `llmsc`/`llmsctl` are
installed), so it is *not* used for the VM.

## Layer terminology (glossary)

"Layer" means a **level of virtualization nesting**, not a role:

| Term | Meaning |
|---|---|
| **Host** | The user's computer (macOS/Linux); `llmsc`/`llmsctl` run here |
| **L1 — VM** (`llmsc-vm`) | Host-native VM running Incus; one kernel shared by L2/L3 |
| **L2 — system container** | **Unprivileged** Incus/LXC system container = the **LLMSC** = a "sandbox" |
| **L3 — app container** | **Rootless** Docker/Podman nested *inside* an L2 container (containerization, not nested virt) |

Discipline to avoid the `machinectl` ambiguity (where containers are also "machines"):
**VM** always means L1; **container / LLMSC** always means an L2 unit. **Services** are not a
layer — each runs in L1 or its own L2 container (an isolation choice).

## Rationale

- **"System container" is the differentiator, so it anchors the name.** We are not just
  sandboxing an agent's *process* — we give the agent a whole little Linux *system*
  (Incus/unprivileged LXC). The name should say that.
- **"Sandbox" is dropped from the name** — it's overloaded (used for countless unrelated
  tools) and it describes a *property* (ephemeral, safer operation), not the thing itself.
  An LLMSC *run as a sandbox* is the common case, but "sandbox" stays in prose, not the
  brand.
- **The LLM double entendre is preserved** in **LLMSC**: *Little Linux Managed* + *Large
  Language Model*. The "system container" expansion also disambiguates from the existing
  `llm-sandbox` PyPI package.
- **The project is a platform/tool**, distinct from any single unit — it enables and manages
  LLM System Containers (plus the VM and services). Repo name is the plain descriptive slug
  **llm-system-containers**.

## CLI command split

Two commands for two audiences/cadences, mirroring the well-worn control-plane vs.
daily-driver pattern (`incus` / `incus admin`, `kubectl` / `kubeadm`):

### `llmsctl` — platform / host control plane (occasional)
```
llmsctl init                      # run the setup wizard
llmsctl up / down                 # start/stop the VM (llmsc-vm)
llmsctl status                    # is the VM running?
llmsctl services enable litellm   # manage services (L1 or own L2 container)
```
The `-ctl` suffix is the universal "control plane" signal (systemctl, kubectl), so it reads
unambiguously as the admin tool.

### `llmsc` — container plane (daily driver)
```
llmsc launch web --image dev      # create an LLMSC
llmsc ls                          # list running containers
llmsc shell user@web              # drop into one as a given user
llmsc rm web                      # tear down
```
This is the command you live in. Note the per-user form `user@container` reflects the
two-user model (agent users + human operator) from
[architecture/system-containers.md](architecture/system-containers.md).

## Still open

- Whether the platform ever gets a distinct proper-noun brand above the
  `llmsc`/`llmsctl` tools (Docker→containers pattern) — currently no; the descriptive
  `llm-system-containers` stands.
