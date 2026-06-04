<script lang="ts">
  import { ui } from "./store.svelte";

  // Bottom-right command/status toast (direction A). Auto-dismisses ~2.6s after the last one.
  let visible = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let shown = $state<{ msg: string; color: string } | null>(null);

  $effect(() => {
    const t = ui.toast;
    if (!t) return;
    shown = { msg: t.msg, color: t.color };
    visible = true;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => (visible = false), 2600);
    return () => { if (timer) clearTimeout(timer); };
  });
</script>

{#if visible && shown}
  <div class="toast" role="status" aria-live="polite">
    <span class="tdot {shown.color}"></span>
    <span class="tmsg">{shown.msg}</span>
  </div>
{/if}

<style>
  .toast {
    position: fixed; bottom: 20px; right: 20px; z-index: 200;
    display: flex; align-items: center; gap: 10px;
    padding: 11px 15px; border-radius: var(--radius-sm);
    background: var(--card); border: 1px solid var(--border-strong);
    box-shadow: var(--shadow-lg);
    font-size: 12px; color: var(--text);
    animation: rise 0.18s ease-out;
  }
  @keyframes rise { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: none; } }
  .tdot { width: 8px; height: 8px; border-radius: 50%; flex: none; background: var(--accent); }
  .tdot.ok { background: var(--ok); }
  .tdot.warn { background: var(--warn); }
  .tdot.danger { background: var(--danger); }
  .tmsg { font-family: var(--mono); }
</style>
