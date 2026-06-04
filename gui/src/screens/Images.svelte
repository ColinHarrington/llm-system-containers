<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { listImages } from "../lib/core";
  import type { ImageInfo } from "../lib/types";

  let images = $state<ImageInfo[]>([]);
  $effect(() => {
    void (async () => { images = await listImages(); })();
  });
</script>

<div class="content">
  <div class="flex mb16">
    <div class="hint">Base distros and custom images with pre-packaged tooling, browsers and runtimes.</div>
    <button class="btn primary right"><Icon name="plus" /><span>Build image</span></button>
  </div>
  <div class="card">
    {#if images.length === 0}
      <div class="empty"><div class="icon"><Icon name="image" size={24} /></div>No images cached yet — launch a sandbox to pull one.</div>
    {:else}
    <table class="tbl">
      <thead><tr><th>Image</th><th>Base</th><th>Size</th><th>Tooling</th><th>Used by</th><th>Updated</th></tr></thead>
      <tbody>
        {#each images as img (img.name)}
          <tr>
            <td><div class="strong mono small">{img.name}</div><div class="muted xsmall">{img.desc}</div></td>
            <td class="small t2">{img.base}</td>
            <td class="mono small">{img.size}</td>
            <td class="small t2">{img.tooling}</td>
            <td class="mono small">{img.usedBy}</td>
            <td class="small t2">{img.updated}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    {/if}
  </div>
</div>
