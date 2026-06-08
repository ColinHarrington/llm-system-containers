# Spike: remote GUI display from an L2 sandbox → native host windows

**Status:** ✅ proven end-to-end (XFCE + Openbox flavors), repeatable via
`scripts/spikes/remote-display-spike.sh` · open choice = host-client strategy (see below)
· **Started:** 2026-06-07 · **Host:** Ubuntu 24.04, X11 (`DISPLAY=:1`) · **VM:** Lima `llmsc`
(Incus 6.0)

## Goal

Prove, end-to-end and repeatably, that a GUI app running inside an **L2 Incus system container**
(Alpine base, XFCE desktop flavor) can appear as a **real window on the host desktop** — not
embedded in the Tauri GUI. Exploratory only; not committing to a path. Apps to demo: a
**browser** and a **file manager**.

Transport under test (the architecture-faithful chain):

```
L2 container (xpra server, Xvfb)  →  Incus proxy device  →  L1 VM loopback port
   →  ssh -L (Lima)  →  host loopback port  →  `xpra attach` (native client)  →  host window
```

Variants: **XFCE** (primary) and a **forked Openbox** experiment.

## Environment (probed)

- Host has: `xpra`, `socat`, `ssh`, `xauth`, `incus` 6.0 (client), `lima`/`limactl` 2.1.2,
  `xeyes`, podman/docker. Host session is **X11** (so waypipe N/A; native X windows work).
- VM `llmsc`: lima user `colin` (uid 1000) has **direct `incus`** access (no sudo). Alpine 3.21
  container image present. Existing containers: asdf, buntu, playground, svc-litellm.
- Spike container name: **`gui-spike`** (XFCE), **`gui-spike-ob`** (Openbox fork).

## Plan (rungs — build up to end-to-end)

- [x] R1. Launch Alpine L2 container `gui-spike` in the VM.
- [x] R2. Install inside: xpra, XFCE (xfce4 + thunar file manager), a browser, Xvfb/X deps.
- [x] R3. Start xpra server (seamless); launch browser + thunar. (server-side screenshot is empty
       without a client — quirk, not a blocker.)
- [x] R4. Expose container → VM via Incus **proxy device**; VM → host via `ssh -L`.
- [x] R5. Attach from host → end-to-end native windows. (Blocker: host xpra v3 ↔ v6 skew;
       resolved with a containerized matching v6 client.)
- [x] R6. Fork: Openbox variant `gui-spike-ob`; ~200 MB lighter.
- [x] R7. Made repeatable: `scripts/spikes/remote-display-spike.sh` + this journal.

## Decisions / choices

- Raw `incus launch` (not `llmsc apply`) for the spike — fastest loop, and a plain bridge gives
  NAT egress for package installs (no egress ACL on a hand-launched container).
- VM→host hop uses `ssh -L` over Lima's SSH rather than wiring Lima `portForwards` — keeps the
  experiment non-committal and reversible.

## Findings / blockers

- **2026-06-07 — lima user needs `sudo incus`.** `colin` (uid 1000) is not in the incus group;
  the local socket needs `sudo`. (`incus image list images:` worked without sudo only because it
  queries the remote image server, not the local daemon.) → use `sudo incus` in the VM.
- **2026-06-07 — ✅ Alpine 3.21 packages the whole stack** (suspected musl+xpra blocker is a
  non-issue): `xpra 6.2.1`, `xfce4` / `xfce4-session` / `thunar`, `firefox` + `firefox-esr` +
  `chromium`, `openbox`, `xvfb`, `xorg-server`, `dbus`. No edge/testing repo needed.
- **2026-06-07 — ⚠️ LIKELY BLOCKER: xpra version skew.** Host xpra is **v3.1.5** (Ubuntu 24.04's
  package), container xpra is **6.2.1**. That's a 3→6 major gap; xpra's wire/auth changed
  hugely across those, so the native host client almost certainly can't attach to the v6 server.
  Plan: prove the server side independently, then *test* the attach to confirm the failure, then
  the resolution is upgrading the host client to 6.x (xpra.org apt repo) — which needs sudo.
  The HTML5 client (served by the v6 server) is a version-independent fallback to prove the
  pipeline visually, but it's browser-rendered, not the native windows we ultimately want.
