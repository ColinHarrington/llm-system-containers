<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui, type IncusTab } from "../lib/store.svelte";
  import IncusProfiles from "./IncusProfiles.svelte";
  import Networking from "./Networking.svelte";
  import Images from "./Images.svelte";

  const tabs: { id: IncusTab; label: string }[] = [
    { id: "profiles", label: "Profiles" },
    { id: "networks", label: "Networks" },
    { id: "storage", label: "Storage" },
    { id: "images", label: "Images" },
    { id: "project", label: "Project" },
  ];
</script>

<div class="tabbar">
  <div class="tabs">
    {#each tabs as t (t.id)}
      <button class:on={ui.incusTab === t.id} onclick={() => (ui.incusTab = t.id)}>{t.label}</button>
    {/each}
  </div>
</div>

{#if ui.incusTab === "profiles"}
  <IncusProfiles />
{:else if ui.incusTab === "networks"}
  <Networking />
{:else if ui.incusTab === "images"}
  <Images />
{:else if ui.incusTab === "storage"}
  <div class="content">
    <div class="card"><div class="empty"><div class="icon"><Icon name="store" size={24} /></div>
      Storage pools &amp; volumes — coming soon (<span class="mono">incus storage</span> / <span class="mono">incus storage volume</span>).</div></div>
  </div>
{:else if ui.incusTab === "project"}
  <div class="content">
    <div class="card"><div class="empty"><div class="icon"><Icon name="layers" size={24} /></div>
      Managed project info &amp; limits — coming soon (<span class="mono">incus project</span>).</div></div>
  </div>
{/if}

<style>
  .tabbar { border-bottom: 1px solid var(--border); background: var(--bg); }
  .tabs { display: flex; gap: 2px; max-width: 1180px; margin: 0 auto; padding: 0 24px; }
  .tabs button {
    border: none; background: transparent; cursor: pointer; font-family: inherit;
    font-size: 13px; font-weight: 500; color: var(--text-3);
    padding: 11px 12px; border-bottom: 2px solid transparent; margin-bottom: -1px;
  }
  .tabs button:hover { color: var(--text); }
  .tabs button.on { color: var(--text); border-bottom-color: var(--accent); }
</style>
