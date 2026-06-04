<script lang="ts">
  import { onDestroy } from "svelte";
  import Icon from "../lib/Icon.svelte";
  import { topology, TOOL_LABELS } from "../lib/core";
  import type { AgentState, TopoSandbox } from "../lib/types";

  let sandboxes = $state<TopoSandbox[]>([]);

  const STATE: Record<AgentState, { dot: string; text: string; label: string; pulse: boolean }> = {
    active: { dot: "var(--ok)", text: "var(--ok)", label: "active", pulse: true },
    thinking: { dot: "var(--warn)", text: "var(--warn)", label: "thinking", pulse: true },
    waiting: { dot: "#38bdf8", text: "var(--text-3)", label: "waiting", pulse: false },
    idle: { dot: "var(--muted-dot)", text: "var(--text-3)", label: "idle", pulse: false },
  };

  const initial = (name: string) => name.replace(/^agent-/, "").slice(0, 2).toUpperCase();
  const agentCount = $derived(sandboxes.flatMap((s) => s.agents).filter((a) => a.kind === "agent").length);
  const sbRunning = $derived(sandboxes.filter((s) => s.status === "running").length);

  $effect(() => {
    void (async () => { sandboxes = await topology(); })();
  });

  // Gentle liveness: occasionally rotate the active tool/action of a random active agent.
  const ACTIONS: Record<string, string[]> = {
    code: ["Editing src/api/router.ts", "Refactoring handlers", "Writing types"],
    llm: ["Calling claude-opus via LiteLLM", "Reasoning about the diff", "Summarizing context"],
    pkg: ["Building image · docker build", "Running compose stack", "Pushing to local registry"],
    db: ["Querying warehouse", "Running migration", "Aggregating rows"],
    web: ["Reading docs · 4 tabs", "Following a link", "Extracting a snippet"],
    run: ["Running test suite", "Linting", "Executing build"],
  };
  let tick = 0;
  const timer = setInterval(() => {
    const live = sandboxes.flatMap((s, si) => s.agents.map((a, ai) => ({ a, si, ai })))
      .filter((x) => x.a.state === "active" && x.a.active);
    if (!live.length) return;
    const pick = live[tick++ % live.length];
    const tools = pick.a.tools.filter((t) => !["shell", "git", "files"].includes(t));
    if (!tools.length) return;
    const nt = tools[tick % tools.length];
    pick.a.active = nt;
    if (ACTIONS[nt]) pick.a.action = ACTIONS[nt][tick % ACTIONS[nt].length];
  }, 2600);
  onDestroy(() => clearInterval(timer));
</script>

