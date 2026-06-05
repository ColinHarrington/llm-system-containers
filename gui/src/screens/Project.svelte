<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui } from "../lib/store.svelte";
  import { listProjects } from "../lib/core";
  import type { ProjectInfo } from "../lib/types";

  let projects = $state<ProjectInfo[]>([]);
  $effect(() => {
    ui.dataVersion;
    void (async () => { projects = await listProjects(); })();
  });

  const groups: { label: string; prefix: string }[] = [
    { label: "Features", prefix: "features." },
    { label: "Limits", prefix: "limits." },
    { label: "Restrictions", prefix: "restricted." },
  ];
  function entries(config: Record<string, string>, prefix: string) {
    return Object.entries(config).filter(([k]) => k.startsWith(prefix));
  }
  function other(config: Record<string, string>) {
    return Object.entries(config).filter(([k]) => !groups.some((g) => k.startsWith(g.prefix)));
  }
</script>

<div class="content">
  <div class="hint mb16">Sandboxes, profiles and images live in an Incus <span class="mono">project</span>; its config sets feature isolation, resource limits, and restrictions for everything inside.</div>

  <div class="grid g-2">
    {#each projects as p (p.name)}
      <div class="card pad">
        <div class="flex gap10 mb8">
          <div class="pico"><Icon name="layers" size={16} /></div>
          <div><div class="strong mono" style="color:var(--text)">{p.name}</div><div class="muted xsmall">{p.description || "project"}</div></div>
          <span class="tag right">{p.usedBy} used</span>
        </div>

        {#each groups as g}
          {#if entries(p.config, g.prefix).length > 0}
            <div class="sub2 mt12">{g.label}</div>
            <div class="kvs mb8">
              {#each entries(p.config, g.prefix) as [k, v]}
                <div class="kvline"><span class="kk mono">{k.slice(g.prefix.length)}</span><span class="vv mono">{v}</span></div>
              {/each}
            </div>
          {/if}
        {/each}
        {#if other(p.config).length > 0}
          <div class="sub2 mt12">Other</div>
          <div class="kvs">
            {#each other(p.config) as [k, v]}
              <div class="kvline"><span class="kk mono">{k}</span><span class="vv mono">{v}</span></div>
            {/each}
          </div>
        {/if}
        {#if Object.keys(p.config).length === 0}
          <div class="muted small">default settings</div>
        {/if}
      </div>
    {/each}
    {#if projects.length === 0}
      <div class="card"><div class="empty"><div class="icon"><Icon name="layers" size={24} /></div>No projects — bring the VM up.</div></div>
    {/if}
  </div>
</div>

<style>
  .pico { width: 32px; height: 32px; border-radius: 8px; background: var(--accent-dim); color: var(--accent-text); display: grid; place-items: center; flex: none; }
  .sub2 { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: .05em; color: var(--text-3); margin-bottom: 6px; }
  .kvs { display: flex; flex-direction: column; gap: 3px; }
  .kvline { display: flex; gap: 10px; font-size: 11.5px; }
  .kk { color: var(--text-3); min-width: 150px; }
  .vv { color: var(--text); overflow-wrap: anywhere; }
</style>
