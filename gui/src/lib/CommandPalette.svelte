<script lang="ts">
  import Icon from "./Icon.svelte";
  import { ui, navigate, openTerminal, toggleTheme, type Screen } from "./store.svelte";

  // ⌘K / Ctrl-K command palette (direction A). Fuzzy-ish filter over a flat command list;
  // ↑/↓ to move, Enter to run, Esc to close.
  type Cmd = { id: string; label: string; hint: string; icon: string; keywords: string; run: () => void };

  function gotoCmd(id: string, label: string, icon: string, key?: string): Cmd {
    return { id: `go-${id}`, label: `Go to ${label}`, hint: key ?? "", icon, keywords: label, run: () => navigate(id as Screen) };
  }

  const commands: Cmd[] = [
    gotoCmd("dashboard", "Dashboard", "home", "1"),
    gotoCmd("sandboxes", "Sandboxes", "box", "2"),
    gotoCmd("topology", "Topology", "layers", "3"),
    gotoCmd("agent", "Agent control", "agent", "4"),
    gotoCmd("networking", "Networking", "net", "5"),
    gotoCmd("services", "Services", "store", "6"),
    gotoCmd("images", "Images", "image", "7"),
    gotoCmd("wizard", "Setup wizard", "cog", ""),
    { id: "new-sandbox", label: "New sandbox", hint: "llmsc launch", icon: "plus", keywords: "create launch sandbox", run: () => (ui.newSandboxOpen = true) },
    { id: "open-shell", label: "Open shell · operator@web-agent-01", hint: "llmsc shell", icon: "terminal", keywords: "shell terminal ssh", run: () => openTerminal("operator@web-agent-01") },
    { id: "toggle-theme", label: "Toggle light / dark theme", hint: "", icon: "moon", keywords: "theme dark light appearance", run: () => toggleTheme() },
  ];

  let query = $state("");
  let selected = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  const filtered = $derived(
    query.trim() === ""
      ? commands
      : commands.filter((c) => (c.label + " " + c.keywords).toLowerCase().includes(query.toLowerCase())),
  );

  // Reset + focus when opened.
  $effect(() => {
    if (ui.paletteOpen) {
      query = "";
      selected = 0;
      queueMicrotask(() => inputEl?.focus());
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
</style>
