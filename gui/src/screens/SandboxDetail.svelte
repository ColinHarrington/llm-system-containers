<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Modal from "../lib/Modal.svelte";
  import { ui, navigate, bump, openTerminal, showToast } from "../lib/store.svelte";
  import {
    topology, removeSandbox, removeAgent, instanceConfig,
    instanceSetConfig, instanceUnsetConfig, instanceAddMount, instanceRemoveDevice,
    instanceAddProfile, instanceRemoveProfile, applySandbox, instanceYaml,
    listSnapshots, snapshotCreate, snapshotRestore, snapshotDelete, setAgentGuardrails,
    egressPolicy, setEgressPolicy, egressAclPreview, applyEgress, egressStatus,
    tetragonPolicies, tetragonPolicyYaml, applyTetragonPolicies, setWorkspaceReadonly,
    enforcementOverview, enforceAll, agentPause, agentResume, agentStop, mountShared,
  } from "../lib/core";
  import type {
    EgressPolicy, EgressPosture, EgressStatus, Guardrails, InstanceConfig, NetworkAclInfo,
    RingStatus, SnapshotInfo, TetragonPolicy, TopoAgent, TopoSandbox,
  } from "../lib/types";

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

  async function controlAgent(agent: string, fn: () => Promise<void>, msg: string) {
    if (!sb) return;
    userBusy = agent;
    try {
      await fn();
      showToast(msg, "ok");
    } catch (e) {
      showToast(String(e), "danger");
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

  // --- Unified enforcement overview (all rings) ---
  let rings = $state<RingStatus[]>([]);
  let enforceBusy = $state(false);
  $effect(() => {
    ui.dataVersion;
    const sel = ui.selectedSandbox;
    rings = [];
    if (sel) void enforcementOverview(sel).then((r) => (rings = r)).catch(() => (rings = []));
  });
  async function doEnforceAll() {
    if (!sb) return;
    enforceBusy = true;
    showToast(`$ llmsctl enforce all ${sb.name}`);
    try {
      rings = await enforceAll(sb.name);
      showToast("Enforcement applied", "ok");
      bump();
    } catch (e) {
      showToast(String(e), "danger");
    } finally { enforceBusy = false; }
  }
  const ringPill = (s: RingStatus["state"]) =>
    s === "enforced" ? "ok" : s === "off" ? "" : "warn";

  // --- Network egress (per-container enforcement ring) ---
  let egress = $state<EgressPolicy | null>(null);
  let egressAcl = $state<NetworkAclInfo | null>(null);
  let egStatus = $state<EgressStatus | null>(null);
  let egressBusy = $state(false);
  let enforcing = $state(false);
  let newAllow = $state("");
  const NAMED_SETS = ["llm", "package-registries", "web"];
  $effect(() => {
    ui.dataVersion;
    const sel = ui.selectedSandbox;
    egress = null;
    egressAcl = null;
    egStatus = null;
    if (sel) {
      void egressPolicy(sel).then((p) => (egress = p)).catch(() => (egress = null));
      void egressAclPreview(sel).then((a) => (egressAcl = a)).catch(() => (egressAcl = null));
      void egressStatus(sel).then((s) => (egStatus = s)).catch(() => (egStatus = null));
    }
  });
  async function saveEgress(next: EgressPolicy) {
    if (!sb) return;
    egressBusy = true;
    try {
      await setEgressPolicy(sb.name, next);
      egress = next;
      egressAcl = await egressAclPreview(sb.name).catch(() => null);
      egStatus = await egressStatus(sb.name).catch(() => null);
      showToast("Egress policy saved (not yet enforced)", "ok");
    } catch (e) {
      showToast(String(e), "danger");
    } finally { egressBusy = false; }
  }
  // Merge a partial change onto the current policy (preserves the other fields).
  function patchEgress(patch: Partial<EgressPolicy>) {
    const cur = egress ?? { posture: "allowlist" as EgressPosture, allow: [], domains: [] };
    saveEgress({ posture: cur.posture, allow: cur.allow, domains: cur.domains, ...patch });
  }
  function setPosture(posture: EgressPosture) {
    patchEgress({ posture });
  }
  function addAllow() {
    const e = newAllow.trim();
    if (!e || !egress || egress.allow.includes(e)) { newAllow = ""; return; }
    newAllow = "";
    patchEgress({ allow: [...egress.allow, e] });
  }
  function removeAllow(entry: string) {
    if (!egress) return;
    patchEgress({ allow: egress.allow.filter((a) => a !== entry) });
  }
  let newDomain = $state("");
  function addDomain() {
    const d = newDomain.trim();
    if (!d || !egress || egress.domains.includes(d)) { newDomain = ""; return; }
    newDomain = "";
    patchEgress({ domains: [...egress.domains, d] });
  }
  function removeDomain(d: string) {
    if (!egress) return;
    patchEgress({ domains: egress.domains.filter((x) => x !== d) });
  }
  // --- Tetragon per-UID kernel policies ---
  let tetraPols = $state<TetragonPolicy[]>([]);
  let tetraBusy = $state(false);
  let tetraYaml = $state<string | null>(null);
  let tetraYamlAgent = $state<string | null>(null);
  $effect(() => {
    ui.dataVersion;
    const sel = ui.selectedSandbox;
    tetraPols = [];
    if (sel) void tetragonPolicies(sel).then((p) => (tetraPols = p)).catch(() => (tetraPols = []));
  });
  async function viewTetraYaml(agent: string) {
    if (!sb) return;
    if (tetraYamlAgent === agent) { tetraYamlAgent = null; tetraYaml = null; return; }
    try {
      tetraYaml = await tetragonPolicyYaml(sb.name, agent);
      tetraYamlAgent = agent;
    } catch (e) { showToast(String(e), "danger"); }
  }
  async function loadTetra() {
    if (!sb) return;
    tetraBusy = true;
    showToast(`$ llmsctl tetragon apply ${sb.name}`);
    try {
      const n = await applyTetragonPolicies(sb.name);
      showToast(n === 0 ? "No agent policies to load" : `Loaded ${n} Tetragon policy(ies)`, "ok");
    } catch (e) {
      showToast(String(e), "danger");
    } finally { tetraBusy = false; }
  }
  // Whether any agent's filesystem posture is read-only (drives the workspace-RO suggestion).
  const anyReadOnly = $derived(tetraPols.some((p) => p.readOnly));
  async function setWorkspaceRo(ro: boolean) {
    if (!sb) return;
    tetraBusy = true;
    try {
      const n = await setWorkspaceReadonly(sb.name, ro);
      showToast(n === 0 ? "No workspace mounts to change" : `Workspace set ${ro ? "read-only" : "read-write"} (${n} mount(s))`, "ok");
      bump();
    } catch (e) {
      showToast(String(e), "danger");
    } finally { tetraBusy = false; }
  }

  async function enforceEgress() {
    if (!sb) return;
    egressBusy = true;
    enforcing = true;
    showToast(`$ llmsc egress apply ${sb.name}`);
    try {
      const n = await applyEgress(sb.name);
      showToast(n === 0 ? "Egress torn down (open)" : `Egress enforced — ${n} ACL change(s)`, "ok");
      egStatus = await egressStatus(sb.name).catch(() => null);
      bump();
    } catch (e) {
      showToast(String(e), "danger");
    } finally { egressBusy = false; enforcing = false; }
  }

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

  async function mountSharedVol() {
    if (!sb) return;
    await edit(() => mountShared(sb.name, "/shared"), "Mounted shared volume at /shared");
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

    <!-- Enforcement overview (all rings) -->
    {#if rings.length > 0}
      <div class="card mb16">
        <div class="card-head"><h3>Enforcement</h3>
          <span class="sub">defense-in-depth rings · live status</span>
          <button class="btn sm primary right" disabled={enforceBusy} onclick={doEnforceAll}
            title="Apply every applicable ring (egress + L7 + workspace RO; loads Tetragon + syncs keys best-effort)">
            <Icon name="shield" size={13} /><span>{enforceBusy ? "Enforcing…" : "Enforce all"}</span></button>
        </div>
        <div class="pad">
          <div class="rings">
            {#each rings as r (r.ring)}
              <div class="ringrow">
                <span class="rname">{r.ring}</span>
                <span class="pill {ringPill(r.state)}">{r.state}</span>
                <span class="muted small">{r.detail}</span>
              </div>
            {/each}
          </div>
        </div>
      </div>
    {/if}

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
                      <button class="btn sm" title="Pause agent" disabled={userBusy === u.name} onclick={() => controlAgent(u.name, () => agentPause(sb.name, u.name), `Paused ${u.name}`)}><Icon name="pause" size={13} /></button>
                      <button class="btn sm" title="Resume agent" disabled={userBusy === u.name} onclick={() => controlAgent(u.name, () => agentResume(sb.name, u.name), `Resumed ${u.name}`)}><Icon name="play" size={13} /></button>
                      <button class="btn sm" title="Stop agent" disabled={userBusy === u.name} onclick={() => controlAgent(u.name, () => agentStop(sb.name, u.name), `Stopped ${u.name}`)}><Icon name="stop" size={13} /></button>
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
          <div class="flex gap8 mb16" style="align-items:center">
            <span class="muted small">Shared storage (SeaweedFS-backed Incus volume, shared across sandboxes):</span>
            <button class="btn sm" disabled={cfgBusy} onclick={mountSharedVol}><Icon name="pkg" size={13} /> Mount shared at /shared</button>
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

    <!-- Network egress (per-container enforcement ring) -->
    <div class="card mt16">
      <div class="card-head"><h3>Network egress</h3>
        <span class="sub">container-level ACL · <span class="mono">incus network acl</span></span>
        {#if egStatus}
          {#if !egStatus.managed || egStatus.posture === "open"}
            <span class="pill" title="No ACL bound to the nic">not enforced</span>
          {:else if egStatus.inSync && egStatus.bound}
            <span class="pill ok"><span class="dot ok"></span> enforced</span>
          {:else if egStatus.bound}
            <span class="pill warn" title="Bound, but live ACL differs from intent">drifted</span>
          {:else}
            <span class="pill warn" title="Policy set but not yet applied">not enforced</span>
          {/if}
        {/if}
        <button class="btn sm primary right" disabled={egressBusy || egress === null || egress.posture === "open"} onclick={enforceEgress}
          title="Compile the policy to an Incus ACL and bind it to the nic (default-drop)">
          <Icon name="shield" size={13} /><span>{enforcing ? "Enforcing…" : "Apply (enforce)"}</span></button>
      </div>
      <div class="pad">
        <p class="hint mb12">Compiles to an Incus network ACL bound to the nic. <strong>Container-level</strong> — applies to every UID in the sandbox. Per-agent egress is a later Tetragon ring. ACLs are L3/L4 only; domain allowlists (mitmproxy) come later, so <span class="mono">web</span> is coarse.</p>

        {#if egress === null}
          <div class="flex gap8" style="align-items:center">
            <span class="muted small">Unmanaged — no ACL applied.</span>
            <button class="btn sm" disabled={egressBusy} onclick={() => saveEgress({ posture: "allowlist", allow: ["llm"], domains: [] })}>Manage egress</button>
          </div>
        {:else}
          <!-- posture -->
          <div class="sub2">Posture</div>
          <div class="flex gap6 mb16">
            {#each (["deny-all", "allowlist", "open"] as EgressPosture[]) as p}
              <button class="btn sm" class:primary={egress.posture === p} disabled={egressBusy} onclick={() => setPosture(p)}>{p}</button>
            {/each}
          </div>

          {#if egress.posture === "allowlist"}
            <div class="sub2">Allowed destinations</div>
            <div class="flex gap6 wrap mb12">
              {#each egress.allow as a}
                <span class="echip">{a}<button class="ex" title="Remove" disabled={egressBusy} onclick={() => removeAllow(a)}>×</button></span>
              {/each}
              {#if egress.allow.length === 0}<span class="muted small">none — all egress dropped</span>{/if}
            </div>
            <div class="flex gap6 wrap mb16" style="align-items:center">
              {#each NAMED_SETS.filter((s) => !egress!.allow.includes(s)) as s}
                <button class="btn sm" disabled={egressBusy} onclick={() => patchEgress({ allow: [...egress!.allow, s] })}>+ {s}</button>
              {/each}
              <input class="input mini mono" bind:value={newAllow} placeholder="CIDR:port (e.g. 10.0.0.0/8:443)" onkeydown={(e) => { if (e.key === 'Enter') addAllow(); }} />
              <button class="btn sm" disabled={egressBusy || !newAllow.trim()} onclick={addAllow}>Add</button>
            </div>
          {/if}

          {#if egress.posture !== "open"}
            <div class="sub2">HTTP(S) domains <span class="hint">(L7 · mitmproxy)</span></div>
            <div class="flex gap6 wrap mb8">
              {#each egress.domains as d}
                <span class="echip">{d}<button class="ex" title="Remove" disabled={egressBusy} onclick={() => removeDomain(d)}>×</button></span>
              {/each}
              {#if egress.domains.length === 0}<span class="muted small">none — no L7 domain allowlist (Incus ACL still applies)</span>{/if}
            </div>
            <div class="flex gap6 wrap mb12" style="align-items:center">
              <input class="input mini mono" bind:value={newDomain} placeholder="domain (e.g. github.com)" onkeydown={(e) => { if (e.key === 'Enter') addDomain(); }} />
              <button class="btn sm" disabled={egressBusy || !newDomain.trim()} onclick={addDomain}>Add domain</button>
            </div>
            {#if egress.domains.length > 0}
              <p class="xsmall muted mb12">Routes the sandbox HTTP(S) through mitmproxy. Provision <span class="mono">mitmproxy</span> in Services; HTTPS interception needs the CA trusted and forced routing (follow-ups).</p>
            {/if}
          {/if}

          <!-- compiled ACL preview -->
          {#if egressAcl}
            <div class="sub2">Compiled ACL <span class="muted mono" style="text-transform:none">{egressAcl.name}</span></div>
            {#if egressAcl.egress.length === 0}
              <div class="muted small">No allow rules — the nic default-drop blocks all egress.</div>
            {:else}
              <table class="tbl">
                <thead><tr><th>Action</th><th>Destination</th><th>Port</th><th>Proto</th><th>Note</th></tr></thead>
                <tbody>
                  {#each egressAcl.egress as r}
                    <tr>
                      <td>{#if r.action === "allow"}<span class="pill ok">allow</span>{:else}<span class="pill">{r.action}</span>{/if}</td>
                      <td class="mono small">{r.destination || "—"}</td>
                      <td class="mono small">{r.port || "any"}</td>
                      <td class="mono small">{r.protocol || "any"}</td>
                      <td class="muted small">{r.description}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/if}
          {:else if egress.posture === "open"}
            <div class="muted small">Open — no ACL is created or bound.</div>
          {/if}
        {/if}
      </div>
    </div>

    <!-- Kernel enforcement (Tetragon per-UID) -->
    <div class="card mt16">
      <div class="card-head"><h3>Kernel enforcement</h3>
        <span class="sub">per-UID Tetragon (eBPF) · <span class="mono">TracingPolicy</span></span>
        <span class="pill warn right" title="Generated draft; requires Tetragon installed in the VM">draft</span>
        <button class="btn sm" disabled={tetraBusy || tetraPols.length === 0} onclick={loadTetra}
          title="Write the compiled policies into the VM and reload Tetragon (requires Tetragon installed)">
          <Icon name="shield" size={13} /><span>{tetraBusy ? "Loading…" : "Load policies"}</span></button>
      </div>
      <div class="pad">
        <p class="hint mb12">One policy per agent (per Linux UID): denies dangerous syscalls and carries the egress posture. The non-bypassable kernel ring. <strong>Draft</strong> — the TracingPolicy schema must be validated against the installed Tetragon.</p>
        {#if tetraPols.length === 0}
          <div class="muted small">No agents — nothing to enforce at the kernel.</div>
        {:else}
          {#each tetraPols as p (p.name)}
            <div class="dev mb8">
              <div class="flex gap8 mb4">
                <span class="mono small strong" style="color:var(--text)">{p.agent}</span>
                <span class="tag">{p.deniedSyscalls.length} syscalls denied</span>
                {#if p.readOnly}<span class="pill" title="Filesystem posture is read-only">RO fs</span>{/if}
                <button class="btn sm right" onclick={() => viewTetraYaml(p.agent)}><Icon name="doc" size={12} /> {tetraYamlAgent === p.agent ? "Hide" : "YAML"}</button>
              </div>
              <div class="muted small mono">egress: {p.egressNote}</div>
              <div class="muted small mono">fs: {p.fsNote}</div>
              {#if tetraYamlAgent === p.agent && tetraYaml}
                <pre class="console yaml mt8">{tetraYaml}</pre>
              {/if}
            </div>
          {/each}
        {/if}

        <!-- Workspace filesystem (per-container mount RO — the real backstop today) -->
        <div class="sub2 mt12">Workspace mounts</div>
        <div class="flex gap8" style="align-items:center">
          <span class="muted small">Set the sandbox's workspace mounts (per-container; affects all UIDs):</span>
          <button class="btn sm" disabled={tetraBusy} onclick={() => setWorkspaceRo(true)}>Read-only</button>
          <button class="btn sm" disabled={tetraBusy} onclick={() => setWorkspaceRo(false)}>Read-write</button>
        </div>
        {#if anyReadOnly}
          <p class="xsmall muted mt4">An agent here has a read-only filesystem posture — set the workspace read-only to back it with a real mount-level grant. Per-UID path rules are Tetragon's job.</p>
        {/if}
      </div>
    </div>

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
  .rings { display: flex; flex-direction: column; gap: 6px; }
  .ringrow { display: grid; grid-template-columns: 150px 90px 1fr; gap: 10px; align-items: center; }
  .ringrow .rname { font-weight: 600; font-size: 12.5px; color: var(--text); }
</style>
