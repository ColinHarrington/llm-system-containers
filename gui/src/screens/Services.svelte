<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { bump } from "../lib/store.svelte";
  import {
    listServices, setService, provisionService, listVirtualKeys,
    DEPLOYABLE_SERVICES, SERVICE_META,
  } from "../lib/core";
  import { ui } from "../lib/store.svelte";
  import type { ServiceEntry, VirtualKey } from "../lib/types";

  let services = $state<ServiceEntry[]>([]);
  let keys = $state<VirtualKey[]>([]);
  let busyName = $state<string | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    ui.dataVersion;
    void refresh();
  });

  async function refresh() {
    [services, keys] = await Promise.all([listServices(), listVirtualKeys()]);
  }

  async function toggle(s: ServiceEntry) {
    error = null;
    busyName = s.name;
    try { await setService(s.name, !s.enabled); bump(); }
    catch (e) { error = `${s.name}: ${e}`; }
    finally { busyName = null; }
  }

  async function provision(s: ServiceEntry) {
    error = null;
    busyName = s.name;
    try { await provisionService(s.name); }
    catch (e) { error = `${s.name}: ${e}`; }
    finally { busyName = null; }
  }

  const meta = (name: string) => SERVICE_META[name] ?? { initials: name.slice(0, 2), color: "#8a90a3", placement: "service" };
</script>

<div class="content">
  {#if error}
    <div class="banner warn mb16" role="alert"><Icon name="warn" size={18} /><span>{error}</span></div>
  {/if}

  <div class="grid g-2 mb16">
    {#each services as s (s.name)}
      <div class="card pad">
        <div class="flex gap12 mb8">
          <div class="svc-ico" style="background:{meta(s.name).color}">{meta(s.name).initials}</div>
          <div><div class="strong">{s.name}</div><div class="muted xsmall">{s.description}</div></div>
          {#if s.enabled}
            <span class="pill ok right"><span class="dot ok"></span> Enabled</span>
          {:else}
            <span class="pill right"><span class="dot muted"></span> Disabled</span>
          {/if}
        </div>
        <div class="flex gap8 wrap mb12">
          <span class="tag">{meta(s.name).placement}</span>
          <span class="tag mono">svc-{s.name}</span>
          <span class="tag">{s.priority}</span>
        </div>
        <div class="flex gap8">
          {#if s.enabled && DEPLOYABLE_SERVICES.has(s.name)}
            <button class="btn sm primary" onclick={() => provision(s)} disabled={busyName !== null}>
              <Icon name="play" size={14} /><span>Provision</span>
            </button>
          {/if}
          <button class="btn sm right" class:primary={!s.enabled} onclick={() => toggle(s)} disabled={busyName !== null}>
            {s.enabled ? "Disable" : "Enable"}
          </button>
        </div>
      </div>
    {/each}

    <div class="card pad" style="border-style:dashed;display:flex;flex-direction:column;justify-content:center;align-items:center;gap:6px;color:var(--text-3)">
      <div class="strong" style="color:var(--text)">Forgejo · NATS</div>
      <div class="xsmall">Optional / future services — enable in the wizard</div>
      <button class="btn sm mt8" onclick={() => (ui.screen = "wizard")}>Configure services</button>
    </div>
  </div>

  <!-- LiteLLM virtual keys -->
  <div class="card">
    <div class="card-head"><h3>LiteLLM virtual keys</h3><span class="sub">Per-agent keys — scoped, rotatable, revocable</span>
      <button class="btn sm right"><Icon name="plus" size={14} /><span>Issue key</span></button></div>
    <table class="tbl">
      <thead><tr><th>Key</th><th>Assigned to</th><th>Models</th><th>Budget</th><th>Used (24h)</th><th>Status</th></tr></thead>
      <tbody>
        {#each keys as k (k.key)}
          <tr>
            <td class="mono small">{k.key}</td>
            <td>{k.assignedTo}</td>
            <td class="small t2">{k.models}</td>
            <td class="mono small">{k.budget}</td>
            <td class="mono small">{k.used}</td>
            <td>
              {#if k.status === "active"}<span class="pill ok"><span class="dot ok"></span> active</span>
              {:else if k.status === "idle"}<span class="pill warn"><span class="dot warn"></span> idle</span>
              {:else}<span class="pill"><span class="dot muted"></span> revoked</span>{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>
