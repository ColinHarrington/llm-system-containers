<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui, bump, showToast } from "../lib/store.svelte";
  import { listIncusProfiles, starterIncusProfiles, applyIncusProfile } from "../lib/core";
  import type { IncusProfileInfo } from "../lib/types";

  let profiles = $state<IncusProfileInfo[]>([]);
  let starters = $state<IncusProfileInfo[]>([]);
  let busy = $state<string | null>(null);

  $effect(() => {
    ui.dataVersion;
    void (async () => { profiles = await listIncusProfiles(); })();
  });
  $effect(() => {
    void starterIncusProfiles().then((s) => (starters = s));
  });

  const existingNames = $derived(new Set(profiles.map((p) => p.name)));

  async function apply(name: string) {
    busy = name;
    showToast(`$ incus profile reconcile ${name}`);
    try {
      await applyIncusProfile(name);
      showToast(`Applied profile '${name}'`, "ok");
      bump();
    } catch (e) {
      showToast(String(e), "danger");
    } finally { busy = null; }
  }
</script>

<div class="content">
  <div class="banner info mb16">
    <Icon name="layers" size={18} />
    <span>
      An Incus profile is a reusable bundle of <strong>config + devices</strong> composed onto
      sandboxes (applied in order; later overrides earlier). The composition layer — distinct from
      <strong>agent profiles</strong> (permission seeds). TOML-owned management + reconcile is next.
    </span>
  </div>

  {#if starters.some((s) => !existingNames.has(s.name))}
    <div class="sec">Recommended <span class="hint">— apply into the project (TOML-owned, reconciled)</span></div>
    <div class="grid g-3 mb20">
      {#each starters.filter((s) => !existingNames.has(s.name)) as s (s.name)}
        <div class="card pad rec">
          <div><div class="strong mono" style="color:var(--text)">{s.name}</div><div class="muted xsmall">{s.description}</div></div>
          <button class="btn sm primary right" disabled={busy !== null} onclick={() => apply(s.name)}>
            <Icon name="plus" size={13} /><span>{busy === s.name ? "Applying…" : "Apply"}</span></button>
        </div>
      {/each}
    </div>
  {/if}

  <div class="sec">In the project</div>
  <div class="grid g-2">
    {#each profiles as p (p.name)}
      <div class="card pad">
        <div class="flex gap10 mb8">
          <div class="pico"><Icon name="layers" size={16} /></div>
          <div><div class="strong mono" style="color:var(--text)">{p.name}</div>
            <div class="muted xsmall">{p.description || "—"}</div></div>
          <span class="tag right">{p.usedBy} {p.usedBy === 1 ? "instance" : "instances"}</span>
        </div>

        {#if Object.keys(p.devices).length > 0}
          <div class="sub2">Devices</div>
          <div class="kvs mb12">
            {#each Object.entries(p.devices) as [dname, dev]}
              <div class="kvline"><span class="kk mono">{dname}</span><span class="vv mono">{dev.type ?? "?"}{dev.network ? ` · ${dev.network}` : ""}{dev.path ? ` · ${dev.path}` : ""}</span></div>
            {/each}
          </div>
        {/if}
        {#if Object.keys(p.config).length > 0}
          <div class="sub2">Config</div>
          <div class="kvs">
            {#each Object.entries(p.config) as [k, v]}
              <div class="kvline"><span class="kk mono">{k}</span><span class="vv mono">{v}</span></div>
            {/each}
          </div>
        {/if}
        {#if Object.keys(p.devices).length === 0 && Object.keys(p.config).length === 0}
          <div class="muted small">empty</div>
        {/if}
      </div>
    {/each}
    {#if profiles.length === 0}
      <div class="card"><div class="empty"><div class="icon"><Icon name="layers" size={24} /></div>No Incus profiles — bring the VM up.</div></div>
    {/if}
  </div>
</div>

<style>
  .pico { width: 32px; height: 32px; border-radius: 8px; background: var(--accent-dim); color: var(--accent-text); display: grid; place-items: center; flex: none; }
  .sub2 { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: .05em; color: var(--text-3); margin-bottom: 6px; }
  .kvs { display: flex; flex-direction: column; gap: 3px; }
  .kvline { display: flex; gap: 10px; font-size: 11.5px; }
  .kk { color: var(--text-3); min-width: 130px; }
  .vv { color: var(--text); overflow-wrap: anywhere; }
  .sec { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: .05em; color: var(--text-3); margin-bottom: 10px; }
  .rec { display: flex; align-items: center; gap: 10px; }
</style>
