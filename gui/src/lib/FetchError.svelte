<script lang="ts">
  import Icon from "./Icon.svelte";
  let { message, onretry, busy = false }: { message: string; onretry: () => void; busy?: boolean } = $props();
</script>

<div class="banner warn mb16" role="alert" data-testid="fetch-error">
  <Icon name="warn" size={18} />
  <span style="flex:1">Couldn't load: {message}</span>
  <button class="btn sm" disabled={busy} onclick={onretry}><Icon name="arrow" size={13} /><span>{busy ? "Retrying…" : "Retry"}</span></button>
</div>
