#!/usr/bin/env bash
#
# Spike B: native host windows via `ssh -X` (X11 forwarding) from an L2 container.
# The app renders on the HOST X server (host GPU) — no xpra, no encode, no proxy device.
# Reaches the container (inside the Lima VM) via ProxyJump through the VM.
# Trade-offs vs xpra: simpler + uses host GPU, BUT no persistence and weaker isolation (-Y).
# Full write-up: planning/spikes/remote-display-gui.md (Spike B).
#
# macOS note: the same approach works, but the host needs XQuartz (no native X server on macOS).
#
# Usage:
#   ./remote-display-sshx.sh setup                         # sshd + xauth + X11Forwarding + key
#   ./remote-display-sshx.sh run "firefox-esr --no-remote" # render an app on host :1 (Ctrl-C closes)
#   ./remote-display-sshx.sh run                           # default: a file manager (thunar)
#
set -euo pipefail

VM=${LLMSC_VM:-llmsc}
SSHCFG="$HOME/.lima/$VM/ssh.config"
NAME=${NAME:-gui-spike}
PUBKEY=${PUBKEY:-$HOME/.ssh/id_ed25519.pub}
KH=/tmp/spike_known_hosts

vexec() { limactl shell "$VM" -- sudo incus exec "$NAME" -- sh -c "$1"; }
cip()   { limactl shell "$VM" -- sudo incus list "$NAME" -c4 --format csv 2>/dev/null | sed 's/ .*//'; }

cmd_setup() {
  echo ">> install openssh + xauth, enable X11Forwarding, (re)start sshd"
  # IMPORTANT: edit the real X11Forwarding line — sshd honors the FIRST occurrence, so an
  # appended 'yes' after Alpine's default 'X11Forwarding no' is ignored.
  vexec 'apk add --no-progress openssh xauth >/dev/null;
    ssh-keygen -A >/dev/null 2>&1;
    sed -i "s/^X11Forwarding no/X11Forwarding yes/" /etc/ssh/sshd_config;
    grep -q "^X11Forwarding yes" /etc/ssh/sshd_config || echo "X11Forwarding yes" >> /etc/ssh/sshd_config;
    mkdir -p /home/agent/.ssh && chmod 700 /home/agent/.ssh && chown agent:agent /home/agent/.ssh;
    pkill -x sshd 2>/dev/null; sleep 1; /usr/sbin/sshd'
  echo ">> install host pubkey ($PUBKEY) for agent"
  cat "$PUBKEY" | limactl shell "$VM" -- sudo incus exec "$NAME" -- sh -c \
    'cat > /home/agent/.ssh/authorized_keys; chmod 600 /home/agent/.ssh/authorized_keys; chown agent:agent /home/agent/.ssh/authorized_keys'
  echo ">> ready. container IP: $(cip)"
}

cmd_run() {
  local app=${1:-"dbus-run-session -- thunar"}
  local ip; ip=$(cip)
  : "${DISPLAY:=:1}"; export DISPLAY
  export XAUTHORITY="${XAUTHORITY:-/run/user/$(id -u)/gdm/Xauthority}"
  xhost +local: >/dev/null 2>&1 || true
  echo ">> ssh -Y -J lima-$VM agent@$ip -- $app   (window renders on host $DISPLAY; Ctrl-C closes it)"
  ssh -F "$SSHCFG" -Y \
    -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile="$KH" -o ConnectTimeout=10 \
    -J "lima-$VM" "agent@$ip" "$app"
}

case "${1:-}" in
  setup) cmd_setup ;;
  run)   shift; cmd_run "$@" ;;
  *) echo "usage: NAME=$NAME $0 {setup | run \"<cmd>\"}"; echo "  e.g. $0 run 'firefox-esr --no-remote'"; exit 1 ;;
esac
