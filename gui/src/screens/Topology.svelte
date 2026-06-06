<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  import { ui, live, openSandbox, showToast } from "../lib/store.svelte";
  import { topology, listServices, vmStatus, fleetEnforcement, agentPause, agentResume, agentStop, TOOL_LABELS } from "../lib/core";
  import type { AgentState, FleetEnforcement, ServiceEntry, TopoSandbox, VmStatus } from "../lib/types";

  let sandboxes = $state<TopoSandbox[]>([]);
  let services = $state<ServiceEntry[]>([]);
  let vm = $state<VmStatus | null>(null);
  let fleet = $state<FleetEnforcement[]>([]);
  let busyAgent = $state<string | null>(null);

  const postureFor = $derived((name: string) => fleet.find((f) => f.sandbox === name)?.egressPosture ?? "");
  const posturePill = (p: string) => (p === "allowlist" || p === "deny-all" ? "ok" : p === "open" ? "warn" : "");

  async function control(sb: string, agent: string, fn: () => Promise<void>, msg: string) {
    busyAgent = `${sb}/${agent}`;
    try { await fn(); showToast(msg, "ok"); }
    catch (e) { showToast(String(e), "danger"); }
    finally { busyAgent = null; }
  }
  function steer(sb: string, agent: string) {
    ui.steerAgent = { id: agent, name: agent, initials: initial(agent), kind: "agent", sandbox: sb, uid: 0, model: "", status: "idle", task: "" };
  }

  const STATE: Record<AgentState, { dot: string; text: string }> = {
    active: { dot: "var(--ok)", text: "var(--ok)" },
    thinking: { dot: "var(--warn)", text: "var(--warn)" },
    waiting: { dot: "#38bdf8", text: "var(--text-3)" },
    idle: { dot: "var(--muted-dot)", text: "var(--text-3)" },
  };

  const initial = (name: string) => name.replace(/^agent-/, "").slice(0, 2).toUpperCase();
  const agentCount = $derived(sandboxes.flatMap((s) => s.agents).filter((a) => a.kind === "agent").length);
  const sbRunning = $derived(sandboxes.filter((s) => s.status === "running").length);
  const enabledServices = $derived(services.filter((s) => s.enabled).map((s) => s.name));

  $effect(() => {
    ui.dataVersion;
    live.tick; // auto-refresh on the live poll
    void (async () => {
      [sandboxes, services, vm] = await Promise.all([topology(), listServices(), vmStatus()]);
    })();
    void fleetEnforcement().then((f) => (fleet = f)).catch(() => (fleet = []));
  });
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
      <span class="dot {vm === 'Running' ? 'ok pulse' : 'muted'}"></span>
      <div>
        <div class="flex gap8"><span class="strong" style="color:var(--text)">llmsc-vm</span>
          <span class="tag">L1 · VM</span></div>
        <div class="muted xsmall mono mt4">{(vm ?? "…").toLowerCase()} · Incus</div>
      </div>
      <div class="right small t2"><span class="strong" style="color:var(--text)">{agentCount}</span> agents · <span class="strong" style="color:var(--text)">{sbRunning}</span> running</div>
    </div>

    {#if sandboxes.length === 0}
      <div class="empty"><div class="icon"><Icon name="box" size={24} /></div>No sandboxes yet — launch one to see it here.</div>
    {:else}
      <div class="sbgrid">
        {#each sandboxes as sb (sb.name)}
          <div class="sb-card" class:stopped={sb.status === "stopped"}>
            <div class="flex gap8 mb4">
              <span class="dot {sb.status === 'stopped' ? 'muted' : 'ok'}"></span>
              <button class="namebtn strong mono small" style="color:var(--text)" onclick={() => openSandbox(sb.name)}>{sb.name}</button>
              <span class="tag">L2</span>
              {#if postureFor(sb.name) && postureFor(sb.name) !== "unmanaged"}
                <span class="pill {posturePill(postureFor(sb.name))}" title="Egress posture">{postureFor(sb.name)}</span>
              {/if}
              <span class="right">
                {#if sb.l3}
                  <span class="l3on"><Icon name="pkg" size={12} /> L3 enabled</span>
                {:else}
                  <span class="l3off">L3 off</span>
                {/if}
              </span>
            </div>
            <div class="muted xsmall mono mb12" style="padding-left:16px">
              {sb.image}{sb.status !== "stopped" && sb.mem && sb.mem !== "—" ? ` · ${sb.mem}` : ""}
            </div>

            {#if sb.status === "stopped"}
              <div class="stopped-note">stopped · no agents running</div>
            {:else if sb.agents.length === 0}
              <div class="stopped-note">no users provisioned</div>
            {:else}
              <div class="agents">
                {#each sb.agents as a (a.name)}
                  <div class="agent">
                    <div class="flex gap10">
                      <div class="ava-wrap">
                        <div class="ava {a.kind}" class:pulse={a.state === 'active' || a.state === 'thinking'} style="--ring:{STATE[a.state].dot}66">{initial(a.name)}</div>
                        <span class="ava-dot" style="background:{STATE[a.state].dot}"></span>
                      </div>
                      <div style="min-width:0;flex:1">
                        <div class="flex gap6 wrap">
                          <span class="mono small strong" style="color:var(--text)">{a.name}</span>
                          {#if a.kind === "human"}<span class="htag">human</span>{/if}
                          {#if a.profile}<span class="ptag">{a.profile}</span>{/if}
                        </div>
                        {#if a.action}
                          <div class="action" style="color:{STATE[a.state].text}">{a.action}</div>
                        {:else}
                          <div class="action muted">{a.kind === "human" ? "human operator" : "agent user"} · {a.state}</div>
                        {/if}
                      </div>
                      {#if a.kind === "agent"}
                        {@const busy = busyAgent === `${sb.name}/${a.name}`}
                        <div class="ctl right flex gap4">
                          <button class="btn sm" title="Pause" disabled={busy} onclick={() => control(sb.name, a.name, () => agentPause(sb.name, a.name), `Paused ${a.name}`)}><Icon name="pause" size={12} /></button>
                          <button class="btn sm" title="Resume" disabled={busy} onclick={() => control(sb.name, a.name, () => agentResume(sb.name, a.name), `Resumed ${a.name}`)}><Icon name="play" size={12} /></button>
                          <button class="btn sm" title="Stop" disabled={busy} onclick={() => control(sb.name, a.name, () => agentStop(sb.name, a.name), `Stopped ${a.name}`)}><Icon name="stop" size={12} /></button>
                          <button class="btn sm" title="Steer" onclick={() => steer(sb.name, a.name)}><Icon name="steer" size={12} /></button>
                        </div>
                      {/if}
                    </div>
                    {#if a.tools.length > 0}
                      <div class="tools">
                        {#each a.tools as t}
                          <span class="tool" class:active={t === a.active} title={TOOL_LABELS[t] ?? t}><Icon name={t} size={14} /></span>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if enabledServices.length > 0}
      <div class="svcband">
        <span class="xsmall muted" style="text-transform:uppercase;letter-spacing:.06em;margin-right:4px">Services (L1 / own L2)</span>
        {#each enabledServices as s}<span class="svc">{s}</span>{/each}
      </div>
    {/if}
  </section>

  <p class="xsmall muted mt12">
    L3 app containers (nested Docker/Podman) are <span class="t2">enabled per sandbox</span> but not drawn here — the badge marks where nesting is allowed. Live per-agent activity is not instrumented yet.
  </p>
</div>

<style>
  .legend { font-size: 11px; color: var(--text-3); }
  .ldot { width: 9px; height: 9px; border-radius: 50%; display: inline-block; }
  .vmblock { border: 1px solid var(--border); border-radius: var(--radius-lg); background: var(--card-2); padding: 16px; }
  .sbgrid { display: grid; gap: 16px; grid-template-columns: repeat(auto-fill, minmax(330px, 1fr)); }
  .sb-card { border: 1px solid var(--border); border-radius: var(--radius); background: var(--card); padding: 12px; transition: border-color .15s; }
  .sb-card:hover { border-color: var(--border-strong); }
  .sb-card.stopped { opacity: .6; }
  .l3on { font-size: 10px; display: inline-flex; align-items: center; gap: 4px; color: #2f8fd0; border: 1px solid #38bdf855; background: #38bdf818; border-radius: 6px; padding: 1px 6px; }
  .l3off { font-size: 10px; color: var(--text-3); border: 1px solid var(--border); border-radius: 6px; padding: 1px 6px; }
  .stopped-note { font-size: 12px; color: var(--text-3); font-style: italic; text-align: center; padding: 18px 0; }
  .agents { display: flex; flex-direction: column; gap: 8px; }
  .agent { border: 1px solid var(--border); border-radius: 10px; background: var(--card-2); padding: 10px; }
  .namebtn { background: none; border: none; padding: 0; cursor: pointer; font-family: var(--mono); text-align: left; }
  .namebtn:hover { color: var(--accent-text); text-decoration: underline; }
  .ctl { flex: none; align-items: flex-start; }
  .ava-wrap { position: relative; flex: none; }
  .ava { width: 36px; height: 36px; border-radius: 50%; display: grid; place-items: center; font-size: 11px; font-weight: 650; border: 1px solid var(--border-strong); background: var(--card); color: var(--text-2); }
  .ava.human { background: var(--accent-soft); color: var(--accent-text); border-color: transparent; }
  .ava.pulse { animation: pulse-ring 2s infinite; }
  @keyframes pulse-ring { 0% { box-shadow: 0 0 0 0 var(--ring); } 70% { box-shadow: 0 0 0 6px transparent; } 100% { box-shadow: 0 0 0 0 transparent; } }
  .ava-dot { position: absolute; bottom: -1px; right: -1px; width: 12px; height: 12px; border-radius: 50%; border: 2px solid var(--card); }
  .htag { font-size: 9px; text-transform: uppercase; letter-spacing: .04em; color: var(--accent-text); border: 1px solid var(--border); border-radius: 4px; padding: 0 4px; }
  .ptag { font-size: 9.5px; color: var(--accent-text); background: var(--accent-dim); border-radius: 4px; padding: 0 5px; font-family: var(--mono); }
  .action { font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tools { display: flex; gap: 6px; margin-top: 8px; padding-left: 46px; flex-wrap: wrap; }
  .tool { width: 28px; height: 28px; display: grid; place-items: center; border: 1px solid var(--border); border-radius: 7px; color: var(--text-3); }
  .tool.active { background: var(--accent-soft); border-color: var(--accent); color: var(--accent-text); }
  .svcband { margin-top: 18px; padding-top: 16px; border-top: 1px solid var(--border); display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .svc { font-size: 11px; color: var(--text-2); border: 1px solid var(--border); border-radius: 999px; padding: 3px 9px; display: inline-flex; align-items: center; gap: 6px; }
  .svc::before { content: ""; width: 6px; height: 6px; border-radius: 50%; background: var(--ok); }
</style>
