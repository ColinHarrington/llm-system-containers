<script lang="ts">
  import { listServices, setService } from "../lib/core";
  import type { ServiceEntry } from "../lib/types";

  let services = $state<ServiceEntry[]>([]);
  let busy = $state(false);

  async function refresh() {
    services = await listServices();
  }

  async function toggle(s: ServiceEntry) {
    busy = true;
    try {
      await setService(s.name, !s.enabled);
      await refresh();
    } finally {
      busy = false;
    }
  }

  $effect(() => { refresh(); });
</script>

<div class="card">
  <h2>Services</h2>
  <p class="muted" style="margin-top:-0.4rem">Each runs in its own service container or the VM.</p>
  <table>
    <thead><tr><th>Service</th><th>Priority</th><th>Description</th><th></th></tr></thead>
    <tbody>
      {#each services as s (s.name)}
        <tr>
          <td class="mono">{s.name}</td>
          <td><span class="pill sm">{s.priority}</span></td>
          <td class="muted">{s.description}</td>
          <td style="text-align:right">
            <button class="btn" class:primary={!s.enabled} onclick={() => toggle(s)} disabled={busy}>
              {s.enabled ? "Disable" : "Enable"}
            </button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>
