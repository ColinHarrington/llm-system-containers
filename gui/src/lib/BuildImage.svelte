<script lang="ts">
  import Modal from "./Modal.svelte";
  import Icon from "./Icon.svelte";
  import { ui, bump, navigate, showToast } from "./store.svelte";
  import { buildImage } from "./core";

  let name = $state("");
  let base = $state("images:debian/12");
  let packagesStr = $state("");
  let script = $state("");
  let description = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const bases = ["images:debian/12", "images:ubuntu/24.04", "images:alpine/3.21", "images:fedora/41"];
  const packages = $derived(packagesStr.split(/[\s,]+/).filter(Boolean));

  function close() {
    ui.buildImageOpen = false;
  }

  async function submit() {
    if (!name.trim() || !base.trim()) return;
    busy = true;
    error = null;
    showToast(`$ incus publish (build ${name.trim()})`);
    try {
      await buildImage({ base: base.trim(), name: name.trim(), packages, script, description: description.trim() });
      close();
      ui.incusTab = "images";
      navigate("incus");
      bump();
      showToast(`Image '${name.trim()}' built`, "ok");
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title="Build image" maxWidth={600} onclose={close}>
  {#snippet body()}
    <p class="hint mb16">
      Builds a custom L2 image: launch a throwaway container from a base, run your setup, then
      <span class="mono">incus publish</span> it. It appears under <span class="mono">Installed</span>.
    </p>
    {#if error}
      <div class="banner warn mb16" role="alert"><Icon name="warn" size={16} /><span>{error}</span></div>
    {/if}

    <div class="field mb16">
      <label for="bi-name">Image name (alias)</label>
      <input id="bi-name" class="input mono" bind:value={name} placeholder="dev-debian-12" />
    </div>

    <div class="field mb16">
      <label for="bi-base">Base image</label>
      <input id="bi-base" class="input mono" bind:value={base} placeholder="images:debian/12" />
      <div class="flex gap6 wrap mt8">
        {#each bases as b}
          <button class="chip" class:on={base === b} onclick={() => (base = b)}>{b}</button>
        {/each}
      </div>
    </div>

    <div class="field mb16">
      <label for="bi-pkgs">Packages <span class="hint">(space/comma separated · apt → apk)</span></label>
      <input id="bi-pkgs" class="input mono" bind:value={packagesStr} placeholder="git curl python3 build-essential" />
    </div>

    <div class="field mb16">
      <label for="bi-script">Setup script <span class="hint">(optional · runs in the builder via sh)</span></label>
      <textarea id="bi-script" class="input mono" rows="4" bind:value={script}
        placeholder={"curl -fsSL https://example.com/install.sh | sh\nnpm i -g some-tool"}></textarea>
    </div>

    <div class="field">
      <label for="bi-desc">Description <span class="hint">(optional)</span></label>
      <input id="bi-desc" class="input" bind:value={description} placeholder="Debian 12 + dev toolchain" />
    </div>
  {/snippet}
  {#snippet foot()}
    <span class="code-chip mono" style="margin-right:auto">incus publish {name || "name"} --reuse</span>
    <button class="btn" onclick={close}>Cancel</button>
    <button class="btn primary" onclick={submit} disabled={busy || !name.trim() || !base.trim()}>
      <Icon name="layers" size={15} /><span>{busy ? "Building…" : "Build image"}</span>
    </button>
  {/snippet}
</Modal>

<style>
  .chip {
    font-family: var(--mono); font-size: 11px; color: var(--text-2);
    background: var(--card-2); border: 1px solid var(--border); border-radius: 6px;
    padding: 3px 8px; cursor: pointer;
  }
  .chip.on { border-color: var(--accent); color: var(--accent-text); background: var(--accent-dim); }
</style>
