<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Skeleton from "../lib/Skeleton.svelte";
  import { ui, live, openSandbox } from "../lib/store.svelte";
  import { fleetEnforcement } from "../lib/core";
  import type { FleetEnforcement } from "../lib/types";

  let fleet = $state<FleetEnforcement[]>([]);
  let loading = $state(true);

  $effect(() => {
    ui.dataVersion;
    live.tick; // auto-refresh on the live poll
    void fleetEnforcement().then((f) => (fleet = f)).catch(() => (fleet = [])).finally(() => (loading = false));
  });

  const managed = $derived(fleet.filter((f) => f.egressPosture !== "unmanaged" && f.egressPosture !== "open").length);
  const totalAgents = $derived(fleet.reduce((n, f) => n + f.agents, 0));
  const roAgents = $derived(fleet.reduce((n, f) => n + f.readOnlyAgents, 0));
  const cpAgents = $derived(fleet.reduce((n, f) => n + f.controlPlaneAgents, 0));
  const withDomains = $derived(fleet.filter((f) => f.domains > 0).length);

  const posturePill = (p: string) =>
    p === "allowlist" || p === "deny-all" ? "ok" : p === "open" ? "warn" : "";
</script>

<div class="content">
  <p class="hint mb16">Configured enforcement intent across every config-managed sandbox. Per-container egress ACLs (L3/L4) + mitmproxy domains (L7) + per-UID Tetragon policies all compile from each agent's guardrails. Open a sandbox for its live ring status and to refine policy.</p>

  <!-- Aggregate -->
  <div class="grid g-4 mb16">
    <div class="card pad stat"><div class="label">Managed egress</div><div class="num">{managed} <small>/ {fleet.length}</small></div><div class="delta t2">sandboxes with a policy</div></div>
    <div class="card pad stat"><div class="label">L7 domain allowlists</div><div class="num">{withDomains}</div><div class="delta t2">via mitmproxy</div></div>
    <div class="card pad stat"><div class="label">Read-only filesystem</div><div class="num">{roAgents} <small>/ {totalAgents}</small></div><div class="delta t2">agents</div></div>
    <div class="card pad stat"><div class="label">Control-plane caps</div><div class="num">{cpAgents}</div><div class="delta t2">privileged agents</div></div>
  </div>

  <!-- Matrix -->
  <div class="card">
    <div class="card-head"><h3>Per-sandbox posture</h3><span class="sub">configured intent · <span class="mono">llmsctl doctor</span></span></div>
    {#if loading}
      <div class="pad"><Skeleton w="100%" h={18} mb={10} /><Skeleton w="100%" h={18} mb={10} /><Skeleton w="80%" h={18} /></div>
    {:else if fleet.length === 0}
      <div class="empty">
        <div class="icon"><Icon name="shield" size={24} /></div>
        No config-managed sandboxes yet — create one to start enforcing policy.
        <button class="btn sm primary mt12" onclick={() => (ui.newSandboxOpen = true)}><Icon name="plus" size={14} /><span>New sandbox</span></button>
      </div>
    {:else}
      <table class="tbl">
        <thead><tr><th>Sandbox</th><th>Egress (L3/L4)</th><th>Domains (L7)</th><th>Agents</th><th>RO fs</th><th>Control-plane</th><th></th></tr></thead>
        <tbody>
          {#each fleet as f (f.sandbox)}
            <tr class="clickable" onclick={() => openSandbox(f.sandbox)}>
              <td class="mono small strong" style="color:var(--text)">{f.sandbox}</td>
              <td><span class="pill {posturePill(f.egressPosture)}">{f.egressPosture}</span></td>
              <td class="mono small">{f.domains || "—"}</td>
              <td class="mono small">{f.agents}</td>
              <td>{#if f.readOnlyAgents > 0}<span class="pill ok">{f.readOnlyAgents}</span>{:else}<span class="muted small">—</span>{/if}</td>
              <td>{#if f.controlPlaneAgents > 0}<span class="pill warn">{f.controlPlaneAgents}</span>{:else}<span class="muted small">none</span>{/if}</td>
              <td style="text-align:right"><button class="btn sm" onclick={(e) => { e.stopPropagation(); openSandbox(f.sandbox); }}>Open ›</button></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <p class="xsmall muted mt12">Egress "open" or "unmanaged" means no ACL is applied — surfaced in amber so it is easy to spot. Enforcement compiles from intent; a sandbox's detail page applies it and shows live status.</p>
</div>