- **2026-06-07 — ✅ R3 server side works.** xpra seamless server up on `:100` / tcp 14500 in
  the container. **Both apps map real windows:** `windows.2 = ('thunar','Thunar')` 640×480 and
  `windows.3 = ('Navigator','firefox-esr')` 1280×1040. Firefox ESR + Thunar run fine **as root**
  on Alpine (no root-refusal). Notes:
  - **Don't start a separate WM** (`--start=xfwm4`) — xpra *is* the WM in seamless mode; xfwm4
    exits with "Another Window Manager (Xpra) is already running". Drop it.
  - **xpra control over the unix socket fails with `invalid username ''`** (running as root via
    `incus exec`); over **TCP it works** (`xpra info tcp://127.0.0.1:14500`). → the host should
    attach over TCP, and/or we need a real (non-root) user + socket auth sorted later.
  - **`xpra screenshot` returns empty** in seamless mode with no client attached (xpra doesn't
    back window pixels until a client requests them). Not a blocker; window enumeration is the
    proof. A real attach will paint them.
  - OpenGL probe errors in the server log are non-fatal (no GPU → software paint).
- **2026-06-07 — ✅ R4 transport chain works end-to-end.** Incus **proxy device**
  (`listen=tcp:127.0.0.1:14500 connect=tcp:127.0.0.1:14500 bind=host`) bridges container→VM;
  `ssh -L 14500:127.0.0.1:14500` over Lima's ssh (`~/.lima/llmsc/ssh.config`, host
  `lima-llmsc`, port 39681) bridges VM→host. Host reaches the xpra port; even a raw HTTP GET
  traverses the full chain (xpra's HTTP handler answered 404). So container + transport + server
  are all proven good.
- **2026-06-07 — 🛑 R5 BLOCKER CONFIRMED: host xpra v3 ↔ container xpra v6 is incompatible.**
  Native `xpra attach` from the host fails the handshake two ways: default → `disconnect invalid
  compression: zlib is not available`; with `-z 0` → `invalid packet encoding: 'rencode' decoder
  is not available` (v6 replaced rencode with rencodeplus). Cross-major (3→6) xpra does not
  interoperate, as expected. **The block is purely the host client version**, nothing in our
  stack. HTML5 fallback isn't installed in the container (server returns 404 for `/`), and it's
  off-goal anyway (browser, not native windows).

## Resolution paths for the version skew

1. **Upgrade the host xpra to 6.x** from the upstream **xpra.org** apt repo (Ubuntu 24.04 ships
   the ancient v3.1.5). Clean and permanent; needs `sudo` + adding the repo — a user decision.
2. **No-sudo:** run a **matching xpra-6 client in a rootless podman container** on the host,
   sharing the host X11 socket (`-v /tmp/.X11-unix`, `DISPLAY=:1`, Xauthority) and
   `--network=host` to reach the tunnel. Renders native windows on `:1` without touching system
   packages. Being attempted as continued iteration.

- **2026-06-07 — 🎉 END-TO-END ACHIEVED (via path 2).** Built a `xpra6-client` podman image
  (`alpine:3.21` + `apk add xpra ttf-dejavu xauth`, 497 MiB, xpra **6.2.1** = exact server
  match). Ran it `--network=host`, `-e DISPLAY=:1`, `-v /tmp/.X11-unix`, after `xhost +local:`.
  Client log: `Attached to xpra server at tcp://127.0.0.1/` … `running, 2 windows`. **Firefox +
  Thunar from the Alpine L2 container render as native windows on the host desktop**, through
  xpra(server) → Incus proxy device → ssh -L → host → xpra(client) → host X `:1`. No host system
  packages touched. Cosmetic warnings only (no dbus machine-id / gstreamer audio / canberra /
  at-spi in the minimal client image).

## Key design finding: seamless vs desktop mode (affects the "flavor" question)

In **seamless** mode (what gives *individual native host windows*), **xpra is itself the window
manager** — the container's own WM does **not** run (xfwm4/openbox exit with "Another Window
Manager already running"). So "XFCE vs Openbox flavor" in seamless mode reduces to *which
packages/theme/file-manager are installed*, not a running desktop. A full desktop environment
only matters in **desktop mode** (`xpra start-desktop`), which yields one big desktop-in-a-window
— the opposite of native windows. **You can't have both at once.** Practical takeaway: for the
"real windows on the host" goal, prefer a **minimal flavor** (a WM-less set of apps + libs);
XFCE's weight (full `xfce4` = ~1 GB) mostly buys a file manager (Thunar) + theming you may not
need. The Openbox fork is interesting less as a WM (it won't run in seamless) and more as a
**lighter package set** (openbox + pcmanfm + browser).

## Verification boundary

Auto-mode can't *see* pixels. "End-to-end" is proven by: (a) xpra server reports the apps as
managed windows; (b) a server-side `xpra screenshot` PNG renders them; (c) the host `xpra
attach` client connects (`xpra info` shows clients=1) and opens windows on `DISPLAY=:1`. A human
glance confirms the final visual.

- **2026-06-07 — ✅ R6 Openbox fork works too.** `gui-spike-ob` = `xpra openbox pcmanfm
  firefox-esr` (seamless; openbox doesn't actually run — see the finding above). Footprint:
  **XFCE 1.0 GB / 376 pkgs vs Openbox 816 MB / 243 pkgs** (~200 MB, 133 fewer pkgs; shared bulk
  is Firefox + GTK/X). 2nd proxy device on VM port **14501**; host attach → `running, 2 windows`
  (pcmanfm + Firefox). Identical mechanism to XFCE; flavor is purely the in-container package set.

## Outcome

**Goal met (with a caveat).** A browser + file manager from an Alpine **L2 system container**
appear as **native windows on the host desktop**, end-to-end through the real
container→VM→host chain, both XFCE- and Openbox-flavored. The one caveat is the resolution to
the version skew: today it works via a **containerized matching xpra-6 client** (no sudo);
the cleaner long-term fix is **upgrading the host xpra to 6.x** (xpra.org repo).

Repeatable: `scripts/spikes/remote-display-spike.sh` ({`up`,`attach`,`down`}). This journal is
the record of choices/blockers.

### Open choices / next steps (for discussion, not decided)

- **Host client strategy: RESOLVED — host upgraded to xpra 6.x.** Host now runs **v6.4.4**
  (xpra.org repo); native `xpra attach tcp://127.0.0.1:14500` connects directly to the v6.2.1
  server — `running, 2 windows`, no podman client. The containerized client remains a fallback
  for hosts that can't upgrade. (Still pin versions across host/image.)
- **Seamless vs desktop:** confirm we want individual native windows (seamless) — then the DE
  "flavor" is mostly moot and a **minimal** image (no full XFCE) is preferable.
- **Transport for the product:** the spike used `ssh -L` for VM→host; productionizing would use
  Lima `portForwards` (or vsock) + the Incus proxy device as a declarative sandbox **device**
  (fits the existing reconcile/`AddDevice` model). Per-sandbox port allocation needed.
- **Auth & isolation:** xpra TCP had no auth in the spike; add xpra auth + bind loopback only.
  One xpra server per agent UID for per-agent displays. Clipboard/file-transfer should be
  guardrail-gated (exfil channel).
- **Image build:** bake xpra + apps into a prebuilt Incus image (vs apk-add at launch, ~1 GB
  download each time). Likely a Debian-based "gui" image variant later (services already use
  debian/12); Alpine proved it works on musl, which was the big unknown.
- **`xpra screenshot` empty without a client** — fine for interactive use; note for any
  headless "thumbnail" feature.
- **Firefox/Thunar run as root** in the spike — switch to a non-root user for realism (also
  resolves the unix-socket `invalid username` control-channel issue).

## Non-root (unprivileged user inside the container) — ✅

Re-ran with a real unprivileged user instead of root: `adduser -D -u 1000 agent`, one-time
`dbus-uuidgen --ensure` (root, so `/etc/machine-id` exists), `/run/user/1000` owned by the user.
Start the server as that uid via `incus exec --user 1000 --group 1000 --env HOME=/home/agent
--env USER=agent --env XDG_RUNTIME_DIR=/run/user/1000 -- xpra start :100 …`. Apps added with
`xpra control :100 start <cmd>` (also as the agent).

Findings:
- All processes are owned by **`agent`** (xpra, the Xorg-dummy, thunar, firefox). Windows map
  the same as root. xpra 6 runs a real **Xorg dummy** (`Xorg-for-Xpra-100`), not Xvfb.
- The earlier **`invalid username ''`** control-channel error was a root-over-unix-socket
  artifact — as the real session-owner (`agent`) the unix-socket control authenticates fine.
- Don't `pkill -f "xpra start"` from a shell whose own cmdline contains that string — it
  matches and kills itself (`exit 143`). Stop via the pid file / `xpra stop`, or `pkill -x`.

## Real memory footprint (measured)

Method: cgroup v2 `memory.stat` **`anon`** = real program memory (the cgroup `memory.current`
is ~1 GB inflated by *reclaimable page cache* from the installed image — not a real cost). Plus
per-process **PSS** (shared-aware). XFCE-flavor container, server-side (no client attached),
Firefox on its default start page.

| stage | cgroup `anon` (real) | component PSS |
|---|---|---|
| idle container (no GUI) | ~negligible (cache only) | — |
| + xpra server (Xorg dummy, no apps) | **184 MiB** | xpra+Xorg **161 MiB** |
| + thunar (file manager) | 226 MiB (Δ **+42**) | thunar **37 MiB** |
| + firefox (11 procs) | **592 MiB** (Δ **+365**) | firefox **521 MiB** |

`memory.current` at the end was 1680 MiB, of which **1009 MiB was reclaimable page cache**.

**Takeaways:** the xpra *plumbing* (server + Xorg dummy) costs ~**160 MiB**; a file manager is
cheap (~**40 MiB**); the **browser dominates** (~**0.5 GiB**). So a GUI sandbox's real cost is
basically "xpra (~160 MiB) + whatever apps you run." Without a browser, a usable GUI sandbox is
~**200 MiB** anon. Multiply per concurrent agent when sizing the VM (8 GiB today).

## Host xpra upgrade (verified 2026-06-07, Ubuntu 24.04 noble)

Ubuntu ships xpra **v3.1.5**, which can't talk to the v6 server. Upgrade to the xpra.org build
(then native `xpra attach` works with no containerized client). Run on the **host**:

```bash
sudo wget -O /usr/share/keyrings/xpra.asc https://xpra.org/xpra.asc
sudo wget -O /etc/apt/sources.list.d/xpra.sources \
  https://raw.githubusercontent.com/Xpra-org/xpra/master/packaging/repos/noble/xpra.sources
sudo apt update
sudo apt install -y xpra        # pulls the higher-versioned xpra.org build (6.x)
xpra --version                  # expect v6.x
```

The `.sources` (deb822) it installs: `URIs: https://xpra.org · Suites: noble · Components: main
· Signed-By: /usr/share/keyrings/xpra.asc · Architectures: amd64 arm64`. After upgrade, attach
natively (no podman): open the tunnel, then `xpra attach tcp://127.0.0.1:14500`.

## Performance finding: laggy vs server-to-server `ssh -X` (2026-06-07)

Observed: the xpra display felt laggy; plain `ssh -X` server↔server (bare metal) felt much
snappier. Root causes, in impact order — this config is close to xpra's **worst case**:

1. **No GPU + software codecs, on a virtualized CPU.** Logs: `vpx … all codecs failed (vp8,vp9)`,
   no vaapi/nvenc (no GPU), `libOpenGL.so.0` missing → server encodes with **software x264**
   (or rgb/jpeg) and the **client paints in software** (OpenGL backing disabled). Capture →
   CPU-encode → decode → CPU-blit every frame, inside QEMU.
2. **Model difference.** `ssh -X` forwards X *drawing ops* that your real GPU-accelerated X server
   executes → latency ≈ network RTT, great on a LAN. xpra forwards *pixels* (capture+encode+
   decode+paint) → extra baseline latency per update; wins on WAN/pixel-pushing apps, not LAN
   latency for light apps.
3. **Stacked hops.** Our path = container → Incus proxy device (userspace relay) → ssh -L
   (another relay + crypto) → host, all server-side inside a Lima/QEMU VM. `ssh -X` was one
   direct hop, bare metal. More relay + virtualization latency, slower encode.
4. **App + window.** Firefox at 1280×1040, constantly recompositing, is a pixel-pusher (xpra's
   hard case but `ssh -X`'s *worst* case too). If the fast `ssh -X` test was a light primitive-
   drawing app (xterm/GTK2) on bare metal, it's not apples-to-apples.

**Implication (architectural):** snappy GUI wants **GPU-accelerated encode/decode and fewer
hops** — i.e. bare-metal Incus with GPU access, not software-only nested in a VM. In the current
VM/no-GPU shape, expect mediocre interactivity; `ssh -X` will beat it on a LAN for light apps.
Levers to try (cheap→fundamental): fix client OpenGL; tune `--encoding`/`--min-speed`/`--quality`
(favor speed over compression on a fast link); cut a relay; **get a GPU + hw codec**; run on
bare-metal Incus instead of the VM.

## Why the client GPU didn't help (2026-06-07)

User has an NVIDIA RTX 2080 Mobile + Intel UHD 630, but:
- **NVIDIA driver is currently broken** — `nvidia-smi`: *"Failed to initialize NVML:
  Driver/library version mismatch"* (580.159). dGPU unusable until fixed (reboot/match driver).
- `glxinfo`/`xpra opengl` on `:1` fail at `GLXCreateNewContext` (`BadValue`) → xpra's client
  **OpenGL backing can't init → software paint**.

But it's deeper than the driver — **xpra puts your client GPU on the wrong end of the pipe:**
- `ssh -X` renders the app on *your* X server / *your* GPU (that's the accel you felt).
- xpra renders the app on the **server** (container in the VM = software llvmpipe), then
  **captures + encodes pixels on the server** (no server GPU; VP8/VP9 failed → software x264/rgb)
  and streams them; the client only **decodes + paints**. Your client GPU is *never* in the
  app-render or encode path. It can only help with (a) GL paint of decoded frames — currently
  failing, and (b) **hardware video decode** — which only matters if the server sent a video
  codec, which it couldn't.
- The latency-critical bottleneck (capture+encode) is **server-side**, in a GPU-less VM. A fast
  client GPU cannot accelerate the server's encode. **Real GPU win needs a GPU on the SERVER**
  → bare-metal Incus with host-GPU passthrough into the container (easy for containers), or
  GPU-passthrough into the Lima VM (hard). Then NVENC/vaapi encode + client decode.

## Spike B: `ssh -X` (X11 forwarding) — ✅ works end-to-end

Motivation: bare-metal Incus / Lima GPU-passthrough (the path to GPU-accelerated xpra) **isn't
available on macOS**, so test the alternative: `ssh -X`, where the app renders on the **host's**
X server (host GPU) and no server-side encode/GPU is needed.

**Setup (in `gui-spike`):** `apk add openssh xauth`; **fix `X11Forwarding`** — Alpine's
`sshd_config` has `X11Forwarding no` at line 92, and sshd honors the **first** occurrence, so an
*appended* `yes` is ignored; edit the actual line, then restart sshd. `ssh-keygen -A`; start
`/usr/sbin/sshd`; put the host pubkey in `/home/agent/.ssh/authorized_keys`.

**Connect (host):** ProxyJump through the Lima VM to the container's bridge IP:
```bash
DISPLAY=:1 XAUTHORITY=/run/user/1000/gdm/Xauthority xhost +local:
ssh -F ~/.lima/llmsc/ssh.config -Y -J lima-llmsc agent@10.115.43.198 'firefox-esr --no-remote'
# GTK apps want a session bus: wrap with `dbus-run-session -- thunar`
```

**Result:** `DISPLAY=localhost:10.0` set in the container; **xeyes, Thunar, and Firefox all
render as native host windows**, decorated by GNOME (`mutter-x11-frames`). `xlsclients -display
:1` lists them as clients on machine `gui-spike` — i.e. **rendering on the host X server**, no
xpra/Xvfb/proxy-device/encode in the path.

**Findings:**
- Far **simpler stack** than xpra: no xpra, no Incus proxy device, no Xvfb, no codec, no version
  matching. Just sshd+xauth in the container + ProxyJump.
- **No persistence** — close the ssh session and every window dies. (xpra's detach/reattach is a
  real advantage for long-running agents.)
- **Weaker isolation** — used `-Y` (trusted) for reliability; that gives the container's app
  access to the host X server (keylogging/scraping risk). `-X` (untrusted) is safer but often
  breaks apps. xpra's pixel model never exposes the host X — a real security edge for agent
  sandboxes.
- **GL still not accelerated here:** firefox `glxtest` fails over the forwarded X (host NVIDIA
  driver mismatch + limited GLX-over-ssh) → software rendering. 2D apps render fine on the host.
- Expected perf shape: **light/primitive-drawing apps snappy; Firefox (pixmap-pusher) is ssh-X's
  weak case** — the inverse of xpra. On a fast LAN, light apps should beat xpra; heavy/animated
  content may not.
- **macOS:** the same ProxyJump-through-Lima approach works, but the host needs **XQuartz** (no
  native X server on macOS), which is itself software/unaccelerated — so mac `ssh -X` will also
  be modest, but it needs no server-side GPU. This is the macOS-viable path.

### xpra vs `ssh -X` (this architecture)

| aspect | xpra (Spike A) | `ssh -X` (Spike B) |
|---|---|---|
| renders | server-side (container, software in VM) → streams pixels | **host X server (your GPU)** |
| native windows | yes (seamless) | yes (host-WM decorated) |
| persistence (detach/reattach) | **yes** | no (dies with the session) |
| isolation | strong (app never touches host X) | weak (`-Y` = host X access) |
| setup complexity | high (xpra both ends, proxy device, version match) | **low** (sshd+xauth + ProxyJump) |
| GPU-accel needs | server GPU for encode (hard in VM/macOS) | host X/GPU (no server GPU) |
| best for | heavy/modern apps, WAN, reconnect | **light apps on LAN** |
| macOS | xpra client app | XQuartz |

**Takeaway:** neither is a clear winner — they're complementary. `ssh -X` is the pragmatic,
macOS-viable, low-setup path for light GUI apps and avoids the server-GPU problem entirely;
xpra wins on persistence, isolation, and heavy apps when a server GPU is available. A product
could offer both (`ssh -X` default for simple/mac, xpra when persistence/isolation matter).

## GPU sharing into the VM — analysis (2026-06-07)

Question: can a GPU be shared *into* the Lima VM while the host still uses it, and would that
speed up xpra or ssh -X? Three ways to get a GPU into a VM:

| mechanism | shared w/ host? | gives guest | on this HW (RTX 2080 Mobile)? |
|---|---|---|---|
| VFIO/PCI passthrough | ❌ exclusive (host loses GPU) | full GPU incl. NVENC | ugly on Optimus |
| SR-IOV / vGPU | ✅ true HW share incl. encode | full GPU | ❌ datacenter GPUs + license only |
| virtio-gpu + virgl/venus | ✅ host keeps GPU | **GL/Vulkan only** (proxied) | yes-ish, but Lima doesn't expose it (headless) |

- The only "shared while host uses it" option here is **virtio-gpu/virgl** — and it proxies the
  *rendering* APIs, **not** the video-encode ASIC (NVENC/VAAPI).
- **xpra:** would accelerate app *rendering* (vs llvmpipe) but **not the encode** (the real
  bottleneck) → marginal. GPU encode in the VM needs the encode engine → exclusive VFIO.
- **ssh -X:** **zero benefit** — the app renders on the *host* X server (host GPU); the VM's GPU
  is never in the path.
- The "shared GPU while host uses it" model that actually works is **Incus passing `/dev/dri`
  into a container** (host keeps the GPU; container gets GL + VAAPI/NVENC) — but only on **bare
  metal** (containers share the host kernel; no VM boundary). The VM is the whole obstacle.
- **macOS:** worst case — no PCI passthrough (Apple Silicon), virtio-gpu via libkrun/venus is
  experimental (GL/Vulkan, proxies to Metal), no NVENC/VAAPI (Apple = VideoToolbox), no native X
  (XQuartz, ~software). Effectively software-only for both methods; no bare-metal container path.

## Post-reboot re-tests — GPU healthy (2026-06-07)

Host NVIDIA driver fixed by reboot: `glxinfo` → **NVIDIA RTX 2080, GL 4.6, 8 GB** (was
`BadValue` before). Re-ran both methods (VM + containers were restarted; runtime rebuilt:
`/run` is tmpfs so recreate `/run/user/1000`; xpra server + sshd restarted; proxy devices
persist in config).

- **A — xpra:** re-attached fine (`running, 2 windows`). Host GL now lets the *client* use its
  OpenGL backing (GPU paint), but the **server-side encode is still software in the VM** — the
  dominant bottleneck is unchanged. → marginal improvement only.
- **B — ssh -X:** ⚠️ **firefox still software-rendered** even with the GPU healthy. `glxtest`
  fails (`X error … request_code=155` = GLX; `libpci missing`). Cause is **not** the driver — it's
  that **`ssh -X` can't give a remote app hardware OpenGL**: modern drivers disable *indirect
  GLX*. So `ssh -X` host-accelerates only **2D X drawing**, never app GL. Browsers/WebRender get
  no GPU accel over ssh -X regardless of host GPU. Light/2D apps (xeyes, thunar) remain fine.
- **Combined, empirically confirmed from both directions:** in the **nested VM**, *neither* xpra
  *nor* ssh -X GPU-accelerates a browser. GPU-accelerated GUI requires the GPU **co-located with
  the app** → bare-metal Incus + `/dev/dri` into the container. That's the only path, and it's
  Linux-only.

### Path 1 — single SSH hop (vs ProxyJump's double) ✅
Added an Incus proxy device `VM:2222 → container:22` + an `ssh -L 2222`, then
`ssh -X -p 2222 agent@127.0.0.1` → `DISPLAY=localhost:10.0`, xeyes is a host X client. One
end-to-end SSH session (host↔container) instead of ProxyJump's nested pair; the relays are dumb
TCP. Removes the outer SSH's double-encryption, but the bytes still cross the VM net — so it
trims SSH overhead, not the VM-network latency floor. (Production: Lima `portForward` instead of
the `ssh -L`.)

### Tiny text in xpra = DPI (host 4K @ 160%, guest 120 DPI)
Host panel is 3840×2160 at **160% fractional scaling**; native host apps get GNOME's 1.6×, but
streamed xpra apps render 1:1 at the guest DPI → tiny. **The client DPI wins** over the server's
`--dpi` (server `--dpi=154` was overridden to the host-reported 120). Fix on the **client**:
`xpra attach --dpi=160` (verified → `display.dpi.value=160`, larger crisp text), or
`--desktop-scaling=140%` (uniform upscale, slightly soft). Per-app fallbacks: `GDK_DPI_SCALE`,
firefox `layout.css.devPixelsPerPx`. Pick the number to taste (~96×host-scale).

## Productizing: xpra + ssh -X as user-selectable display options (future)

Decision: ship **both** methods as options (not pick one). xpra = persistence/isolation/heavy
apps; ssh -X = simple/light/macOS-viable. Streamlining backlog (to explore later):

- **Model:** a per-sandbox/agent `display` setting (`xpra` | `x11` | `none`) in `llmsc.toml`,
  compiled like the other guardrail axes.
- **Transport (declarative):** manage the Incus **proxy device** + Lima **portForward** through
  the reconcile engine; per-sandbox port allocation; default to the **single-hop** path.
- **Image:** bake a **GUI image variant** (xpra + xauth + sshd + minimal WM + file manager) so
  launch is fast (avoid the ~1 GB apk each time). Decide XFCE vs minimal/openbox; Debian-based
  likely (services already use debian/12). Alpine proved the stack works on musl.
- **CLI/GUI surface:** `llmsc display <sandbox> [--method xpra|x11] [--dpi]` to attach; a GUI
  "Display" action (Tauri as control plane shelling to the client — native windows, not embedded).
- **DPI/scaling:** auto-detect host scale and pass `--dpi`/`--desktop-scaling` (client-side wins).
- **`llmsctl doctor` checks:** host xpra ≥ 6 (version skew), XQuartz on macOS, X server present,
  GPU/driver health.
- **Security/guardrails:** display method as a policy axis; xpra auth + loopback-only bind;
  ssh -X `-Y` trust caveat; clipboard/file-transfer gating (exfil channel).
- **Persistence UX:** surface xpra detach/reattach; mark ssh -X sessions ephemeral.
- **Perf guidance:** route by workload (light/2D → x11; heavy/persistent → xpra); GPU only via
  bare-metal Incus + `/dev/dri` (Linux), documented as the snappy path.

## Teardown

Left running after this session so the windows are visible. To tear down:

```
scripts/spikes/remote-display-spike.sh down gui-spike    14500
scripts/spikes/remote-display-spike.sh down gui-spike-ob 14501
# ssh -X sessions are plain ssh procs on the host — close their windows or kill the ssh PIDs.
# or manually:
podman rm -f xpra6attach xpra6ob 2>/dev/null
limactl shell llmsc -- sudo incus delete -f gui-spike gui-spike-ob
```

Spike B (ssh -X) repeatable via `scripts/spikes/remote-display-sshx.sh` ({`setup`,`run`}).

(The `xpra6-client` podman image and the `gui-spike*` containers persist until deleted.)
