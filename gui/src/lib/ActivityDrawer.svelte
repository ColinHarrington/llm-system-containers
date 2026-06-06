<script lang="ts">
  import Icon from "./Icon.svelte";
  import { ui, activity, clearActivity, type ActivityKind } from "./store.svelte";

  type Filter = "all" | "alerts" | "progress";
  let filter = $state<Filter>("all");

  function close() { ui.activityOpen = false; }
  const dotClass = (k: ActivityKind) =>
    k === "ok" ? "ok" : k === "danger" ? "danger" : k === "warn" ? "warn" : k === "progress" ? "muted" : "accent";
  const fmtTime = (t: number) =>
    new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  const isAlert = (k: ActivityKind) => k === "danger" || k === "warn";
  const alertCount = $derived(activity.filter((a) => isAlert(a.kind)).length);
  const progressCount = $derived(activity.filter((a) => a.kind === "progress").length);
  const shown = $derived(
    activity.filter((a) => filter === "all" || (filter === "alerts" ? isAlert(a.kind) : a.kind === "progress")),
  );
</script>

{#if ui.activityOpen}
  <div class="act-bg" role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
    <div class="act" role="dialog" aria-modal="true" aria-label="Activity log">
      <div class="act-head">
        <h3><Icon name="bell" size={15} /> Activity</h3>
        <div class="right flex gap6">
          <button class="btn sm" onclick={clearActivity} disabled={activity.length === 0}>Clear</button>
          <button class="btn sm" onclick={close} title="Close">✕</button>
        </div>
      </div>
      <div class="act-filter">
        <div class="seg">
          <button class:on={filter === "all"} onclick={() => (filter = "all")}>All <span class="fc">{activity.length}</span></button>
          <button class:on={filter === "alerts"} onclick={() => (filter = "alerts")}>Alerts <span class="fc">{alertCount}</span></button>
          <button class:on={filter === "progress"} onclick={() => (filter = "progress")}>Steps <span class="fc">{progressCount}</span></button>
        </div>
      </div>
      <div class="act-list">
        {#if activity.length === 0}
          <div class="act-empty">No activity yet. Operations and notifications will appear here.</div>
        {:else if shown.length === 0}
          <div class="act-empty">Nothing in this view.</div>
        {:else}
          {#each shown as a (a.id)}
            <div class="act-row">
              <span class="dot {dotClass(a.kind)}"></span>
              <span class="act-msg">{a.msg}</span>
              <span class="act-time mono">{fmtTime(a.time)}</span>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .act-bg { position: fixed; inset: 0; z-index: 240; background: rgba(5, 6, 8, 0.4); display: flex; justify-content: flex-end; }
  .act { width: 100%; max-width: 380px; height: 100%; background: var(--card); border-left: 1px solid var(--border-strong); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; animation: slidein 0.16s ease-out; }
  @keyframes slidein { from { transform: translateX(20px); opacity: 0; } to { transform: none; opacity: 1; } }
  .act-head { display: flex; align-items: center; gap: 8px; padding: 14px 16px; border-bottom: 1px solid var(--border); }
  .act-head h3 { display: flex; align-items: center; gap: 8px; font-size: 14px; margin: 0; }
  .act-filter { padding: 10px 12px; border-bottom: 1px solid var(--border); }
  .act-filter .seg { width: 100%; display: flex; }
  .act-filter .seg button { flex: 1; display: inline-flex; align-items: center; justify-content: center; gap: 5px; }
  .act-filter .fc { font-size: 9.5px; color: var(--text-3); }
  .act-filter .seg button.on .fc { color: var(--accent-text); }
  .act-list { flex: 1; overflow-y: auto; padding: 6px 8px; }
  .act-row { display: grid; grid-template-columns: auto 1fr auto; gap: 9px; align-items: baseline; padding: 7px 8px; border-radius: var(--radius-sm); }
  .act-row:hover { background: var(--card-2); }
  .act-row .dot { margin-top: 5px; }
  .act-msg { font-size: 12.5px; color: var(--text); overflow-wrap: anywhere; }
  .act-time { font-size: 10.5px; color: var(--text-3); white-space: nowrap; }
  .act-empty { padding: 24px 16px; color: var(--text-3); font-size: 12px; text-align: center; }
</style>
