<script lang="ts">
  import { ICON } from "./icons";

  // Renders a named inline SVG. Size is applied via a wrapping span so callers can
  // size it with CSS (the .btn / .nav-item rules target svg directly too).
  let { name, size = 18 }: { name: keyof typeof ICON | string; size?: number } = $props();
  const svg = $derived(ICON[name] ?? "");
</script>

<span class="icon" style="width:{size}px;height:{size}px">{@html svg}</span>

<style>
  .icon { display: inline-grid; place-items: center; }
  .icon :global(svg) { width: 100%; height: 100%; stroke-width: 1.8; }
</style>
