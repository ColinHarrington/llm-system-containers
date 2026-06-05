<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Modal from "../lib/Modal.svelte";
  import { ui, navigate, bump, openTerminal, showToast } from "../lib/store.svelte";
  import {
    topology, removeSandbox, removeAgent, instanceConfig,
    instanceSetConfig, instanceUnsetConfig, instanceAddMount, instanceRemoveDevice,
    instanceAddProfile, instanceRemoveProfile, applySandbox, instanceYaml,
    listSnapshots, snapshotCreate, snapshotRestore, snapshotDelete, setAgentGuardrails,
  } from "../lib/core";
  import type { Guardrails, InstanceConfig, SnapshotInfo, TopoAgent, TopoSandbox } from "../lib/types";

  let all = $state<TopoSandbox[]>([]);
  let inst = $state<InstanceConfig | null>(null);
  let busy = $state(false);
  let userBusy = $state<string | null>(null);
  let cfgBusy = $state(false);

  // edit inputs
  let newProfile = $state("");
  let newKey = $state("");
  let newVal = $state("");
  let mSource = $state("");
  let mPath = $state("");
  let mRo = $state(false);

  async function edit(fn: () => Promise<void>, msg: string) {
    if (!sb) return;
    cfgBusy = true;
    try {
      await fn();
      showToast(msg, "ok");
      bump();
    } catch (e) {
      showToast(String(e), "danger");
    } finally {
      cfgBusy = false;
    }
  }

  $effect(() => {
    ui.dataVersion;
    void (async () => { all = await topology(); })();
  });
  $effect(() => {
    ui.dataVersion;
    const sel = ui.selectedSandbox;
    inst = null;
    if (sel) void instanceConfig(sel).then((c) => (inst = c)).catch(() => (inst = null));
  });

  const deviceEntries = $derived(inst ? Object.entries(inst.devices) : []);
  const configEntries = $derived(inst ? Object.entries(inst.config) : []);

  let snaps = $state<SnapshotInfo[]>([]);
  let newSnap = $state("");
  let snapBusy = $state<string | null>(null);
  $effect(() => {
    ui.dataVersion;
    const sel = ui.selectedSandbox;
    snaps = [];
    if (sel) void listSnapshots(sel).then((s) => (snaps = s)).catch(() => (snaps = []));
  });
  async function snapEdit(key: string, fn: () => Promise<void>, msg: string) {
    if (!sb) return;
    snapBusy = key;
    try {
      await fn();
      showToast(msg, "ok");
      bump();
    } catch (e) {
      showToast(String(e), "danger");
    } finally { snapBusy = null; }
  }

  // Guardrails editor
  let gAgent = $state<string | null>(null);
  let gForm = $state<Guardrails>({ filesystem: "", network: "", l3: false, llmBudget: "", controlPlane: "" });
  let gBusy = $state(false);
  function openGuardrails(u: TopoAgent) {
    gAgent = u.name;
    gForm = u.guardrails
      ? { ...u.guardrails }
      : { filesystem: "", network: "", l3: false, llmBudget: "", controlPlane: "" };
  }
  async function saveGuardrails() {
    if (!sb || !gAgent) return;
    gBusy = true;
    try {
      await setAgentGuardrails(sb.name, gAgent, gForm);
      showToast(`Guardrails updated for ${gAgent}`, "ok");
      gAgent = null;
      bump();
    } catch (e) {
      showToast(String(e), "danger");
    } finally { gBusy = false; }
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

  let yamlText = $state<string | null>(null);
  async function toggleYaml() {
    if (!sb) return;
    if (yamlText !== null) { yamlText = null; return; }
    try {
      yamlText = await instanceYaml(sb.name);
    } catch (e) {
      showToast(String(e), "danger");
    }
  }
  function copyYaml() {
    if (yamlText) {
      void navigator.clipboard?.writeText(yamlText);
      showToast("YAML copied", "ok");
    }
  }

  async function applyConfig() {
    if (!sb) return;
    cfgBusy = true;
    showToast(`$ llmsc apply ${sb.name}`);
    try {
      const n = await applySandbox(sb.name);
      showToast(n === 0 ? "Already in sync" : `Converged — ${n} change(s)`, "ok");
      bump();
    } catch (e) {
      showToast(String(e), "danger");
    } finally {
      cfgBusy = false;
    }
  }

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
            <thead><tr><th>User</th><th>Role</th><th>Guardrails</th><th></th></tr></thead>
            <tbody>
              {#each sb.agents as u (u.name)}
                <tr>
                  <td><div class="flex gap8"><div class="avatar {u.kind === 'human' ? 'human' : 'agent'} sm">{initials(u.name)}</div><span class="mono small strong" style="color:var(--text)">{u.name}</span></div></td>
                  <td>{#if u.kind === "human"}<span class="pill">human</span>{:else}<span class="pill accent">agent</span>{/if}</td>
                  <td>
                    {#if u.kind === "human"}
                      <span class="muted small">full (operator)</span>
                    {:else if u.profile}
                      <span class="tag mono" title="Guardrails seeded from the {u.profile} profile">from {u.profile}</span>
                    {:else}
                      <span class="muted small">custom</span>
                    {/if}
                  </td>
                  <td style="text-align:right; white-space:nowrap">
                    <button class="btn sm" title="Open shell" onclick={() => openTerminal(`${u.name}@${sb.name}`)}><Icon name="terminal" size={13} /></button>
                    {#if u.kind !== "human"}
                      <button class="btn sm" title="Guardrails" onclick={() => openGuardrails(u)}><Icon name="shield" size={13} /></button>
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

    <!-- Live Incus surface (round-trip read from the server) -->
    {#if inst}
      <div class="card mt16">
        <div class="card-head"><h3>Incus configuration</h3><span class="sub">live surface · editable · <span class="mono">incus config show {sb.name}</span></span>
          <button class="btn sm right" onclick={toggleYaml} title="Render the Incus instance YAML">
            <Icon name="doc" size={13} /><span>{yamlText !== null ? "Hide YAML" : "YAML"}</span></button>
          <button class="btn sm" disabled={cfgBusy} onclick={applyConfig} title="Converge the running instance to your config intent"><Icon name="check" size={13} /><span>Apply config</span></button>
        </div>
        {#if yamlText !== null}
          <div class="pad" style="padding-bottom:0">
            <div class="flex mb8"><span class="sub2">Rendered intent <span class="muted" style="text-transform:none">· incus create &lt; config.yaml</span></span>
              <button class="btn sm right" onclick={copyYaml}><Icon name="copy" size={13} /> Copy</button></div>
            <pre class="console yaml">{yamlText}</pre>
          </div>
        {/if}
        <div class="pad">
          <!-- profiles -->
          <div class="sub2">Profiles</div>
          <div class="flex gap6 wrap mb16">
            {#each inst.profiles as p}
              <span class="echip">{p}<button class="ex" title="Remove profile" disabled={cfgBusy} onclick={() => edit(() => instanceRemoveProfile(sb.name, p), `Removed profile ${p}`)}>×</button></span>
            {/each}
            <input class="input mini" bind:value={newProfile} placeholder="add profile…" />
            <button class="btn sm" disabled={cfgBusy || !newProfile.trim()} onclick={() => { const p = newProfile.trim(); newProfile = ""; void edit(() => instanceAddProfile(sb.name, p), `Applied profile ${p}`); }}>Add</button>
          </div>

          <!-- devices -->
          <div class="sub2">Devices</div>
          {#if deviceEntries.length === 0}<div class="muted small mb8">none</div>{:else}
            <div class="devs mb12">
              {#each deviceEntries as [dname, dev]}
                <div class="dev">
                  <div class="flex gap8 mb4"><span class="mono small strong" style="color:var(--text)">{dname}</span><span class="tag">{dev.type ?? "?"}</span>
                    {#if inst.localDevices.includes(dname)}
                      <button class="ex right" title="Remove device" disabled={cfgBusy} onclick={() => edit(() => instanceRemoveDevice(sb.name, dname), `Removed device ${dname}`)}>×</button>
                    {/if}
                  </div>
                  <div class="kvs">
                    {#each Object.entries(dev).filter(([k]) => k !== "type") as [k, v]}
                      <div class="kvline"><span class="kk mono">{k}</span><span class="vv mono">{v}</span></div>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
          <div class="addmount mb16">
            <input class="input mono" bind:value={mSource} placeholder="host source" />
            <input class="input mono" bind:value={mPath} placeholder="/container/path" />
            <label class="ro"><input type="checkbox" bind:checked={mRo} /> ro</label>
            <button class="btn sm" disabled={cfgBusy || !mSource.trim() || !mPath.trim()} onclick={() => { const s = mSource.trim(), p = mPath.trim(), ro = mRo; mSource = ""; mPath = ""; mRo = false; void edit(() => instanceAddMount(sb.name, s, p, ro), `Added mount ${p}`); }}>Add mount</button>
          </div>

          <!-- config -->
          <div class="sub2">Config</div>
          {#if configEntries.length === 0}<div class="muted small mb8">none</div>{:else}
            <div class="kvs mb12">
              {#each configEntries as [k, v]}
                <div class="kvline"><span class="kk mono">{k}</span><span class="vv mono">{v}</span>
                  <button class="ex" title="Unset" disabled={cfgBusy} onclick={() => edit(() => instanceUnsetConfig(sb.name, k), `Unset ${k}`)}>×</button>
                </div>
              {/each}
            </div>
          {/if}
          <div class="addcfg">
            <input class="input mono" bind:value={newKey} placeholder="config key (e.g. limits.processes)" />
            <input class="input mono" bind:value={newVal} placeholder="value" />
            <button class="btn sm" disabled={cfgBusy || !newKey.trim()} onclick={() => { const k = newKey.trim(), v = newVal; newKey = ""; newVal = ""; void edit(() => instanceSetConfig(sb.name, k, v), `Set ${k}`); }}>Set</button>
          </div>
        </div>
      </div>
    {/if}

    <!-- Snapshots -->
    <div class="card mt16">
      <div class="card-head"><h3>Snapshots</h3><span class="sub">checkpoint &amp; restore · <span class="mono">incus snapshot</span></span></div>
      <div class="pad">
        <div class="flex gap8 mb12">
          <input class="input mono" style="max-width:280px" bind:value={newSnap} placeholder="snapshot name (e.g. before-deploy)" />
          <button class="btn sm primary" disabled={snapBusy !== null || !newSnap.trim()}
            onclick={() => { const n = newSnap.trim(); newSnap = ""; void snapEdit("create", () => snapshotCreate(sb.name, n), `Snapshot '${n}' created`); }}>
            <Icon name="plus" size={13} /><span>Snapshot</span></button>
        </div>
        {#if snaps.length === 0}
          <div class="muted small">No snapshots yet.</div>
        {:else}
          <table class="tbl">
            <thead><tr><th>Name</th><th>Created</th><th>Mode</th><th></th></tr></thead>
            <tbody>
              {#each snaps as s (s.name)}
                <tr>
                  <td class="mono small strong" style="color:var(--text)">{s.name}</td>
                  <td class="small t2">{s.created}</td>
                  <td>{#if s.stateful}<span class="tag">stateful</span>{:else}<span class="muted small">stateless</span>{/if}</td>
                  <td style="text-align:right; white-space:nowrap">
                    <button class="btn sm" disabled={snapBusy !== null} title="Restore"
                      onclick={() => snapEdit(`r-${s.name}`, () => snapshotRestore(sb.name, s.name), `Restored to '${s.name}'`)}><Icon name="arrow" size={13} /></button>
                    <button class="btn sm danger" disabled={snapBusy !== null} title="Delete"
                      onclick={() => snapEdit(`d-${s.name}`, () => snapshotDelete(sb.name, s.name), `Deleted '${s.name}'`)}><Icon name="x" size={13} /></button>
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

{#if gAgent}
  <Modal title={`Guardrails · ${gAgent}`} maxWidth={520} onclose={() => (gAgent = null)}>
    {#snippet body()}
      <p class="hint mb16">An agent's permission bundle — seeded from a profile, refined here. Presets, not yet enforced (Tetragon / Incus ACLs / LiteLLM are the backstops).</p>
      <div class="field mb12"><label for="g-fs">Filesystem</label>
        <input id="g-fs" class="input" bind:value={gForm.filesystem} placeholder="RO repo + docs, RW scratch" /></div>
      <div class="field mb12"><label for="g-net">Network egress</label>
        <input id="g-net" class="input" bind:value={gForm.network} placeholder="Web/docs allowlist via mitmproxy" /></div>
      <div class="field mb12"><label for="g-llm">LLM budget</label>
        <input id="g-llm" class="input mono" bind:value={gForm.llmBudget} placeholder="generous / medium / small" /></div>
      <div class="field mb12"><label for="g-cp">Control-plane</label>
        <input id="g-cp" class="input" bind:value={gForm.controlPlane} placeholder="none" /></div>
      <div class="flex gap12" style="align-items:flex-start">
        <label class="switch"><input type="checkbox" bind:checked={gForm.l3} /><span class="track"></span></label>
        <div><div class="strong small">Nested containers (L3)</div><div class="hint">Allow rootless Docker/Podman.</div></div>
      </div>
    {/snippet}
    {#snippet foot()}
      <button class="btn" onclick={() => (gAgent = null)}>Cancel</button>
      <button class="btn primary" onclick={saveGuardrails} disabled={gBusy}>
        <Icon name="shield" size={15} /><span>{gBusy ? "Saving…" : "Save guardrails"}</span>
      </button>
    {/snippet}
  </Modal>
{/if}

<style>
  .hico { width: 44px; height: 44px; border-radius: 11px; background: var(--accent-dim); color: var(--accent-text); display: grid; place-items: center; flex: none; }
  .hico.off { background: var(--card-2); color: var(--text-3); border: 1px solid var(--border); }
  .btn.sm + .btn.sm { margin-left: 4px; }
  .sub2 { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: .05em; color: var(--text-3); margin-bottom: 8px; }
  .devs { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 10px; }
  .dev { border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--card-2); padding: 10px; }
  .kvs { display: flex; flex-direction: column; gap: 3px; }
  .kvline { display: flex; gap: 10px; font-size: 11.5px; }
  .kk { color: var(--text-3); min-width: 120px; }
  .vv { color: var(--text); overflow-wrap: anywhere; flex: 1; }
  .echip { display: inline-flex; align-items: center; gap: 4px; font-family: var(--mono); font-size: 11px; color: var(--text-2); background: var(--card-2); border: 1px solid var(--border); border-radius: 6px; padding: 2px 4px 2px 8px; }
  .ex { border: none; background: transparent; color: var(--text-3); cursor: pointer; font-size: 14px; line-height: 1; padding: 0 2px; }
  .ex:hover { color: var(--danger); }
  .ex:disabled { opacity: .4; cursor: not-allowed; }
  .input.mini { width: 130px; padding: 4px 8px; font-size: 11.5px; }
  .addmount { display: grid; grid-template-columns: 1fr 1fr auto auto; gap: 8px; align-items: center; }
  .addcfg { display: grid; grid-template-columns: 1fr 1fr auto; gap: 8px; align-items: center; }
  .yaml { white-space: pre; margin: 0; }
</style>
