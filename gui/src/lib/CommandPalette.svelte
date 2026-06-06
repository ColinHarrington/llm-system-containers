<script lang="ts">
  import Icon from "./Icon.svelte";
  import { ui, navigate, openSandbox, openTerminal, toggleTheme, bump, showToast, type Screen, type IncusTab } from "./store.svelte";
  import { listSandboxes, vmStatus, vmUp, vmDown, syncVirtualKeys } from "./core";
  import type { Sandbox, VmStatus } from "./types";

  // ⌘K / Ctrl-K command palette (direction A): a real launcher — navigate, jump to any sandbox,
  // and run actions. Fuzzy filter over a flat list; ↑/↓ to move, Enter to run, Esc to close.
  type Cmd = { id: string; label: string; hint: string; icon: string; keywords: string; group: string; run: () => void };

  function gotoCmd(id: string, label: string, icon: string, key?: string): Cmd {
    return { id: `go-${id}`, label: `Go to ${label}`, hint: key ?? "", icon, keywords: label, group: "Navigate", run: () => navigate(id as Screen) };
  }
  function incusCmd(tab: IncusTab, label: string, icon: string): Cmd {
    return { id: `incus-${tab}`, label: `Incus · ${label}`, hint: "", icon, keywords: `incus ${tab} ${label}`, group: "Navigate", run: () => { ui.incusTab = tab; navigate("incus"); } };
  }

  // Live data loaded when the palette opens (sandbox jumps + VM toggle label).
  let sandboxes = $state<Sandbox[]>([]);
  let vm = $state<VmStatus | null>(null);

  async function vmToggle() {
    showToast(vm === "Running" ? "$ llmsctl down" : "$ llmsctl up");
    try { if (vm === "Running") await vmDown(); else await vmUp(); showToast(vm === "Running" ? "VM stopped" : "VM is up", "ok"); bump(); }
    catch (e) { showToast(String(e), "danger"); }
  }
  async function doSyncKeys() {
    showToast("$ llmsctl keys sync");
    try { const n = await syncVirtualKeys(); showToast(n === 0 ? "No agent keys to sync" : `Synced ${n} virtual key(s)`, "ok"); }
    catch (e) { showToast(String(e), "danger"); }
  }

  const navCmds: Cmd[] = [
    gotoCmd("dashboard", "Dashboard", "home", "1"),
    gotoCmd("sandboxes", "Sandboxes", "box", "2"),
    gotoCmd("topology", "Topology", "layers", "3"),
    gotoCmd("agent", "Agent control", "agent", "4"),
    gotoCmd("incus", "Incus", "layers", "5"),
    incusCmd("profiles", "Profiles", "layers"),
    incusCmd("networks", "Networks", "net"),
    incusCmd("images", "Images", "image"),
    gotoCmd("services", "Services", "cog", "6"),
    gotoCmd("security", "Security posture", "shield", ""),
    gotoCmd("profiles", "Agent profiles", "shield", ""),
    gotoCmd("wizard", "Setup wizard", "cog", ""),
  ];

  const actionCmds = $derived<Cmd[]>([
    { id: "new-sandbox", label: "New sandbox", hint: "llmsc launch", icon: "plus", keywords: "create launch sandbox", group: "Actions", run: () => (ui.newSandboxOpen = true) },
    { id: "build-image", label: "Build image", hint: "incus publish", icon: "image", keywords: "build custom image distro", group: "Actions", run: () => (ui.buildImageOpen = true) },
    { id: "vm-toggle", label: vm === "Running" ? "Stop the VM" : "Start the VM", hint: "llmsctl", icon: vm === "Running" ? "stop" : "play", keywords: "vm up down start stop lima", group: "Actions", run: () => void vmToggle() },
    { id: "sync-keys", label: "Sync virtual keys", hint: "llmsctl keys sync", icon: "key", keywords: "litellm keys budget sync", group: "Actions", run: () => void doSyncKeys() },
    { id: "toggle-theme", label: "Toggle light / dark theme", hint: "t", icon: "moon", keywords: "theme dark light appearance", group: "Actions", run: () => toggleTheme() },
    { id: "shortcuts", label: "Keyboard shortcuts", hint: "?", icon: "doc", keywords: "keyboard shortcuts help keys cheat sheet", group: "Actions", run: () => (ui.shortcutsOpen = true) },
  ]);

  const sandboxCmds = $derived<Cmd[]>(
    sandboxes.map((s) => ({
      id: `sb-${s.name}`, label: `Open ${s.name}`, hint: s.status, icon: "box",
      keywords: `sandbox ${s.name} ${s.image ?? ""}`, group: "Sandboxes", run: () => openSandbox(s.name),
    })),
  );

  const commands = $derived([...navCmds, ...sandboxCmds, ...actionCmds]);

  let query = $state("");
  let selected = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  const filtered = $derived(
    query.trim() === ""
      ? commands
      : commands.filter((c) => (c.label + " " + c.keywords).toLowerCase().includes(query.toLowerCase())),
  );

  // Reset + focus + load live data when opened.
  $effect(() => {
    if (ui.paletteOpen) {
      query = "";
      selected = 0;
      queueMicrotask(() => inputEl?.focus());
      void listSandboxes().then((s) => (sandboxes = s)).catch(() => (sandboxes = []));
      void vmStatus().then((s) => (vm = s)).catch(() => (vm = null));
    }
  });

  // Keep the selection in range as the filter changes.
  $effect(() => {
    if (selected >= filtered.length) selected = Math.max(0, filtered.length - 1);
  });

  function close() { ui.paletteOpen = false; }
  function run(c: Cmd) { close(); c.run(); }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") { e.preventDefault(); selected = Math.min(selected + 1, filtered.length - 1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); selected = Math.max(selected - 1, 0); }
    else if (e.key === "Enter") { e.preventDefault(); if (filtered[selected]) run(filtered[selected]); }
    else if (e.key === "Escape") { e.preventDefault(); close(); }
  }
