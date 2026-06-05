<script lang="ts">
  import Modal from "./Modal.svelte";
  import Icon from "./Icon.svelte";
  import { ui, bump, navigate, showToast } from "./store.svelte";
  import { launchSandbox, operatorDefault, type SandboxMount } from "./core";

  // Create form modeled on the Incus instance surface (breadth-first; some sections stubbed).
  let name = $state("");
  let image = $state("images:alpine/3.21");
  let operator = $state("");
  let description = $state("");
  let nesting = $state(true);
  let ephemeral = $state(false);
  let mounts = $state<SandboxMount[]>([]);
  let network = $state("");
  let cloudInit = $state("");
  let profilesStr = $state("");
  let cpuLimit = $state("");
  let memoryLimit = $state("");
  let busy = $state(false);

  $effect(() => {
    if (!operator) void operatorDefault().then((o) => (operator = o));
  });

  function addMount() {
    mounts = [...mounts, { source: "", path: "", readonly: false }];
  }
  function removeMount(i: number) {
    mounts = mounts.filter((_, j) => j !== i);
  }

  function close() { ui.newSandboxOpen = false; }

  async function submit() {
    if (!name.trim() || !operator.trim()) return;
    busy = true;
    showToast(`$ llmsc launch ${name.trim()} --image ${image}`);
    try {
      await launchSandbox({
        name: name.trim(),
        image: image.trim(),
        operator: operator.trim(),
        description: description.trim(),
        ephemeral,
        nesting,
        profiles: profilesStr.split(/[\s,]+/).filter(Boolean),
        mounts: mounts.filter((m) => m.source.trim() && m.path.trim()),
        cloudInit,
        network: network.trim(),
        cpuLimit: cpuLimit.trim(),
        memoryLimit: memoryLimit.trim(),
      });
      close();
      navigate("sandboxes");
      bump();
      showToast(`Launched ${name.trim()}`, "ok");
    } finally {
      busy = false;
    }
  }
</script>

<Modal title="New sandbox" maxWidth={640} onclose={close}>
  {#snippet body()}
    <div class="sec">Basics</div>
    <div class="field mb12"><label for="ns-name">Name</label>
      <input id="ns-name" class="input mono" bind:value={name} placeholder="web-agent-02" /></div>
    <div class="field mb12"><label for="ns-op">Your username <span class="hint">(human operator — default Linux user)</span></label>
      <input id="ns-op" class="input mono" bind:value={operator} placeholder="operator" /></div>
    <div class="field mb12"><label for="ns-img">Image</label>
      <input id="ns-img" class="input mono" bind:value={image} placeholder="images:alpine/3.21" />
      <div class="flex gap6 wrap mt8">
        {#each ["images:alpine/3.21", "images:debian/12", "images:ubuntu/24.04"] as b}
          <button class="chip" class:on={image === b} onclick={() => (image = b)}>{b}</button>
        {/each}
      </div></div>
    <div class="field mb16"><label for="ns-desc">Description <span class="hint">(optional)</span></label>
      <input id="ns-desc" class="input" bind:value={description} placeholder="what this sandbox is for" /></div>

    <div class="sec">Options</div>
    <div class="flex gap12 mb12" style="align-items:flex-start">
      <label class="switch"><input type="checkbox" bind:checked={nesting} /><span class="track"></span></label>
      <div><div class="strong small">Nested containers (L3)</div><div class="hint">Rootless Docker/Podman — <span class="mono">security.nesting</span>.</div></div>
    </div>
    <div class="flex gap12 mb16" style="align-items:flex-start">
      <label class="switch"><input type="checkbox" bind:checked={ephemeral} /><span class="track"></span></label>
      <div><div class="strong small">Ephemeral</div><div class="hint">Delete the sandbox when it stops.</div></div>
    </div>

    <div class="sec flex"><span>Workspace mounts</span><button class="btn sm right" onclick={addMount}><Icon name="plus" size={13} /> Add</button></div>
    {#if mounts.length === 0}
      <div class="hint mb12">No mounts. Host directories bind-mounted into the sandbox (idmapped) as <span class="mono">disk</span> devices.</div>
    {:else}
      {#each mounts as m, i (i)}
        <div class="mountrow mb8">
          <input class="input mono" bind:value={m.source} placeholder="~/projects/app (host)" />
          <input class="input mono" bind:value={m.path} placeholder="/work/app (container)" />
          <label class="ro"><input type="checkbox" bind:checked={m.readonly} /> ro</label>
          <button class="btn sm danger" onclick={() => removeMount(i)}><Icon name="x" size={13} /></button>
        </div>
      {/each}
    {/if}

    <div class="sec mt16">Networking</div>
    <div class="field mb16"><label for="ns-net">Network <span class="hint">(optional — Incus bridge for eth0; blank = default profile)</span></label>
      <input id="ns-net" class="input mono" bind:value={network} placeholder="incusbr0" /></div>

    <div class="sec">Provisioning</div>
    <div class="field mb16"><label for="ns-ci">cloud-init <span class="hint">(optional · <span class="mono">cloud-init.user-data</span>)</span></label>
      <textarea id="ns-ci" class="input mono" rows="3" bind:value={cloudInit} placeholder={"#cloud-config\npackages: [git, curl]"}></textarea></div>

    <div class="sec">Incus profiles</div>
    <div class="field mb16"><label for="ns-prof">Profiles <span class="hint">(comma/space separated; applied in order)</span></label>
      <input id="ns-prof" class="input mono" bind:value={profilesStr} placeholder="default" /></div>

    <div class="sec">Resources <span class="hint">(optional · Incus <span class="mono">limits.*</span>)</span></div>
    <div class="grid g-2">
      <div class="field"><label for="ns-cpu">CPU limit <span class="hint">(cores)</span></label><input id="ns-cpu" class="input mono" bind:value={cpuLimit} placeholder="2" /></div>
      <div class="field"><label for="ns-mem">Memory limit</label><input id="ns-mem" class="input mono" bind:value={memoryLimit} placeholder="4GiB" /></div>
    </div>
  {/snippet}
  {#snippet foot()}
    <span class="code-chip mono" style="margin-right:auto">incus launch {image} {name || "name"}</span>
    <button class="btn" onclick={close}>Cancel</button>
    <button class="btn primary" onclick={submit} disabled={busy || !name.trim() || !operator.trim()}>
      <Icon name="play" size={15} /><span>{busy ? "Launching…" : "Launch sandbox"}</span>
    </button>
  {/snippet}
</Modal>

<style>
  .sec { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: .05em; color: var(--text-3); margin: 4px 0 10px; padding-top: 4px; border-top: 1px solid var(--border); }
  .sec:first-child { border-top: none; padding-top: 0; }
  .mountrow { display: grid; grid-template-columns: 1fr 1fr auto auto; gap: 8px; align-items: center; }
  .ro { display: flex; align-items: center; gap: 4px; font-size: 11px; color: var(--text-3); }
  .chip { font-family: var(--mono); font-size: 11px; color: var(--text-2); background: var(--card-2); border: 1px solid var(--border); border-radius: 6px; padding: 3px 8px; cursor: pointer; }
  .chip.on { border-color: var(--accent); color: var(--accent-text); background: var(--accent-dim); }
</style>
