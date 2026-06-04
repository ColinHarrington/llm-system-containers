<script lang="ts">
  import Dashboard from "./screens/Dashboard.svelte";
  import Sandboxes from "./screens/Sandboxes.svelte";
  import Services from "./screens/Services.svelte";
  import Agent from "./screens/Agent.svelte";
  import Images from "./screens/Images.svelte";
  import Wizard from "./screens/Wizard.svelte";
  import Progress from "./lib/Progress.svelte";
  import Icon from "./lib/Icon.svelte";
  import Modal from "./lib/Modal.svelte";
  import {
    ui, navigate, bump, toggleTheme, SCREEN_TITLES, type Screen,
  } from "./lib/store.svelte";
  import {
    vmStatus, vmUp, vmDown, listSandboxes, listServices, listAgents, launchSandbox,
  } from "./lib/core";
  import type { VmStatus } from "./lib/types";

  const workspaceNav: { id: Screen; label: string; icon: string }[] = [
    { id: "dashboard", label: "Home", icon: "home" },
    { id: "sandboxes", label: "Sandboxes", icon: "box" },
    { id: "agent", label: "Agent control", icon: "agent" },
  ];
  const platformNav: { id: Screen; label: string; icon: string }[] = [
    { id: "services", label: "Services", icon: "layers" },
    { id: "images", label: "Images", icon: "image" },
    { id: "wizard", label: "Setup wizard", icon: "cog" },
  ];

  let vm = $state<VmStatus | null>(null);
  let vmBusy = $state(false);
  let counts = $state({ sandboxes: 0, agents: 0, services: 0 });

  // Apply theme to <html> whenever it changes.
  $effect(() => {
    document.documentElement.dataset.theme = ui.theme;
  });

  $effect(() => {
    ui.dataVersion; // re-run on data changes
    void refreshChrome();
  });

  async function refreshChrome() {
    vm = await vmStatus();
    const [sb, svc, ag] = await Promise.all([listSandboxes(), listServices(), listAgents()]);
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
    vmBusy = true;
    try {
      if (vmRunning) await vmDown();
      else await vmUp();
      bump();
    } finally {
      vmBusy = false;
    }
  }

  // New-sandbox modal form
  let sbName = $state("");
  let sbImage = $state("dev-ubuntu-24.04");
  let sbNesting = $state(true);
  let sbBusy = $state(false);

  async function createSandbox() {
    if (!sbName.trim()) return;
    sbBusy = true;
    try {
      await launchSandbox(sbName.trim(), sbImage.trim(), sbNesting);
      ui.newSandboxOpen = false;
      sbName = "";
      navigate("sandboxes");
      bump();
    } finally {
      sbBusy = false;
    }
  }

  // Steer modal
  let steerText = $state("");
  function sendSteer() {
    steerText = "";
    ui.steerAgent = null;
  }

  const title = $derived(SCREEN_TITLES[ui.screen]);
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
      <button class="nav-item" class:active={ui.screen === n.id} onclick={() => navigate(n.id)}>
        <Icon name={n.icon} />
        <span>{n.label}</span>
        {#if n.id === "sandboxes" && counts.sandboxes}<span class="badge">{counts.sandboxes}</span>{/if}
        {#if n.id === "agent" && counts.agents}<span class="badge">{counts.agents}</span>{/if}
      </button>
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
          <div class="strong" style="font-size:12.5px">operator</div>
          <div class="muted xsmall">colin · host</div>
        </div>
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
      <button class="btn" onclick={() => (ui.newSandboxOpen = true)}>
        <Icon name="plus" /><span>New sandbox</span>
      </button>
      <button class="iconbtn" title="Notifications" aria-label="Notifications"><Icon name="bell" /></button>
      <button class="iconbtn" title="Toggle theme" aria-label="Toggle theme" onclick={toggleTheme}>
        <Icon name={ui.theme === "dark" ? "sun" : "moon"} />
      </button>
    </div>

    {#if ui.screen === "dashboard"}
      <Dashboard />
    {:else if ui.screen === "sandboxes"}
      <Sandboxes />
    {:else if ui.screen === "agent"}
      <Agent />
    {:else if ui.screen === "services"}
      <Services />
    {:else if ui.screen === "images"}
      <Images />
    {:else if ui.screen === "wizard"}
      <Wizard />
    {/if}
  </main>

  <Progress />
</div>

{#if ui.newSandboxOpen}
  <Modal title="New sandbox" onclose={() => (ui.newSandboxOpen = false)}>
    {#snippet body()}
      <div class="field mb16"><label for="sb-name">Name</label>
        <input id="sb-name" class="input mono" bind:value={sbName} placeholder="web-agent-02" /></div>
      <div class="field mb16"><label for="sb-image">Image</label>
        <select id="sb-image" class="input" bind:value={sbImage}>
          <option value="dev-ubuntu-24.04">dev-ubuntu-24.04 — general dev workspace</option>
          <option value="browser-tools">browser-tools — headed browser automation</option>
          <option value="data-tools">data-tools — data pipelines</option>
          <option value="images:alpine/3.21">images:alpine/3.21 — minimal sandbox</option>
          <option value="base-debian-12">base-debian-12 — minimal</option>
        </select></div>
      <div class="flex gap12" style="align-items:flex-start">
        <label class="switch"><input type="checkbox" bind:checked={sbNesting} /><span class="track"></span></label>
        <div><div class="strong small">Enable nested containers (L3)</div>
          <div class="hint">Rootless Docker/Podman inside the sandbox — no privileged DinD.</div></div>
      </div>
    {/snippet}
    {#snippet foot()}
      <span class="code-chip mono" style="margin-right:auto">llmsc launch {sbName || "name"} --image {sbImage}</span>
      <button class="btn" onclick={() => (ui.newSandboxOpen = false)}>Cancel</button>
      <button class="btn primary" onclick={createSandbox} disabled={sbBusy || !sbName.trim()}>
        <Icon name="play" size={15} /><span>{sbBusy ? "Launching…" : "Launch sandbox"}</span>
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
      <button class="btn primary" onclick={sendSteer}><Icon name="arrow" size={15} /><span>Send to agent</span></button>
    {/snippet}
  </Modal>
{/if}
