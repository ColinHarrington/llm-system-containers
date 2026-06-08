<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui, live, toggleLive, toggleTheme, showToast, bump, confirmAction, navigate } from "../lib/store.svelte";
  import { getSettings, saveSettings, vmStatus, vmDestroy } from "../lib/core";
  import type { Settings, VmStatus } from "../lib/types";

  let form = $state<Settings>({ operator: "", vmName: "llmsc", cpus: 4, memoryGib: 8, diskGib: 60 });
  let loaded = $state<Settings | null>(null);
  let vm = $state<VmStatus | null>(null);
  let busy = $state(false);

  $effect(() => {
    ui.dataVersion;
    void (async () => {
      const s = await getSettings();
      loaded = s;
      form = { ...s };
    })();
    void vmStatus().then((v) => (vm = v)).catch(() => (vm = null));
  });

  const dirty = $derived(
    !!loaded && (form.operator !== loaded.operator || form.cpus !== loaded.cpus || form.memoryGib !== loaded.memoryGib || form.diskGib !== loaded.diskGib),
  );

  async function save() {
    busy = true;
    showToast("$ saving settings");
    try {
      await saveSettings(form);
      loaded = { ...form };
      showToast("Settings saved", "ok");
      bump();
    } catch (e) {
      showToast(String(e), "danger");
    } finally { busy = false; }
  }

  function reset() { if (loaded) form = { ...loaded }; }

  async function destroy() {
    if (!(await confirmAction({
      title: "Destroy the VM",
      message: `Stop and delete '${form.vmName}' and every sandbox, service and image inside it? This cannot be undone.`,
      confirmLabel: "Destroy VM", danger: true,
    }))) return;
    busy = true;
    showToast("$ llmsctl destroy");
    try { await vmDestroy(); showToast("VM destroyed", "ok"); bump(); }
    catch (e) { showToast(String(e), "danger"); }
    finally { busy = false; }
  }
</script>

<div class="content" style="max-width:720px">
  <!-- Operator -->
  <div class="card mb16">
    <div class="card-head"><h3>Operator</h3><span class="sub">the human user created in every new sandbox</span></div>
    <div class="pad">
      <div class="field"><label for="op">Default operator username</label>
        <input id="op" class="input mono" style="max-width:280px" bind:value={form.operator} placeholder="operator" /></div>
      <p class="xsmall muted mt8">Used as the human login in each sandbox; overridable per sandbox at creation.</p>
    </div>
  </div>

  <!-- Appearance -->
  <div class="card mb16">
    <div class="card-head"><h3>Appearance &amp; updates</h3></div>
    <div class="pad">
      <div class="row"><div><div class="strong small">Theme</div><div class="muted xsmall">Dark-first (matches the dev-tool design)</div></div>
        <button class="btn sm right" onclick={toggleTheme}><Icon name={ui.theme === "dark" ? "sun" : "moon"} size={14} /><span>{ui.theme === "dark" ? "Switch to light" : "Switch to dark"}</span></button></div>
      <div class="divider"></div>
      <div class="row"><div><div class="strong small">Live updates</div><div class="muted xsmall">Auto-refresh dashboards while the tab is visible</div></div>
        <button class="btn sm right" class:primary={live.paused} onclick={toggleLive}>{live.paused ? "Resume" : "Pause"}</button></div>
    </div>
  </div>

  <!-- VM resources -->
  <div class="card mb16">
    <div class="card-head"><h3>Playground VM</h3><span class="sub"><span class="mono">{form.vmName}</span> · {vm ? vm.toLowerCase() : "…"}</span></div>
    <div class="pad">
      <div class="row mb8"><div><div class="strong small">Deployment target</div><div class="muted xsmall">Where Incus runs (vm = this VM; local = host Incus)</div></div>
        <span class="pill right mono">{loaded?.target ?? "vm"}</span></div>
      <div class="grid g-3" style="gap:12px">
        <div class="field"><label for="cpu">CPU cores</label><input id="cpu" class="input mono" type="number" min="1" bind:value={form.cpus} /></div>
        <div class="field"><label for="mem">Memory (GiB)</label><input id="mem" class="input mono" type="number" min="1" bind:value={form.memoryGib} /></div>
        <div class="field"><label for="disk">Disk (GiB)</label><input id="disk" class="input mono" type="number" min="1" bind:value={form.diskGib} /></div>
      </div>
      <p class="xsmall muted mt8">Resource changes are written to config and apply when the VM is next (re)created — they do not resize a running VM.</p>
    </div>
  </div>

  <div class="flex gap8 mb16">
    <button class="btn primary" disabled={busy || !dirty} onclick={save}><Icon name="check" size={15} /><span>{busy ? "Saving…" : "Save settings"}</span></button>
    <button class="btn" disabled={busy || !dirty} onclick={reset}>Reset</button>
    <span class="right muted xsmall" style="align-self:center">{dirty ? "Unsaved changes" : "All changes saved"}</span>
  </div>

  <!-- Danger zone -->
  <div class="card danger-zone">
    <div class="card-head"><h3>Danger zone</h3></div>
    <div class="pad flex" style="align-items:center;gap:12px">
      <div><div class="strong small">Destroy the Playground VM</div><div class="muted xsmall">Deletes the VM and everything inside it. You can re-run the setup wizard afterwards.</div></div>
      <button class="btn danger right" disabled={busy} onclick={destroy}>Destroy VM</button>
    </div>
  </div>

  <p class="xsmall muted mt12">First-time setup? <button class="linkbtn" onclick={() => navigate("wizard")}>Open the setup wizard</button>.</p>
</div>

<style>
  .row { display: flex; align-items: center; gap: 12px; padding: 4px 0; }
  .danger-zone { border-color: var(--danger); }
  .danger-zone :global(h3) { color: var(--danger); }
  .linkbtn { background: none; border: none; padding: 0; color: var(--accent-text); cursor: pointer; font: inherit; text-decoration: underline; }
</style>