</script>

{#if ui.paletteOpen}
  <div class="pal-bg" role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
    <div class="pal" role="dialog" aria-modal="true" aria-label="Command palette">
      <div class="pal-search">
        <Icon name="search" size={16} />
        <!-- svelte-ignore a11y_autofocus -->
        <input
          bind:this={inputEl}
          bind:value={query}
          onkeydown={onKey}
          placeholder="Type a command…"
          autofocus
        />
        <kbd>esc</kbd>
      </div>
      <div class="pal-list">
        {#each filtered as c, i (c.id)}
          {#if i === 0 || filtered[i - 1].group !== c.group}
            <div class="pal-group">{c.group}</div>
          {/if}
          <button class="pal-item" class:sel={i === selected} onmouseenter={() => (selected = i)} onclick={() => run(c)}>
            <span class="pal-ic"><Icon name={c.icon} size={15} /></span>
            <span class="pal-label">{c.label}</span>
            {#if c.hint}<span class="pal-hint mono">{c.hint}</span>{/if}
          </button>
        {/each}
        {#if filtered.length === 0}
          <div class="pal-empty">No matching commands</div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .pal-bg { position: fixed; inset: 0; z-index: 250; background: rgba(5, 6, 8, 0.6); backdrop-filter: blur(3px); display: flex; justify-content: center; align-items: flex-start; padding-top: 14vh; }
  .pal { width: 100%; max-width: 560px; background: var(--card); border: 1px solid var(--border-strong); border-radius: var(--radius); box-shadow: var(--shadow-lg); overflow: hidden; animation: rise 0.16s ease-out; }
  @keyframes rise { from { opacity: 0; transform: translateY(-6px); } to { opacity: 1; transform: none; } }
  .pal-search { display: flex; align-items: center; gap: 10px; padding: 12px 14px; border-bottom: 1px solid var(--border); color: var(--text-3); }
  .pal-search input { flex: 1; border: none; background: transparent; outline: none; color: var(--text); font-family: inherit; font-size: 14px; }
  .pal-list { max-height: 50vh; overflow-y: auto; padding: 6px; }
  .pal-item { display: flex; align-items: center; gap: 11px; width: 100%; text-align: left; border: none; background: transparent; cursor: pointer; padding: 9px 10px; border-radius: var(--radius-sm); font-family: inherit; color: var(--text-2); }
  .pal-item.sel { background: var(--accent-soft-bg); color: var(--accent-text); }
  .pal-ic { width: 24px; height: 24px; display: grid; place-items: center; color: var(--text-3); flex: none; }
  .pal-item.sel .pal-ic { color: var(--accent-text); }
  .pal-label { flex: 1; font-size: 13px; }
  .pal-hint { font-size: 10.5px; color: var(--text-3); }
  .pal-empty { padding: 20px; text-align: center; color: var(--text-3); font-size: 12px; }
  .pal-group { font-size: 10px; font-weight: 600; text-transform: uppercase; letter-spacing: .06em; color: var(--text-3); padding: 8px 10px 4px; }
</style>
