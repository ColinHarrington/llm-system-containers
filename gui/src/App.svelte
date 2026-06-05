<script lang="ts">
  import Dashboard from "./screens/Dashboard.svelte";
  import Sandboxes from "./screens/Sandboxes.svelte";
  import SandboxDetail from "./screens/SandboxDetail.svelte";
  import Topology from "./screens/Topology.svelte";
  import Incus from "./screens/Incus.svelte";
  import Services from "./screens/Services.svelte";
  import Agent from "./screens/Agent.svelte";
  import Profiles from "./screens/Profiles.svelte";
  import Security from "./screens/Security.svelte";
  import Wizard from "./screens/Wizard.svelte";
  import Progress from "./lib/Progress.svelte";
  import Toast from "./lib/Toast.svelte";
  import Terminal from "./lib/Terminal.svelte";
  import CommandPalette from "./lib/CommandPalette.svelte";
  import ActivityDrawer from "./lib/ActivityDrawer.svelte";
  import BuildImage from "./lib/BuildImage.svelte";
  import NewSandbox from "./lib/NewSandbox.svelte";
  import Icon from "./lib/Icon.svelte";
  import Modal from "./lib/Modal.svelte";
  import { onMount } from "svelte";
  import {
    ui, navigate, openSandbox, bump, toggleTheme, showToast, activity, logActivity, SCREEN_TITLES, type Screen,
  } from "./lib/store.svelte";
  import {
    vmStatus, vmUp, vmDown, listSandboxes, listServices, listAgents, addAgent, listProfiles, agentSteer, onProgress,
  } from "./lib/core";

  // Mirror progress steps into the activity log so long operations leave a reviewable trail.
  onMount(() => onProgress((msg) => logActivity(msg, "progress")));
  import type { Guardrails, ProfileInfo, Sandbox, VmStatus } from "./lib/types";

  const EMPTY_GUARDRAILS: Guardrails = { filesystem: "", network: "", l3: false, llmBudget: "", controlPlane: "" };

  const workspaceNav: { id: Screen; label: string; icon: string }[] = [
    { id: "dashboard", label: "Home", icon: "home" },
    { id: "sandboxes", label: "Sandbox Containers", icon: "box" },
    { id: "topology", label: "Topology", icon: "layers" },
    { id: "agent", label: "Agent control", icon: "agent" },
  ];
  const onSandboxes = $derived(ui.screen === "sandboxes" || ui.screen === "sandbox-detail");
  const platformNav: { id: Screen; label: string; icon: string }[] = [
    { id: "incus", label: "Incus", icon: "layers" },
    { id: "services", label: "Services", icon: "store" },
    { id: "security", label: "Security posture", icon: "shield" },
    { id: "profiles", label: "Agent profiles", icon: "shield" },
    { id: "wizard", label: "Setup wizard", icon: "cog" },
  ];

  let vm = $state<VmStatus | null>(null);
  let vmBusy = $state(false);
  let counts = $state({ sandboxes: 0, agents: 0, services: 0 });
  let sbList = $state<Sandbox[]>([]);

  // Apply theme to <html> whenever it changes.
  $effect(() => {
    document.documentElement.dataset.theme = ui.theme;
  });

  // Keyboard nav: Escape closes overlays; number keys jump between screens (direction A).
  const NUM_NAV: Record<string, Screen> = {
    "1": "dashboard", "2": "sandboxes", "3": "topology", "4": "agent",
    "5": "incus", "6": "services", "7": "profiles",
  };
  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        ui.paletteOpen = !ui.paletteOpen;
        return;
      }
      if (e.key === "Escape") {
        ui.newSandboxOpen = false;
        ui.steerAgent = null;
        ui.terminalTarget = null;
        ui.paletteOpen = false;
        ui.buildImageOpen = false;
        ui.addAgentSandbox = null;
        return;
      }
      const el = e.target as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;
      if (e.key === "/") { e.preventDefault(); ui.paletteOpen = true; return; }
      if (NUM_NAV[e.key]) navigate(NUM_NAV[e.key]);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  $effect(() => {
    ui.dataVersion; // re-run on data changes
    void refreshChrome();
  });

  async function refreshChrome() {
    vm = await vmStatus();
    const [sb, svc, ag] = await Promise.all([listSandboxes(), listServices(), listAgents()]);
    sbList = sb;
    counts = {
      sandboxes: sb.length,
      agents: ag.filter((a) => a.status !== "idle").length,
      services: svc.filter((s) => s.enabled).length,
    };
  }

  const vmDotClass = $derived(
    vm === "Running" ? "ok" : vm === "Starting" ? "warn pulse" : "muted",
  );
  const vmLabel = $derived(vm ?? "…");
  const vmRunning = $derived(vm === "Running");

  async function toggleVm() {
    const wasRunning = vmRunning;
    vmBusy = true;
    showToast(wasRunning ? "$ llmsctl down" : "$ llmsctl up");
    try {
      if (wasRunning) await vmDown();
      else await vmUp();
      bump();
      showToast(wasRunning ? "VM stopped" : "VM is up", "ok");
    } finally {
      vmBusy = false;
    }
  }

  // Add-agent modal
  let agentName = $state("");
  let agentProfile = $state("");
  let agentBusy = $state(false);
  let agentGuardrails = $state<Guardrails>({ ...EMPTY_GUARDRAILS });
  let agentGuardrailsOpen = $state(false);
  let profileList = $state<ProfileInfo[]>([]);
  $effect(() => {
    void listProfiles().then((p) => (profileList = p));
  });
  // Picking a profile seeds the agent's guardrails (the operator can then refine them
  // before creating the agent). Clearing the profile resets to an empty bundle.
  function seedGuardrailsFromProfile() {
    const p = profileList.find((x) => x.name === agentProfile);
    agentGuardrails = p
      ? { filesystem: p.filesystem, network: p.network, l3: p.l3, llmBudget: p.llmBudget, controlPlane: p.controlPlane }
      : { ...EMPTY_GUARDRAILS };
  }
  async function addAgentToSandbox() {
    const sandbox = ui.addAgentSandbox;
    if (!sandbox || !agentName.trim()) return;
    const name = agentName.trim();
    agentBusy = true;
    showToast(`$ llmsc agent add ${name}@${sandbox}${agentProfile ? ` --profile ${agentProfile}` : ""}`);
    try {
      // Send refined guardrails whenever a profile seeded them or the operator opened the editor;
      // otherwise let the backend decide (no profile, no guardrails).
      const g = agentProfile || agentGuardrailsOpen ? agentGuardrails : undefined;
      await addAgent(sandbox, name, agentProfile, g);
      ui.addAgentSandbox = null;
      agentName = "";
      agentProfile = "";
      agentGuardrails = { ...EMPTY_GUARDRAILS };
      agentGuardrailsOpen = false;
      bump();
      showToast(`Agent '${name}' added`, "ok");
    } finally {
      agentBusy = false;
    }
  }

  // Steer modal
  let steerText = $state("");
  let steerBusy = $state(false);
  async function sendSteer() {
    const a = ui.steerAgent;
    if (!a || !steerText.trim()) return;
    steerBusy = true;
    try {
      await agentSteer(a.sandbox, a.name, steerText.trim());
      steerText = "";
      ui.steerAgent = null;
      showToast(`Steering message delivered to ${a.name}`, "ok");
    } catch (e) {
      showToast(String(e), "danger");
    } finally { steerBusy = false; }
  }

  const title = $derived(
    ui.screen === "sandbox-detail"
      ? [ui.selectedSandbox ?? "Sandbox", `Sandbox containers › ${ui.selectedSandbox ?? ""}`]
      : SCREEN_TITLES[ui.screen],
  );
