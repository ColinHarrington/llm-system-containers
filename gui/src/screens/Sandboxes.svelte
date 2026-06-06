<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Skeleton from "../lib/Skeleton.svelte";
  import { ui, bump, openTerminal, openSandbox, confirmAction } from "../lib/store.svelte";
  import { listSandboxes, removeSandbox } from "../lib/core";
  import type { Sandbox } from "../lib/types";

  let sandboxes = $state<Sandbox[]>([]);
  let loading = $state(true);
  let filter = $state<"All" | "Running" | "Stopped">("All");
  let query = $state("");
  let busy = $state<string | null>(null);

  $effect(() => {
    ui.dataVersion;
    void refresh();
  });

  async function refresh() {
    try { sandboxes = await listSandboxes(); }
    finally { loading = false; }
  }

  async function remove(n: string) {
    if (!(await confirmAction({
      title: "Remove sandbox",
      message: `Delete sandbox '${n}' and everything inside it? This cannot be undone.`,
      confirmLabel: "Remove sandbox", danger: true,
    }))) return;
    busy = n;
    try {
      await removeSandbox(n);
      bump();
    } finally { busy = null; }
  }

  const shown = $derived(
    sandboxes
      .filter((s) => filter === "All" || s.status === filter)
      .filter((s) => s.name.toLowerCase().includes(query.toLowerCase())),
  );
</script>

<div class="content">
  <div class="banner info mb16">
    <Icon name="shield" size={18} />
    <span>Every sandbox is an <strong>unprivileged</strong> system container. Agents authenticate to LLMs with <strong>virtual keys</strong> — never real API keys.</span>
  </div>

  <div class="flex gap12 mb16 wrap">
    <div class="code-chip" style="flex:1;max-width:420px">
      <Icon name="search" size={16} />
      <input class="bare" placeholder="Search sandboxes…" bind:value={query} />
    </div>
    <div class="seg right">
      {#each ["All", "Running", "Stopped"] as f}
        <button class:on={filter === f} onclick={() => (filter = f as typeof filter)}>{f}</button>
      {/each}
    </div>
    <button class="btn primary" onclick={() => (ui.newSandboxOpen = true)}>
      <Icon name="plus" /><span>New sandbox</span>
    </button>
  </div>

  <div class="grid g-3">
    {#if loading}
      {#each Array(3) as _, i (i)}
        <div class="card pad">
          <div class="flex gap10 mb12"><Skeleton w="36px" h={36} r={9} /><div style="flex:1"><Skeleton w="55%" h={13} mb={6} /><Skeleton w="40%" h={10} /></div></div>
          <Skeleton w="100%" h={42} mb={10} />
          <Skeleton w="70%" h={30} />
        </div>
      {/each}
    {:else}
    {#each shown as s (s.name)}
      <div class="card pad sb">
        <div class="flex gap10 mb12">
          <div class="sb-ico" class:off={s.status !== "Running"}><Icon name="box" size={18} /></div>
          <div><button class="namebtn strong" onclick={() => openSandbox(s.name)}>{s.name}</button><div class="muted xsmall mono">{s.image ?? "—"}</div></div>
          {#if s.status === "Running"}
            <span class="pill ok right"><span class="dot ok pulse"></span> Running</span>
          {:else}
            <span class="pill right"><span class="dot muted"></span> Stopped</span>
          {/if}
        </div>
        <div class="flex gap6 wrap mb12">
          {#each s.tags ?? ["unprivileged"] as t}<span class="tag">{t}</span>{/each}
        </div>
        <div class="grid g-2" style="gap:10px">
          <div><div class="muted xsmall">Users</div>
            <div class="flex gap6 mt4">
              {#each s.users ?? [] as u}<div class="avatar {u.kind} sm">{u.initials}</div>{/each}
              {#if !s.users}<span class="muted small">—</span>{/if}
            </div>
          </div>
          <div><div class="muted xsmall">Nested L3</div><div class="strong mono mt4">{s.nested ?? "—"}{s.nested != null ? " containers" : ""}</div></div>
          <div><div class="muted xsmall">CPU</div><div class="strong mono mt4">{s.cpuCores ?? "—"}{s.cpuCores ? (s.cpuCores === 1 ? " core" : " cores") : ""}</div></div>
          <div><div class="muted xsmall">Memory</div><div class="strong mono mt4">{s.memTotal ? `${s.memUsed} / ${s.memTotal} GB` : "—"}</div></div>
        </div>
        <div class="divider"></div>
        <div class="flex gap8">
          <button class="btn sm" onclick={() => openTerminal(`operator@${s.name}`)}><Icon name="terminal" size={14} /><span>Open shell</span></button>
          {#if s.status === "Running"}
            <button class="btn sm" onclick={() => (ui.addAgentSandbox = s.name)}><Icon name="agent" size={14} /><span>Add agent</span></button>
          {/if}
          <button class="btn sm danger right" onclick={() => remove(s.name)} disabled={busy === s.name}>
            {busy === s.name ? "Removing…" : "Remove"}
          </button>
        </div>
      </div>
    {/each}

    <!-- New sandbox tile -->
    <button class="card pad newtile" onclick={() => (ui.newSandboxOpen = true)}>
      <div class="nt-ico"><Icon name="plus" size={22} /></div>
      <div class="strong" style="color:var(--text)">New sandbox</div>
      <div class="xsmall muted">{sandboxes.length === 0 ? "Launch your first LLMSC workspace" : "Launch a fresh LLMSC workspace"}</div>
    </button>
    {/if}
  </div>

  {#if !loading && sandboxes.length > 0 && shown.length === 0}
    <div class="empty mt16"><div class="icon"><Icon name="search" size={22} /></div>No sandboxes match the current filter.</div>
  {/if}
</div>

<style>
  .namebtn { border: none; background: transparent; padding: 0; color: var(--text); font-family: inherit; font-size: 13px; cursor: pointer; text-align: left; }
  .namebtn:hover { color: var(--accent-text); }
  .sb-ico { width: 36px; height: 36px; border-radius: 10px; background: var(--accent-soft); color: var(--accent); display: grid; place-items: center; flex: none; }
  .sb-ico.off { background: var(--card-2); color: var(--text-3); border: 1px solid var(--border); }
  .newtile {
    border-style: dashed; display: flex; flex-direction: column; justify-content: center; align-items: center;
    gap: 8px; color: var(--text-3); min-height: 200px; cursor: pointer; font-family: inherit;
  }
  .nt-ico { width: 44px; height: 44px; border-radius: 12px; background: var(--accent-soft); color: var(--accent); display: grid; place-items: center; }
</style>
