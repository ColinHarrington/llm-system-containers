<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import Skeleton from "../lib/Skeleton.svelte";
  import { ui, live, openSandbox, openTerminal, showToast } from "../lib/store.svelte";
  import { topology, agentPause, agentResume, agentStop } from "../lib/core";
  import type { TopoSandbox } from "../lib/types";

  let sandboxes = $state<TopoSandbox[]>([]);
  let loading = $state(true);
  let selected = $state<string | null>(null);
  let busy = $state(false);

  $effect(() => {
    ui.dataVersion;
    live.tick; // auto-refresh on the live poll
    void (async () => {
      try { sandboxes = await topology(); }
      finally { loading = false; }
    })();
  });

  const initial = (name: string) => name.replace(/^agent-/, "").slice(0, 2).toUpperCase();
  // One entry per agent (one Linux user per agent); the human operator is excluded.
  const agentList = $derived(
    sandboxes.flatMap((s) => s.agents.filter((a) => a.kind === "agent").map((a) => ({ sb: s.name, running: s.status === "running", agent: a }))),
  );
  $effect(() => {
    if ((!selected || !agentList.some((x) => `${x.sb}/${x.agent.name}` === selected)) && agentList.length) {
      selected = `${agentList[0].sb}/${agentList[0].agent.name}`;
    }
  });
  const focused = $derived(agentList.find((x) => `${x.sb}/${x.agent.name}` === selected) ?? null);

  async function control(fn: () => Promise<void>, msg: string) {
    if (!focused) return;
    busy = true;
    try { await fn(); showToast(msg, "ok"); }
    catch (e) { showToast(String(e), "danger"); }
    finally { busy = false; }
  }
  function steer() {
    if (!focused) return;
    ui.steerAgent = { id: focused.agent.name, name: focused.agent.name, initials: initial(focused.agent.name), kind: "agent", sandbox: focused.sb, uid: 0, model: "", status: "idle", task: "" };
  }
</script>

<div class="content">
  {#if loading}
    <div class="grid agent-grid">
      <div class="card pad"><Skeleton w="60%" h={16} mb={12} /><Skeleton w="100%" h={40} mb={10} /><Skeleton w="100%" h={40} /></div>
      <div class="card pad"><Skeleton w="40%" h={14} mb={10} /><Skeleton w="100%" h={60} /></div>
    </div>
  {:else if agentList.length === 0}
    <div class="card"><div class="empty"><div class="icon"><Icon name="agent" size={26} /></div>No agents yet. Add an agent to a running sandbox to observe and control it here.</div></div>
  {:else}
    <div class="grid agent-grid">
      <!-- Agent list -->
      <div class="card">
        <div class="card-head"><h3>Agents</h3><span class="sub">one Linux user each · across {sandboxes.length} sandboxes</span></div>
        <div class="pad grid" style="gap:8px">
          {#each agentList as x (`${x.sb}/${x.agent.name}`)}
            <button class="arow" class:sel={`${x.sb}/${x.agent.name}` === selected} onclick={() => (selected = `${x.sb}/${x.agent.name}`)}>
              <div class="avatar agent sm">{initial(x.agent.name)}</div>
              <div style="min-width:0;flex:1">
                <div class="mono small strong" style="color:var(--text)">{x.agent.name}</div>
                <div class="muted xsmall mono">{x.sb}{x.agent.profile ? ` · from ${x.agent.profile}` : ""}</div>
              </div>
              <span class="dot {x.running ? 'ok' : 'muted'} right"></span>
            </button>
          {/each}
        </div>
      </div>

      <!-- Focused agent -->
      {#if focused}
        {@const g = focused.agent.guardrails}
        <div class="grid" style="gap:16px">
          <div class="card pad">
            <div class="flex gap12 wrap">
              <div class="avatar agent" style="width:40px;height:40px;border-radius:11px;font-size:14px">{initial(focused.agent.name)}</div>
              <div style="min-width:0">
                <div class="flex gap10"><span class="strong" style="font-size:16px">{focused.agent.name}</span>
                  {#if focused.running}<span class="pill ok"><span class="dot ok pulse"></span> running</span>
                  {:else}<span class="pill"><span class="dot muted"></span> stopped</span>{/if}
                </div>
                <div class="muted small mt4 mono">
                  <button class="linkbtn" onclick={() => openSandbox(focused.sb)}>{focused.sb}</button>{focused.agent.profile ? ` · seeded from ${focused.agent.profile}` : ""}
                </div>
              </div>
              <div class="right flex gap8 wrap">
                <button class="btn" disabled={busy} onclick={() => control(() => agentPause(focused.sb, focused.agent.name), `Paused ${focused.agent.name}`)}><Icon name="pause" size={15} /><span>Pause</span></button>
                <button class="btn" disabled={busy} onclick={() => control(() => agentResume(focused.sb, focused.agent.name), `Resumed ${focused.agent.name}`)}><Icon name="play" size={15} /><span>Resume</span></button>
                <button class="btn primary" onclick={steer}><Icon name="steer" size={15} /><span>Steer</span></button>
                <button class="btn danger" disabled={busy} onclick={() => control(() => agentStop(focused.sb, focused.agent.name), `Stopped ${focused.agent.name}`)}><Icon name="stop" size={15} /><span>Stop</span></button>
              </div>
            </div>
            <div class="divider"></div>
            <div class="flex gap16 wrap small t2">
              <button class="linkbtn flex gap6" onclick={() => openTerminal(`${focused.agent.name}@${focused.sb}`)}><Icon name="terminal" size={15} /> Open shell</button>
              <span class="flex gap6"><Icon name="key" size={15} /> LLM budget: <span class="strong" style="color:var(--text)">{g?.llmBudget || "—"}</span></span>
            </div>
          </div>

          <!-- Guardrails (the agent's real permission bundle) -->
          <div class="card">
            <div class="card-head"><h3>Guardrails</h3><span class="sub">the agent's permission bundle · refine on the sandbox detail page</span></div>
            <div class="pad">
              {#if g}
                <div class="kv"><span class="k">Filesystem</span><span class="v">{g.filesystem || "—"}</span></div>
                <div class="kv"><span class="k">Network egress</span><span class="v">{g.network || "—"}</span></div>
                <div class="kv"><span class="k">LLM budget</span><span class="v mono">{g.llmBudget || "—"}</span></div>
                <div class="kv"><span class="k">Control-plane</span><span class="v">{g.controlPlane || "none"}</span></div>
                <div class="kv"><span class="k">Nested L3</span><span class="v">{g.l3 ? "allowed" : "off"}</span></div>
              {:else}
                <div class="muted small">No guardrails recorded for this agent (added outside the GUI?).</div>
              {/if}
              <p class="xsmall muted mt8">Guardrails are the legible intent; the egress ACL / Tetragon / LiteLLM rings enforce them. Steering delivers a message to the agent's mailbox (an agent runtime must read it).</p>
            </div>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .agent-grid { display: grid; grid-template-columns: 320px 1fr; gap: 16px; }
  @media (max-width: 960px) { .agent-grid { grid-template-columns: 1fr; } }
  .arow { display: flex; align-items: center; gap: 10px; width: 100%; text-align: left; background: transparent; border: 1px solid var(--border); border-radius: 10px; cursor: pointer; font-family: inherit; padding: 8px 10px; }
  .arow:hover { border-color: var(--border-strong); }
  .arow.sel { background: var(--accent-soft-bg); border-color: var(--accent); }
  .linkbtn { background: none; border: none; padding: 0; color: var(--accent-text); cursor: pointer; font: inherit; }
  .linkbtn:hover { text-decoration: underline; }
</style>
