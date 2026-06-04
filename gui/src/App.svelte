<script lang="ts">
  import Dashboard from "./screens/Dashboard.svelte";

  type Screen = "dashboard" | "sandboxes" | "services";
  const nav: { id: Screen; label: string }[] = [
    { id: "dashboard", label: "Dashboard" },
    { id: "sandboxes", label: "Sandboxes" },
    { id: "services", label: "Services" },
  ];
  let screen = $state<Screen>("dashboard");
</script>

<div class="app">
  <aside class="sidebar">
    <div class="brand">
      <strong>llmsc</strong>
      <span class="muted">Little Linux Managed</span>
    </div>
    <nav>
      {#each nav as n (n.id)}
        <button class="navitem" class:active={screen === n.id} onclick={() => (screen = n.id)}>
          {n.label}
        </button>
      {/each}
    </nav>
  </aside>

  <main class="content">
    {#if screen === "dashboard"}
      <Dashboard />
    {:else}
      <div class="card">
        <h2>{nav.find((n) => n.id === screen)?.label}</h2>
        <p class="muted">Coming soon.</p>
      </div>
    {/if}
  </main>
</div>

<style>
  .app { display: grid; grid-template-columns: var(--sidebar-w) 1fr; min-height: 100vh; }
  .sidebar {
    background: var(--surface);
    border-right: 1px solid var(--border);
    padding: 1rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }
  .brand { display: flex; flex-direction: column; line-height: 1.2; padding: 0.25rem 0.5rem; }
  .brand strong { font-size: 1.05rem; }
  .brand .muted { font-size: 0.72rem; }
  nav { display: flex; flex-direction: column; gap: 2px; }
  .navitem {
    text-align: left;
    border: none;
    background: transparent;
    color: var(--text);
    padding: 0.5rem 0.65rem;
    border-radius: 8px;
    font-size: 0.92rem;
    cursor: pointer;
  }
  .navitem:hover { background: var(--surface-2); }
  .navitem.active { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .content { padding: 1.5rem; max-width: 1000px; }
</style>
