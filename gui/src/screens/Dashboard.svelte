<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Skeleton from "../lib/Skeleton.svelte";
  import { ui, live, navigate, openSandbox, bump, openTerminal, showToast, confirmAction } from "../lib/store.svelte";
  import {
    vmStatus, vmUp, vmDown, listSandboxes, listServices, topology, hostResources,
    serviceStates, fleetEnforcement,
  } from "../lib/core";
  import type {
    FleetEnforcement, HostResources, Sandbox, ServiceEntry, ServiceState, TopoSandbox, VmStatus,
  } from "../lib/types";

  let vm = $state<VmStatus | null>(null);
  let sandboxes = $state<Sandbox[]>([]);
  let services = $state<ServiceEntry[]>([]);
  let topo = $state<TopoSandbox[]>([]);
  let states = $state<Record<string, ServiceState>>({});
  let fleet = $state<FleetEnforcement[]>([]);
  let res = $state<HostResources | null>(null);
  let loading = $state(true);
  let vmBusy = $state(false);

  $effect(() => {
    ui.dataVersion;
    live.tick; // auto-refresh on the live poll
    void refresh();
  });

  async function refresh() {
    try {
      [vm, sandboxes, services, topo, res] = await Promise.all([
        vmStatus(), listSandboxes(), listServices(), topology().catch(() => []), hostResources().catch(() => null),
      ]);
    } finally { loading = false; }
    void serviceStates().then((s) => (states = s)).catch(() => (states = {}));
    void fleetEnforcement().then((f) => (fleet = f)).catch(() => (fleet = []));
  }

  async function toggleVm() {
    const stopping = vm === "Running";
    if (stopping && !(await confirmAction({
      title: "Stop the VM",
      message: "Stop the Playground VM? Every running sandbox and service stops with it. You can start it again afterwards.",
      confirmLabel: "Stop VM",
    }))) return;
    vmBusy = true;
    showToast(stopping ? "$ llmsctl down" : "$ llmsctl up");
    try {
      if (stopping) await vmDown(); else await vmUp();
      showToast(stopping ? "VM stopped" : "VM is up", "ok");
      bump();
    } catch (e) { showToast(String(e), "danger"); } finally { vmBusy = false; }
  }

  const running = $derived(sandboxes.filter((s) => s.status === "Running").length);
  const stopped = $derived(sandboxes.length - running);
  // Real agent count from the topology tree (one Linux user per agent; humans excluded).
  const agentCount = $derived(topo.reduce((n, s) => n + s.agents.filter((a) => a.kind === "agent").length, 0));
  const servicesRunning = $derived(Object.values(states).filter((s) => s === "running").length);
  const knownServices = $derived(Object.keys(states).length || services.length);
  const managed = $derived(fleet.filter((f) => f.egressPosture !== "unmanaged" && f.egressPosture !== "open").length);
  const totalAgents = $derived(fleet.reduce((n, f) => n + f.agents, 0));
  const roAgents = $derived(fleet.reduce((n, f) => n + f.readOnlyAgents, 0));
  const cpAgents = $derived(fleet.reduce((n, f) => n + f.controlPlaneAgents, 0));

  const pct = (used: number, total: number) => (total ? Math.round((used / total) * 100) : 0);
  const humanBytes = (n: number) =>
    n >= 1024 ** 3 ? `${(n / 1024 ** 3).toFixed(1)} GB` : `${Math.round(n / 1024 ** 2)} MB`;
  const recent = $derived(sandboxes.slice(0, 4));
  // Services to show health for: those with a known live state (deployable), sorted running-first.
  const healthServices = $derived(
    Object.entries(states).sort((a, b) => (a[1] === "running" ? -1 : 1) - (b[1] === "running" ? -1 : 1)),
  );
  const statePill = (s: ServiceState) => (s === "running" ? "ok" : s === "not-provisioned" ? "" : "warn");
</script>

