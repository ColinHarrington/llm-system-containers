<script lang="ts">
  import { vmStatus, vmUp, vmDown, listSandboxes } from "../lib/core";
  import type { Sandbox, VmStatus } from "../lib/types";

  let status = $state<VmStatus | null>(null);
  let sandboxes = $state<Sandbox[]>([]);
  let busy = $state(false);

  async function refresh() {
    status = await vmStatus();
    sandboxes = await listSandboxes();
  }

  async function start() {
    busy = true;
    try { await vmUp(); await refresh(); } finally { busy = false; }
  }

  async function stop() {
    busy = true;
    try { await vmDown(); await refresh(); } finally { busy = false; }
  }

  $effect(() => { refresh(); });

  let running = $derived(sandboxes.filter((s) => s.status === "Running").length);
</script>

<section class="dash">
  <header class="card hero">
    <div>
      <h1>VM</h1>
      <p class="muted">llmsc-vm · the L1 host</p>
    </div>
    <div class="hero-status">
      <span class="pill" data-status={status ?? "NotCreated"}>{status ?? "…"}</span>
      {#if status === "Running"}
        <button class="btn" onclick={stop} disabled={busy}>Stop</button>
      {:else}
        <button class="btn primary" onclick={start} disabled={busy}>Start</button>
      {/if}
    </div>
  </header>

  <div class="tiles">
    <div class="card tile"><span class="num">{running}</span><span class="muted">sandboxes running</span></div>
    <div class="card tile"><span class="num">{sandboxes.length}</span><span class="muted">sandboxes total</span></div>
  </div>

  <div class="card">
    <h2>Sandboxes</h2>
    {#if sandboxes.length === 0}
      <p class="muted">No sandboxes yet.</p>
    {:else}
      <table>
        <thead><tr><th>Name</th><th>Status</th></tr></thead>
        <tbody>
          {#each sandboxes as s (s.name)}
            <tr>
              <td class="mono">{s.name}</td>
              <td><span class="pill sm" data-status={s.status}>{s.status}</span></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</section>

<style>
  .dash { display: flex; flex-direction: column; gap: 1rem; }
  .hero { display: flex; align-items: center; justify-content: space-between; }
  .hero-status { display: flex; align-items: center; gap: 0.75rem; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 1rem; }
  .tile { display: flex; flex-direction: column; gap: 0.25rem; }
  .num { font-size: 1.8rem; font-weight: 700; }
</style>
