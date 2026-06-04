<script lang="ts">
  import { listSandboxes, launchSandbox, removeSandbox } from "../lib/core";
  import type { Sandbox } from "../lib/types";

  let sandboxes = $state<Sandbox[]>([]);
  let name = $state("");
  let image = $state("images:alpine/3.21");
  let nesting = $state(false);
  let busy = $state(false);

  async function refresh() {
    sandboxes = await listSandboxes();
  }

  async function launch(e: Event) {
    e.preventDefault();
    if (!name.trim()) return;
    busy = true;
    try {
      await launchSandbox(name.trim(), image.trim(), nesting);
      name = "";
      await refresh();
    } finally {
      busy = false;
    }
  }

  async function remove(n: string) {
    busy = true;
    try {
      await removeSandbox(n);
      await refresh();
    } finally {
      busy = false;
    }
  }

  $effect(() => { refresh(); });
</script>

<section class="wrap">
  <div class="card">
    <h2>New sandbox</h2>
    <form class="newform" onsubmit={launch}>
      <input class="in" placeholder="name (e.g. web-agent-02)" bind:value={name} />
      <input class="in" placeholder="image" bind:value={image} />
      <label class="chk"><input type="checkbox" bind:checked={nesting} /> nesting (L3)</label>
      <button class="btn primary" type="submit" disabled={busy || !name.trim()}>Launch</button>
    </form>
  </div>

  <div class="card">
    <h2>Sandboxes</h2>
    {#if sandboxes.length === 0}
      <p class="muted">No sandboxes yet.</p>
    {:else}
      <table>
        <thead><tr><th>Name</th><th>Image</th><th>Status</th><th></th></tr></thead>
        <tbody>
          {#each sandboxes as s (s.name)}
            <tr>
              <td class="mono">{s.name}</td>
              <td class="mono muted">{s.image ?? "—"}</td>
              <td><span class="pill sm" data-status={s.status}>{s.status}</span></td>
              <td style="text-align:right">
                <button class="btn" onclick={() => remove(s.name)} disabled={busy}>Remove</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</section>

<style>
  .wrap { display: flex; flex-direction: column; gap: 1rem; }
  .newform { display: flex; gap: 0.6rem; align-items: center; flex-wrap: wrap; }
  .in { padding: 0.5rem 0.7rem; border: 1px solid var(--border); border-radius: 8px; font-size: 0.9rem; min-width: 200px; }
  .chk { display: flex; align-items: center; gap: 0.35rem; font-size: 0.9rem; color: var(--muted); }
</style>
