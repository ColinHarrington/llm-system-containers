<script lang="ts">
  import { ui, dismissToast } from "./store.svelte";

  // Bottom-right toast stack. Each toast auto-dismisses; ones carrying an action
  // (e.g. Undo) linger a little longer so it's actually clickable.
  const scheduled = new Set<number>();
  $effect(() => {
    for (const t of ui.toasts) {
      if (scheduled.has(t.id)) continue;
      scheduled.add(t.id);
      const ms = t.action ? 6000 : 2800;
      setTimeout(() => {
        dismissToast(t.id);
        scheduled.delete(t.id);
      }, ms);
    }
  });
</script>

{#if ui.toasts.length}
  <div class="toast-stack">
    {#each ui.toasts as t (t.id)}
      <div class="toast" role="status" aria-live="polite">
        <span class="tdot {t.color}"></span>
        <span class="tmsg">{t.msg}</span>
        {#if t.action}
          <button class="toast-action" onclick={() => { t.action!.run(); dismissToast(t.id); }}>{t.action.label}</button>
        {/if}
        <button class="toast-x" aria-label="Dismiss" onclick={() => dismissToast(t.id)}>×</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-stack {
    position: fixed; bottom: 20px; right: 20px; z-index: 200;
    display: flex; flex-direction: column; gap: 8px; align-items: flex-end;
  }
  .toast {
    display: flex; align-items: center; gap: 10px;
    padding: 11px 13px 11px 15px; border-radius: var(--radius-sm);
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
  .toast-action {
    border: 1px solid var(--border-strong); background: var(--card-2); color: var(--accent-text);
    font-family: inherit; font-size: 11px; font-weight: 600; cursor: pointer;
    padding: 3px 9px; border-radius: 6px;
  }
  .toast-action:hover { background: var(--accent-soft); }
  .toast-x {
    border: none; background: transparent; color: var(--text-3); cursor: pointer;
    font-size: 15px; line-height: 1; padding: 2px 4px;
  }
  .toast-x:hover { color: var(--text); }
</style>
