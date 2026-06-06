<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Modal from "../lib/Modal.svelte";
  import Skeleton from "../lib/Skeleton.svelte";
  import { bump } from "../lib/store.svelte";
  import {
    listServices, setService, provisionService, listVirtualKeys, syncVirtualKeys, setProviderKey,
    serviceStates, restartService, stopService, SERVICE_PORTS, DEPLOYABLE_SERVICES, SERVICE_META,
  } from "../lib/core";
  import { ui, live, showToast } from "../lib/store.svelte";
  import type { ServiceEntry, ServiceState, VirtualKey } from "../lib/types";

  let services = $state<ServiceEntry[]>([]);
  let keys = $state<VirtualKey[]>([]);
  let states = $state<Record<string, ServiceState>>({});
  let loading = $state(true);
  let busyName = $state<string | null>(null);
  let keysBusy = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    ui.dataVersion;
    live.tick; // auto-refresh on the live poll
    void refresh();
  });

  async function refresh() {
    try { [services, keys] = await Promise.all([listServices(), listVirtualKeys()]); }
    finally { loading = false; }
    void serviceStates().then((s) => (states = s)).catch(() => (states = {}));
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
    try { await provisionService(s.name); await refresh(); }
    catch (e) { error = `${s.name}: ${e}`; }
    finally { busyName = null; }
  }

  // Service detail drawer + lifecycle actions.
  let detail = $state<ServiceEntry | null>(null);
  let detailBusy = $state(false);
  async function lifecycle(name: string, fn: () => Promise<void>, msg: string) {
    detailBusy = true;
    showToast(`$ ${msg}`);
    try {
      await fn();
      showToast("Done", "ok");
      await refresh();
    } catch (e) {
      showToast(String(e), "danger");
    } finally { detailBusy = false; }
  }

  async function syncKeys() {
    keysBusy = true;
    showToast("$ llmsctl keys sync");
    try {
      const n = await syncVirtualKeys();
      showToast(n === 0 ? "No agent keys to sync" : `Synced ${n} virtual key(s)`, "ok");
      await refresh();
    } catch (e) {
      showToast(String(e), "danger");
    } finally { keysBusy = false; }
  }

  // Provider key (real upstream credential — injected only into the LiteLLM container).
  let provider = $state("openai");
  let providerKey = $state("");
  async function saveProviderKey() {
    if (!providerKey.trim()) return;
    keysBusy = true;
    showToast(`$ llmsctl keys set-provider ${provider} ****`);
    try {
      await setProviderKey(provider, providerKey.trim());
      providerKey = "";
      showToast("Provider key set (stored only in the LiteLLM container)", "ok");
    } catch (e) {
      showToast(String(e), "danger");
    } finally { keysBusy = false; }
  }

  const meta = (name: string) => SERVICE_META[name] ?? { initials: name.slice(0, 2), color: "#8a90a3", placement: "service" };
</script>

