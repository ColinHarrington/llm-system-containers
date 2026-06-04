<script lang="ts">
  import { listServices, setService, provisionService, DEPLOYABLE_SERVICES } from "../lib/core";
  import type { ServiceEntry } from "../lib/types";

  let services = $state<ServiceEntry[]>([]);
  let busyName = $state<string | null>(null);
  let error = $state<string | null>(null);

  async function refresh() {
    services = await listServices();
  }

  async function toggle(s: ServiceEntry) {
    error = null;
    busyName = s.name;
    try {
      await setService(s.name, !s.enabled);
      await refresh();
    } catch (e) {
      error = `${s.name}: ${e}`;
    } finally {
      busyName = null;
    }
  }

  async function provision(s: ServiceEntry) {
    error = null;
    busyName = s.name;
    try {
      await provisionService(s.name);
    } catch (e) {
      error = `${s.name}: ${e}`;
    } finally {
      busyName = null;
    }
  }

  $effect(() => { refresh(); });
</script>

<div class="card">
  <h2>Services</h2>
  <p class="muted" style="margin-top:-0.4rem">Each runs in its own service container or the VM.</p>
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
  <table>
    <thead><tr><th>Service</th><th>Priority</th><th>Description</th><th></th></tr></thead>
    <tbody>
      {#each services as s (s.name)}
        <tr>
          <td class="mono">{s.name}</td>
          <td><span class="pill sm">{s.priority}</span></td>
          <td class="muted">{s.description}</td>
          <td style="text-align:right; white-space:nowrap">
            {#if s.enabled && DEPLOYABLE_SERVICES.has(s.name)}
              <button class="btn primary" onclick={() => provision(s)} disabled={busyName !== null}>
                Provision
              </button>
            {/if}
            <button class="btn" class:primary={!s.enabled} onclick={() => toggle(s)} disabled={busyName !== null}>
              {s.enabled ? "Disable" : "Enable"}
            </button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .error {
    color: #b91c1c;
    background: #fef2f2;
    border: 1px solid #fecaca;
    border-radius: 8px;
    padding: 0.5rem 0.7rem;
    font-size: 0.85rem;
  }
  .btn + .btn { margin-left: 0.4rem; }
</style>
