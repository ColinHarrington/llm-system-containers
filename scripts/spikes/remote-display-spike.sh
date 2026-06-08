#!/usr/bin/env bash
#
# Spike: native host windows from an L2 xpra sandbox.
# Proves a GUI app inside an Alpine L2 Incus container (in the Lima `llmsc` VM) can appear as a
# real window on the host desktop, end-to-end:
#
#   L2 container (xpra server :100, tcp 14500)
#     -> Incus proxy device (container -> VM loopback <vmport>)
#     -> ssh -L (VM -> host loopback <vmport>)
#     -> xpra attach (matching v6 client, containerized) -> host X DISPLAY
#
# Full write-up + findings/choices: planning/spikes/remote-display-gui.md
#
# WHY a containerized client: Ubuntu 24.04 ships xpra v3.1.5, but Alpine has v6.2.1; xpra v3<->v6
# cannot interoperate (compression/encoding handshake fails). Rather than touch host system
# packages, we run a matching xpra-6 client in rootless podman sharing the host X11 socket.
# The clean long-term alternative is upgrading the host xpra to 6.x from the xpra.org apt repo.
#
# Apps run as an UNPRIVILEGED user (uid 1000 'agent') inside the container, not root.
#
# Usage:
#   ./remote-display-spike.sh up     [name] [flavor: xfce|openbox] [vmport]
#   ./remote-display-spike.sh attach [vmport] [dpi]      # native client (dpi default 160); Ctrl-C detaches
#   ./remote-display-spike.sh mem    [name]              # real memory breakdown (anon + PSS)
#   ./remote-display-spike.sh down   [name] [vmport]
#   ./remote-display-spike.sh client-image               # (re)build the xpra-6 client image
#
# Examples:
#   ./remote-display-spike.sh up gui-spike    xfce    14500 && ./remote-display-spike.sh attach 14500
#   ./remote-display-spike.sh up gui-spike-ob openbox 14501 && ./remote-display-spike.sh attach 14501
#
set -euo pipefail

VM=${LLMSC_VM:-llmsc}
SSHCFG="$HOME/.lima/$VM/ssh.config"
SSHHOST="lima-$VM"
CLIENT_IMAGE=xpra6-client
DISPLAY=${DISPLAY:-:1}

UID_AGENT=1000
vincus() { limactl shell "$VM" -- sudo incus "$@"; }
vexec()  { limactl shell "$VM" -- sudo incus exec "$1" -- sh -c "$2"; }
# run a command inside the container AS THE UNPRIVILEGED agent user (uid 1000)
vexecu() { limactl shell "$VM" -- sudo incus exec "$1" \
  --user "$UID_AGENT" --group "$UID_AGENT" --cwd /home/agent \
  --env HOME=/home/agent --env USER=agent --env XDG_RUNTIME_DIR=/run/user/$UID_AGENT -- "${@:2}"; }

build_client_image() {
  podman image exists "$CLIENT_IMAGE" && return 0
  echo ">> building $CLIENT_IMAGE (xpra 6 client, ~500 MiB)"
  podman build -t "$CLIENT_IMAGE" - <<'EOF'
FROM docker.io/library/alpine:3.21
RUN apk add --no-cache xpra ttf-dejavu xauth
EOF
}

cmd_up() {
  local name=${1:-gui-spike} flavor=${2:-xfce} vmport=${3:-14500}
  local pkgs fm
  case "$flavor" in
    xfce)    pkgs="xpra xfce4 xfce4-session thunar firefox-esr dbus xvfb ttf-dejavu xfce4-terminal"; fm=thunar ;;
    openbox) pkgs="xpra openbox pcmanfm firefox-esr dbus ttf-dejavu";                                fm=pcmanfm ;;
    *) echo "unknown flavor: $flavor (want xfce|openbox)"; exit 2 ;;
  esac

  echo ">> launching L2 container $name ($flavor)"
  vincus launch images:alpine/3.21 "$name" -c security.nesting=true || true
  sleep 3
  echo ">> installing GUI stack (apk)"
  vexec "$name" "apk add --no-progress $pkgs"
  echo ">> creating unprivileged user 'agent' (uid $UID_AGENT) + runtime dir"
  vexec "$name" "id agent 2>/dev/null || adduser -D -u $UID_AGENT agent; \
    dbus-uuidgen --ensure 2>/dev/null || true; \
    mkdir -p /run/user/$UID_AGENT && chown agent:agent /run/user/$UID_AGENT && chmod 700 /run/user/$UID_AGENT"
  echo ">> starting xpra seamless server AS agent (non-root)"
  # NOTE: do NOT start a window manager — xpra is the WM in seamless mode.
  vexecu "$name" xpra start :100 --bind-tcp=127.0.0.1:14500 \
    --daemon=yes --no-mdns --no-pulseaudio --log-file=/home/agent/xpra.log
  sleep 5
  echo ">> launching apps via xpra control (as agent): firefox-esr + $fm"
  vexecu "$name" xpra control :100 start firefox-esr
  vexecu "$name" xpra control :100 start "$fm"
  echo ">> adding Incus proxy device (VM 127.0.0.1:$vmport -> container 127.0.0.1:14500)"
  vincus config device add "$name" xpra proxy \
    listen="tcp:127.0.0.1:$vmport" connect="tcp:127.0.0.1:14500" bind=host || true
  echo ">> container side ready: $name ($flavor) on VM port $vmport"
  echo "   next: $0 attach $vmport"
}

