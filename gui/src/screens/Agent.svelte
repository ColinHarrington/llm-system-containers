<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui } from "../lib/store.svelte";
  import { listAgents } from "../lib/core";
  import type { AgentInfo } from "../lib/types";

  let agents = $state<AgentInfo[]>([]);
  let selectedId = $state<string | null>(null);
  let paused = $state(false);

  $effect(() => {
    void (async () => {
      agents = await listAgents();
      if (!selectedId && agents.length) selectedId = agents[0].id;
    })();
  });

  const focused = $derived(agents.find((a) => a.id === selectedId) ?? null);
  const others = $derived(agents.filter((a) => a.id !== selectedId));

  // Demo trace + log content (no backend trace stream yet).
  const trace = [
    { t: "12:47:02", w: 34, kind: "", label: "llm.chat · 1.2s · 980 tok" },
    { t: "12:47:04", w: 18, kind: "tool", label: "tool · read_file(auth_test.py)" },
    { t: "12:47:09", w: 46, kind: "tool", label: "tool · run(pytest -k auth) · 3.4s" },
    { t: "12:47:21", w: 52, kind: "", label: "llm.chat · 2.1s · 1,540 tok" },
    { t: "12:47:26", w: 24, kind: "tool", label: "tool · edit_file(auth.py)" },
    { t: "12:47:38", w: 14, kind: "retr", label: "retrieval · docs (4 chunks)" },
    { t: "12:47:44", w: 60, kind: "", label: "llm.chat · 2.6s · 1,910 tok" },
  ];
</script>

<div class="content">
  {#if focused}
    <div class="card pad mb16">
      <div class="flex gap12 wrap">
        <div class="avatar {focused.kind}" style="width:40px;height:40px;border-radius:11px;font-size:14px">{focused.initials}</div>
        <div>
          <div class="flex gap10"><span class="strong" style="font-size:16px">{focused.name}</span>
            {#if paused}<span class="pill warn"><span class="dot warn"></span> Paused</span>
            {:else}<span class="pill ok"><span class="dot ok pulse"></span> Working</span>{/if}
          </div>
          <div class="muted small mt4 mono">{focused.sandbox} · UID {focused.uid} · model {focused.model}</div>
        </div>
        <div class="right flex gap8 wrap">
          <button class="btn" onclick={() => (paused = !paused)}>
            <Icon name={paused ? "play" : "pause"} size={15} /><span>{paused ? "Resume" : "Pause"}</span>
          </button>
          <button class="btn"><Icon name="hand" size={15} /><span>Interrupt</span></button>
          <button class="btn primary" onclick={() => (ui.steerAgent = focused)}><Icon name="steer" size={15} /><span>Steer</span></button>
          <button class="btn danger"><Icon name="stop" size={15} /><span>Terminate</span></button>
        </div>
      </div>
      <div class="divider"></div>
      <div class="flex gap16 wrap small t2">
        <span class="flex gap6"><Icon name="key" size={15} /> LLM via <span class="mono">litellm</span> virtual key <span class="mono">sk-vk-…a91f</span></span>
        <span class="flex gap6"><Icon name="shield" size={15} /> Tetragon enforcing</span>
        <span class="flex gap6">Current task: <span class="strong" style="color:var(--text)">{focused.task}</span></span>
      </div>
    </div>

    <div class="grid agent-grid">
      <div class="grid" style="gap:16px">
        <div class="card">
          <div class="card-head"><h3>Live activity</h3><span class="sub">LLM call trace · Phoenix</span>
            <span class="pill ok right"><span class="dot ok pulse"></span> streaming</span></div>
          <div class="pad trace">
            {#each trace as r}
              <div class="trace-row">
                <span style="width:64px" class="t2">{r.t}</span>
                <div class="trace-bar {r.kind}" style="width:{r.w}%"></div>
                <span class="t2">{r.label}</span>
              </div>
            {/each}
          </div>
        </div>

        <div class="card">
          <div class="card-head"><h3>Logs</h3><span class="sub">stdout · Loki</span></div>
          <div class="pad">
            <div class="console">
              <div><span class="ts">12:47:09</span> <span class="lvl-info">[run]</span> $ pytest -k auth -q</div>
              <div><span class="ts">12:47:12</span> <span class="lvl-err">FAILED</span> tests/test_auth.py::test_refresh_token</div>
              <div><span class="ts">12:47:21</span> <span class="agent">[{focused.name}]</span> Reading failure; token TTL mismatch suspected.</div>
              <div><span class="ts">12:47:26</span> <span class="lvl-info">[edit]</span> auth.py — set refresh TTL to 30m</div>
              <div><span class="ts">12:47:40</span> <span class="lvl-warn">[net]</span> egress to api.anthropic.com via litellm proxy (vk)</div>
              <div><span class="ts">12:47:55</span> <span class="lvl-info">[run]</span> $ pytest -k auth -q</div>
              <div><span class="ts">12:48:01</span> <span class="lvl-ok">PASSED</span> 2 passed in 1.9s</div>
              <div><span class="ts">12:48:03</span> <span class="agent">[{focused.name}]</span> Tests green. Preparing PR…</div>
            </div>
          </div>
        </div>
      </div>

      <div class="grid" style="gap:16px">
        <div class="card">
          <div class="card-head"><h3>Token usage</h3><span class="sub">this session</span></div>
          <div class="pad">
            <div class="flex"><div class="stat"><div class="num" style="font-size:24px">142.8K <small>tokens</small></div></div>
              <div class="right center"><div class="strong" style="font-size:18px">$0.86</div><div class="muted xsmall">est. cost</div></div></div>
            <div class="bars mt16">
              {#each [30, 55, 40, 80, 50, 95, 60, 45, 70, 38, 62, 88] as h, i}
                <i class:hi={h > 75} style="height:{h}%"></i>
              {/each}
            </div>
            <div class="divider"></div>
            <div class="kv"><span class="k">Input</span><span class="v mono">118.2K</span></div>
            <div class="kv"><span class="k">Output</span><span class="v mono">24.6K</span></div>
            <div class="kv"><span class="k">LLM calls</span><span class="v mono">37</span></div>
            <div class="kv"><span class="k">Tool calls</span><span class="v mono">58</span></div>
          </div>
        </div>

        <div class="card">
          <div class="card-head"><h3>Other agents</h3></div>
          <div class="pad grid" style="gap:10px">
            {#each others as a (a.id)}
              <button class="other" onclick={() => (selectedId = a.id)}>
                <div class="avatar {a.kind} sm">{a.initials}</div>
                <div><div class="strong small">{a.name}</div><div class="muted xsmall">{a.sandbox} · {a.status}</div></div>
                {#if a.status === "working"}<span class="pill ok right"><span class="dot ok pulse"></span></span>
                {:else}<span class="pill warn right"><span class="dot warn"></span></span>{/if}
              </button>
            {/each}
            {#if others.length === 0}<div class="muted small">No other agents.</div>{/if}
          </div>
        </div>
      </div>
    </div>
  {:else}
    <div class="card"><div class="empty"><div class="icon"><Icon name="agent" size={26} /></div>No running agents.</div></div>
  {/if}
</div>

<style>
  .agent-grid { grid-template-columns: 1.55fr 1fr; gap: 16px; }
  @media (max-width: 960px) { .agent-grid { grid-template-columns: 1fr; } }
  .other { display: flex; align-items: center; gap: 10px; width: 100%; text-align: left; background: transparent; border: none; cursor: pointer; font-family: inherit; padding: 4px; border-radius: 8px; }
  .other:hover { background: var(--card-2); }
</style>
