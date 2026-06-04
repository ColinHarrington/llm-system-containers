<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui, navigate, bump, openTerminal, showToast } from "../lib/store.svelte";
  import { topology, removeSandbox, listProfiles, removeAgent, setAgentProfile } from "../lib/core";
  import type { ProfileInfo, TopoSandbox } from "../lib/types";

  let all = $state<TopoSandbox[]>([]);
  let profiles = $state<ProfileInfo[]>([]);
  let busy = $state(false);
  let userBusy = $state<string | null>(null);

  $effect(() => {
    ui.dataVersion;
    void (async () => { all = await topology(); })();
  });
  $effect(() => {
    void listProfiles().then((p) => (profiles = p));
  });

  async function changeProfile(agent: string, profile: string) {
    if (!sb) return;
    userBusy = agent;
    try {
      await setAgentProfile(sb.name, agent, profile);
      showToast(`${agent}: profile → ${profile || "none"}`, "ok");
      bump();
    } finally { userBusy = null; }
  }

  async function removeUser(agent: string) {
    if (!sb) return;
    userBusy = agent;
    showToast(`$ llmsc agent rm ${agent}@${sb.name}`);
    try {
      await removeAgent(sb.name, agent);
      showToast(`Agent '${agent}' removed`, "ok");
      bump();
    } finally { userBusy = null; }
  }

  const sb = $derived(all.find((s) => s.name === ui.selectedSandbox) ?? null);
  const initials = (name: string) => name.replace(/^agent-/, "").slice(0, 2).toUpperCase();

  async function remove() {
    if (!sb) return;
    busy = true;
    try {
      await removeSandbox(sb.name);
      navigate("sandboxes");
      bump();
    } finally { busy = false; }
  }
</script>

<div class="content">
  <button class="btn ghost sm mb12" onclick={() => navigate("sandboxes")}>‹ Sandbox containers</button>

  {#if !sb}
    <div class="card"><div class="empty"><div class="icon"><Icon name="box" size={24} /></div>Sandbox not found.</div></div>
  {:else}
    <!-- header -->
    <div class="card pad mb16" style="display:flex; align-items:center; gap:16px;">
      <div class="hico" class:off={sb.status !== "running"}><Icon name="box" size={22} /></div>
      <div>
        <div class="flex gap10">
          <span class="strong mono" style="font-size:16px;color:var(--text)">{sb.name}</span>
          {#if sb.status === "running"}<span class="pill ok"><span class="dot ok pulse"></span> running</span>
          {:else}<span class="pill"><span class="dot muted"></span> stopped</span>{/if}
          <span class="tag">L2 · system container</span>
          {#if sb.l3}<span class="pill accent"><Icon name="pkg" size={11} /> L3 enabled</span>{/if}
        </div>
        <div class="muted small mono mt4">{sb.image}{sb.status === "running" && sb.mem && sb.mem !== "—" ? ` · ${sb.mem}` : ""}</div>
      </div>
      <div class="right flex gap8">
        <button class="btn" onclick={() => openTerminal(`operator@${sb.name}`)}><Icon name="terminal" size={15} /><span>Open shell</span></button>
        {#if sb.status === "running"}
          <button class="btn" onclick={() => (ui.addAgentSandbox = sb.name)}><Icon name="agent" size={15} /><span>Add agent</span></button>
        {/if}
        <button class="btn danger" onclick={remove} disabled={busy}>{busy ? "Removing…" : "Remove"}</button>
      </div>
    </div>

    <div class="grid g-2">
      <!-- details -->
      <div class="card">
        <div class="card-head"><h3>Details</h3></div>
        <div class="pad">
          <div class="kv"><span class="k">Image</span><span class="v mono small">{sb.image}</span></div>
          <div class="kv"><span class="k">Status</span><span class="v">{sb.status}</span></div>
          <div class="kv"><span class="k">Privilege</span><span class="v">unprivileged LXC</span></div>
          <div class="kv"><span class="k">Nested L3</span><span class="v">{sb.l3 ? "enabled" : "off"}</span></div>
          <div class="kv"><span class="k">Memory</span><span class="v mono small">{sb.mem}</span></div>
        </div>
      </div>

      <!-- users -->
      <div class="card">
        <div class="card-head"><h3>Users</h3><span class="sub">one human operator + one Linux user per agent</span></div>
        {#if sb.agents.length === 0}
          <div class="empty"><div class="icon"><Icon name="user" size={22} /></div>No users provisioned.</div>
        {:else}
          <table class="tbl">
            <thead><tr><th>User</th><th>Role</th><th>Profile</th><th></th></tr></thead>
            <tbody>
              {#each sb.agents as u (u.name)}
                <tr>
                  <td><div class="flex gap8"><div class="avatar {u.kind === 'human' ? 'human' : 'agent'} sm">{initials(u.name)}</div><span class="mono small strong" style="color:var(--text)">{u.name}</span></div></td>
                  <td>{#if u.kind === "human"}<span class="pill">human</span>{:else}<span class="pill accent">agent</span>{/if}</td>
                  <td>
                    {#if u.kind === "human"}
                      <span class="muted small">—</span>
                    {:else}
                      <select class="input prof" value={u.profile ?? ""} disabled={userBusy === u.name}
                        onchange={(e) => changeProfile(u.name, (e.currentTarget as HTMLSelectElement).value)}>
                        <option value="">none</option>
                        {#each profiles as p}<option value={p.name}>{p.name}</option>{/each}
                      </select>
                    {/if}
                  </td>
                  <td style="text-align:right; white-space:nowrap">
                    <button class="btn sm" title="Open shell" onclick={() => openTerminal(`${u.name}@${sb.name}`)}><Icon name="terminal" size={13} /></button>
                    {#if u.kind !== "human"}
                      <button class="btn sm danger" title="Remove agent" onclick={() => removeUser(u.name)} disabled={userBusy === u.name}><Icon name="x" size={13} /></button>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    </div>

    <p class="xsmall muted mt12">Live per-agent activity (sessions, trace, tokens) is not instrumented yet.</p>
  {/if}
</div>

<style>
  .hico { width: 44px; height: 44px; border-radius: 11px; background: var(--accent-dim); color: var(--accent-text); display: grid; place-items: center; flex: none; }
  .hico.off { background: var(--card-2); color: var(--text-3); border: 1px solid var(--border); }
  .prof { width: auto; padding: 4px 8px; font-size: 12px; }
  .btn.sm + .btn.sm { margin-left: 4px; }
</style>
