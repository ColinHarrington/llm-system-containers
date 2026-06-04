<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { listImages, listAvailableImages } from "../lib/core";
  import type { ImageInfo } from "../lib/types";

  type Tab = "installed" | "available";
  let tab = $state<Tab>("installed");
  let query = $state("");

  let installed = $state<ImageInfo[]>([]);
  let available = $state<ImageInfo[]>([]);
  let availableLoaded = $state(false);
  let loading = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    void (async () => { installed = await listImages(); })();
  });

  async function selectTab(t: Tab) {
    tab = t;
    query = "";
    if (t === "available" && !availableLoaded) {
      loading = true;
      error = null;
      try {
        available = await listAvailableImages();
        availableLoaded = true;
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    }
  }

  const source = $derived(tab === "installed" ? installed : available);
  const shown = $derived(
    query.trim() === ""
      ? source
      : source.filter((i) =>
          `${i.name} ${i.base} ${i.arch} ${i.desc}`.toLowerCase().includes(query.toLowerCase()),
        ),
  );
</script>

<div class="content">
  <div class="flex gap12 mb16 wrap">
    <div class="seg">
      <button class:on={tab === "installed"} onclick={() => selectTab("installed")}>Installed</button>
      <button class:on={tab === "available"} onclick={() => selectTab("available")}>All available</button>
    </div>
    <div class="code-chip" style="flex:1;max-width:360px">
      <Icon name="search" size={14} />
      <input class="bare" placeholder="Search images…" bind:value={query} />
    </div>
    <span class="hint">{shown.length}{query ? ` of ${source.length}` : ""} images</span>
    <button class="btn primary right"><Icon name="plus" size={14} /><span>Build image</span></button>
  </div>

  {#if tab === "available"}
    <div class="hint mb12">
      The full catalog from the <span class="mono">images:</span> remote (images.linuxcontainers.org).
      Launching a sandbox from one pulls and caches it into <span class="mono">Installed</span>.
    </div>
  {/if}

  <div class="card">
    {#if loading}
      <div class="empty"><div class="icon"><Icon name="image" size={24} /></div>Fetching the remote catalog…</div>
    {:else if error}
      <div class="empty"><div class="icon"><Icon name="warn" size={24} /></div>Could not load remote images: {error}</div>
    {:else if shown.length === 0}
      <div class="empty"><div class="icon"><Icon name="image" size={24} /></div>
        {tab === "installed" ? "No images cached yet — launch a sandbox to pull one." : "No images match your search."}
      </div>
    {:else}
      <table class="tbl">
        <thead><tr><th>Image</th><th>Base</th><th>Arch</th><th>Size</th>{#if tab === "installed"}<th>Used by</th>{/if}<th>Updated</th></tr></thead>
        <tbody>
          {#each shown as img (img.name + img.arch + img.updated)}
            <tr>
              <td><div class="strong mono small">{img.name}</div><div class="muted xsmall">{img.desc}</div></td>
              <td class="small t2">{img.base}</td>
              <td class="mono small t2">{img.arch}</td>
              <td class="mono small">{img.size}</td>
              {#if tab === "installed"}<td class="mono small">{img.usedBy}</td>{/if}
              <td class="small t2">{img.updated}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .bare { border: none; background: transparent; outline: none; color: var(--text); font-family: inherit; font-size: 12.5px; width: 100%; }
</style>
