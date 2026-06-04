<script lang="ts">
  import { ui } from "./store.svelte";

  // Faux shell drawer (direction A). Opened with a `user@host` target via openTerminal().
  // It does not run a real shell yet — it shows what `llmsc shell <target>` would attach to.
  const target = $derived(ui.terminalTarget ?? "");
  const user = $derived(target.split("@")[0] ?? "operator");
  const host = $derived(target.split("@")[1] ?? "");
  const idLine = $derived(
    user === "operator"
      ? "uid=1000(operator) groups=sudo"
      : user === "agent-claude"
        ? "uid=1001(agent-claude)"
        : "uid=1002(agent-aux)",
  );
  const role = $derived(
    user === "operator" ? "human operator · full rw · sudo" : "agent user · scoped permissions · virtual key only",
  );

  function close() { ui.terminalTarget = null; }
</script>

{#if ui.terminalTarget}
  <div class="drawer">
    <button class="scrim" aria-label="Close terminal" onclick={close}></button>
    <div class="panel">
      <div class="bar">
        <span class="mac r"></span><span class="mac y"></span><span class="mac g"></span>
        <span class="title mono">{target}</span>
        <span class="tag mono">L2 · unprivileged</span>
        <div style="flex:1"></div>
        <button class="close" onclick={close}>✕ close</button>
      </div>
      <div class="body mono">
        <div class="dim">$ <span class="br">llmsc shell {target}</span></div>
        <div class="dim">Connecting to L2 sandbox <span class="br">{host}</span> (unprivileged Incus container)…</div>
        <div class="ok">Welcome to {host} — dev-ubuntu-24.04</div>
        <div class="faint">{role}</div>
        <div class="prompt">{user}@{host}:~$ <span class="br">id</span></div>
        <div class="dim">{idLine}</div>
        <div class="prompt">{user}@{host}:~$ <span class="br">podman ps --format '{'{{'}.Names{'}}'}'</span></div>
        <div class="dim">app<br/>postgres<br/>redis</div>
        <div class="prompt">{user}@{host}:~$ <span class="cursor">▍</span></div>
      </div>
    </div>
  </div>
{/if}

<style>
  .drawer { position: fixed; inset: 0; z-index: 150; }
  .scrim { position: absolute; inset: 0; background: rgba(5, 6, 8, 0.72); backdrop-filter: blur(3px); border: none; cursor: pointer; }
  .panel {
    position: absolute; left: 0; right: 0; bottom: 0; height: 58%;
    background: var(--sidebar); border-top: 1px solid var(--border-strong);
    box-shadow: 0 -12px 40px rgba(0, 0, 0, 0.6); display: flex; flex-direction: column;
    animation: rise 0.18s ease-out;
  }
  @keyframes rise { from { transform: translateY(12px); opacity: 0; } to { transform: none; opacity: 1; } }
  .bar { height: 40px; display: flex; align-items: center; gap: 8px; padding: 0 14px; border-bottom: 1px solid var(--border); background: var(--card); }
  .mac { width: 11px; height: 11px; border-radius: 50%; }
  .mac.r { background: rgba(244, 63, 94, 0.7); }
  .mac.y { background: rgba(245, 158, 11, 0.7); }
  .mac.g { background: rgba(16, 185, 129, 0.7); }
  .title { margin-left: 6px; font-size: 12px; color: var(--text-2); }
  .close { margin-left: auto; border: none; background: transparent; color: var(--text-3); font-size: 12px; cursor: pointer; }
  .close:hover { color: var(--text); }
  .body { flex: 1; overflow-y: auto; padding: 16px; font-size: 12px; line-height: 1.65; background: #060709; }
  .dim { color: var(--text-3); }
  .faint { color: #4b5563; }
  .ok { color: var(--ok); }
  .br { color: var(--text); }
  .prompt { color: var(--text-2); margin-top: 6px; }
  .cursor { animation: blink 1.4s steps(2, start) infinite; }
  @keyframes blink { to { opacity: 0.25; } }
</style>
