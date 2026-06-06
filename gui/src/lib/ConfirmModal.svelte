<script lang="ts">
  import Modal from "./Modal.svelte";
  import { ui, resolveConfirm } from "./store.svelte";
</script>

{#if ui.confirm}
  <Modal title={ui.confirm.title} maxWidth={420} onclose={() => resolveConfirm(false)}>
    {#snippet body()}
      <p class="confirm-msg">{ui.confirm!.message}</p>
    {/snippet}
    {#snippet foot()}
      <button class="btn" onclick={() => resolveConfirm(false)}>Cancel</button>
      <button class="btn" class:danger={ui.confirm!.danger} class:primary={!ui.confirm!.danger} onclick={() => resolveConfirm(true)}>
        {ui.confirm!.confirmLabel}
      </button>
    {/snippet}
  </Modal>
{/if}

<style>
  .confirm-msg { margin: 0; font-size: 13px; color: var(--text); line-height: 1.5; }
</style>