<div class="content">
  <div class="flex gap16 mb16">
    <div class="legend right flex gap16">
      <span class="flex gap6"><span class="ldot" style="background:var(--ok)"></span>active</span>
      <span class="flex gap6"><span class="ldot" style="background:var(--warn)"></span>thinking</span>
      <span class="flex gap6"><span class="ldot" style="background:#38bdf8"></span>waiting</span>
      <span class="flex gap6"><span class="ldot" style="background:var(--muted-dot)"></span>idle</span>
    </div>
  </div>

  <section class="vmblock">
    <div class="flex gap10 mb16" style="padding:0 4px">
      <span class="dot ok pulse"></span>
      <div>
        <div class="flex gap8"><span class="strong" style="color:var(--text)">llmsc-vm</span>
          <span class="tag">L1 · VM</span></div>
        <div class="muted xsmall mono mt4">running · 8 vCPU · 16 GB · Incus</div>
      </div>
      <div class="right small t2"><span class="strong" style="color:var(--text)">{agentCount}</span> agents · <span class="strong" style="color:var(--text)">{sbRunning}</span> sandboxes</div>
    </div>

    <div class="sbgrid">
      {#each sandboxes as sb (sb.name)}
        <div class="sb-card" class:stopped={sb.status === "stopped"}>
          <div class="flex gap8 mb4">
            <span class="dot {sb.status === 'stopped' ? 'muted' : 'ok'}"></span>
            <span class="strong mono small" style="color:var(--text)">{sb.name}</span>
            <span class="tag">L2</span>
            <span class="right">
              {#if sb.l3}
                <span class="l3on"><Icon name="pkg" size={12} /> L3 enabled</span>
              {:else}
                <span class="l3off">L3 off</span>
              {/if}
            </span>
          </div>
          <div class="muted xsmall mono mb12" style="padding-left:16px">{sb.image} · {sb.status === "stopped" ? "—" : `${sb.cpu} vCPU · ${sb.mem}`}</div>

          {#if sb.status === "stopped"}
            <div class="stopped-note">stopped · no agents running</div>
          {:else}
            <div class="agents">
              {#each sb.agents as a (a.name)}
                <div class="agent">
                  <div class="flex gap10">
                    <div class="ava-wrap">
                      <div class="ava {a.kind}" class:pulse={STATE[a.state].pulse} style="--ring:{STATE[a.state].dot}66">{initial(a.name)}</div>
                      <span class="ava-dot" style="background:{STATE[a.state].dot}"></span>
                    </div>
                    <div style="min-width:0;flex:1">
                      <div class="flex gap6">
                        <span class="mono small strong" style="color:var(--text)">{a.name}</span>
                        {#if a.kind === "human"}<span class="htag">human</span>{/if}
                      </div>
                      <div class="action" style="color:{STATE[a.state].text}">{a.action}</div>
                    </div>
                    <div class="agent-actions">
                      <button class="mini" title="Watch"><Icon name="eye" size={14} /></button>
                      <button class="mini" title="Pause / steer"><Icon name="pause" size={14} /></button>
                    </div>
                  </div>
                  <div class="tools">
                    {#each a.tools as t}
                      <span class="tool" class:active={t === a.active} title={TOOL_LABELS[t] ?? t}><Icon name={t} size={14} /></span>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <div class="svcband">
      <span class="xsmall muted" style="text-transform:uppercase;letter-spacing:.06em;margin-right:4px">Services (L1 / own L2)</span>
      {#each ["LiteLLM", "Phoenix", "Grafana", "mitmproxy", "SeaweedFS"] as s}<span class="svc">{s}</span>{/each}
    </div>
  </section>

  <p class="xsmall muted mt12">
    L3 app containers (nested Docker/Podman) are <span class="t2">enabled per sandbox</span> but intentionally not drawn here — the badge marks where nesting is allowed.
  </p>
</div>

<style>
  .legend { font-size: 11px; color: var(--text-3); }
  .ldot { width: 9px; height: 9px; border-radius: 50%; display: inline-block; }
  .vmblock { border: 1px solid var(--border); border-radius: var(--radius-lg); background: var(--card-2); padding: 16px; }
  .sbgrid { display: grid; gap: 16px; grid-template-columns: repeat(auto-fill, minmax(330px, 1fr)); }
  .sb-card { border: 1px solid var(--border); border-radius: var(--radius); background: var(--card); padding: 12px; transition: border-color .15s; box-shadow: var(--shadow-sm); }
  .sb-card:hover { border-color: var(--border-strong); }
  .sb-card.stopped { opacity: .6; }
  .l3on { font-size: 10px; display: inline-flex; align-items: center; gap: 4px; color: #2f8fd0; border: 1px solid #38bdf855; background: #38bdf818; border-radius: 6px; padding: 1px 6px; }
  .l3off { font-size: 10px; color: var(--text-3); border: 1px solid var(--border); border-radius: 6px; padding: 1px 6px; }
  .stopped-note { font-size: 12px; color: var(--text-3); font-style: italic; text-align: center; padding: 22px 0; }
  .agents { display: flex; flex-direction: column; gap: 8px; }
  .agent { border: 1px solid var(--border); border-radius: 10px; background: var(--card-2); padding: 10px; }
  .ava-wrap { position: relative; flex: none; }
  .ava { width: 36px; height: 36px; border-radius: 50%; display: grid; place-items: center; font-size: 11px; font-weight: 650; border: 1px solid var(--border-strong); background: var(--card); color: var(--text-2); }
  .ava.human { background: var(--accent-soft); color: var(--accent-strong); border-color: transparent; }
  .ava.pulse { animation: pulse-ring 2s infinite; }
  @keyframes pulse-ring { 0% { box-shadow: 0 0 0 0 var(--ring); } 70% { box-shadow: 0 0 0 6px transparent; } 100% { box-shadow: 0 0 0 0 transparent; } }
  .ava-dot { position: absolute; bottom: -1px; right: -1px; width: 12px; height: 12px; border-radius: 50%; border: 2px solid var(--card); }
  .htag { font-size: 9px; text-transform: uppercase; letter-spacing: .04em; color: var(--accent-strong); border: 1px solid var(--accent-soft); border-radius: 4px; padding: 0 4px; }
  .action { font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .agent-actions { display: flex; gap: 2px; opacity: 0; transition: opacity .12s; }
  .agent:hover .agent-actions { opacity: 1; }
  .mini { width: 24px; height: 24px; display: grid; place-items: center; border: none; background: transparent; color: var(--text-3); border-radius: 6px; cursor: pointer; }
  .mini:hover { background: var(--card); color: var(--text); }
  .tools { display: flex; gap: 6px; margin-top: 8px; padding-left: 46px; flex-wrap: wrap; }
  .tool { width: 28px; height: 28px; display: grid; place-items: center; border: 1px solid var(--border); border-radius: 7px; color: var(--text-3); }
  .tool.active { background: var(--accent-soft); border-color: var(--accent); color: var(--accent); }
  .svcband { margin-top: 18px; padding-top: 16px; border-top: 1px solid var(--border); display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .svc { font-size: 11px; color: var(--text-2); border: 1px solid var(--border); border-radius: 999px; padding: 3px 9px; display: inline-flex; align-items: center; gap: 6px; }
  .svc::before { content: ""; width: 6px; height: 6px; border-radius: 50%; background: var(--ok); }
</style>
