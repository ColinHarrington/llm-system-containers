<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { listImages, listAvailableImages } from "../lib/core";
  import type { ImageInfo } from "../lib/types";

  type Tab = "installed" | "available";
  let tab = $state<Tab>("installed");
  let query = $state("");
  let archFilter = $state<string>("all");

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
        // Default the arch filter to the VM's architecture (from an installed image) when possible.
        const vmArch = installed[0]?.arch;
        const arches = new Set(available.map((i) => i.arch));
        archFilter = vmArch && arches.has(vmArch) ? vmArch : "all";
      } catch (e) {
        error = String(e);
      } finally {
        loading = false;
      }
    }
  }

  const source = $derived(tab === "installed" ? installed : available);
  const arches = $derived([...new Set(available.map((i) => i.arch))].sort());

  const filtered = $derived(
    source
      .filter((i) => tab === "installed" || archFilter === "all" || i.arch === archFilter)
      .filter((i) =>
        query.trim() === ""
          ? true
          : `${i.name} ${i.base} ${i.arch} ${i.desc}`.toLowerCase().includes(query.toLowerCase()),
      ),
  );

  // Group the available catalog by flavor (distro), flavors sorted, images sorted within.
  const groups = $derived.by(() => {
    const m = new Map<string, ImageInfo[]>();
    for (const i of filtered) {
      const list = m.get(i.flavor) ?? [];
      list.push(i);
      m.set(i.flavor, list);
    }
    return [...m.entries()]
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([flavor, items]) => ({
        flavor,
        items: items.sort((a, b) => a.name.localeCompare(b.name) || a.arch.localeCompare(b.arch)),
      }));
  });
</script>

<div class="content">
  <div class="flex gap12 mb16 wrap">
    <div class="seg">
      <button class:on={tab === "installed"} onclick={() => selectTab("installed")}>Installed</button>
      <button class:on={tab === "available"} onclick={() => selectTab("available")}>All available</button>
    </div>
    <div class="code-chip" style="flex:1;max-width:320px">
      <Icon name="search" size={14} />
      <input class="bare" placeholder="Search images…" bind:value={query} />
    </div>
    {#if tab === "available" && arches.length > 1}
      <select class="input arch" bind:value={archFilter}>
        <option value="all">All architectures</option>
        {#each arches as a}<option value={a}>{a}</option>{/each}
      </select>
    {/if}
    <span class="hint">{filtered.length}{query || archFilter !== "all" ? ` of ${source.length}` : ""} images</span>
    <button class="btn primary right"><Icon name="plus" size={14} /><span>Build image</span></button>
  </div>

  {#if tab === "available"}
    <div class="hint mb12">
      Container images from the <span class="mono">images:</span> remote (images.linuxcontainers.org), grouped by distro.
      Launching a sandbox from one pulls and caches it into <span class="mono">Installed</span>. (VM images excluded.)
    </div>
  {/if}

  <div class="card">
    {#if loading}
      <div class="empty"><div class="icon"><Icon name="image" size={24} /></div>Fetching the remote catalog…</div>
    {:else if error}
      <div class="empty"><div class="icon"><Icon name="warn" size={24} /></div>Could not load remote images: {error}</div>
    {:else if filtered.length === 0}
      <div class="empty"><div class="icon"><Icon name="image" size={24} /></div>
        {tab === "installed" ? "No images cached yet — launch a sandbox to pull one." : "No images match your filters."}
      </div>
    {:else if tab === "installed"}
      <table class="tbl">
        <thead><tr><th>Image</th><th>Base</th><th>Arch</th><th>Size</th><th>Used by</th><th>Updated</th></tr></thead>
        <tbody>
          {#each filtered as img (img.name + img.arch)}
            <tr>
              <td><div class="strong mono small">{img.name}</div><div class="muted xsmall">{img.desc}</div></td>
              <td class="small t2">{img.base}</td>
              <td class="mono small t2">{img.arch}</td>
              <td class="mono small">{img.size}</td>
              <td class="mono small">{img.usedBy}</td>
              <td class="small t2">{img.updated}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <table class="tbl">
        <thead><tr><th>Image</th><th>Base</th><th>Arch</th><th>Size</th><th>Updated</th></tr></thead>
        <tbody>
          {#each groups as g (g.flavor)}
            <tr class="grouphead"><td colspan="5">{g.flavor} <span class="muted">· {g.items.length}</span></td></tr>
            {#each g.items as img (img.name + img.arch)}
              <tr>
                <td><div class="strong mono small">{img.name}</div><div class="muted xsmall">{img.desc}</div></td>
                <td class="small t2">{img.base}</td>
                <td class="mono small t2">{img.arch}</td>
                <td class="mono small">{img.size}</td>
                <td class="small t2">{img.updated}</td>
              </tr>
            {/each}
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .bare { border: none; background: transparent; outline: none; color: var(--text); font-family: inherit; font-size: 12.5px; width: 100%; }
  .arch { width: auto; padding: 6px 10px; font-size: 12px; }
  .grouphead td { background: var(--card-2); font-weight: 600; color: var(--text); font-size: 11px; text-transform: uppercase; letter-spacing: .04em; padding: 7px 14px; position: sticky; top: 0; }
</style>
