<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Skeleton from "../lib/Skeleton.svelte";
  import FetchError from "../lib/FetchError.svelte";
  import SortHeader from "../lib/SortHeader.svelte";
  import { ui, live, openSandbox, showToast, bump } from "../lib/store.svelte";
  import { fleetEnforcement, enforceAll } from "../lib/core";
  import type { FleetEnforcement } from "../lib/types";

  let fleet = $state<FleetEnforcement[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let busyRow = $state<string | null>(null);
  let bulkBusy = $state(false);

  const managedSandboxes = $derived(fleet.filter((f) => f.egressPosture !== "unmanaged" && f.egressPosture !== "open"));

  async function enforceRow(name: string) {
    busyRow = name;
    showToast(`$ llmsctl enforce ${name}`);
    try { await enforceAll(name); showToast(`Enforced ${name}`, "ok"); bump(); }
    catch (e) { showToast(String(e), "danger"); }
    finally { busyRow = null; }
  }

  async function enforceFleet() {
    const targets = managedSandboxes.map((f) => f.sandbox);
    if (targets.length === 0) return;
    bulkBusy = true;
    showToast(`Enforcing ${targets.length} sandbox(es)…`);
    let ok = 0;
    for (const name of targets) {
      busyRow = name;
      try { await enforceAll(name); ok++; }
      catch { /* keep going; reported in summary */ }
    }
    busyRow = null;
    bulkBusy = false;
    showToast(ok === targets.length ? `Enforced all ${ok} sandbox(es)` : `Enforced ${ok}/${targets.length} (some failed)`, ok === targets.length ? "ok" : "warn");
    bump();
  }

  $effect(() => {
    ui.dataVersion;
    live.tick; // auto-refresh on the live poll
    void load();
  });
  async function load() {
    try { fleet = await fleetEnforcement(); loadError = null; }
    catch (e) { loadError = String(e); }
    finally { loading = false; }
  }

  const managed = $derived(fleet.filter((f) => f.egressPosture !== "unmanaged" && f.egressPosture !== "open").length);
  const totalAgents = $derived(fleet.reduce((n, f) => n + f.agents, 0));
  const roAgents = $derived(fleet.reduce((n, f) => n + f.readOnlyAgents, 0));
  const cpAgents = $derived(fleet.reduce((n, f) => n + f.controlPlaneAgents, 0));
  const withDomains = $derived(fleet.filter((f) => f.domains > 0).length);

  const posturePill = (p: string) =>
    p === "allowlist" || p === "deny-all" ? "ok" : p === "open" ? "warn" : "";

  let query = $state("");
  let pfilter = $state<"all" | "managed" | "unmanaged">("all");

  type SortKey = "sandbox" | "egressPosture" | "domains" | "agents" | "readOnlyAgents" | "controlPlaneAgents";
  let sort = $state<{ key: SortKey; dir: 1 | -1 }>({ key: "sandbox", dir: 1 });
  function toggleSort(key: string) {
    const k = key as SortKey;
    if (sort.key === k) sort.dir = sort.dir === 1 ? -1 : 1;
    else { sort.key = k; sort.dir = 1; }
  }

  const shown = $derived(
    fleet
      .filter((f) => f.sandbox.toLowerCase().includes(query.toLowerCase()))
      .filter((f) => {
        const managed = f.egressPosture !== "unmanaged" && f.egressPosture !== "open";
        return pfilter === "all" || (pfilter === "managed" ? managed : !managed);
      })
      .slice()
      .sort((a, b) => {
        const av = a[sort.key];
        const bv = b[sort.key];
        const cmp = typeof av === "number" && typeof bv === "number" ? av - bv : String(av).localeCompare(String(bv));
        return cmp * sort.dir;
      }),
  );
</script>

<div class="content">
  {#if loadError}
    <FetchError message={loadError} onretry={() => { loading = true; void load(); }} busy={loading} />
  {/if}
  <p class="hint mb16">Configured enforcement intent across every config-managed sandbox. Per-container egress ACLs (L3/L4) + mitmproxy domains (L7) + per-UID Tetragon policies all compile from each agent's guardrails. Open a sandbox for its live ring status and to refine policy.</p>

  <!-- Aggregate -->
  <div class="grid g-4 mb16">
    <div class="card pad stat"><div class="label">Managed egress</div><div class="num">{managed} <small>/ {fleet.length}</small></div><div class="delta t2">sandboxes with a policy</div></div>
    <div class="card pad stat"><div class="label">L7 domain allowlists</div><div class="num">{withDomains}</div><div class="delta t2">via mitmproxy</div></div>
    <div class="card pad stat"><div class="label">Read-only filesystem</div><div class="num">{roAgents} <small>/ {totalAgents}</small></div><div class="delta t2">agents</div></div>
    <div class="card pad stat"><div class="label">Control-plane caps</div><div class="num">{cpAgents}</div><div class="delta t2">privileged agents</div></div>
  </div>

  {#if !loading && fleet.length > 0}
    <div class="flex gap12 mb16 wrap">
      <div class="code-chip" style="flex:1;max-width:360px"><Icon name="search" size={16} /><input class="bare" placeholder="Search sandboxes…" bind:value={query} /></div>
      <div class="seg right">
        {#each [["all", "All"], ["managed", "Managed"], ["unmanaged", "Unmanaged"]] as [v, label]}
          <button class:on={pfilter === v} onclick={() => (pfilter = v as typeof pfilter)}>{label}</button>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Matrix -->
  <div class="card">
    <div class="card-head"><h3>Per-sandbox posture</h3><span class="sub">configured intent · <span class="mono">llmsctl doctor</span></span>
      <button class="btn sm primary right" disabled={bulkBusy || managedSandboxes.length === 0} onclick={enforceFleet}
        title="Apply every ring for every managed sandbox">
        <Icon name="shield" size={13} /><span>{bulkBusy ? "Enforcing…" : `Enforce all (${managedSandboxes.length})`}</span></button>
    </div>
    {#if loading}
      <div class="pad"><Skeleton w="100%" h={18} mb={10} /><Skeleton w="100%" h={18} mb={10} /><Skeleton w="80%" h={18} /></div>
    {:else if fleet.length === 0}
      <div class="empty">
        <div class="icon"><Icon name="shield" size={24} /></div>
        No config-managed sandboxes yet — create one to start enforcing policy.
        <button class="btn sm primary mt12" onclick={() => (ui.newSandboxOpen = true)}><Icon name="plus" size={14} /><span>New sandbox</span></button>
      </div>
    {:else if shown.length === 0}
      <div class="empty"><div class="icon"><Icon name="search" size={22} /></div>No sandboxes match the current filter.</div>
    {:else}
      <table class="tbl">
        <thead><tr>
          <SortHeader label="Sandbox" col="sandbox" {sort} onsort={toggleSort} />
          <SortHeader label="Egress (L3/L4)" col="egressPosture" {sort} onsort={toggleSort} />
          <SortHeader label="Domains (L7)" col="domains" {sort} onsort={toggleSort} />
          <SortHeader label="Agents" col="agents" {sort} onsort={toggleSort} />
          <SortHeader label="RO fs" col="readOnlyAgents" {sort} onsort={toggleSort} />
          <SortHeader label="Control-plane" col="controlPlaneAgents" {sort} onsort={toggleSort} />
          <th></th>
        </tr></thead>
        <tbody>
          {#each shown as f (f.sandbox)}
            <tr class="clickable" onclick={() => openSandbox(f.sandbox)}>
              <td class="mono small strong" style="color:var(--text)">{f.sandbox}</td>
              <td><span class="pill {posturePill(f.egressPosture)}">{f.egressPosture}</span></td>
              <td class="mono small">{f.domains || "—"}</td>
              <td class="mono small">{f.agents}</td>
              <td>{#if f.readOnlyAgents > 0}<span class="pill ok">{f.readOnlyAgents}</span>{:else}<span class="muted small">—</span>{/if}</td>
              <td>{#if f.controlPlaneAgents > 0}<span class="pill warn">{f.controlPlaneAgents}</span>{:else}<span class="muted small">none</span>{/if}</td>
              <td style="text-align:right; white-space:nowrap">
                {#if f.egressPosture !== "unmanaged" && f.egressPosture !== "open"}
                  <button class="btn sm" disabled={busyRow === f.sandbox || bulkBusy} title="Apply every ring for this sandbox" onclick={(e) => { e.stopPropagation(); enforceRow(f.sandbox); }}>{busyRow === f.sandbox ? "…" : "Enforce"}</button>
                {/if}
                <button class="btn sm" onclick={(e) => { e.stopPropagation(); openSandbox(f.sandbox); }}>Open ›</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <p class="xsmall muted mt12">Egress "open" or "unmanaged" means no ACL is applied — surfaced in amber so it is easy to spot. Enforcement compiles from intent; a sandbox's detail page applies it and shows live status.</p>
</div>