<div class="content">
  <!-- First-run onboarding: the VM has not been created yet -->
  {#if vm === "NotCreated"}
    <div class="card pad onboard mb16">
      <div class="ob-ico"><Icon name="layers" size={26} /></div>
      <div class="ob-body">
        <div class="strong" style="font-size:15px;color:var(--text)">Welcome to llmsc</div>
        <p class="muted small mt4 mb0">No Playground VM yet. The setup wizard provisions a host-native VM running Incus, then your sandboxes and services live inside it. Takes a few minutes.</p>
      </div>
      <div class="right flex gap8">
        <button class="btn primary" onclick={() => navigate("wizard")}><Icon name="cog" size={15} /><span>Run setup wizard</span></button>
        <button class="btn" onclick={toggleVm} disabled={vmBusy}><Icon name="play" size={15} /><span>Quick start</span></button>
      </div>
    </div>
  {/if}

  <!-- VM hero -->
  <div class="card pad mb16" style="display:flex; align-items:center; gap:20px;">
    <div style="width:54px;height:54px;border-radius:14px;background:var(--accent-soft);display:grid;place-items:center;color:var(--accent)">
      <Icon name="layers" size={28} />
    </div>
    <div>
      <div class="flex gap10">
        <span class="strong" style="font-size:16px">llmsc-vm</span>
        {#if vm === "Running"}
          <span class="pill ok"><span class="dot ok pulse"></span> Running</span>
        {:else if vm === "Starting"}
          <span class="pill warn"><span class="dot warn pulse"></span> Starting</span>
        {:else}
          <span class="pill"><span class="dot muted"></span> {vm ?? "…"}</span>
        {/if}
        <span class="tag">L1 · the VM</span>
      </div>
      <div class="muted small mt4">Host-native VM running Incus · Lima driver</div>
    </div>
    <div class="right flex gap8">
      <button class="btn" onclick={() => (ui.newSandboxOpen = true)}><Icon name="plus" /><span>New sandbox</span></button>
      <button class="btn" onclick={toggleVm} disabled={vmBusy}>
        <Icon name={vm === "Running" ? "stop" : "play"} size={15} />
        <span>{vmBusy ? "…" : vm === "Running" ? "Stop" : "Start"}</span>
      </button>
    </div>
  </div>

  <!-- Stat tiles -->
  <div class="grid g-4 mb16">
    <button class="card pad stat clickable" onclick={() => navigate("sandboxes")}>
      <div class="label">Sandboxes</div>
      <div class="num">{sandboxes.length}</div>
      <div class="delta" style="color:var(--ok)">{running} running · {stopped} stopped</div>
    </button>
    <button class="card pad stat clickable" onclick={() => navigate("topology")}>
      <div class="label">Agents</div>
      <div class="num">{agentCount}</div>
      <div class="delta t2">one Linux user each · across {topo.length} sandboxes</div>
    </button>
    <button class="card pad stat clickable" onclick={() => navigate("services")}>
      <div class="label">Services running</div>
      <div class="num">{servicesRunning} <small>/ {knownServices}</small></div>
      <div class="delta t2">{services.filter((s) => s.enabled).length} enabled in config</div>
    </button>
    <button class="card pad stat clickable" onclick={() => navigate("security")}>
      <div class="label">Managed egress</div>
      <div class="num">{managed} <small>/ {sandboxes.length}</small></div>
      <div class="delta" style="color:var(--ok)">sandboxes with an egress policy</div>
    </button>
  </div>

  <div class="grid g-2 mb16">
    <!-- Host resources -->
    <div class="card">
      <div class="card-head"><h3>Host resources</h3><span class="right hint">Allocated to llmsc-vm</span></div>
      <div class="pad">
        {#if res}
          <div class="mb16">
            <div class="flex"><span class="small strong">CPU</span><span class="right small mono t2">{res.cpuUsed} / {res.cpuTotal} cores</span></div>
            <div class="meter mt8"><i style="width:{pct(res.cpuUsed, res.cpuTotal)}%"></i></div>
          </div>
          <div class="mb16">
            <div class="flex"><span class="small strong">Memory</span><span class="right small mono t2">{humanBytes(res.memUsed)} / {humanBytes(res.memTotal)}</span></div>
            <div class="meter mt8"><i class="warn" style="width:{pct(res.memUsed, res.memTotal)}%"></i></div>
          </div>
          <div>
            <div class="flex"><span class="small strong">Disk</span><span class="right small mono t2">{humanBytes(res.diskUsed)} / {humanBytes(res.diskTotal)}</span></div>
            <div class="meter mt8"><i class="ok" style="width:{pct(res.diskUsed, res.diskTotal)}%"></i></div>
          </div>
        {:else}
          <div class="muted small">Host metrics unavailable (VM not running?).</div>
        {/if}
      </div>
    </div>

    <!-- Service health -->
    <div class="card">
      <div class="card-head"><h3>Service health</h3><span class="sub">live container state</span>
        <button class="btn sm right" onclick={() => navigate("services")}>Manage</button>
      </div>
      <div class="pad">
        {#if healthServices.length === 0}
          <div class="muted small">No live service state (VM not running, or nothing provisioned).</div>
        {:else}
          {#each healthServices as [name, st]}
            <div class="flex svc-row">
              <span class="mono small strong" style="color:var(--text)">{name}</span>
              <span class="right pill {statePill(st)}">{st}</span>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>

  <div class="grid g-2 mb16">
    <!-- Security posture -->
    <div class="card">
      <div class="card-head"><h3>Security posture</h3><span class="sub">configured enforcement intent</span>
        <button class="btn sm right" onclick={() => navigate("security")}>Fleet view</button>
      </div>
      <div class="pad">
        <div class="flex gap8 wrap">
          <div class="posture"><div class="pnum">{managed}</div><div class="muted xsmall">managed egress</div></div>
          <div class="posture"><div class="pnum">{totalAgents}</div><div class="muted xsmall">agents</div></div>
          <div class="posture"><div class="pnum">{roAgents}</div><div class="muted xsmall">read-only fs</div></div>
          <div class="posture"><div class="pnum">{cpAgents}</div><div class="muted xsmall">control-plane caps</div></div>
        </div>
        <p class="xsmall muted mt12">Per-container egress ACLs + per-UID Tetragon policies compile from each agent's guardrails. Open the fleet view for the per-sandbox matrix; a sandbox's detail page shows live ring status.</p>
      </div>
    </div>

    <!-- Quick actions -->
    <div class="card">
      <div class="card-head"><h3>Quick actions</h3></div>
      <div class="pad grid" style="gap:10px">
        <button class="qa" onclick={() => (ui.newSandboxOpen = true)}>
          <span class="qa-ico"><Icon name="plus" size={18} /></span>
          <span><span class="strong small">New sandbox</span><span class="muted xsmall">Spin up a fresh LLMSC workspace</span></span>
        </button>
        <button class="qa" onclick={() => navigate("services")}>
          <span class="qa-ico"><Icon name="cog" size={18} /></span>
          <span><span class="strong small">Manage services</span><span class="muted xsmall">Provision LiteLLM, Phoenix, storage…</span></span>
        </button>
        <button class="qa" onclick={() => (ui.paletteOpen = true)}>
          <span class="qa-ico"><Icon name="search" size={18} /></span>
          <span><span class="strong small">Command palette</span><span class="muted xsmall mono">⌘K — jump anywhere, run anything</span></span>
        </button>
      </div>
    </div>
  </div>

  <!-- Recent sandboxes -->
  <div class="card">
    <div class="card-head"><h3>Sandboxes</h3><span class="sub">L2 system containers</span>
      <button class="btn sm right" onclick={() => navigate("sandboxes")}><span>View all</span></button>
    </div>
    {#if loading}
      <div class="pad"><Skeleton w="100%" h={20} mb={10} /><Skeleton w="100%" h={20} mb={10} /><Skeleton w="70%" h={20} /></div>
    {:else if sandboxes.length === 0}
      <div class="empty">
        <div class="icon"><Icon name="box" size={26} /></div>
        No sandboxes yet.
        <button class="btn sm primary mt12" onclick={() => (ui.newSandboxOpen = true)}><Icon name="plus" size={14} /><span>Create your first sandbox</span></button>
      </div>
    {:else}
      <table class="tbl">
        <thead><tr><th>Name</th><th>Image</th><th>Status</th><th></th></tr></thead>
        <tbody>
          {#each recent as s (s.name)}
            <tr class="clickable" onclick={() => openSandbox(s.name)}>
              <td><div class="strong">{s.name}</div></td>
              <td><span class="tag mono">{s.image ?? "—"}</span></td>
              <td>
                {#if s.status === "Running"}<span class="pill ok"><span class="dot ok"></span> Running</span>
                {:else}<span class="pill"><span class="dot muted"></span> Stopped</span>{/if}
              </td>
              <td style="text-align:right">
                <button class="btn sm" onclick={(e) => { e.stopPropagation(); openTerminal(`operator@${s.name}`); }}><Icon name="terminal" size={13} /></button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .onboard { display: flex; align-items: center; gap: 16px; border-color: var(--accent); background: var(--accent-soft); }
  .ob-ico { width: 48px; height: 48px; border-radius: 12px; background: var(--accent-soft-bg); color: var(--accent); display: grid; place-items: center; flex: none; }
  .ob-body { min-width: 0; }
  .mb0 { margin-bottom: 0; }
  .stat.clickable { text-align: left; cursor: pointer; font-family: inherit; }
  .stat.clickable:hover { border-color: var(--border-strong); }
  .svc-row { padding: 6px 0; border-bottom: 1px solid var(--border); }
  .svc-row:last-child { border-bottom: none; }
  .posture { flex: 1; min-width: 90px; padding: 10px; border: 1px solid var(--border); border-radius: 10px; background: var(--card-2); text-align: center; }
  .posture .pnum { font-size: 22px; font-weight: 700; color: var(--text); }
  .qa {
    display: flex; gap: 12px; align-items: center; text-align: left;
    padding: 14px; border: 1px solid var(--border); border-radius: 12px;
    background: var(--card-2); cursor: pointer; font-family: inherit; width: 100%;
  }
  .qa:hover { border-color: var(--border-strong); }
  .qa > span:last-child { display: flex; flex-direction: column; gap: 2px; }
  .qa-ico { width: 36px; height: 36px; border-radius: 10px; background: var(--accent-soft); color: var(--accent); display: grid; place-items: center; flex: none; }
</style>
