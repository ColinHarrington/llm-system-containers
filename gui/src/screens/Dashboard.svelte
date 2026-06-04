<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui, navigate, bump, openTerminal } from "../lib/store.svelte";
  import { vmStatus, vmUp, vmDown, listSandboxes, listServices, listAgents, hostResources } from "../lib/core";
  import type { AgentInfo, HostResources, Sandbox, ServiceEntry, VmStatus } from "../lib/types";

  let vm = $state<VmStatus | null>(null);
  let sandboxes = $state<Sandbox[]>([]);
  let services = $state<ServiceEntry[]>([]);
  let agents = $state<AgentInfo[]>([]);
  let res = $state<HostResources | null>(null);
  let vmBusy = $state(false);

  $effect(() => {
    ui.dataVersion;
    void refresh();
  });

  async function refresh() {
    [vm, sandboxes, services, agents, res] = await Promise.all([
      vmStatus(), listSandboxes(), listServices(), listAgents(), hostResources(),
    ]);
  }

  async function toggleVm() {
    vmBusy = true;
    try {
      if (vm === "Running") await vmDown(); else await vmUp();
      bump();
    } finally { vmBusy = false; }
  }

  const running = $derived(sandboxes.filter((s) => s.status === "Running").length);
  const stopped = $derived(sandboxes.length - running);
  const activeAgents = $derived(agents.filter((a) => a.status !== "idle").length);
  const pausedAgents = $derived(agents.filter((a) => a.status === "paused").length);
  const nested = $derived(sandboxes.reduce((n, s) => n + (s.nested ?? 0), 0));
  const enabledServices = $derived(services.filter((s) => s.enabled).length);
  const pct = (used: number, total: number) => (total ? Math.round((used / total) * 100) : 0);
  const recent = $derived(sandboxes.slice(0, 4));
</script>

<div class="content">
  <!-- VM hero -->
  <div class="card pad mb16" style="display:flex; align-items:center; gap:20px;">
    <div style="width:54px;height:54px;border-radius:14px;background:var(--accent-soft);display:grid;place-items:center;color:var(--accent)">
      <Icon name="layers" size={28} />
    </div>
    <div>
      <div class="flex gap10">
        <span class="strong" style="font-size:16px">llmsc-vm</span>
        {#if vm === "Running"}
          <span class="pill ok"><span class="dot ok"></span> Running</span>
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
        <span>{vm === "Running" ? "Stop" : "Start"}</span>
      </button>
    </div>
  </div>

  <!-- Stat tiles -->
  <div class="grid g-4 mb16">
    <div class="card pad stat">
      <div class="label">Sandboxes</div>
      <div class="num">{sandboxes.length}</div>
      <div class="delta" style="color:var(--ok)">{running} running · {stopped} stopped</div>
    </div>
    <div class="card pad stat">
      <div class="label">Active agents</div>
      <div class="num">{activeAgents} <small>working</small></div>
      <div class="delta t2">{pausedAgents} paused by operator</div>
    </div>
    <div class="card pad stat">
      <div class="label">Nested containers <span class="muted">(L3)</span></div>
      <div class="num">{nested} <small>rootless</small></div>
      <div class="delta t2">across {sandboxes.filter((s) => (s.nested ?? 0) > 0).length} sandboxes</div>
    </div>
    <div class="card pad stat">
      <div class="label">Services</div>
      <div class="num">{enabledServices} <small>/ {services.length}</small></div>
      <div class="delta" style="color:var(--ok)">enabled</div>
    </div>
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
            <div class="flex"><span class="small strong">Memory</span><span class="right small mono t2">{res.memUsed} / {res.memTotal} GB</span></div>
            <div class="meter mt8"><i class="warn" style="width:{pct(res.memUsed, res.memTotal)}%"></i></div>
          </div>
          <div>
            <div class="flex"><span class="small strong">Disk</span><span class="right small mono t2">{res.diskUsed} / {res.diskTotal} GB</span></div>
            <div class="meter mt8"><i class="ok" style="width:{pct(res.diskUsed, res.diskTotal)}%"></i></div>
          </div>
        {/if}
        <div class="divider"></div>
        <div class="flex gap8 small t2">
          <Icon name="shield" size={16} />
          Tetragon eBPF enforcement active · network policy applied to all sandboxes
        </div>
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
        <button class="qa" onclick={() => openTerminal("operator@web-agent-01")}>
          <span class="qa-ico"><Icon name="terminal" size={18} /></span>
          <span><span class="strong small">Open a shell</span><span class="muted xsmall mono">llmsc shell operator@web-agent-01</span></span>
        </button>
        <button class="qa" onclick={() => navigate("agent")}>
          <span class="qa-ico"><Icon name="steer" size={18} /></span>
          <span><span class="strong small">Steer an agent</span><span class="muted xsmall">Observe, pause or inject a message</span></span>
        </button>
      </div>
    </div>
  </div>

  <!-- Recent sandboxes -->
  <div class="card">
    <div class="card-head"><h3>Sandboxes</h3><span class="sub">L2 system containers</span>
      <button class="btn sm right" onclick={() => navigate("sandboxes")}><span>View all</span></button>
    </div>
    {#if sandboxes.length === 0}
      <div class="empty"><div class="icon"><Icon name="box" size={26} /></div>No sandboxes yet.</div>
    {:else}
      <table class="tbl">
        <thead><tr><th>Name</th><th>Image</th><th>Users</th><th>L3</th><th>Mem</th><th>Status</th></tr></thead>
        <tbody>
          {#each recent as s (s.name)}
            <tr class="clickable" onclick={() => navigate("sandboxes")}>
              <td><div class="strong">{s.name}</div></td>
              <td><span class="tag mono">{s.image ?? "—"}</span></td>
              <td>
                <div class="flex gap6">
                  {#each s.users ?? [] as u}<div class="avatar {u.kind} sm">{u.initials}</div>{/each}
                  {#if !s.users}<span class="muted small">—</span>{/if}
                </div>
              </td>
              <td><span class="mono small">{s.nested ?? "—"}</span></td>
              <td class="mono small t2">{s.memTotal ? `${s.memUsed} / ${s.memTotal} GB` : "—"}</td>
              <td>
                {#if s.status === "Running"}<span class="pill ok"><span class="dot ok"></span> Running</span>
                {:else}<span class="pill"><span class="dot muted"></span> Stopped</span>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .qa {
    display: flex; gap: 12px; align-items: center; text-align: left;
    padding: 14px; border: 1px solid var(--border); border-radius: 12px;
    background: var(--card-2); cursor: pointer; font-family: inherit; width: 100%;
  }
  .qa:hover { border-color: var(--border-strong); }
  .qa > span:last-child { display: flex; flex-direction: column; gap: 2px; }
  .qa-ico { width: 36px; height: 36px; border-radius: 10px; background: var(--accent-soft); color: var(--accent); display: grid; place-items: center; flex: none; }
</style>