cmd_attach() {
  local vmport=${1:-14500} dpi=${2:-160}
  : "${DISPLAY:=:1}"; export DISPLAY
  if ! pgrep -f "L $vmport:127.0.0.1:$vmport" >/dev/null 2>&1; then
    echo ">> opening ssh tunnel host:$vmport -> VM:$vmport"
    ssh -F "$SSHCFG" "$SSHHOST" -L "$vmport:127.0.0.1:$vmport" -N -f -o ExitOnForwardFailure=yes
  fi
  echo ">> granting local X access"
  xhost +local: >/dev/null 2>&1 || true
  # Client DPI wins over the server's --dpi, so set it here (HiDPI host vs 96dpi guest).
  # Prefer the native host client (host xpra >= 6); else the containerized matching client.
  if command -v xpra >/dev/null 2>&1 && xpra --version 2>/dev/null | grep -qE 'v?[6-9]\.'; then
    echo ">> native xpra attach (dpi=$dpi); Ctrl-C detaches, sandbox session keeps running"
    xpra attach "tcp://127.0.0.1:$vmport" --no-tray --dpi="$dpi"
  else
    echo ">> host xpra missing/old → containerized xpra-6 client (dpi=$dpi)"
    build_client_image
    podman run --rm --network=host -e DISPLAY="$DISPLAY" \
      -v /tmp/.X11-unix:/tmp/.X11-unix \
      "$CLIENT_IMAGE" xpra attach "tcp://127.0.0.1:$vmport" --no-tray --dpi="$dpi"
  fi
}

cmd_mem() {   # mem <name> — real memory: cgroup anon + page cache + per-app PSS
  local name=${1:-gui-spike}
  vexec "$name" '
    anon=$(awk "/^anon /{print \$2}" /sys/fs/cgroup/memory.stat)
    file=$(awk "/^file /{print \$2}" /sys/fs/cgroup/memory.stat)
    cur=$(cat /sys/fs/cgroup/memory.current)
    mib(){ awk "BEGIN{printf \"%.1f MiB\", $1/1048576}"; }
    echo "anon (real program mem) : $(mib $anon)"
    echo "page cache (reclaimable): $(mib $file)"
    echo "memory.current (total)  : $(mib $cur)"
    pss(){ t=0; for p in $1; do v=$(awk "/^Pss:/{print \$2}" /proc/$p/smaps_rollup 2>/dev/null); t=$((t+${v:-0})); done; awk "BEGIN{printf \"%.1f MiB\", $t/1024}"; }
    echo "PSS xpra+Xorg : $(pss "$(pgrep -x python3) $(pgrep -x Xorg)")"
    echo "PSS firefox   : $(pss "$(pgrep -f firefox-esr)")"
    echo "PSS thunar    : $(pss "$(pgrep -x thunar)")"
    echo "PSS pcmanfm   : $(pss "$(pgrep -x pcmanfm)")"
  '
}

cmd_down() {
  local name=${1:-gui-spike} vmport=${2:-}
  echo ">> deleting container $name"
  vincus delete -f "$name" || true
  if [ -n "$vmport" ]; then
    echo ">> killing tunnel on $vmport"
    pkill -f "L $vmport:127.0.0.1:$vmport" || true
  fi
}

case "${1:-}" in
  up)           shift; cmd_up "$@" ;;
  attach)       shift; cmd_attach "$@" ;;
  mem)          shift; cmd_mem "$@" ;;
  down)         shift; cmd_down "$@" ;;
  client-image) build_client_image ;;
  *) sed -n '2,40p' "$0"; exit 1 ;;
esac
