<script lang="ts">
  import { onProgress } from "./core";

  // Rolling log of recent steps from long-running operations. Shows a toast while steps stream in,
  // then auto-dismisses a few seconds after the last one. Works in both the Tauri shell (real
  // events) and the browser (mock steps) since both feed `onProgress`.
  let lines = $state<string[]>([]);
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const off = onProgress((msg) => {
      lines = [...lines, msg].slice(-6);
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => (lines = []), 4000);
    });
    return () => {
      off();
      if (timer) clearTimeout(timer);
    };
  });
</script>

{#if lines.length > 0}
  <div class="toast" role="status" aria-live="polite">
    <div class="head">
      <span class="spinner" aria-hidden="true"></span>
      <span>Working…</span>
      <button class="close" onclick={() => (lines = [])} aria-label="Dismiss">×</button>
    </div>
    <ul>
      {#each lines as line, i (i + line)}
        <li class:last={i === lines.length - 1}>{line}</li>
      {/each}
    </ul>
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    right: 1rem;
    bottom: 1rem;
    width: 280px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
    padding: 0.7rem 0.85rem;
    font-size: 0.85rem;
    z-index: 50;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 600;
    margin-bottom: 0.4rem;
  }
  .close {
    margin-left: auto;
    border: none;
    background: transparent;
    color: var(--muted, #888);
    font-size: 1.1rem;
    line-height: 1;
    cursor: pointer;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  li {
    color: var(--muted, #888);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  li.last {
    color: var(--text);
    font-weight: 500;
  }
  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--accent-soft, #dde);
    border-top-color: var(--accent, #46c);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
