<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    maxWidth = 640,
    onclose,
    body,
    foot,
  }: {
    title: string;
    maxWidth?: number;
    onclose: () => void;
    body: Snippet;
    foot?: Snippet;
  } = $props();

  function onbg(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }
</script>

<div class="modal-bg" role="presentation" onclick={onbg}>
  <div class="modal" style="max-width:{maxWidth}px" role="dialog" aria-modal="true" aria-label={title}>
    <div class="modal-head">
      <h3>{title}</h3>
      <button class="iconbtn right" aria-label="Close" onclick={onclose}><Icon name="x" /></button>
    </div>
    <div class="modal-body">{@render body()}</div>
    {#if foot}
      <div class="modal-foot">{@render foot()}</div>
    {/if}
  </div>
</div>