<div class="content">
  {#if error}
    <div class="banner warn mb16" role="alert"><Icon name="warn" size={18} /><span>{error}</span></div>
  {/if}

  <div class="grid g-2 mb16">
    {#if loading}
      {#each Array(4) as _, i (i)}
        <div class="card pad">
          <div class="flex gap12 mb8"><Skeleton w="34px" h={34} r={9} /><div style="flex:1"><Skeleton w="40%" h={13} mb={6} /><Skeleton w="65%" h={10} /></div></div>
          <Skeleton w="80%" h={20} mb={12} /><Skeleton w="50%" h={26} />
        </div>
      {/each}
    {:else}
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
          {#if states[s.name] === "running"}
            <span class="pill ok"><span class="dot ok pulse"></span> running</span>
          {:else if states[s.name] === "stopped"}
            <span class="pill warn"><span class="dot warn"></span> stopped</span>
          {:else if states[s.name] === "not-provisioned"}
            <span class="pill"><span class="dot muted"></span> not provisioned</span>
          {/if}
        </div>
        <div class="flex gap8">
          {#if s.enabled && DEPLOYABLE_SERVICES.has(s.name)}
            <button class="btn sm primary" onclick={() => provision(s)} disabled={busyName !== null}>
              <Icon name="play" size={14} /><span>Provision</span>
            </button>
          {/if}
          {#if DEPLOYABLE_SERVICES.has(s.name)}
            <button class="btn sm" onclick={() => (detail = s)} title="Service detail + lifecycle"><Icon name="cog" size={14} /></button>
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
    {/if}
  </div>

  <!-- LiteLLM virtual keys -->
  <div class="card">
    <div class="card-head"><h3>LiteLLM virtual keys</h3><span class="sub">Per-agent keys compiled from each agent's <span class="mono">llm_budget</span> guardrail</span>
      <button class="btn sm primary right" disabled={keysBusy} onclick={syncKeys} title="Mint/refresh the compiled keys against the running LiteLLM proxy">
        <Icon name="key" size={14} /><span>{keysBusy ? "Syncing…" : "Sync keys"}</span></button></div>
    <div class="pad" style="border-bottom:1px solid var(--border)">
      <div class="sub2">Provider key</div>
      <p class="xsmall muted mb8">The real upstream credential. Injected only into the LiteLLM container — never written to <span class="mono">llmsc.toml</span>. Agents only ever see virtual keys.</p>
      <div class="flex gap8" style="align-items:center;flex-wrap:wrap">
        <select class="input" style="max-width:140px" bind:value={provider}>
          <option value="openai">openai</option>
          <option value="anthropic">anthropic</option>
        </select>
        <input class="input mono" style="max-width:320px" type="password" bind:value={providerKey} placeholder="sk-… (provider API key)" />
        <button class="btn sm" disabled={keysBusy || !providerKey.trim()} onclick={saveProviderKey}>Set provider key</button>
      </div>
    </div>
    {#if keys.length === 0}
      <div class="empty"><div class="icon"><Icon name="key" size={22} /></div>No agents with virtual keys yet.</div>
    {:else}
    <table class="tbl">
      <thead><tr><th>Key alias</th><th>Assigned to</th><th>Models</th><th>Budget</th><th>Used</th><th>Status</th></tr></thead>
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
              {:else if k.status === "planned"}<span class="pill" title="Compiled from guardrails; not yet synced to the proxy"><span class="dot muted"></span> planned</span>
              {:else}<span class="pill"><span class="dot muted"></span> revoked</span>{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    {/if}
    <p class="xsmall muted" style="padding:0 14px 12px">Usage metering is not instrumented yet; keys show as <span class="mono">planned</span> until synced to a running proxy.</p>
  </div>
</div>

{#if detail}
  <Modal title={`Service · ${detail.name}`} maxWidth={460} onclose={() => (detail = null)}>
    {#snippet body()}
      <div class="kv"><span class="k">Container</span><span class="v mono">svc-{detail!.name}</span></div>
      <div class="kv"><span class="k">State</span><span class="v">
        {#if states[detail!.name] === "running"}<span class="pill ok"><span class="dot ok"></span> running</span>
        {:else if states[detail!.name] === "stopped"}<span class="pill warn"><span class="dot warn"></span> stopped</span>
        {:else}<span class="pill"><span class="dot muted"></span> not provisioned</span>{/if}
      </span></div>
      <div class="kv"><span class="k">Port</span><span class="v mono">{SERVICE_PORTS[detail!.name] || "—"}</span></div>
      <div class="kv"><span class="k">Placement</span><span class="v">{meta(detail!.name).placement}</span></div>
      <p class="hint mt12">{detail!.description}</p>
    {/snippet}
    {#snippet foot()}
      <button class="btn" onclick={() => (detail = null)}>Close</button>
      {#if states[detail!.name] === "not-provisioned"}
        <button class="btn primary" disabled={detailBusy} onclick={() => detail && lifecycle(detail.name, () => provisionService(detail!.name), `llmsctl services up ${detail!.name}`)}>
          <Icon name="play" size={15} /><span>Provision</span></button>
      {:else}
        <button class="btn" disabled={detailBusy} onclick={() => detail && lifecycle(detail.name, () => stopService(detail!.name), `incus stop svc-${detail!.name}`)}>Stop</button>
        <button class="btn primary" disabled={detailBusy} onclick={() => detail && lifecycle(detail.name, () => restartService(detail!.name), `incus restart svc-${detail!.name}`)}>
          <Icon name="play" size={15} /><span>Restart</span></button>
      {/if}
    {/snippet}
  </Modal>
{/if}