</script>

<div class="app">
  <aside class="sidebar">
    <div class="brand">
      <div class="logo">L</div>
      <div>
        <div class="name">LLM System Containers</div>
        <div class="sub">llmsc · llmsctl</div>
      </div>
    </div>

    <div class="nav-label">Workspace</div>
    {#each workspaceNav as n (n.id)}
      {#if n.id === "sandboxes"}
        <button class="nav-item" class:active={onSandboxes} onclick={() => navigate("sandboxes")}>
          <Icon name={n.icon} />
          <span class="twoline">Sandbox<br />Containers</span>
          {#if counts.sandboxes}<span class="badge">{counts.sandboxes}</span>{/if}
        </button>
        {#if onSandboxes}
          <div class="submenu">
            {#each sbList as s (s.name)}
              <button class="subitem" class:active={ui.screen === "sandbox-detail" && ui.selectedSandbox === s.name} onclick={() => openSandbox(s.name)}>
                <span class="dot {s.status === 'Running' ? 'ok' : 'muted'}" style="width:6px;height:6px"></span>
                <span class="sname mono">{s.name}</span>
              </button>
            {/each}
            {#if sbList.length === 0}<div class="subempty">no sandboxes</div>{/if}
          </div>
        {/if}
      {:else}
        <button class="nav-item" class:active={ui.screen === n.id} onclick={() => navigate(n.id)}>
          <Icon name={n.icon} />
          <span>{n.label}</span>
          {#if n.id === "agent" && counts.agents}<span class="badge">{counts.agents}</span>{/if}
        </button>
      {/if}
    {/each}

    <div class="nav-label">Platform</div>
    {#each platformNav as n (n.id)}
      <button class="nav-item" class:active={ui.screen === n.id} onclick={() => navigate(n.id)}>
        <Icon name={n.icon} />
        <span>{n.label}</span>
        {#if n.id === "services" && counts.services}<span class="badge">{counts.services}</span>{/if}
      </button>
    {/each}

    <div class="sidebar-foot">
      <div class="vm-card">
        <div class="row">
          <span class="dot {vmDotClass}"></span>
          <span class="vm-name">VM</span>
          <span class="right small t2">{vmLabel}</span>
        </div>
        <div class="vm-meta">llmsc-vm · Lima</div>
        <button
          class="btn sm"
          style="width:100%; margin-top:10px; justify-content:center"
          onclick={toggleVm}
          disabled={vmBusy}
        >
          <Icon name={vmRunning ? "stop" : "play"} size={14} />
          <span>{vmRunning ? "Stop" : "Start"}</span>
        </button>
      </div>
      <div class="flex gap8" style="padding:0 2px">
        <div class="avatar human sm">CH</div>
        <div class="small">
          <div class="strong" style="font-size:12px">operator</div>
          <div class="muted xsmall">colin · host</div>
        </div>
        <kbd style="margin-left:auto">⌘K</kbd>
      </div>
    </div>
  </aside>

  <main class="main">
    <div class="topbar">
      <div>
        <h1>{title[0]}</h1>
        <div class="crumb">{title[1]}</div>
      </div>
      <div class="spacer"></div>
      <button class="searchbox" onclick={() => (ui.paletteOpen = true)}>
        <Icon name="search" size={14} />
        <span>Search sandboxes, users, traces…</span>
        <kbd>⌘K</kbd>
      </button>
      <button class="iconbtn" class:on={ui.activityOpen} title="Activity log" aria-label="Activity log" onclick={() => (ui.activityOpen = !ui.activityOpen)}>
        <Icon name="bell" />
        {#if activity.length > 0}<span class="badge">{activity.length > 99 ? "99+" : activity.length}</span>{/if}
      </button>
      <button class="iconbtn" title="Toggle theme" aria-label="Toggle theme" onclick={toggleTheme}>
        <Icon name={ui.theme === "dark" ? "sun" : "moon"} />
      </button>
      <button class="btn primary" onclick={() => (ui.newSandboxOpen = true)}>
        <Icon name="plus" size={14} /><span>New sandbox</span>
      </button>
    </div>

    <div class="main-scroll">
    {#if ui.screen === "dashboard"}
      <Dashboard />
    {:else if ui.screen === "sandboxes"}
      <Sandboxes />
    {:else if ui.screen === "sandbox-detail"}
      <SandboxDetail />
    {:else if ui.screen === "topology"}
      <Topology />
    {:else if ui.screen === "agent"}
      <Agent />
    {:else if ui.screen === "incus"}
      <Incus />
    {:else if ui.screen === "services"}
      <Services />
    {:else if ui.screen === "profiles"}
      <Profiles />
    {:else if ui.screen === "security"}
      <Security />
    {:else if ui.screen === "wizard"}
      <Wizard />
    {/if}
    </div>
  </main>

  <Progress />
  <Toast />
  <Terminal />
  <CommandPalette />
  <ActivityDrawer />
</div>

{#if ui.newSandboxOpen}
  <NewSandbox />
{/if}

{#if ui.buildImageOpen}
  <BuildImage />
{/if}

{#if ui.addAgentSandbox}
  <Modal title={`Add agent to ${ui.addAgentSandbox}`} maxWidth={460} onclose={() => (ui.addAgentSandbox = null)}>
    {#snippet body()}
      <p class="hint mb12">An agent is one Linux user in the sandbox (scoped, its own virtual key later). Add a username.</p>
      <div class="field mb16"><label for="ag-name">Agent username</label>
        <input id="ag-name" class="input mono" bind:value={agentName} placeholder="agent-claude" /></div>
      <div class="field mb16"><label for="ag-profile">Profile <span class="hint">(seeds guardrails)</span></label>
        <select id="ag-profile" class="input" bind:value={agentProfile} onchange={seedGuardrailsFromProfile}>
          <option value="">none</option>
          {#each profileList as p}<option value={p.name}>{p.name} — {p.summary}</option>{/each}
        </select></div>

      <button type="button" class="disclosure" class:open={agentGuardrailsOpen} onclick={() => (agentGuardrailsOpen = !agentGuardrailsOpen)}>
        <span class="chev"><Icon name="chevron" size={14} /></span>
        <span>Guardrails {agentProfile ? `(seeded from ${agentProfile})` : "(custom)"}</span>
      </button>
      {#if agentGuardrailsOpen}
        <div class="guardrails-edit">
          <p class="hint mb12">Refine the agent's permission bundle before creating it. Presets, not yet enforced (Tetragon / Incus ACLs / LiteLLM are the backstops).</p>
          <div class="field mb12"><label for="ag-fs">Filesystem</label>
            <input id="ag-fs" class="input" bind:value={agentGuardrails.filesystem} placeholder="RO repo + docs, RW scratch" /></div>
          <div class="field mb12"><label for="ag-net">Network egress</label>
            <input id="ag-net" class="input" bind:value={agentGuardrails.network} placeholder="Web/docs allowlist via mitmproxy" /></div>
          <div class="field mb12"><label for="ag-llm">LLM budget</label>
            <input id="ag-llm" class="input mono" bind:value={agentGuardrails.llmBudget} placeholder="generous / medium / small" /></div>
          <div class="field mb12"><label for="ag-cp">Control-plane</label>
            <input id="ag-cp" class="input" bind:value={agentGuardrails.controlPlane} placeholder="none" /></div>
          <div class="flex gap12" style="align-items:flex-start">
            <label class="switch"><input type="checkbox" bind:checked={agentGuardrails.l3} /><span class="track"></span></label>
            <div><div class="strong small">Nested containers (L3)</div><div class="hint">Allow rootless Docker/Podman.</div></div>
          </div>
        </div>
      {/if}
    {/snippet}
    {#snippet foot()}
      <button class="btn" onclick={() => (ui.addAgentSandbox = null)}>Cancel</button>
      <button class="btn primary" onclick={addAgentToSandbox} disabled={agentBusy || !agentName.trim()}>
        <Icon name="plus" size={15} /><span>{agentBusy ? "Adding…" : "Add agent"}</span>
      </button>
    {/snippet}
  </Modal>
{/if}

{#if ui.steerAgent}
  <Modal title={`Steer ${ui.steerAgent.name}`} maxWidth={540} onclose={() => (ui.steerAgent = null)}>
    {#snippet body()}
      <p class="hint mb12">Inject a message into the running agent's context. It will be delivered at the next safe checkpoint.</p>
      <textarea class="input mono" rows="4" bind:value={steerText}
        placeholder="e.g. Don't touch the migrations — only fix the token TTL, then open a draft PR."></textarea>
      <div class="flex gap8 mt12">
        <span class="pill warn"><Icon name="hand" size={13} /> pause after delivering</span>
        <span class="pill accent">high priority</span>
      </div>
    {/snippet}
    {#snippet foot()}
      <button class="btn" onclick={() => (ui.steerAgent = null)}>Cancel</button>
      <button class="btn primary" onclick={sendSteer} disabled={steerBusy || !steerText.trim()}><Icon name="arrow" size={15} /><span>{steerBusy ? "Sending…" : "Send to agent"}</span></button>
    {/snippet}
  </Modal>
{/if}

<style>
  .searchbox {
    display: flex; align-items: center; gap: 8px;
    width: 280px; padding: 7px 10px; border-radius: var(--radius-sm);
    background: var(--card-2); border: 1px solid var(--border);
    color: var(--text-3); font-size: 12px; font-family: inherit;
    cursor: pointer; text-align: left;
  }
  .searchbox:hover { border-color: var(--border-strong); }
  .searchbox span { flex: 1; }

  .twoline { line-height: 1.15; }
  .submenu { display: flex; flex-direction: column; gap: 1px; margin: 2px 0 4px 0; padding-left: 12px; }
  .submenu::before { content: ""; }
  .subitem {
    display: flex; align-items: center; gap: 8px; width: 100%; text-align: left;
    border: none; background: transparent; cursor: pointer; font-family: inherit;
    color: var(--text-3); font-size: 12px; padding: 5px 9px; border-radius: var(--radius-sm);
    border-left: 1px solid var(--border); padding-left: 12px; margin-left: 6px;
  }
  .subitem:hover { background: rgba(255, 255, 255, 0.03); color: var(--text); }
  .subitem.active { background: var(--accent-soft-bg); color: var(--accent-text); }
  .subitem .sname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .subempty { color: var(--text-3); font-size: 11px; padding: 4px 9px 4px 18px; }
  @media (max-width: 820px) { .searchbox { display: none; } }

  .disclosure {
    display: flex; align-items: center; gap: 7px; width: 100%;
    background: none; border: none; padding: 4px 0; cursor: pointer;
    color: var(--text-2); font-size: 13px; font-weight: 500;
  }
  .disclosure:hover { color: var(--text); }
  .disclosure .chev { display: inline-flex; transition: transform 0.12s ease; }
  .disclosure.open .chev { transform: rotate(90deg); }
  .guardrails-edit {
    margin-top: 10px; padding: 12px; border: 1px solid var(--border);
    border-radius: 9px; background: rgba(255, 255, 255, 0.02);
  }
</style>
