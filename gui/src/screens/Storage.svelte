<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui } from "../lib/store.svelte";
  import { listStorage } from "../lib/core";
  import type { StoragePoolInfo } from "../lib/types";

  let pools = $state<StoragePoolInfo[]>([]);
  $effect(() => {
    ui.dataVersion;
    void (async () => { pools = await listStorage(); })();
  });
</script>

<div class="content">
  <div class="hint mb16">Incus storage pools back every sandbox's root disk; <span class="mono">custom</span> volumes hold shared/persistent data (<span class="mono">incus storage</span> / <span class="mono">incus storage volume</span>).</div>

  <div class="grid g-2">
    {#each pools as p (p.name)}
      <div class="card pad">
        <div class="flex gap10 mb8">
          <div class="pico"><Icon name="store" size={16} /></div>
          <div><div class="strong mono" style="color:var(--text)">{p.name}</div><div class="muted xsmall">{p.description || "storage pool"}</div></div>
          <span class="tag right">{p.driver}</span>
        </div>
        <div class="kv"><span class="k">Used by</span><span class="v mono small">{p.usedBy}</span></div>
        {#each Object.entries(p.config) as [k, v]}
          <div class="kv"><span class="k mono">{k}</span><span class="v mono small">{v}</span></div>
        {/each}

        {#if p.volumes.length > 0}
          <div class="sub2 mt12">Custom volumes</div>
          <table class="tbl mini">
            <tbody>
              {#each p.volumes as vol (vol.name)}
                <tr>
                  <td class="mono small strong" style="color:var(--text)">{vol.name}</td>
                  <td class="mono small t2">{vol.config.size ?? "—"}</td>
                  <td class="mono small t2" style="text-align:right">{vol.usedBy} used</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <div class="muted small mt8">no custom volumes</div>
        {/if}
      </div>
    {/each}
    {#if pools.length === 0}
      <div class="card"><div class="empty"><div class="icon"><Icon name="store" size={24} /></div>No storage pools — bring the VM up.</div></div>
    {/if}
  </div>
</div>

<style>
  .pico { width: 32px; height: 32px; border-radius: 8px; background: var(--accent-dim); color: var(--accent-text); display: grid; place-items: center; flex: none; }
  .sub2 { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: .05em; color: var(--text-3); margin-bottom: 6px; }
  .tbl.mini td { padding: 6px 0; border-bottom: 1px solid var(--border); }
  .tbl.mini tr:last-child td { border-bottom: none; }
</style>
