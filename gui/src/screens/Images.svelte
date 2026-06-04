<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { listImages, listAvailableImages } from "../lib/core";
  import type { ImageInfo } from "../lib/types";

  type Tab = "installed" | "available";
  let tab = $state<Tab>("installed");
  let query = $state("");
  let archFilter = $state<string>("all");
  let selectedFlavor = $state<string | null>(null); // null = show the distro picker

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
    selectedFlavor = null;
    if (t === "available" && !availableLoaded) {
      loading = true;
      error = null;
      try {
        available = await listAvailableImages();
        availableLoaded = true;
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

  const arches = $derived([...new Set(available.map((i) => i.arch))].sort());

  // Available images after the architecture filter (drives both the picker counts and the lists).
  const archImages = $derived(
    available.filter((i) => archFilter === "all" || i.arch === archFilter),
  );

  // Distro picker: one entry per flavor with a count (respecting the arch filter).
  const distros = $derived.by(() => {
    const m = new Map<string, number>();
    for (const i of archImages) m.set(i.flavor, (m.get(i.flavor) ?? 0) + 1);
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0])).map(([flavor, count]) => ({ flavor, count }));
  });

  const matches = (i: ImageInfo) =>
    `${i.name} ${i.base} ${i.arch} ${i.desc}`.toLowerCase().includes(query.toLowerCase());

  // What the available tab shows: search results (flat) > a selected distro's images > picker.
  const searchResults = $derived(query.trim() ? archImages.filter(matches).sort((a, b) => a.name.localeCompare(b.name)) : []);
  const flavorImages = $derived(
    selectedFlavor ? archImages.filter((i) => i.flavor === selectedFlavor).sort((a, b) => a.name.localeCompare(b.name) || a.arch.localeCompare(b.arch)) : [],
  );

  // Installed tab (flat, small).
  const installedShown = $derived(query.trim() ? installed.filter(matches) : installed);

  // Stable-ish color per distro badge.
  function hue(s: string): number {
    let h = 0;
    for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) % 360;
    return h;
  }
</script>

<div class="content">
  <div class="flex gap12 mb16 wrap">
    <div class="seg">
      <button class:on={tab === "installed"} onclick={() => selectTab("installed")}>Installed</button>
      <button class:on={tab === "available"} onclick={() => selectTab("available")}>All available</button>
    </div>
    <div class="code-chip" style="flex:1;max-width:300px">
      <Icon name="search" size={14} />
      <input class="bare" placeholder={tab === "available" ? "Search all distros…" : "Search images…"} bind:value={query} />
    </div>
    {#if tab === "available" && arches.length > 1}
      <select class="input arch" bind:value={archFilter}>
        <option value="all">All architectures</option>
        {#each arches as a}<option value={a}>{a}</option>{/each}
      </select>
    {/if}
    <button class="btn primary right"><Icon name="plus" size={14} /><span>Build image</span></button>
  </div>

  {#if tab === "installed"}
    <div class="card">
      {#if installedShown.length === 0}
        <div class="empty"><div class="icon"><Icon name="image" size={24} /></div>
          {query ? "No images match your search." : "No images cached yet — launch a sandbox to pull one."}</div>
      {:else}
        <table class="tbl">
          <thead><tr><th>Image</th><th>Base</th><th>Arch</th><th>Size</th><th>Used by</th><th>Updated</th></tr></thead>
          <tbody>
            {#each installedShown as img (img.name + img.arch)}
              <tr>
                <td><div class="strong mono small">{img.name}</div><div class="muted xsmall">{img.desc}</div></td>
                <td class="small t2">{img.base}</td><td class="mono small t2">{img.arch}</td>
                <td class="mono small">{img.size}</td><td class="mono small">{img.usedBy}</td><td class="small t2">{img.updated}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>

  {:else if loading}
    <div class="card"><div class="empty"><div class="icon"><Icon name="image" size={24} /></div>Fetching the remote catalog…</div></div>
  {:else if error}
    <div class="card"><div class="empty"><div class="icon"><Icon name="warn" size={24} /></div>Could not load remote images: {error}</div></div>

  {:else if query.trim()}
    <!-- search jumps straight to flat results across all distros -->
    <div class="hint mb12">{searchResults.length} matches across all distros</div>
    <div class="card">
      {#if searchResults.length === 0}
        <div class="empty"><div class="icon"><Icon name="image" size={24} /></div>No images match your search.</div>
      {:else}
        <table class="tbl">
          <thead><tr><th>Image</th><th>Base</th><th>Arch</th><th>Size</th><th>Updated</th></tr></thead>
          <tbody>
            {#each searchResults as img (img.name + img.arch)}
              <tr>
                <td><div class="strong mono small">{img.name}</div><div class="muted xsmall">{img.desc}</div></td>
                <td class="small t2">{img.base}</td><td class="mono small t2">{img.arch}</td>
                <td class="mono small">{img.size}</td><td class="small t2">{img.updated}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>

  {:else if selectedFlavor === null}
    <!-- step 1: distro picker -->
    <div class="hint mb12">
      {distros.length} distros · {archImages.length} container images{archFilter !== "all" ? ` (${archFilter})` : ""}.
      Pick a distro to see its releases. VM images excluded.
    </div>
    <div class="grid g-4">
      {#each distros as d (d.flavor)}
        <button class="distro" onclick={() => (selectedFlavor = d.flavor)}>
          <span class="badge2" style="background:hsl({hue(d.flavor)} 55% 42%)">{d.flavor.slice(0, 2)}</span>
          <span class="dinfo"><span class="dname">{d.flavor}</span><span class="dcount muted">{d.count} images</span></span>
          <span class="chev">›</span>
        </button>
      {/each}
    </div>

  {:else}
    <!-- step 2: a distro's images -->
    <div class="flex gap8 mb12">
      <button class="btn sm" onclick={() => (selectedFlavor = null)}>‹ All distros</button>
      <span class="crumb mono">{selectedFlavor}</span>
      <span class="hint">{flavorImages.length} images{archFilter !== "all" ? ` · ${archFilter}` : ""}</span>
    </div>
    <div class="card">
      <table class="tbl">
        <thead><tr><th>Image</th><th>Base</th><th>Arch</th><th>Size</th><th>Updated</th></tr></thead>
        <tbody>
          {#each flavorImages as img (img.name + img.arch)}
            <tr>
              <td><div class="strong mono small">{img.name}</div><div class="muted xsmall">{img.desc}</div></td>
              <td class="small t2">{img.base}</td><td class="mono small t2">{img.arch}</td>
              <td class="mono small">{img.size}</td><td class="small t2">{img.updated}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .bare { border: none; background: transparent; outline: none; color: var(--text); font-family: inherit; font-size: 12.5px; width: 100%; }
  .arch { width: auto; padding: 6px 10px; font-size: 12px; }
  .crumb { color: var(--text); font-weight: 600; }
  .distro {
    display: flex; align-items: center; gap: 11px; text-align: left; cursor: pointer; font-family: inherit;
    background: var(--card); border: 1px solid var(--border); border-radius: var(--radius); padding: 12px 14px;
    transition: border-color .12s, background .12s;
  }
  .distro:hover { border-color: var(--border-strong); background: var(--card-2); }
  .badge2 { width: 34px; height: 34px; border-radius: 8px; flex: none; display: grid; place-items: center; color: #fff; font-weight: 700; font-size: 12px; text-transform: capitalize; }
  .dinfo { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .dname { font-weight: 600; font-size: 13px; color: var(--text); }
  .dcount { font-size: 11px; }
  .chev { color: var(--text-3); font-size: 16px; }
</style>
