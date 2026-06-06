<script lang="ts">
  import Icon from "./Icon.svelte";
  import { showToast } from "./store.svelte";

  // Inline copy-to-clipboard affordance. Flips to a checkmark briefly and confirms via toast.
  let { value, label, size = 13 }: { value: string; label?: string; size?: number } = $props();
  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(e: MouseEvent) {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(value);
      copied = true;
      showToast(`Copied ${label ?? value}`, "ok");
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => (copied = false), 1200);
    } catch {
      showToast("Clipboard unavailable", "warn");
    }
  }
</script>

<button class="copy-btn" title={`Copy ${label ?? value}`} aria-label={`Copy ${label ?? value}`} onclick={copy}>
  <Icon name={copied ? "check" : "copy"} {size} />
</button>

<style>
  .copy-btn {
    border: none; background: transparent; color: var(--text-3); cursor: pointer;
    padding: 2px; border-radius: 5px; display: inline-grid; place-items: center;
    vertical-align: middle; line-height: 0;
  }
  .copy-btn:hover { color: var(--accent-text); background: var(--card-2); }
</style>
